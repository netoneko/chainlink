//! Database layer for chainlink issue tracker
//!
//! This module provides the `Database` struct which wraps a `DatabaseBackend`
//! and provides high-level methods for managing issues, comments, sessions, etc.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeSet as HashSet;
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::collections::HashSet;

use chrono::{DateTime, Utc};

use crate::backend::{BackendError, DatabaseBackend, Row, Value};
use crate::models::{Comment, Issue, Milestone, Session};

const SCHEMA_VERSION: i32 = 7;

// Timestamp helper for no_std environments
#[cfg(feature = "std")]
fn current_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(not(feature = "std"))]
fn current_timestamp() -> DateTime<Utc> {
    // Use a static counter to provide ordering in no_std mode
    use core::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let secs = COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
    // Create a DateTime from Unix timestamp (starting from 2024-01-01)
    DateTime::from_timestamp(1704067200 + secs, 0).unwrap_or_else(|| {
        DateTime::from_timestamp(1704067200, 0).unwrap()
    })
}

#[cfg(feature = "std")]
fn default_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(not(feature = "std"))]
fn default_timestamp() -> DateTime<Utc> {
    DateTime::from_timestamp(1704067200, 0).unwrap()
}

/// Database wrapper that provides high-level issue tracking operations
pub struct Database<B: DatabaseBackend> {
    backend: B,
}

/// Error type for database operations
#[derive(Debug)]
pub enum DbError {
    Backend(BackendError),
    NotFound(String),
    Validation(String),
}

impl From<BackendError> for DbError {
    fn from(e: BackendError) -> Self {
        DbError::Backend(e)
    }
}

impl core::fmt::Display for DbError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DbError::Backend(e) => write!(f, "{}", e),
            DbError::NotFound(msg) => write!(f, "{}", msg),
            DbError::Validation(msg) => write!(f, "{}", msg),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DbError {}

pub type Result<T> = core::result::Result<T, DbError>;

impl<B: DatabaseBackend> Database<B> {
    /// Create a new Database with the given backend
    pub fn new(backend: B) -> Result<Self> {
        let db = Database { backend };
        db.init_schema()?;
        Ok(db)
    }

    /// Open a database at the given path
    pub fn open(path: &str) -> Result<Self> {
        let backend = B::open(path)?;
        Self::new(backend)
    }

    /// Get a reference to the underlying backend
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Execute a closure within a database transaction.
    /// If the closure returns Ok, the transaction is committed.
    /// If the closure returns Err, the transaction is rolled back.
    pub fn transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.backend.execute("BEGIN TRANSACTION", &[])?;
        match f() {
            Ok(result) => {
                self.backend.execute("COMMIT", &[])?;
                Ok(result)
            }
            Err(e) => {
                let _ = self.backend.execute("ROLLBACK", &[]);
                Err(e)
            }
        }
    }

    fn init_schema(&self) -> Result<()> {
        // Check if we need to initialize
        let version_result = self.backend.execute(
            "SELECT COALESCE(MAX(version), 0) FROM pragma_user_version",
            &[],
        );
        let version: i32 = version_result
            .ok()
            .and_then(|r| r.rows.into_iter().next())
            .and_then(|row| row.get_string(0))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            self.backend.execute_batch(
                r#"
                -- Core issues table
                CREATE TABLE IF NOT EXISTS issues (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    description TEXT,
                    status TEXT NOT NULL DEFAULT 'open',
                    priority TEXT NOT NULL DEFAULT 'medium',
                    parent_id INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    closed_at TEXT,
                    FOREIGN KEY (parent_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Labels (many-to-many)
                CREATE TABLE IF NOT EXISTS labels (
                    issue_id INTEGER NOT NULL,
                    label TEXT NOT NULL,
                    PRIMARY KEY (issue_id, label),
                    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Dependencies (blocker blocks blocked)
                CREATE TABLE IF NOT EXISTS dependencies (
                    blocker_id INTEGER NOT NULL,
                    blocked_id INTEGER NOT NULL,
                    PRIMARY KEY (blocker_id, blocked_id),
                    FOREIGN KEY (blocker_id) REFERENCES issues(id) ON DELETE CASCADE,
                    FOREIGN KEY (blocked_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Comments
                CREATE TABLE IF NOT EXISTS comments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    issue_id INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Sessions (for context preservation)
                CREATE TABLE IF NOT EXISTS sessions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    active_issue_id INTEGER,
                    handoff_notes TEXT,
                    FOREIGN KEY (active_issue_id) REFERENCES issues(id) ON DELETE SET NULL
                );

                -- Time tracking
                CREATE TABLE IF NOT EXISTS time_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    issue_id INTEGER NOT NULL,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    duration_seconds INTEGER,
                    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Relations (related issues, bidirectional)
                CREATE TABLE IF NOT EXISTS relations (
                    issue_id_1 INTEGER NOT NULL,
                    issue_id_2 INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (issue_id_1, issue_id_2),
                    FOREIGN KEY (issue_id_1) REFERENCES issues(id) ON DELETE CASCADE,
                    FOREIGN KEY (issue_id_2) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Milestones
                CREATE TABLE IF NOT EXISTS milestones (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    description TEXT,
                    status TEXT NOT NULL DEFAULT 'open',
                    created_at TEXT NOT NULL,
                    closed_at TEXT
                );

                -- Milestone-Issue relationship (many-to-many)
                CREATE TABLE IF NOT EXISTS milestone_issues (
                    milestone_id INTEGER NOT NULL,
                    issue_id INTEGER NOT NULL,
                    PRIMARY KEY (milestone_id, issue_id),
                    FOREIGN KEY (milestone_id) REFERENCES milestones(id) ON DELETE CASCADE,
                    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
                );

                -- Indexes
                CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
                CREATE INDEX IF NOT EXISTS idx_issues_priority ON issues(priority);
                CREATE INDEX IF NOT EXISTS idx_labels_issue ON labels(issue_id);
                CREATE INDEX IF NOT EXISTS idx_comments_issue ON comments(issue_id);
                CREATE INDEX IF NOT EXISTS idx_deps_blocker ON dependencies(blocker_id);
                CREATE INDEX IF NOT EXISTS idx_deps_blocked ON dependencies(blocked_id);
                CREATE INDEX IF NOT EXISTS idx_issues_parent ON issues(parent_id);
                CREATE INDEX IF NOT EXISTS idx_time_entries_issue ON time_entries(issue_id);
                CREATE INDEX IF NOT EXISTS idx_relations_1 ON relations(issue_id_1);
                CREATE INDEX IF NOT EXISTS idx_relations_2 ON relations(issue_id_2);
                CREATE INDEX IF NOT EXISTS idx_milestone_issues_m ON milestone_issues(milestone_id);
                CREATE INDEX IF NOT EXISTS idx_milestone_issues_i ON milestone_issues(issue_id);
                "#,
            )?;

            // Migration: add parent_id column if upgrading from v1
            let _ = self.backend.execute(
                "ALTER TABLE issues ADD COLUMN parent_id INTEGER REFERENCES issues(id) ON DELETE CASCADE",
                &[],
            );

            // Migration v7: Recreate sessions table with ON DELETE SET NULL for active_issue_id
            if version < 7 {
                let _ = self.backend.execute_batch(
                    r#"
                    CREATE TABLE IF NOT EXISTS sessions_new (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        started_at TEXT NOT NULL,
                        ended_at TEXT,
                        active_issue_id INTEGER,
                        handoff_notes TEXT,
                        FOREIGN KEY (active_issue_id) REFERENCES issues(id) ON DELETE SET NULL
                    );
                    INSERT OR IGNORE INTO sessions_new SELECT * FROM sessions;
                    DROP TABLE IF EXISTS sessions;
                    ALTER TABLE sessions_new RENAME TO sessions;
                    "#,
                );
            }

            self.backend.execute(
                &format!("PRAGMA user_version = {}", SCHEMA_VERSION),
                &[],
            )?;
        }

        // Enable foreign keys
        self.backend.execute("PRAGMA foreign_keys = ON", &[])?;

        Ok(())
    }

    // ==================== Issue CRUD ====================

    pub fn create_issue(
        &self,
        title: &str,
        description: Option<&str>,
        priority: &str,
    ) -> Result<i64> {
        self.create_issue_with_parent(title, description, priority, None)
    }

    pub fn create_subissue(
        &self,
        parent_id: i64,
        title: &str,
        description: Option<&str>,
        priority: &str,
    ) -> Result<i64> {
        self.create_issue_with_parent(title, description, priority, Some(parent_id))
    }

    fn create_issue_with_parent(
        &self,
        title: &str,
        description: Option<&str>,
        priority: &str,
        parent_id: Option<i64>,
    ) -> Result<i64> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT INTO issues (title, description, priority, parent_id, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?5)",
            &[
                Value::Text(title.into()),
                description.map(|s| Value::Text(s.into())).unwrap_or(Value::Null),
                Value::Text(priority.into()),
                parent_id.map(Value::Integer).unwrap_or(Value::Null),
                Value::Text(now),
            ],
        )?;
        Ok(result.last_insert_rowid)
    }

    pub fn get_subissues(&self, parent_id: i64) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            "SELECT id, title, description, status, priority, parent_id, created_at, updated_at, closed_at FROM issues WHERE parent_id = ?1 ORDER BY id",
            &[Value::Integer(parent_id)],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    pub fn get_issue(&self, id: i64) -> Result<Option<Issue>> {
        let result = self.backend.execute(
            "SELECT id, title, description, status, priority, parent_id, created_at, updated_at, closed_at FROM issues WHERE id = ?1",
            &[Value::Integer(id)],
        )?;
        Ok(result.rows.first().map(issue_from_row))
    }

    /// Get an issue by ID, returning an error if not found.
    pub fn require_issue(&self, id: i64) -> Result<Issue> {
        self.get_issue(id)?
            .ok_or_else(|| DbError::NotFound(format!("Issue #{} not found", id)))
    }

    pub fn list_issues(
        &self,
        status_filter: Option<&str>,
        label_filter: Option<&str>,
        priority_filter: Option<&str>,
    ) -> Result<Vec<Issue>> {
        let mut sql = String::from(
            "SELECT DISTINCT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at FROM issues i",
        );
        let mut conditions = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        if label_filter.is_some() {
            sql.push_str(" JOIN labels l ON i.id = l.issue_id");
        }

        if let Some(status) = status_filter {
            if status != "all" {
                params.push(Value::Text(status.into()));
                conditions.push(format!("i.status = ?{}", params.len()));
            }
        }

        if let Some(label) = label_filter {
            params.push(Value::Text(label.into()));
            conditions.push(format!("l.label = ?{}", params.len()));
        }

        if let Some(priority) = priority_filter {
            params.push(Value::Text(priority.into()));
            conditions.push(format!("i.priority = ?{}", params.len()));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY i.id DESC");

        let result = self.backend.execute(&sql, &params)?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    pub fn update_issue(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        priority: Option<&str>,
    ) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut params: Vec<Value> = vec![Value::Text(now)];

        if let Some(t) = title {
            params.push(Value::Text(t.into()));
            updates.push(format!("title = ?{}", params.len()));
        }

        if let Some(d) = description {
            params.push(Value::Text(d.into()));
            updates.push(format!("description = ?{}", params.len()));
        }

        if let Some(p) = priority {
            params.push(Value::Text(p.into()));
            updates.push(format!("priority = ?{}", params.len()));
        }

        params.push(Value::Integer(id));
        let sql = format!(
            "UPDATE issues SET {} WHERE id = ?{}",
            updates.join(", "),
            params.len()
        );

        let result = self.backend.execute(&sql, &params)?;
        Ok(result.changes > 0)
    }

    pub fn close_issue(&self, id: i64) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE issues SET status = 'closed', closed_at = ?1, updated_at = ?1 WHERE id = ?2",
            &[Value::Text(now), Value::Integer(id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn reopen_issue(&self, id: i64) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE issues SET status = 'open', closed_at = NULL, updated_at = ?1 WHERE id = ?2",
            &[Value::Text(now), Value::Integer(id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn delete_issue(&self, id: i64) -> Result<bool> {
        let result = self
            .backend
            .execute("DELETE FROM issues WHERE id = ?1", &[Value::Integer(id)])?;
        Ok(result.changes > 0)
    }

    // ==================== Labels ====================

    pub fn add_label(&self, issue_id: i64, label: &str) -> Result<bool> {
        let result = self.backend.execute(
            "INSERT OR IGNORE INTO labels (issue_id, label) VALUES (?1, ?2)",
            &[Value::Integer(issue_id), Value::Text(label.into())],
        )?;
        Ok(result.changes > 0)
    }

    pub fn remove_label(&self, issue_id: i64, label: &str) -> Result<bool> {
        let result = self.backend.execute(
            "DELETE FROM labels WHERE issue_id = ?1 AND label = ?2",
            &[Value::Integer(issue_id), Value::Text(label.into())],
        )?;
        Ok(result.changes > 0)
    }

    pub fn get_labels(&self, issue_id: i64) -> Result<Vec<String>> {
        let result = self.backend.execute(
            "SELECT label FROM labels WHERE issue_id = ?1 ORDER BY label",
            &[Value::Integer(issue_id)],
        )?;
        Ok(result
            .rows
            .iter()
            .filter_map(|row| row.get_string(0))
            .collect())
    }

    // ==================== Comments ====================

    pub fn add_comment(&self, issue_id: i64, content: &str) -> Result<i64> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT INTO comments (issue_id, content, created_at) VALUES (?1, ?2, ?3)",
            &[
                Value::Integer(issue_id),
                Value::Text(content.into()),
                Value::Text(now),
            ],
        )?;
        Ok(result.last_insert_rowid)
    }

    pub fn get_comments(&self, issue_id: i64) -> Result<Vec<Comment>> {
        let result = self.backend.execute(
            "SELECT id, issue_id, content, created_at FROM comments WHERE issue_id = ?1 ORDER BY created_at",
            &[Value::Integer(issue_id)],
        )?;
        Ok(result.rows.iter().map(comment_from_row).collect())
    }

    // ==================== Dependencies ====================

    pub fn add_dependency(&self, blocked_id: i64, blocker_id: i64) -> Result<bool> {
        // Prevent self-blocking
        if blocked_id == blocker_id {
            return Err(DbError::Validation(
                "An issue cannot block itself".to_string(),
            ));
        }

        // Check for circular dependencies before inserting
        if self.would_create_cycle(blocked_id, blocker_id)? {
            return Err(DbError::Validation(
                "Adding this dependency would create a circular dependency chain".to_string(),
            ));
        }

        let result = self.backend.execute(
            "INSERT OR IGNORE INTO dependencies (blocker_id, blocked_id) VALUES (?1, ?2)",
            &[Value::Integer(blocker_id), Value::Integer(blocked_id)],
        )?;
        Ok(result.changes > 0)
    }

    /// Check if adding blocker_id -> blocked_id would create a cycle.
    fn would_create_cycle(&self, blocked_id: i64, blocker_id: i64) -> Result<bool> {
        let mut visited = HashSet::new();
        let mut stack = vec![blocked_id];

        while let Some(current) = stack.pop() {
            if current == blocker_id {
                return Ok(true);
            }

            if visited.insert(current) {
                let blocking = self.get_blocking(current)?;
                for next in blocking {
                    if !visited.contains(&next) {
                        stack.push(next);
                    }
                }
            }
        }

        Ok(false)
    }

    pub fn remove_dependency(&self, blocked_id: i64, blocker_id: i64) -> Result<bool> {
        let result = self.backend.execute(
            "DELETE FROM dependencies WHERE blocker_id = ?1 AND blocked_id = ?2",
            &[Value::Integer(blocker_id), Value::Integer(blocked_id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn get_blockers(&self, issue_id: i64) -> Result<Vec<i64>> {
        let result = self.backend.execute(
            "SELECT blocker_id FROM dependencies WHERE blocked_id = ?1",
            &[Value::Integer(issue_id)],
        )?;
        Ok(result
            .rows
            .iter()
            .filter_map(|row| row.get_i64(0).ok())
            .collect())
    }

    pub fn get_blocking(&self, issue_id: i64) -> Result<Vec<i64>> {
        let result = self.backend.execute(
            "SELECT blocked_id FROM dependencies WHERE blocker_id = ?1",
            &[Value::Integer(issue_id)],
        )?;
        Ok(result
            .rows
            .iter()
            .filter_map(|row| row.get_i64(0).ok())
            .collect())
    }

    pub fn list_blocked_issues(&self) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            r#"
            SELECT DISTINCT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at
            FROM issues i
            JOIN dependencies d ON i.id = d.blocked_id
            JOIN issues blocker ON d.blocker_id = blocker.id
            WHERE i.status = 'open' AND blocker.status = 'open'
            ORDER BY i.id
            "#,
            &[],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    pub fn list_ready_issues(&self) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            r#"
            SELECT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at
            FROM issues i
            WHERE i.status = 'open'
            AND NOT EXISTS (
                SELECT 1 FROM dependencies d
                JOIN issues blocker ON d.blocker_id = blocker.id
                WHERE d.blocked_id = i.id AND blocker.status = 'open'
            )
            ORDER BY i.id
            "#,
            &[],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    // ==================== Sessions ====================

    pub fn start_session(&self) -> Result<i64> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT INTO sessions (started_at) VALUES (?1)",
            &[Value::Text(now)],
        )?;
        Ok(result.last_insert_rowid)
    }

    pub fn end_session(&self, id: i64, notes: Option<&str>) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE sessions SET ended_at = ?1, handoff_notes = ?2 WHERE id = ?3",
            &[
                Value::Text(now),
                notes.map(|s| Value::Text(s.into())).unwrap_or(Value::Null),
                Value::Integer(id),
            ],
        )?;
        Ok(result.changes > 0)
    }

    pub fn get_current_session(&self) -> Result<Option<Session>> {
        let result = self.backend.execute(
            "SELECT id, started_at, ended_at, active_issue_id, handoff_notes FROM sessions WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
            &[],
        )?;
        Ok(result.rows.first().map(session_from_row))
    }

    pub fn get_last_session(&self) -> Result<Option<Session>> {
        let result = self.backend.execute(
            "SELECT id, started_at, ended_at, active_issue_id, handoff_notes FROM sessions WHERE ended_at IS NOT NULL ORDER BY id DESC LIMIT 1",
            &[],
        )?;
        Ok(result.rows.first().map(session_from_row))
    }

    pub fn set_session_issue(&self, session_id: i64, issue_id: i64) -> Result<bool> {
        let result = self.backend.execute(
            "UPDATE sessions SET active_issue_id = ?1 WHERE id = ?2",
            &[Value::Integer(issue_id), Value::Integer(session_id)],
        )?;
        Ok(result.changes > 0)
    }

    // ==================== Time Tracking ====================

    pub fn start_timer(&self, issue_id: i64) -> Result<i64> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT INTO time_entries (issue_id, started_at) VALUES (?1, ?2)",
            &[Value::Integer(issue_id), Value::Text(now)],
        )?;
        Ok(result.last_insert_rowid)
    }

    pub fn stop_timer(&self, issue_id: i64) -> Result<bool> {
        let now = current_timestamp();
        let now_str = now.to_rfc3339();

        // Get the active entry
        let result = self.backend.execute(
            "SELECT started_at FROM time_entries WHERE issue_id = ?1 AND ended_at IS NULL",
            &[Value::Integer(issue_id)],
        )?;

        if let Some(row) = result.rows.first() {
            if let Some(started) = row.get_string(0) {
                let start_dt = DateTime::parse_from_rfc3339(&started)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(now);
                let duration = now.signed_duration_since(start_dt).num_seconds();

                let update_result = self.backend.execute(
                    "UPDATE time_entries SET ended_at = ?1, duration_seconds = ?2 WHERE issue_id = ?3 AND ended_at IS NULL",
                    &[Value::Text(now_str), Value::Integer(duration), Value::Integer(issue_id)],
                )?;
                return Ok(update_result.changes > 0);
            }
        }
        Ok(false)
    }

    pub fn get_active_timer(&self) -> Result<Option<(i64, DateTime<Utc>)>> {
        let result = self.backend.execute(
            "SELECT issue_id, started_at FROM time_entries WHERE ended_at IS NULL ORDER BY id DESC LIMIT 1",
            &[],
        )?;

        Ok(result.rows.first().and_then(|row| {
            let id = row.get_i64(0).ok()?;
            let started = row.get_string(1)?;
            Some((id, parse_datetime(started)))
        }))
    }

    pub fn get_total_time(&self, issue_id: i64) -> Result<i64> {
        let result = self.backend.execute(
            "SELECT COALESCE(SUM(duration_seconds), 0) FROM time_entries WHERE issue_id = ?1 AND duration_seconds IS NOT NULL",
            &[Value::Integer(issue_id)],
        )?;
        Ok(result
            .rows
            .first()
            .and_then(|row| row.get_i64(0).ok())
            .unwrap_or(0))
    }

    // ==================== Search ====================

    pub fn search_issues(&self, query: &str) -> Result<Vec<Issue>> {
        // Escape SQL LIKE wildcards to prevent unintended pattern matching
        let escaped = query.replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
        let result = self.backend.execute(
            r#"
            SELECT DISTINCT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at
            FROM issues i
            LEFT JOIN comments c ON i.id = c.issue_id
            WHERE i.title LIKE ?1 ESCAPE '\' COLLATE NOCASE
               OR i.description LIKE ?1 ESCAPE '\' COLLATE NOCASE
               OR c.content LIKE ?1 ESCAPE '\' COLLATE NOCASE
            ORDER BY i.id DESC
            "#,
            &[Value::Text(pattern)],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    // ==================== Relations ====================

    pub fn add_relation(&self, issue_id_1: i64, issue_id_2: i64) -> Result<bool> {
        if issue_id_1 == issue_id_2 {
            return Err(DbError::Validation(
                "Cannot relate an issue to itself".to_string(),
            ));
        }
        // Store with smaller ID first for consistency
        let (a, b) = if issue_id_1 < issue_id_2 {
            (issue_id_1, issue_id_2)
        } else {
            (issue_id_2, issue_id_1)
        };
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT OR IGNORE INTO relations (issue_id_1, issue_id_2, created_at) VALUES (?1, ?2, ?3)",
            &[Value::Integer(a), Value::Integer(b), Value::Text(now)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn remove_relation(&self, issue_id_1: i64, issue_id_2: i64) -> Result<bool> {
        let (a, b) = if issue_id_1 < issue_id_2 {
            (issue_id_1, issue_id_2)
        } else {
            (issue_id_2, issue_id_1)
        };
        let result = self.backend.execute(
            "DELETE FROM relations WHERE issue_id_1 = ?1 AND issue_id_2 = ?2",
            &[Value::Integer(a), Value::Integer(b)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn update_parent(&self, id: i64, parent_id: Option<i64>) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE issues SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
            &[
                parent_id.map(Value::Integer).unwrap_or(Value::Null),
                Value::Text(now),
                Value::Integer(id),
            ],
        )?;
        Ok(result.changes > 0)
    }

    pub fn get_related_issues(&self, issue_id: i64) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            r#"
            SELECT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at
            FROM issues i
            WHERE i.id IN (
                SELECT issue_id_2 FROM relations WHERE issue_id_1 = ?1
                UNION
                SELECT issue_id_1 FROM relations WHERE issue_id_2 = ?1
            )
            ORDER BY i.id
            "#,
            &[Value::Integer(issue_id)],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    // ==================== Milestones ====================

    pub fn create_milestone(&self, name: &str, description: Option<&str>) -> Result<i64> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "INSERT INTO milestones (name, description, status, created_at) VALUES (?1, ?2, 'open', ?3)",
            &[
                Value::Text(name.into()),
                description.map(|s| Value::Text(s.into())).unwrap_or(Value::Null),
                Value::Text(now),
            ],
        )?;
        Ok(result.last_insert_rowid)
    }

    pub fn get_milestone(&self, id: i64) -> Result<Option<Milestone>> {
        let result = self.backend.execute(
            "SELECT id, name, description, status, created_at, closed_at FROM milestones WHERE id = ?1",
            &[Value::Integer(id)],
        )?;
        Ok(result.rows.first().map(milestone_from_row))
    }

    pub fn list_milestones(&self, status: Option<&str>) -> Result<Vec<Milestone>> {
        let sql = if let Some(s) = status {
            if s == "all" {
                "SELECT id, name, description, status, created_at, closed_at FROM milestones ORDER BY id DESC".to_string()
            } else {
                format!("SELECT id, name, description, status, created_at, closed_at FROM milestones WHERE status = '{}' ORDER BY id DESC", s)
            }
        } else {
            "SELECT id, name, description, status, created_at, closed_at FROM milestones WHERE status = 'open' ORDER BY id DESC".to_string()
        };

        let result = self.backend.execute(&sql, &[])?;
        Ok(result.rows.iter().map(milestone_from_row).collect())
    }

    pub fn add_issue_to_milestone(&self, milestone_id: i64, issue_id: i64) -> Result<bool> {
        let result = self.backend.execute(
            "INSERT OR IGNORE INTO milestone_issues (milestone_id, issue_id) VALUES (?1, ?2)",
            &[Value::Integer(milestone_id), Value::Integer(issue_id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn remove_issue_from_milestone(&self, milestone_id: i64, issue_id: i64) -> Result<bool> {
        let result = self.backend.execute(
            "DELETE FROM milestone_issues WHERE milestone_id = ?1 AND issue_id = ?2",
            &[Value::Integer(milestone_id), Value::Integer(issue_id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn get_milestone_issues(&self, milestone_id: i64) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            r#"
            SELECT i.id, i.title, i.description, i.status, i.priority, i.parent_id, i.created_at, i.updated_at, i.closed_at
            FROM issues i
            JOIN milestone_issues mi ON i.id = mi.issue_id
            WHERE mi.milestone_id = ?1
            ORDER BY i.id
            "#,
            &[Value::Integer(milestone_id)],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    pub fn close_milestone(&self, id: i64) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE milestones SET status = 'closed', closed_at = ?1 WHERE id = ?2",
            &[Value::Text(now), Value::Integer(id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn delete_milestone(&self, id: i64) -> Result<bool> {
        let result = self
            .backend
            .execute("DELETE FROM milestones WHERE id = ?1", &[Value::Integer(id)])?;
        Ok(result.changes > 0)
    }

    pub fn get_issue_milestone(&self, issue_id: i64) -> Result<Option<Milestone>> {
        let result = self.backend.execute(
            r#"
            SELECT m.id, m.name, m.description, m.status, m.created_at, m.closed_at
            FROM milestones m
            JOIN milestone_issues mi ON m.id = mi.milestone_id
            WHERE mi.issue_id = ?1
            LIMIT 1
            "#,
            &[Value::Integer(issue_id)],
        )?;
        Ok(result.rows.first().map(milestone_from_row))
    }

    // ==================== Archiving ====================

    pub fn archive_issue(&self, id: i64) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE issues SET status = 'archived', updated_at = ?1 WHERE id = ?2 AND status = 'closed'",
            &[Value::Text(now), Value::Integer(id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn unarchive_issue(&self, id: i64) -> Result<bool> {
        let now = current_timestamp().to_rfc3339();
        let result = self.backend.execute(
            "UPDATE issues SET status = 'closed', updated_at = ?1 WHERE id = ?2 AND status = 'archived'",
            &[Value::Text(now), Value::Integer(id)],
        )?;
        Ok(result.changes > 0)
    }

    pub fn list_archived_issues(&self) -> Result<Vec<Issue>> {
        let result = self.backend.execute(
            "SELECT id, title, description, status, priority, parent_id, created_at, updated_at, closed_at FROM issues WHERE status = 'archived' ORDER BY id DESC",
            &[],
        )?;
        Ok(result.rows.iter().map(issue_from_row).collect())
    }

    pub fn archive_older_than(&self, days: i64) -> Result<i32> {
        let cutoff = current_timestamp() - chrono::Duration::days(days);
        let cutoff_str = cutoff.to_rfc3339();
        let now = current_timestamp().to_rfc3339();

        let result = self.backend.execute(
            "UPDATE issues SET status = 'archived', updated_at = ?1 WHERE status = 'closed' AND closed_at < ?2",
            &[Value::Text(now), Value::Text(cutoff_str)],
        )?;

        Ok(result.changes as i32)
    }
}

// ==================== Helper Functions ====================

fn parse_datetime(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| current_timestamp())
}

fn issue_from_row(row: &Row) -> Issue {
    Issue {
        id: row.get_i64(0).unwrap_or(0),
        title: row.get_string(1).unwrap_or_default(),
        description: row.get_string(2),
        status: row.get_string(3).unwrap_or_else(|| "open".into()),
        priority: row.get_string(4).unwrap_or_else(|| "medium".into()),
        parent_id: row.get_optional_i64(5).ok().flatten(),
        created_at: row.get_string(6).map(parse_datetime).unwrap_or_else(default_timestamp),
        updated_at: row.get_string(7).map(parse_datetime).unwrap_or_else(default_timestamp),
        closed_at: row.get_string(8).map(parse_datetime),
    }
}

fn comment_from_row(row: &Row) -> Comment {
    Comment {
        id: row.get_i64(0).unwrap_or(0),
        issue_id: row.get_i64(1).unwrap_or(0),
        content: row.get_string(2).unwrap_or_default(),
        created_at: row.get_string(3).map(parse_datetime).unwrap_or_else(default_timestamp),
    }
}

fn session_from_row(row: &Row) -> Session {
    Session {
        id: row.get_i64(0).unwrap_or(0),
        started_at: row.get_string(1).map(parse_datetime).unwrap_or_else(default_timestamp),
        ended_at: row.get_string(2).map(parse_datetime),
        active_issue_id: row.get_optional_i64(3).ok().flatten(),
        handoff_notes: row.get_string(4),
    }
}

fn milestone_from_row(row: &Row) -> Milestone {
    Milestone {
        id: row.get_i64(0).unwrap_or(0),
        name: row.get_string(1).unwrap_or_default(),
        description: row.get_string(2),
        status: row.get_string(3).unwrap_or_else(|| "open".into()),
        created_at: row.get_string(4).map(parse_datetime).unwrap_or_else(default_timestamp),
        closed_at: row.get_string(5).map(parse_datetime),
    }
}

#[cfg(all(test, feature = "rusqlite-backend"))]
mod tests {
    use super::*;
    use crate::backend::RusqliteBackend;

    fn setup_test_db() -> Database<RusqliteBackend> {
        let backend = RusqliteBackend::open(":memory:").unwrap();
        Database::new(backend).unwrap()
    }

    // ==================== Issue CRUD Tests ====================

    #[test]
    fn test_create_and_get_issue() {
        let db = setup_test_db();

        let id = db.create_issue("Test issue", None, "medium").unwrap();
        assert!(id > 0);

        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.id, id);
        assert_eq!(issue.title, "Test issue");
        assert_eq!(issue.description, None);
        assert_eq!(issue.status, "open");
        assert_eq!(issue.priority, "medium");
        assert_eq!(issue.parent_id, None);
        assert!(issue.closed_at.is_none());
    }

    #[test]
    fn test_create_issue_with_description() {
        let db = setup_test_db();

        let id = db
            .create_issue("Test issue", Some("Detailed description"), "high")
            .unwrap();
        let issue = db.get_issue(id).unwrap().unwrap();

        assert_eq!(issue.title, "Test issue");
        assert_eq!(issue.description, Some("Detailed description".to_string()));
        assert_eq!(issue.priority, "high");
    }

    #[test]
    fn test_create_subissue() {
        let db = setup_test_db();

        let parent_id = db.create_issue("Parent issue", None, "high").unwrap();
        let child_id = db
            .create_subissue(parent_id, "Child issue", None, "medium")
            .unwrap();

        let child = db.get_issue(child_id).unwrap().unwrap();
        assert_eq!(child.parent_id, Some(parent_id));

        let subissues = db.get_subissues(parent_id).unwrap();
        assert_eq!(subissues.len(), 1);
        assert_eq!(subissues[0].id, child_id);
    }

    #[test]
    fn test_get_nonexistent_issue() {
        let db = setup_test_db();
        let issue = db.get_issue(99999).unwrap();
        assert!(issue.is_none());
    }

    #[test]
    fn test_list_issues() {
        let db = setup_test_db();

        db.create_issue("Issue 1", None, "low").unwrap();
        db.create_issue("Issue 2", None, "medium").unwrap();
        db.create_issue("Issue 3", None, "high").unwrap();

        let issues = db.list_issues(None, None, None).unwrap();
        assert_eq!(issues.len(), 3);
    }

    #[test]
    fn test_update_issue() {
        let db = setup_test_db();

        let id = db.create_issue("Original", None, "low").unwrap();
        db.update_issue(id, Some("Updated"), Some("New desc"), Some("high"))
            .unwrap();

        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.title, "Updated");
        assert_eq!(issue.description, Some("New desc".to_string()));
        assert_eq!(issue.priority, "high");
    }

    #[test]
    fn test_close_and_reopen_issue() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();

        db.close_issue(id).unwrap();
        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.status, "closed");
        assert!(issue.closed_at.is_some());

        db.reopen_issue(id).unwrap();
        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.status, "open");
    }

    #[test]
    fn test_delete_issue() {
        let db = setup_test_db();

        let id = db.create_issue("To delete", None, "low").unwrap();
        assert!(db.get_issue(id).unwrap().is_some());

        db.delete_issue(id).unwrap();
        assert!(db.get_issue(id).unwrap().is_none());
    }

    // ==================== Labels Tests ====================

    #[test]
    fn test_add_and_get_labels() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        db.add_label(id, "bug").unwrap();
        db.add_label(id, "urgent").unwrap();

        let labels = db.get_labels(id).unwrap();
        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"bug".to_string()));
        assert!(labels.contains(&"urgent".to_string()));
    }

    #[test]
    fn test_remove_label() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        db.add_label(id, "bug").unwrap();
        db.add_label(id, "urgent").unwrap();

        db.remove_label(id, "bug").unwrap();
        let labels = db.get_labels(id).unwrap();
        assert_eq!(labels.len(), 1);
        assert!(labels.contains(&"urgent".to_string()));
    }

    // ==================== Comments Tests ====================

    #[test]
    fn test_add_and_get_comments() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        db.add_comment(id, "First comment").unwrap();
        db.add_comment(id, "Second comment").unwrap();

        let comments = db.get_comments(id).unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].content, "First comment");
        assert_eq!(comments[1].content, "Second comment");
    }

    // ==================== Dependencies Tests ====================

    #[test]
    fn test_add_dependency() {
        let db = setup_test_db();

        let a = db.create_issue("A", None, "medium").unwrap();
        let b = db.create_issue("B", None, "medium").unwrap();

        db.add_dependency(a, b).unwrap(); // B blocks A

        let blockers = db.get_blockers(a).unwrap();
        assert_eq!(blockers, vec![b]);

        let blocking = db.get_blocking(b).unwrap();
        assert_eq!(blocking, vec![a]);
    }

    #[test]
    fn test_self_blocking_prevented() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        let result = db.add_dependency(id, id);
        assert!(result.is_err());
    }

    #[test]
    fn test_circular_dependency_prevented() {
        let db = setup_test_db();

        let a = db.create_issue("A", None, "medium").unwrap();
        let b = db.create_issue("B", None, "medium").unwrap();
        let c = db.create_issue("C", None, "medium").unwrap();

        db.add_dependency(a, b).unwrap(); // B blocks A
        db.add_dependency(b, c).unwrap(); // C blocks B

        let result = db.add_dependency(c, a); // A blocks C - would create cycle
        assert!(result.is_err());
    }

    // ==================== Sessions Tests ====================

    #[test]
    fn test_session_lifecycle() {
        let db = setup_test_db();

        let id = db.start_session().unwrap();
        assert!(id > 0);

        let session = db.get_current_session().unwrap().unwrap();
        assert_eq!(session.id, id);
        assert!(session.ended_at.is_none());

        db.end_session(id, Some("Test notes")).unwrap();
        assert!(db.get_current_session().unwrap().is_none());

        let last = db.get_last_session().unwrap().unwrap();
        assert_eq!(last.id, id);
        assert_eq!(last.handoff_notes, Some("Test notes".to_string()));
    }

    // ==================== Search Tests ====================

    #[test]
    fn test_search_issues() {
        let db = setup_test_db();

        db.create_issue("Bug in login", None, "high").unwrap();
        db.create_issue("Feature request", Some("Add login history"), "medium")
            .unwrap();
        db.create_issue("Unrelated", None, "low").unwrap();

        let results = db.search_issues("login").unwrap();
        assert_eq!(results.len(), 2);
    }

    // ==================== Milestones Tests ====================

    #[test]
    fn test_milestone_crud() {
        let db = setup_test_db();

        let id = db.create_milestone("v1.0", Some("First release")).unwrap();
        let milestone = db.get_milestone(id).unwrap().unwrap();
        assert_eq!(milestone.name, "v1.0");
        assert_eq!(milestone.description, Some("First release".to_string()));

        db.close_milestone(id).unwrap();
        let milestone = db.get_milestone(id).unwrap().unwrap();
        assert_eq!(milestone.status, "closed");

        db.delete_milestone(id).unwrap();
        assert!(db.get_milestone(id).unwrap().is_none());
    }

    #[test]
    fn test_milestone_issues() {
        let db = setup_test_db();

        let milestone_id = db.create_milestone("Sprint 1", None).unwrap();
        let issue_id = db.create_issue("Task 1", None, "medium").unwrap();

        db.add_issue_to_milestone(milestone_id, issue_id).unwrap();
        let issues = db.get_milestone_issues(milestone_id).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, issue_id);

        let issue_milestone = db.get_issue_milestone(issue_id).unwrap().unwrap();
        assert_eq!(issue_milestone.id, milestone_id);
    }

    // ==================== Archiving Tests ====================

    #[test]
    fn test_archive_issue() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        db.close_issue(id).unwrap();
        db.archive_issue(id).unwrap();

        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.status, "archived");

        let archived = db.list_archived_issues().unwrap();
        assert_eq!(archived.len(), 1);
    }

    #[test]
    fn test_unarchive_issue() {
        let db = setup_test_db();

        let id = db.create_issue("Test", None, "medium").unwrap();
        db.close_issue(id).unwrap();
        db.archive_issue(id).unwrap();
        db.unarchive_issue(id).unwrap();

        let issue = db.get_issue(id).unwrap().unwrap();
        assert_eq!(issue.status, "closed");
    }
}
