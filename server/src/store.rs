use crate::model::Project;
use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub struct Store {
    pub(crate) db: Connection,
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = Connection::open(path).context("open project database")?;
        db.busy_timeout(std::time::Duration::from_secs(3))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS project (id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS commands (id TEXT PRIMARY KEY, body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, run TEXT NOT NULL, at TEXT NOT NULL, kind TEXT NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS output_destinations (id TEXT PRIMARY KEY, url TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS ingest_credentials (source_id TEXT PRIMARY KEY, credential TEXT NOT NULL);
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

    pub fn plugin_put(
        &self,
        namespace: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        self.db.execute("INSERT INTO plugin_storage(namespace,key,body) VALUES(?1,?2,?3) ON CONFLICT(namespace,key) DO UPDATE SET body=excluded.body", params![namespace, key, serde_json::to_string(value)?])?;
        Ok(())
    }

    pub fn destination_put(&self, id: &str, url: &str) -> anyhow::Result<()> {
        self.db.execute(
            "INSERT INTO output_destinations(id,url) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET url=excluded.url",
            params![id, url],
        )?;
        Ok(())
    }

    pub fn destination_get(&self, id: &str) -> anyhow::Result<Option<String>> {
        self.db
            .query_row(
                "SELECT url FROM output_destinations WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn ingest_credential(&self, source_id: &str) -> anyhow::Result<String> {
        if let Some(value) = self
            .db
            .query_row(
                "SELECT credential FROM ingest_credentials WHERE source_id=?1",
                [source_id],
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(value);
        }
        let credential = format!(
            "lp{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        self.db.execute(
            "INSERT INTO ingest_credentials(source_id,credential) VALUES(?1,?2)",
            params![source_id, credential],
        )?;
        Ok(credential)
    }

    pub fn delete_ingest_credential(&self, source_id: &str) -> anyhow::Result<()> {
        self.db.execute(
            "DELETE FROM ingest_credentials WHERE source_id=?1",
            [source_id],
        )?;
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

    pub fn plugin_list(&self, namespace: &str) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
        let mut query = self
            .db
            .prepare("SELECT key,body FROM plugin_storage WHERE namespace=?1 ORDER BY key")?;
        let rows = query.query_map([namespace], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (key, body) = row?;
            Ok((key, serde_json::from_str(&body)?))
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_survives_actual_database_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("project.sqlite");
        let mut project = Project::empty();
        project.plugin_settings.insert(
            "unavailable-plugin".into(),
            serde_json::json!({"future": 42}),
        );
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
        assert_eq!(
            restored.plugin_settings.get("unavailable-plugin"),
            Some(&serde_json::json!({"future": 42}))
        );
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

    #[test]
    fn ingest_credential_is_private_random_and_stable_across_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("project.sqlite");
        let source_id = "public-source-id";
        let first = Store::open(&path)
            .expect("database")
            .ingest_credential(source_id)
            .expect("credential");
        let second = Store::open(&path)
            .expect("reopened database")
            .ingest_credential(source_id)
            .expect("stable credential");
        assert_eq!(first, second);
        assert!(!first.contains(source_id));
        assert!(first.len() >= 64);
    }
}
