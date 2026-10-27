use anyhow::Context;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{snapshot::Snapshot, store::Store};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub cursor: String,
    pub engine_run_id: String,
    pub occurred_at_ns: String,
    pub kind: String,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub events: Vec<Event>,
    pub gap: Option<EventGap>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventGap {
    pub requested_after: String,
    pub oldest_available: String,
}

#[cfg(not(test))]
const EVENT_RETENTION: i64 = 4_096;
#[cfg(test)]
const EVENT_RETENTION: i64 = 4;

impl Store {
    pub fn append_observation(
        &self,
        command_id: &str,
        engine_run_id: &str,
        occurred_at_ns: &str,
        kind: &str,
        receipt: serde_json::Value,
        snapshot: &mut Snapshot,
    ) -> anyhow::Result<Event> {
        let transaction = self.db.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO commands(id,body) VALUES(?1,?2)",
            params![command_id, serde_json::to_string(&receipt)?],
        )?;
        transaction.execute(
            "DELETE FROM commands WHERE rowid <= (SELECT MAX(rowid)-4096 FROM commands)",
            [],
        )?;
        transaction.execute(
            "INSERT INTO events(run,at,kind,data) VALUES(?1,?2,?3,?4)",
            params![engine_run_id, occurred_at_ns, kind, "{}"],
        )?;
        let cursor = transaction.last_insert_rowid().to_string();
        snapshot.cursor.clone_from(&cursor);
        let data = serde_json::json!({"receipt": receipt, "snapshot": snapshot});
        transaction.execute(
            "UPDATE events SET data=?1 WHERE cursor=?2",
            params![serde_json::to_string(&data)?, cursor],
        )?;
        transaction.execute(
            "DELETE FROM events WHERE cursor <= (SELECT MAX(cursor)-?1 FROM events)",
            [EVENT_RETENTION],
        )?;
        transaction.commit()?;
        Ok(Event {
            cursor,
            engine_run_id: engine_run_id.to_owned(),
            occurred_at_ns: occurred_at_ns.to_owned(),
            kind: kind.to_owned(),
            data,
        })
    }

    pub fn cursor(&self) -> anyhow::Result<String> {
        let cursor: i64 =
            self.db
                .query_row("SELECT COALESCE(MAX(cursor),0) FROM events", [], |row| {
                    row.get(0)
                })?;
        Ok(cursor.to_string())
    }

    pub fn events(&self, after: &str) -> anyhow::Result<EventPage> {
        let after = after
            .parse::<i64>()
            .context("event cursor must be an integer")?;
        anyhow::ensure!(after >= 0, "event cursor must be non-negative");
        let oldest: i64 =
            self.db
                .query_row("SELECT COALESCE(MIN(cursor),0) FROM events", [], |row| {
                    row.get(0)
                })?;
        let gap = (oldest > 0 && after < oldest - 1).then(|| EventGap {
            requested_after: after.to_string(),
            oldest_available: oldest.to_string(),
        });
        let mut query = self.db.prepare(
            "SELECT cursor,run,at,kind,data FROM events WHERE cursor>?1 ORDER BY cursor LIMIT 4096",
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
        Ok(EventPage { events, gap })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_event_cursor_reports_retention_gap() -> anyhow::Result<()> {
        // Given
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("project.sqlite"))?;
        for _ in 0..=EVENT_RETENTION {
            store.db.execute(
                "INSERT INTO events(run,at,kind,data) VALUES('run','0','test','{}')",
                [],
            )?;
            store.db.execute(
                "DELETE FROM events WHERE cursor <= (SELECT MAX(cursor)-?1 FROM events)",
                [EVENT_RETENTION],
            )?;
        }

        // When
        let page = store.events("0")?;

        // Then
        let gap = page
            .gap
            .ok_or_else(|| anyhow::anyhow!("retention gap missing"))?;
        assert_eq!(gap.requested_after, "0");
        assert_eq!(gap.oldest_available, "2");
        assert_eq!(page.events.len(), usize::try_from(EVENT_RETENTION)?);
        Ok(())
    }
}
