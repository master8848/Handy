use anyhow::Result;
use chrono::Utc;
use log::{debug, error, info};
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_specta::Event;

static MIGRATIONS: &[M] = &[M::up(
    "CREATE TABLE IF NOT EXISTS prompt_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt_text TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );",
)];

/// Maximum number of prompt entries retained; the oldest are pruned on
/// insert so the home page stays a quick-scannable log of pasted prompts.
const MAX_PROMPT_HISTORY_ENTRIES: usize = 100;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PromptHistoryEntry {
    pub id: i64,
    pub prompt_text: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum PromptHistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: PromptHistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "cleared")]
    Cleared,
}

/// Log of prompts written in the Home prompt box and pasted into the active
/// application. SQLite-backed (own db file, no shared migrations) so the log
/// survives restarts; entries are appended on paste and pruned to a cap.
pub struct PromptHistoryManager {
    app_handle: AppHandle,
    db_path: PathBuf,
}

impl PromptHistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let db_path = app_data_dir.join("prompt_history.db");

        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
        };
        manager.init_database()?;
        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing prompt history database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let migrations = Migrations::new(MIGRATIONS.to_vec());
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid prompt_history migrations");
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        migrations.to_latest(&mut conn)?;
        let version_after: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version_after > version_before {
            info!(
                "Prompt history database migrated from {} to {}",
                version_before, version_after
            );
        }
        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn map_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptHistoryEntry> {
        Ok(PromptHistoryEntry {
            id: row.get("id")?,
            prompt_text: row.get("prompt_text")?,
            timestamp: row.get("timestamp")?,
        })
    }

    /// Record a prompt and prune the log down to the cap. If the most recent
    /// entry already holds the same text, its timestamp is bumped instead of
    /// inserting a duplicate — so a written prompt that is re-edited (or
    /// re-pasted) updates its entry rather than piling up copies.
    pub fn save_entry(&self, prompt_text: String) -> Result<PromptHistoryEntry> {
        let timestamp = Utc::now().timestamp();

        let conn = self.get_connection()?;
        let updated = conn.execute(
            "UPDATE prompt_history
             SET timestamp = ?2
             WHERE id = (SELECT id FROM prompt_history ORDER BY id DESC LIMIT 1)
               AND prompt_text = ?1",
            params![&prompt_text, timestamp],
        )?;

        let entry = if updated == 0 {
            conn.execute(
                "INSERT INTO prompt_history (prompt_text, timestamp) VALUES (?1, ?2)",
                params![&prompt_text, timestamp],
            )?;
            PromptHistoryEntry {
                id: conn.last_insert_rowid(),
                prompt_text,
                timestamp,
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, prompt_text, timestamp
                 FROM prompt_history
                 ORDER BY id DESC
                 LIMIT 1",
            )?;
            let mut rows = stmt.query_map([], Self::map_entry)?;
            rows.next()
                .transpose()?
                .expect("the deduped row still exists")
        };
        conn.execute(
            "DELETE FROM prompt_history WHERE id NOT IN (
                SELECT id FROM prompt_history ORDER BY id DESC LIMIT ?1
            )",
            params![MAX_PROMPT_HISTORY_ENTRIES as i64],
        )?;

        debug!("Saved prompt history entry with id {}", entry.id);

        if let Err(e) = (PromptHistoryUpdatePayload::Added {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit prompt-history-updated event: {}", e);
        }

        Ok(entry)
    }

    /// List entries, newest first. `limit` is capped at 100 to bound
    /// frontend payloads.
    pub fn list_entries(&self, limit: Option<usize>) -> Result<Vec<PromptHistoryEntry>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, prompt_text, timestamp
             FROM prompt_history
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let limit = limit.map(|l| l.min(100)).unwrap_or(100) as i64;
        let entries = stmt
            .query_map(params![limit], Self::map_entry)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM prompt_history WHERE id = ?1", params![id])?;
        debug!("Deleted prompt history entry with id: {}", id);

        if let Err(e) = (PromptHistoryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit prompt-history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM prompt_history", [])?;
        debug!("Cleared prompt history");

        if let Err(e) = (PromptHistoryUpdatePayload::Cleared).emit(&self.app_handle) {
            error!("Failed to emit prompt-history-updated event: {}", e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE prompt_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt_text TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );",
        )
        .expect("create prompt_history table");
        conn
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str) {
        conn.execute(
            "INSERT INTO prompt_history (prompt_text, timestamp) VALUES (?1, ?2)",
            params![text, timestamp],
        )
        .expect("insert prompt entry");
    }

    #[test]
    fn entries_list_newest_first() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first");
        insert_entry(&conn, 200, "second");

        let mut stmt = conn
            .prepare(
                "SELECT id, prompt_text, timestamp FROM prompt_history ORDER BY id DESC LIMIT ?1",
            )
            .expect("prepare");
        let entries: Vec<PromptHistoryEntry> = stmt
            .query_map(params![100], PromptHistoryManager::map_entry)
            .expect("query")
            .collect::<std::result::Result<_, _>>()
            .expect("collect");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].prompt_text, "second");
        assert_eq!(entries[1].prompt_text, "first");
    }

    #[test]
    fn dedupe_bumps_only_the_latest_matching_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first");
        insert_entry(&conn, 200, "second");

        // Re-saving the latest entry's text updates its timestamp, no insert
        let updated = conn
            .execute(
                "UPDATE prompt_history
                 SET timestamp = 300
                 WHERE id = (SELECT id FROM prompt_history ORDER BY id DESC LIMIT 1)
                   AND prompt_text = 'second'",
                [],
            )
            .expect("update");
        assert_eq!(updated, 1);

        // The same text for an older, non-latest entry is not deduped
        let updated = conn
            .execute(
                "UPDATE prompt_history
                 SET timestamp = 400
                 WHERE id = (SELECT id FROM prompt_history ORDER BY id DESC LIMIT 1)
                   AND prompt_text = 'first'",
                [],
            )
            .expect("update");
        assert_eq!(updated, 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM prompt_history", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 2);

        let latest_ts: i64 = conn
            .query_row(
                "SELECT timestamp FROM prompt_history ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("timestamp");
        assert_eq!(latest_ts, 300);
    }
}
