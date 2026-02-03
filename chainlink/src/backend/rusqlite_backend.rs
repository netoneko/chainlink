//! Rusqlite backend implementation
//!
//! This module provides a DatabaseBackend implementation using rusqlite.

use rusqlite::{params_from_iter, Connection, ToSql};
use std::cell::Cell;
use std::path::Path;

use super::{BackendError, DatabaseBackend, QueryResult, Row, Value};

/// Rusqlite-based database backend
pub struct RusqliteBackend {
    conn: Connection,
    last_rowid: Cell<i64>,
}

impl RusqliteBackend {
    /// Create a new backend from an existing connection
    pub fn from_connection(conn: Connection) -> Self {
        Self {
            conn,
            last_rowid: Cell::new(0),
        }
    }

    /// Get a reference to the underlying connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

/// Convert our Value type to rusqlite's ToSql
fn value_to_boxed(v: &Value) -> Box<dyn ToSql> {
    match v {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Integer(i) => Box::new(*i),
        Value::Text(s) => Box::new(s.clone()),
    }
}

impl DatabaseBackend for RusqliteBackend {
    fn open(path: &str) -> Result<Self, BackendError> {
        let conn =
            Connection::open(Path::new(path)).map_err(|e| BackendError::new(e.to_string()))?;
        Ok(Self {
            conn,
            last_rowid: Cell::new(0),
        })
    }

    fn execute(&self, sql: &str, params: &[Value]) -> Result<QueryResult, BackendError> {
        // Convert params to boxed ToSql values
        let boxed_params: Vec<Box<dyn ToSql>> = params.iter().map(value_to_boxed).collect();
        let param_refs: Vec<&dyn ToSql> = boxed_params.iter().map(|b| b.as_ref()).collect();

        // Check if this is a SELECT-like statement that returns rows
        let sql_upper = sql.trim().to_uppercase();
        let is_query = sql_upper.starts_with("SELECT")
            || sql_upper.starts_with("PRAGMA")
            || sql_upper.starts_with("WITH");

        if is_query {
            // Prepare statement and get column info
            let mut stmt = self
                .conn
                .prepare(sql)
                .map_err(|e| BackendError::new(e.to_string()))?;

            // Get column names
            let columns: Vec<String> = stmt
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();

            // Execute and collect rows
            let rows_result = stmt.query_map(params_from_iter(param_refs), |row| {
                let mut values = Vec::new();
                for i in 0..columns.len() {
                    // Get the value as a reference and convert to String
                    let val: Option<String> = match row.get_ref(i) {
                        Ok(value_ref) => {
                            use rusqlite::types::ValueRef;
                            match value_ref {
                                ValueRef::Null => None,
                                ValueRef::Integer(n) => Some(n.to_string()),
                                ValueRef::Real(r) => Some(r.to_string()),
                                ValueRef::Text(s) => {
                                    Some(std::str::from_utf8(s).unwrap_or("").to_string())
                                }
                                ValueRef::Blob(_) => Some("[blob]".to_string()),
                            }
                        }
                        Err(_) => None,
                    };
                    values.push(val);
                }
                Ok(Row::new(values))
            });

            let rows: Vec<Row> = match rows_result {
                Ok(iter) => iter
                    .filter_map(|r| r.ok())
                    .collect(),
                Err(e) => return Err(BackendError::new(e.to_string())),
            };

            let last_rowid = self.conn.last_insert_rowid();
            self.last_rowid.set(last_rowid);

            Ok(QueryResult::with_rows(columns, rows, last_rowid))
        } else {
            // Non-query statement (INSERT, UPDATE, DELETE, etc.)
            let changes = self
                .conn
                .execute(sql, params_from_iter(param_refs))
                .map_err(|e| BackendError::new(e.to_string()))?;

            let last_rowid = self.conn.last_insert_rowid();
            self.last_rowid.set(last_rowid);

            Ok(QueryResult::empty(changes as u64, last_rowid))
        }
    }

    fn execute_batch(&self, sql: &str) -> Result<(), BackendError> {
        self.conn
            .execute_batch(sql)
            .map_err(|e| BackendError::new(e.to_string()))
    }

    fn last_insert_rowid(&self) -> i64 {
        self.last_rowid.get()
    }

    fn transaction<T, F>(&self, f: F) -> Result<T, BackendError>
    where
        F: FnOnce() -> Result<T, BackendError>,
    {
        self.conn
            .execute("BEGIN TRANSACTION", [])
            .map_err(|e| BackendError::new(e.to_string()))?;

        match f() {
            Ok(result) => {
                self.conn
                    .execute("COMMIT", [])
                    .map_err(|e| BackendError::new(e.to_string()))?;
                Ok(result)
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_execute() {
        let backend = RusqliteBackend::open(":memory:").unwrap();

        // Create a table
        backend
            .execute_batch("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();

        // Insert a row
        let result = backend
            .execute(
                "INSERT INTO test (name) VALUES (?1)",
                &[Value::Text("hello".into())],
            )
            .unwrap();
        assert_eq!(result.changes, 1);
        assert_eq!(result.last_insert_rowid, 1);

        // Query it back
        let result = backend
            .execute("SELECT id, name FROM test", &[])
            .unwrap();
        assert_eq!(result.columns, vec!["id", "name"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].get_i64(0).unwrap(), 1);
        assert_eq!(result.rows[0].get_str(1), Some("hello"));
    }

    #[test]
    fn test_null_values() {
        let backend = RusqliteBackend::open(":memory:").unwrap();

        backend
            .execute_batch("CREATE TABLE test (id INTEGER, name TEXT)")
            .unwrap();

        // Insert a NULL
        backend
            .execute(
                "INSERT INTO test (id, name) VALUES (?1, ?2)",
                &[Value::Integer(1), Value::Null],
            )
            .unwrap();

        let result = backend.execute("SELECT * FROM test", &[]).unwrap();
        assert_eq!(result.rows[0].get_i64(0).unwrap(), 1);
        assert!(result.rows[0].get_str(1).is_none());
    }

    #[test]
    fn test_transaction() {
        let backend = RusqliteBackend::open(":memory:").unwrap();

        backend
            .execute_batch("CREATE TABLE test (id INTEGER PRIMARY KEY)")
            .unwrap();

        // Successful transaction
        let result = backend.transaction(|| {
            backend.execute("INSERT INTO test (id) VALUES (1)", &[])?;
            backend.execute("INSERT INTO test (id) VALUES (2)", &[])?;
            Ok(())
        });
        assert!(result.is_ok());

        let count = backend.execute("SELECT COUNT(*) FROM test", &[]).unwrap();
        assert_eq!(count.rows[0].get_str(0), Some("2"));

        // Failed transaction (should rollback)
        let result: Result<(), BackendError> = backend.transaction(|| {
            backend.execute("INSERT INTO test (id) VALUES (3)", &[])?;
            Err(BackendError::new("rollback"))
        });
        assert!(result.is_err());

        // Should still be 2 rows
        let count = backend.execute("SELECT COUNT(*) FROM test", &[]).unwrap();
        assert_eq!(count.rows[0].get_str(0), Some("2"));
    }
}
