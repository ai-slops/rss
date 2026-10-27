use crate::model::Project;
use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct Store {
    db: Connection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub cursor: String,
    pub engine_run_id: String,
    pub occurred_at_ns: String,
    pub kind: String,
    pub data: serde_json::Value,
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = Connection::open(path).context("open project database")?;
        db.busy_timeout(std::time::Duration::from_secs(3))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS project (id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS commands (id TEXT PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, run TEXT NOT NULL, at TEXT NOT NULL, kind TEXT NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS plugin_storage (namespace TEXT NOT NULL, key TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(namespace,key));")?;
        Ok(Self { db })
    }

    pub fn load(&self) -> anyhow::Result<Project> {
        let body: Option<String> = self
            .db
            .query_row("SELECT body FROM project WHERE id=1", [], |row| row.get(0))
            .optional()?;
        body.map_or_else(
            || Ok(Project::empty()),
            |json| serde_json::from_str(&json).context("parse saved project"),
        )
    }

    pub fn save(&mut self, project: &Project) -> anyhow::Result<()> {
        let body = serde_json::to_string(project)?;
        self.db.execute("INSERT INTO project(id,body) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body", [body])?;
        Ok(())
    }

    pub fn command(&self, id: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let json: Option<String> = self
            .db
            .query_row("SELECT body FROM commands WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        json.map(|body| serde_json::from_str(&body).context("parse command receipt"))
            .transpose()
    }

    pub fn receipt(&self, id: &str, response: &serde_json::Value) -> anyhow::Result<()> {
        self.db.execute(
            "INSERT INTO commands(id,body) VALUES(?1,?2)",
            params![id, serde_json::to_string(response)?],
        )?;
        self.db.execute(
            "DELETE FROM commands WHERE rowid <= (SELECT MAX(rowid)-4096 FROM commands)",
            [],
        )?;
        Ok(())
    }

    pub fn append(&self, event: &Event) -> anyhow::Result<String> {
        self.db.execute(
            "INSERT INTO events(run,at,kind,data) VALUES(?1,?2,?3,?4)",
            params![
                event.engine_run_id,
                event.occurred_at_ns,
                event.kind,
                serde_json::to_string(&event.data)?
            ],
        )?;
        Ok(self.db.last_insert_rowid().to_string())
    }

    pub fn cursor(&self) -> anyhow::Result<String> {
        let cursor: i64 =
            self.db
                .query_row("SELECT COALESCE(MAX(cursor),0) FROM events", [], |row| {
                    row.get(0)
                })?;
        Ok(cursor.to_string())
    }

    pub fn events(&self, after: i64) -> anyhow::Result<Vec<Event>> {
        let mut query = self.db.prepare(
            "SELECT cursor,run,at,kind,data FROM events WHERE cursor>?1 ORDER BY cursor LIMIT 512",
        )?;
        let mut rows = query.query([after])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let cursor: i64 = row.get(0)?;
            let data: String = row.get(4)?;
            events.push(Event {
                cursor: cursor.to_string(),
                engine_run_id: row.get(1)?,
                occurred_at_ns: row.get(2)?,
                kind: row.get(3)?,
                data: serde_json::from_str(&data)?,
            });
        }
        Ok(events)
    }

    pub fn plugin_put(
        &self,
        namespace: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        self.db.execute("INSERT INTO plugin_storage(namespace,key,body) VALUES(?1,?2,?3) ON CONFLICT(namespace,key) DO UPDATE SET body=excluded.body", params![namespace, key, serde_json::to_string(value)?])?;
        Ok(())
    }

    pub fn plugin_get(
        &self,
        namespace: &str,
        key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM plugin_storage WHERE namespace=?1 AND key=?2",
                params![namespace, key],
                |row| row.get(0),
            )
            .optional()?;
        body.map(|json| serde_json::from_str(&json).context("parse plugin value"))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_survives_actual_database_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("project.sqlite");
        let project = Project::empty();
        {
            let mut store = Store::open(&path).expect("open database");
            store.save(&project).expect("save project");
        }
        let restored = Store::open(&path)
            .expect("reopen database")
            .load()
            .expect("load project");
        assert_eq!(restored.id, project.id);
        assert!(restored.outputs.is_empty());
        assert!(restored.audio_routes.is_empty());
    }

    #[test]
    fn unknown_plugin_configuration_survives_without_plugin() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("project.sqlite");
        let store = Store::open(&path).expect("database");
        store
            .plugin_put(
                "unavailable-plugin",
                "configuration",
                &serde_json::json!({"future": 42}),
            )
            .expect("save plugin data");
        let value = store
            .plugin_get("unavailable-plugin", "configuration")
            .expect("read plugin data");
        assert_eq!(value, Some(serde_json::json!({"future": 42})));
    }
}
