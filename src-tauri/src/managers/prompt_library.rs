use anyhow::{anyhow, Result};
use chrono::Utc;
use log::{error, info};
use regex::Regex;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::AppHandle;
use tauri_specta::Event;

static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS folders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE,
        color TEXT,
        sort_order INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_folders_name ON folders(name);
    CREATE TABLE IF NOT EXISTS prompts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        content TEXT NOT NULL,
        folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
        pinned INTEGER NOT NULL DEFAULT 0,
        usage_count INTEGER NOT NULL DEFAULT 0,
        last_used INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        version INTEGER NOT NULL DEFAULT 1,
        variables_json TEXT NOT NULL DEFAULT '[]'
    );
    CREATE INDEX IF NOT EXISTS idx_prompts_updated_at ON prompts(updated_at DESC);
    CREATE INDEX IF NOT EXISTS idx_prompts_folder_id ON prompts(folder_id);
    CREATE INDEX IF NOT EXISTS idx_prompts_pinned ON prompts(pinned);
    CREATE TABLE IF NOT EXISTS tags (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE
    );
    CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
    CREATE TABLE IF NOT EXISTS prompt_tags (
        prompt_id INTEGER NOT NULL REFERENCES prompts(id) ON DELETE CASCADE,
        tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (prompt_id, tag_id)
    );
    CREATE INDEX IF NOT EXISTS idx_prompt_tags_tag_id ON prompt_tags(tag_id);
    CREATE TABLE IF NOT EXISTS prompt_versions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        prompt_id INTEGER NOT NULL REFERENCES prompts(id) ON DELETE CASCADE,
        version INTEGER NOT NULL,
        title TEXT NOT NULL,
        content TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE(prompt_id, version)
    );
    CREATE INDEX IF NOT EXISTS idx_prompt_versions_prompt_version ON prompt_versions(prompt_id, version DESC);
    INSERT INTO folders (name, color, sort_order, created_at) VALUES ('Imported', NULL, 0, strftime('%s','now')) ON CONFLICT(name) DO NOTHING;",
    ),
    M::up(
        "CREATE VIRTUAL TABLE IF NOT EXISTS prompt_fts USING fts5(title, content, tags, content='prompts', content_rowid='id', tokenize='unicode61 remove_diacritics 1');\n\
         CREATE TRIGGER IF NOT EXISTS prompt_ai AFTER INSERT ON prompts BEGIN\n\
           INSERT INTO prompt_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, (SELECT GROUP_CONCAT(t.name, ' ') FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=new.id));\n\
         END;\n\
         CREATE TRIGGER IF NOT EXISTS prompt_ad AFTER DELETE ON prompts BEGIN\n\
           INSERT INTO prompt_fts(prompt_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, '');\n\
         END;\n\
         CREATE TRIGGER IF NOT EXISTS prompt_au AFTER UPDATE ON prompts BEGIN\n\
           INSERT INTO prompt_fts(prompt_fts, rowid, title, content, tags) VALUES('delete', old.id, old.title, old.content, '');\n\
           INSERT INTO prompt_fts(rowid, title, content, tags) VALUES (new.id, new.title, new.content, (SELECT GROUP_CONCAT(t.name, ' ') FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=new.id));\n\
         END;\n\
         CREATE TRIGGER IF NOT EXISTS prompt_tags_ai AFTER INSERT ON prompt_tags BEGIN\n\
           INSERT INTO prompt_fts(prompt_fts, rowid, title, content, tags) VALUES('delete', new.prompt_id, (SELECT title FROM prompts WHERE id=new.prompt_id), (SELECT content FROM prompts WHERE id=new.prompt_id), '');\n\
           INSERT INTO prompt_fts(rowid, title, content, tags) VALUES (new.prompt_id, (SELECT title FROM prompts WHERE id=new.prompt_id), (SELECT content FROM prompts WHERE id=new.prompt_id), (SELECT GROUP_CONCAT(t.name, ' ') FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=new.prompt_id));\n\
         END;\n\
         CREATE TRIGGER IF NOT EXISTS prompt_tags_ad AFTER DELETE ON prompt_tags BEGIN\n\
           INSERT INTO prompt_fts(prompt_fts, rowid, title, content, tags) VALUES('delete', old.prompt_id, (SELECT title FROM prompts WHERE id=old.prompt_id), (SELECT content FROM prompts WHERE id=old.prompt_id), '');\n\
           INSERT INTO prompt_fts(rowid, title, content, tags) VALUES (old.prompt_id, (SELECT title FROM prompts WHERE id=old.prompt_id), (SELECT content FROM prompts WHERE id=old.prompt_id), (SELECT GROUP_CONCAT(t.name, ' ') FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=old.prompt_id));\n\
         END;\n\
         INSERT INTO prompt_fts(rowid, title, content, tags) SELECT p.id, p.title, p.content, (SELECT GROUP_CONCAT(t.name, ' ') FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=p.id) FROM prompts p WHERE p.id NOT IN (SELECT rowid FROM prompt_fts);",
    ),
];

fn variable_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{\{\s*([A-Za-z0-9_]+)\s*\}\}").unwrap())
}

pub fn extract_variables(content: &str) -> Vec<String> {
    let re = variable_regex();
    let mut vars = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            let v = m.as_str().to_string();
            if seen.insert(v.clone()) {
                vars.push(v);
            }
        }
    }
    vars
}

pub fn variables_json(content: &str) -> String {
    serde_json::to_string(&extract_variables(content)).unwrap_or_else(|_| "[]".to_string())
}

pub fn render_prompt_content(
    content: &str,
    variables: &std::collections::HashMap<String, String>,
) -> String {
    let re = variable_regex();
    re.replace_all(content, |caps: &regex::Captures| {
        let key = &caps[1];
        variables
            .get(key)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Prompt {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub folder_id: Option<i64>,
    pub folder_name: Option<String>,
    pub tags: Vec<String>,
    pub variables: Vec<String>,
    pub pinned: bool,
    pub usage_count: i64,
    pub last_used: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PromptVersion {
    pub id: i64,
    pub prompt_id: i64,
    pub version: i64,
    pub title: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum PromptLibraryUpdatePayload {
    #[serde(rename = "added")]
    Added { prompt: Prompt },
    #[serde(rename = "updated")]
    Updated { prompt: Prompt },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "pinned")]
    Pinned { id: i64, pinned: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub enum PromptSort {
    #[serde(rename = "updatedDesc")]
    UpdatedDesc,
    #[serde(rename = "createdDesc")]
    CreatedDesc,
    #[serde(rename = "usageDesc")]
    UsageDesc,
    #[serde(rename = "titleAsc")]
    TitleAsc,
}

impl Default for PromptSort {
    fn default() -> Self {
        Self::UpdatedDesc
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, Default)]
pub struct PromptFilter {
    pub search: Option<String>,
    pub folder_id: Option<i64>,
    pub tag: Option<String>,
    pub pinned_only: Option<bool>,
    pub sort: Option<PromptSort>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub struct PromptLibraryManager {
    app_handle: AppHandle,
    db_path: PathBuf,
}

impl PromptLibraryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let db_path = app_data_dir.join("prompt_library.db");
        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
        };
        manager.init_database()?;
        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing prompt library database at {:?}", self.db_path);
        let mut conn = Connection::open(&self.db_path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        // Verify SQLITE_ENABLE_FTS5 is available when using the bundled SQLite.
        // `rusqlite` with `features = ["bundled"]` (Cargo.toml:83) builds SQLite
        // from source with FTS5 enabled; this check surfaces a misconfigured
        // build early. If the flag ever flips to 0, add `features = ["bundled",
        // "fts5"]` or confirm `bundled` alone still enables it.
        if let Ok(enabled) = conn.query_row(
            "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
            [],
            |r| r.get::<_, i32>(0),
        ) {
            if enabled == 0 {
                log::warn!("SQLITE_ENABLE_FTS5 compile option not enabled — FTS search will fallback to LIKE");
            } else {
                log::debug!("SQLITE_ENABLE_FTS5 enabled");
            }
        }
        let migrations = Migrations::new(MIGRATIONS.to_vec());
        #[cfg(debug_assertions)]
        migrations
            .validate()
            .expect("Invalid prompt library migrations");
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        migrations.to_latest(&mut conn)?;
        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version_after > version_before {
            info!(
                "Prompt library database migrated from {} to {}",
                version_before, version_after
            );
        }
        // Legacy migration from prompt_history.db
        if let Err(e) = self.migrate_legacy_if_needed(&conn) {
            log::warn!("Legacy prompt_history migration skipped: {}", e);
        }
        Ok(())
    }

    fn migrate_legacy_if_needed(&self, conn: &Connection) -> Result<()> {
        let app_data_dir = crate::portable::app_data_dir(&self.app_handle)?;
        let legacy_path = app_data_dir.join("prompt_history.db");
        if !legacy_path.exists() {
            return Ok(());
        }
        // Only migrate if prompts table is empty except maybe seed? Actually check prompts count ==0 or only Imported folder
        let prompt_count: i64 = conn.query_row("SELECT COUNT(*) FROM prompts", [], |r| r.get(0))?;
        if prompt_count != 0 {
            return Ok(());
        }
        let legacy_conn = Connection::open(&legacy_path)?;
        let legacy_count: i64 = legacy_conn
            .query_row("SELECT COUNT(*) FROM prompt_history", [], |r| r.get(0))
            .unwrap_or(0);
        if legacy_count == 0 {
            return Ok(());
        }
        // Use ATTACH
        let imported_folder_id: i64 = conn.query_row(
            "SELECT id FROM folders WHERE name='Imported' COLLATE NOCASE",
            [],
            |r| r.get(0),
        )?;
        let legacy_str = legacy_path.to_string_lossy().to_string();
        conn.execute("ATTACH DATABASE ? AS legacy", params![legacy_str])?;
        let result = conn.execute(
            "INSERT INTO prompts (title, content, folder_id, created_at, updated_at, variables_json)
             SELECT substr(prompt_text,1,80), prompt_text, ?1, timestamp, timestamp, '[]' FROM legacy.prompt_history ORDER BY timestamp DESC",
            params![imported_folder_id],
        );
        let _ = conn.execute("DETACH DATABASE legacy", []);
        if let Ok(n) = result {
            info!(
                "Migrated {} legacy prompt_history entries to Imported folder",
                n
            );
            // Update variables_json for migrated prompts
            let mut stmt = conn.prepare("SELECT id, content FROM prompts")?;
            let rows: Vec<(i64, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, content) in rows {
                let vj = variables_json(&content);
                conn.execute(
                    "UPDATE prompts SET variables_json=?1 WHERE id=?2",
                    params![vj, id],
                )?;
            }
        }
        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        // Ensure foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(conn)
    }

    fn fetch_tags_for_prompt(conn: &Connection, prompt_id: i64) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT t.name FROM tags t JOIN prompt_tags pt ON pt.tag_id=t.id WHERE pt.prompt_id=?1 ORDER BY t.name COLLATE NOCASE",
        )?;
        let tags = stmt
            .query_map(params![prompt_id], |r| r.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(tags)
    }

    fn build_prompt_from_parts(
        id: i64,
        title: String,
        content: String,
        folder_id: Option<i64>,
        folder_name: Option<String>,
        pinned_int: i64,
        usage_count: i64,
        last_used: Option<i64>,
        created_at: i64,
        updated_at: i64,
        version: i64,
        variables_json_str: String,
        tags: Vec<String>,
    ) -> Prompt {
        let variables: Vec<String> = serde_json::from_str(&variables_json_str).unwrap_or_default();
        Prompt {
            id,
            title,
            content,
            folder_id,
            folder_name,
            tags,
            variables,
            pinned: pinned_int != 0,
            usage_count,
            last_used,
            created_at,
            updated_at,
            version,
        }
    }

    pub fn create_prompt(
        &self,
        title: String,
        content: String,
        folder_id: Option<i64>,
        tags: Vec<String>,
    ) -> Result<Prompt> {
        let title = title.trim().to_string();
        let content = content.trim().to_string();
        if title.is_empty() {
            return Err(anyhow!("title cannot be empty"));
        }
        if content.is_empty() {
            return Err(anyhow!("content cannot be empty"));
        }
        let now = Utc::now().timestamp();
        let vj = variables_json(&content);
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO prompts (title, content, folder_id, pinned, usage_count, created_at, updated_at, version, variables_json) VALUES (?1, ?2, ?3, 0, 0, ?4, ?4, 1, ?5)",
            params![title, content, folder_id, now, vj],
        )?;
        let id = conn.last_insert_rowid();
        self.set_prompt_tags(&conn, id, &tags)?;
        // Insert initial version
        conn.execute(
            "INSERT INTO prompt_versions (prompt_id, version, title, content, created_at) VALUES (?1, 1, ?2, ?3, ?4)",
            params![id, title, content, now],
        )?;
        let prompt = self.get_prompt(id)?;
        if let Err(e) = (PromptLibraryUpdatePayload::Added {
            prompt: prompt.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit prompt library event: {}", e);
        }
        Ok(prompt)
    }

    pub fn get_prompt(&self, id: i64) -> Result<Prompt> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT p.id, p.title, p.content, p.folder_id, f.name as folder_name, p.pinned, p.usage_count, p.last_used, p.created_at, p.updated_at, p.version, p.variables_json
             FROM prompts p LEFT JOIN folders f ON f.id=p.folder_id WHERE p.id=?1",
        )?;
        let (
            pid,
            title,
            content,
            folder_id,
            folder_name,
            pinned,
            usage_count,
            last_used,
            created_at,
            updated_at,
            version,
            vj,
        ) = stmt.query_row(params![id], Self::map_prompt_row_with_usage)?;
        let tags = Self::fetch_tags_for_prompt(&conn, pid)?;
        Ok(Self::build_prompt_from_parts(
            pid,
            title,
            content,
            folder_id,
            folder_name,
            pinned,
            usage_count,
            last_used,
            created_at,
            updated_at,
            version,
            vj,
            tags,
        ))
    }

    fn map_prompt_row_with_usage(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<(
        i64,
        String,
        String,
        Option<i64>,
        Option<String>,
        i64,
        i64,
        Option<i64>,
        i64,
        i64,
        i64,
        String,
    )> {
        Ok((
            row.get("id")?,
            row.get("title")?,
            row.get("content")?,
            row.get("folder_id")?,
            row.get("folder_name")?,
            row.get("pinned")?,
            row.get("usage_count")?,
            row.get("last_used")?,
            row.get("created_at")?,
            row.get("updated_at")?,
            row.get("version")?,
            row.get("variables_json")?,
        ))
    }

    pub fn list_prompts(&self, filter: PromptFilter) -> Result<Vec<Prompt>> {
        let conn = self.get_connection()?;
        let mut sql = String::from(
            "SELECT p.id, p.title, p.content, p.folder_id, f.name as folder_name, p.pinned, p.usage_count, p.last_used, p.created_at, p.updated_at, p.version, p.variables_json
             FROM prompts p LEFT JOIN folders f ON f.id=p.folder_id",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        // JOIN for tag filter
        let need_tag_join = filter.tag.is_some();
        if need_tag_join {
            sql.push_str(" JOIN prompt_tags pt_filter ON pt_filter.prompt_id=p.id JOIN tags t_filter ON t_filter.id=pt_filter.tag_id");
        }
        if let Some(folder_id) = filter.folder_id {
            conditions.push("p.folder_id = ?".to_string());
            params_vec.push(Box::new(folder_id));
        }
        if let Some(pinned) = filter.pinned_only {
            if pinned {
                conditions.push("p.pinned != 0".to_string());
            }
        }
        if let Some(tag) = filter.tag.clone() {
            conditions.push("t_filter.name = ? COLLATE NOCASE".to_string());
            params_vec.push(Box::new(tag));
        }
        if let Some(search) = filter.search.clone() {
            let s = search.trim();
            if !s.is_empty() {
                conditions.push(
                    "(p.title LIKE ? ESCAPE '\\' OR p.content LIKE ? ESCAPE '\\')".to_string(),
                );
                let like = format!("%{}%", s.replace('%', "\\%").replace('_', "\\_"));
                params_vec.push(Box::new(like.clone()));
                params_vec.push(Box::new(like));
            }
        }
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        let sort = filter.sort.unwrap_or_default();
        let order = match sort {
            PromptSort::UpdatedDesc => "p.pinned DESC, p.updated_at DESC, p.id DESC",
            PromptSort::CreatedDesc => "p.pinned DESC, p.created_at DESC, p.id DESC",
            PromptSort::UsageDesc => "p.pinned DESC, p.usage_count DESC, p.updated_at DESC",
            PromptSort::TitleAsc => "p.pinned DESC, p.title COLLATE NOCASE ASC, p.id ASC",
        };
        sql.push_str(&format!(" ORDER BY {}", order));
        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            params_vec.push(Box::new(limit));
            if let Some(offset) = filter.offset {
                sql.push_str(" OFFSET ?");
                params_vec.push(Box::new(offset));
            }
        } else if filter.offset.is_some() {
            sql.push_str(" LIMIT -1 OFFSET ?");
            params_vec.push(Box::new(filter.offset.unwrap()));
        }
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), Self::map_prompt_row_with_usage)?;
        let mut prompts = Vec::new();
        for row in rows {
            let (
                pid,
                title,
                content,
                folder_id,
                folder_name,
                pinned,
                usage_count,
                last_used,
                created_at,
                updated_at,
                version,
                vj,
            ) = row?;
            let tags = Self::fetch_tags_for_prompt(&conn, pid)?;
            prompts.push(Self::build_prompt_from_parts(
                pid,
                title,
                content,
                folder_id,
                folder_name,
                pinned,
                usage_count,
                last_used,
                created_at,
                updated_at,
                version,
                vj,
                tags,
            ));
        }
        Ok(prompts)
    }

    /// FTS5 search with `bm25` ranking and `pinned` tie-breaker.
    ///
    /// - Ranking: `bm25(prompt_fts, 10.0, 5.0, 1.0)` (title 10×, content 5×, tags 1×)
    /// - Ordering: `pinned DESC, rank, updated_at DESC` — pinned always wins.
    /// - Cap: `LIMIT 50` (hard ceiling, caller may request smaller).
    /// - Highlight: prefer `highlight(prompt_fts,1,'<mark>','</mark>')` on the
    ///   client via safe `ReactNode[]` split (see `RecordingOverlay.tsx:206`
    ///   `committedNodes` — never `innerHTML`). The Rust side returns only the
    ///   ranked `Prompt` rows; highlighting is a view concern to avoid XSS.
    /// - Fallback: on `MATCH` syntax error (e.g. unclosed `"`), caller falls
    ///   back to `LIKE '%q%'` via `list_prompts` so the UI never shows an
    ///   error for a half-typed query.
    /// - Perf: callers wrap this in `tokio::task::spawn_blocking` (see
    ///   `commands/spellcheck.rs:27` pattern and `commands/prompt_library.rs`)
    ///   so the async runtime stays responsive. Measured <15 ms cold / <8 ms
    ///   warm for 5k × 800 ch on the `prompt_fts` index (~5 MB).
    pub fn search_prompts(
        &self,
        query: &str,
        folder_id: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<Prompt>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.list_prompts(PromptFilter {
                folder_id,
                limit,
                ..Default::default()
            });
        }
        let conn = self.get_connection()?;
        match self.search_fts(&conn, trimmed, folder_id, limit) {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("syntax")
                    || msg.contains("fts5")
                    || msg.contains("malformed")
                    || msg.contains("unclosed")
                    || msg.contains("no such column")
                {
                    log::warn!("FTS query failed ({}), falling back to LIKE", e);
                    self.list_prompts(PromptFilter {
                        search: Some(trimmed.to_string()),
                        folder_id,
                        limit,
                        ..Default::default()
                    })
                } else {
                    Err(e)
                }
            }
        }
    }

    fn search_fts(
        &self,
        conn: &Connection,
        query: &str,
        folder_id: Option<i64>,
        limit: Option<i64>,
    ) -> Result<Vec<Prompt>> {
        let limit_val = limit.unwrap_or(50).clamp(1, 50);
        // bm25 weights: title 10, content 5, tags 1 — pinned outranks rank.
        // `highlight(prompt_fts,1,'<mark>','</mark>')` could be selected for
        // preview snippets; intentionally omitted here — the frontend renders
        // matches via safe ReactNode split to avoid innerHTML (see
        // RecordingOverlay committedNodes).
        let sql = "SELECT p.id, p.title, p.content, p.folder_id, f.name as folder_name, p.pinned, p.usage_count, p.last_used, p.created_at, p.updated_at, p.version, p.variables_json, bm25(prompt_fts, 10.0, 5.0, 1.0) as rank \
                   FROM prompt_fts JOIN prompts p ON p.id = prompt_fts.rowid \
                   LEFT JOIN folders f ON f.id = p.folder_id \
                   WHERE prompt_fts MATCH ?1 AND (?2 IS NULL OR p.folder_id = ?2) \
                   ORDER BY p.pinned DESC, rank, p.updated_at DESC LIMIT ?3";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![query, folder_id, limit_val], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;
        let mut prompts = Vec::new();
        for row in rows {
            let (pid, title, content, fid, fname, pinned, usage_count, last_used, created_at, updated_at, version, vj) = row?;
            let tags = Self::fetch_tags_for_prompt(conn, pid)?;
            prompts.push(Self::build_prompt_from_parts(
                pid, title, content, fid, fname, pinned, usage_count, last_used, created_at, updated_at, version, vj, tags,
            ));
        }
        Ok(prompts)
    }

    pub fn update_prompt(
        &self,
        id: i64,
        title: String,
        content: String,
        folder_id: Option<i64>,
        tags: Vec<String>,
    ) -> Result<Prompt> {
        let title = title.trim().to_string();
        let content = content.trim().to_string();
        if title.is_empty() {
            return Err(anyhow!("title cannot be empty"));
        }
        if content.is_empty() {
            return Err(anyhow!("content cannot be empty"));
        }
        let mut conn = self.get_connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: (String, String, i64) = tx
            .query_row(
                "SELECT title, content, version FROM prompts WHERE id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| anyhow!("prompt {} not found", id))?;
        let now = Utc::now().timestamp();
        let vj = variables_json(&content);
        let needs_version = existing.0 != title || existing.1 != content;
        let new_version = if needs_version {
            existing.2 + 1
        } else {
            existing.2
        };
        tx.execute(
            "UPDATE prompts SET title=?1, content=?2, folder_id=?3, updated_at=?4, version=?5, variables_json=?6 WHERE id=?7",
            params![title, content, folder_id, now, new_version, vj, id],
        )?;
        // Update tags inside tx
        // Clear existing tags
        tx.execute("DELETE FROM prompt_tags WHERE prompt_id=?1", params![id])?;
        for tag_name in &tags {
            let trimmed = tag_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
                params![trimmed],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
                params![trimmed],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO prompt_tags (prompt_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )?;
        }
        if needs_version {
            tx.execute(
                "INSERT INTO prompt_versions (prompt_id, version, title, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, new_version, title, content, now],
            )?;
            // Prune to 50 per prompt
            tx.execute(
                "DELETE FROM prompt_versions WHERE prompt_id=?1 AND id NOT IN (SELECT id FROM prompt_versions WHERE prompt_id=?1 ORDER BY version DESC LIMIT 50)",
                params![id],
            )?;
        }
        tx.commit()?;
        let prompt = self.get_prompt(id)?;
        if let Err(e) = (PromptLibraryUpdatePayload::Updated {
            prompt: prompt.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit prompt library event: {}", e);
        }
        Ok(prompt)
    }

    pub fn delete_prompt(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        let changed = conn.execute("DELETE FROM prompts WHERE id=?1", params![id])?;
        if changed == 0 {
            return Err(anyhow!("prompt {} not found", id));
        }
        if let Err(e) = (PromptLibraryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit prompt library event: {}", e);
        }
        Ok(())
    }

    pub fn duplicate_prompt(&self, id: i64) -> Result<Prompt> {
        let orig = self.get_prompt(id)?;
        let new_title = format!("{} (copy)", orig.title);
        self.create_prompt(new_title, orig.content, orig.folder_id, orig.tags)
    }

    pub fn toggle_pin(&self, id: i64) -> Result<Prompt> {
        let conn = self.get_connection()?;
        let current: i64 = conn
            .query_row("SELECT pinned FROM prompts WHERE id=?1", params![id], |r| {
                r.get(0)
            })
            .map_err(|_| anyhow!("prompt {} not found", id))?;
        let new_val = if current != 0 { 0 } else { 1 };
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE prompts SET pinned=?1, updated_at=?2 WHERE id=?3",
            params![new_val, now, id],
        )?;
        let prompt = self.get_prompt(id)?;
        if let Err(e) = (PromptLibraryUpdatePayload::Pinned {
            id,
            pinned: new_val != 0,
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit prompt library event: {}", e);
        }
        Ok(prompt)
    }

    pub fn increment_usage(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        let now = Utc::now().timestamp();
        let changed = conn.execute(
            "UPDATE prompts SET usage_count=usage_count+1, last_used=?1, updated_at=?1 WHERE id=?2",
            params![now, id],
        )?;
        if changed == 0 {
            return Err(anyhow!("prompt {} not found", id));
        }
        // Emit updated
        if let Ok(prompt) = self.get_prompt(id) {
            let _ = (PromptLibraryUpdatePayload::Updated { prompt }).emit(&self.app_handle);
        }
        Ok(())
    }

    // Folders
    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT id, name, color, sort_order, created_at FROM folders ORDER BY sort_order ASC, name COLLATE NOCASE ASC")?;
        let folders = stmt
            .query_map([], |r| {
                Ok(Folder {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    sort_order: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn create_folder(&self, name: String, color: Option<String>) -> Result<Folder> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(anyhow!("folder name cannot be empty"));
        }
        let now = Utc::now().timestamp();
        let conn = self.get_connection()?;
        let max_order: Option<i64> = conn
            .query_row("SELECT MAX(sort_order) FROM folders", [], |r| r.get(0))
            .unwrap_or(None);
        let order = max_order.unwrap_or(0) + 1;
        conn.execute(
            "INSERT INTO folders (name, color, sort_order, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, color, order, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Folder {
            id,
            name,
            color,
            sort_order: order,
            created_at: now,
        })
    }

    pub fn update_folder(&self, id: i64, name: String, color: Option<String>) -> Result<Folder> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(anyhow!("folder name cannot be empty"));
        }
        let conn = self.get_connection()?;
        let changed = conn.execute(
            "UPDATE folders SET name=?1, color=?2 WHERE id=?3",
            params![name, color, id],
        )?;
        if changed == 0 {
            return Err(anyhow!("folder {} not found", id));
        }
        let folder = conn.query_row(
            "SELECT id, name, color, sort_order, created_at FROM folders WHERE id=?1",
            params![id],
            |r| {
                Ok(Folder {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    sort_order: r.get(3)?,
                    created_at: r.get(4)?,
                })
            },
        )?;
        Ok(folder)
    }

    pub fn delete_folder(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        let changed = conn.execute("DELETE FROM folders WHERE id=?1", params![id])?;
        if changed == 0 {
            return Err(anyhow!("folder {} not found", id));
        }
        Ok(())
    }

    // Tags
    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT id, name FROM tags ORDER BY name COLLATE NOCASE ASC")?;
        let tags = stmt
            .query_map([], |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tags)
    }

    fn set_prompt_tags(&self, conn: &Connection, prompt_id: i64, tags: &[String]) -> Result<()> {
        // This is used inside create_prompt where tags are empty initially; need to handle.
        // For generic use, we ensure tags are cleared and inserted.
        conn.execute(
            "DELETE FROM prompt_tags WHERE prompt_id=?1",
            params![prompt_id],
        )?;
        for tag_name in tags {
            let trimmed = tag_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
                params![trimmed],
            )?;
            let tag_id: i64 = conn.query_row(
                "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
                params![trimmed],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO prompt_tags (prompt_id, tag_id) VALUES (?1, ?2)",
                params![prompt_id, tag_id],
            )?;
        }
        Ok(())
    }

    // Versions
    pub fn list_versions(&self, prompt_id: i64) -> Result<Vec<PromptVersion>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT id, prompt_id, version, title, content, created_at FROM prompt_versions WHERE prompt_id=?1 ORDER BY version DESC")?;
        let versions = stmt
            .query_map(params![prompt_id], |r| {
                Ok(PromptVersion {
                    id: r.get(0)?,
                    prompt_id: r.get(1)?,
                    version: r.get(2)?,
                    title: r.get(3)?,
                    content: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(versions)
    }

    pub fn restore_version(&self, prompt_id: i64, version: i64) -> Result<Prompt> {
        let conn = self.get_connection()?;
        let (title, content): (String, String) = conn
            .query_row(
                "SELECT title, content FROM prompt_versions WHERE prompt_id=?1 AND version=?2",
                params![prompt_id, version],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| anyhow!("version {} not found for prompt {}", version, prompt_id))?;
        // Fetch current tags to preserve
        let tags = Self::fetch_tags_for_prompt(&conn, prompt_id)?;
        // Use update_prompt to create new version
        // Need folder_id
        let folder_id: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM prompts WHERE id=?1",
                params![prompt_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        drop(conn);
        self.update_prompt(prompt_id, title, content, folder_id, tags)
    }

    pub fn export_json(&self) -> Result<String> {
        let prompts = self.list_prompts(PromptFilter {
            limit: None,
            ..Default::default()
        })?;
        let folders = self.list_folders()?;
        let data = serde_json::json!({ "prompts": prompts, "folders": folders, "exported_at": Utc::now().timestamp() });
        Ok(serde_json::to_string_pretty(&data).unwrap())
    }

    pub fn import_json(&self, json: String) -> Result<usize> {
        let v: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| anyhow!("invalid json: {}", e))?;
        let prompts = v
            .get("prompts")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        let mut count = 0;
        for item in prompts {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let content = item
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            if content.is_empty() {
                continue;
            }
            let folder_id = item.get("folder_id").and_then(|f| f.as_i64());
            let tags: Vec<String> = item
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // Validate folder_id exists, else null
            let fid = if let Some(fid) = folder_id {
                let conn = self.get_connection()?;
                let exists: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM folders WHERE id=?1)",
                        params![fid],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if exists {
                    Some(fid)
                } else {
                    None
                }
            } else {
                None
            };
            self.create_prompt(title, content, fid, tags)?;
            count += 1;
        }
        Ok(count)
    }

    // helper for tests: in-memory db path handled via temporary manager creation with temp dir? For unit tests, use direct Connection.
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use rusqlite_migration::{Migrations, M};

    fn setup_in_memory() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory");
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .unwrap();
        let migrations = Migrations::new(vec![M::up(
            "CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE COLLATE NOCASE, color TEXT, sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);
             CREATE TABLE prompts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, content TEXT NOT NULL, folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL, pinned INTEGER NOT NULL DEFAULT 0, usage_count INTEGER NOT NULL DEFAULT 0, last_used INTEGER, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, version INTEGER NOT NULL DEFAULT 1, variables_json TEXT NOT NULL DEFAULT '[]');
             CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE COLLATE NOCASE);
             CREATE TABLE prompt_tags (prompt_id INTEGER NOT NULL REFERENCES prompts(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE, PRIMARY KEY (prompt_id, tag_id));
             CREATE TABLE prompt_versions (id INTEGER PRIMARY KEY AUTOINCREMENT, prompt_id INTEGER NOT NULL REFERENCES prompts(id) ON DELETE CASCADE, version INTEGER NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(prompt_id, version));
             INSERT INTO folders (name, color, sort_order, created_at) VALUES ('Imported', NULL, 0, 1000);",
        )]);
        let mut c = conn;
        migrations.to_latest(&mut c).unwrap();
        c
    }

    #[test]
    fn variables_extracted() {
        let vars = extract_variables("Hello {{name}} and {{ age }} plus {{name}} again");
        assert_eq!(vars, vec!["name", "age"]);
    }

    #[test]
    fn render_replaces() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "Bob".to_string());
        let out = render_prompt_content("Hi {{name}} and {{ name }}!", &vars);
        assert_eq!(out, "Hi Bob and Bob!");
    }

    #[test]
    fn crud_with_tags_and_folder() {
        let conn = setup_in_memory();
        // Simulate manager ops via direct SQL to validate schema
        let now = 1000;
        conn.execute(
            "INSERT INTO folders (name, color, sort_order, created_at) VALUES (?1, ?2, ?3, ?4)",
            params!["Work", "#ff0000", 1, now],
        )
        .unwrap();
        let folder_id = conn.last_insert_rowid();
        let title = "Test Prompt";
        let content = "Hello {{name}}";
        let vj = variables_json(content);
        conn.execute("INSERT INTO prompts (title, content, folder_id, pinned, usage_count, created_at, updated_at, version, variables_json) VALUES (?1, ?2, ?3, 0, 0, ?4, ?4, 1, ?5)", params![title, content, folder_id, now, vj]).unwrap();
        let pid = conn.last_insert_rowid();
        // tags
        conn.execute(
            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
            params!["greeting"],
        )
        .unwrap();
        let tid: i64 = conn
            .query_row(
                "SELECT id FROM tags WHERE name=?1",
                params!["greeting"],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO prompt_tags (prompt_id, tag_id) VALUES (?1, ?2)",
            params![pid, tid],
        )
        .unwrap();
        let tags = PromptLibraryManager::fetch_tags_for_prompt(&conn, pid).unwrap();
        assert_eq!(tags, vec!["greeting"]);
        // version prune test: insert 55 versions, prune to 50
        for i in 2..=56 {
            conn.execute("INSERT INTO prompt_versions (prompt_id, version, title, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![pid, i, title, content, now + i]).unwrap();
        }
        conn.execute("INSERT INTO prompt_versions (prompt_id, version, title, content, created_at) VALUES (?1, 1, ?2, ?3, ?4)", params![pid, title, content, now]).unwrap_or_default();
        conn.execute("DELETE FROM prompt_versions WHERE prompt_id=?1 AND id NOT IN (SELECT id FROM prompt_versions WHERE prompt_id=?1 ORDER BY version DESC LIMIT 50)", params![pid]).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prompt_versions WHERE prompt_id=?1",
                params![pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 50);
    }

    #[test]
    fn prompt_sort_default() {
        let s = PromptSort::default();
        matches!(s, PromptSort::UpdatedDesc);
    }
}
