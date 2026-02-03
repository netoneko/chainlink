//! Database backend abstraction layer
//!
//! This module provides a trait-based abstraction over SQLite implementations,
//! allowing chainlink to work with different SQLite libraries (rusqlite, custom OS implementations, etc.)
//!
//! # Implementing a Custom Backend
//!
//! To use chainlink with a custom SQLite library, implement the [`DatabaseBackend`] trait:
//!
//! ```ignore
//! use chainlink::backend::{BackendError, DatabaseBackend, QueryResult, Row, Value};
//!
//! pub struct MyBackend {
//!     // Your SQLite connection handle
//!     db: *mut MyDatabase,
//! }
//!
//! impl DatabaseBackend for MyBackend {
//!     fn open(path: &str) -> Result<Self, BackendError> {
//!         // Open the database at the given path
//!         let db = my_sqlite_open(path)
//!             .map_err(|e| BackendError::new(format!("Failed to open: {}", e)))?;
//!         Ok(Self { db })
//!     }
//!
//!     fn execute(&self, sql: &str, params: &[Value]) -> Result<QueryResult, BackendError> {
//!         // Convert params from Value enum to your library's format
//!         let converted_params: Vec<_> = params.iter().map(|p| {
//!             match p {
//!                 Value::Null => MyParam::Null,
//!                 Value::Integer(n) => MyParam::Int(*n),
//!                 Value::Text(s) => MyParam::Text(s.clone()),
//!             }
//!         }).collect();
//!
//!         // Execute the query
//!         let result = my_sqlite_execute(self.db, sql, &converted_params)
//!             .map_err(|e| BackendError::new(e.to_string()))?;
//!
//!         // Convert results to Row format
//!         let rows: Vec<Row> = result.rows.iter().map(|r| {
//!             let values: Vec<Option<String>> = r.columns.iter()
//!                 .map(|c| c.as_string()) // Convert to Option<String>
//!                 .collect();
//!             Row::new(values)
//!         }).collect();
//!
//!         Ok(QueryResult {
//!             columns: result.column_names,
//!             rows,
//!             changes: result.changes_count,
//!             last_insert_rowid: result.last_rowid,
//!         })
//!     }
//!
//!     fn execute_batch(&self, sql: &str) -> Result<(), BackendError> {
//!         // Execute multiple statements (for schema creation)
//!         my_sqlite_exec(self.db, sql)
//!             .map_err(|e| BackendError::new(e.to_string()))
//!     }
//!
//!     fn last_insert_rowid(&self) -> i64 {
//!         my_sqlite_last_rowid(self.db)
//!     }
//! }
//! ```
//!
//! # Using with Chainlink
//!
//! Once you have a backend implementation, use it with the Database struct:
//!
//! ```ignore
//! use chainlink::db::Database;
//!
//! // Create a database with your custom backend
//! let db = Database::<MyBackend>::open("/path/to/db").unwrap();
//!
//! // Use all the normal chainlink operations
//! let id = db.create_issue("Bug fix", None, "high").unwrap();
//! db.add_label(id, "bug").unwrap();
//! let issue = db.get_issue(id).unwrap();
//! ```
//!
//! # Key Implementation Notes
//!
//! 1. **Row Values**: All column values should be converted to `Option<String>`.
//!    Integers become their string representation ("42"), NULLs become `None`.
//!
//! 2. **Parameter Binding**: Parameters use SQLite's positional syntax (?1, ?2, etc.).
//!    The `params` slice is 0-indexed, mapping to ?1 at index 0.
//!
//! 3. **Transactions**: The default `transaction()` implementation uses BEGIN/COMMIT/ROLLBACK.
//!    Override if your backend needs different transaction handling.
//!
//! 4. **Error Handling**: Wrap all errors in [`BackendError`]. Include enough context
//!    for debugging (e.g., the SQL that failed).
//!
//! 5. **`execute_batch`**: Must handle multiple semicolon-separated statements.
//!    Used for schema creation during database initialization.
//!
//! # no_std Support
//!
//! This module is `no_std` compatible when compiled without the `std` feature.
//! All collections use the `alloc` crate.

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

use core::fmt;

#[cfg(feature = "rusqlite-backend")]
pub mod rusqlite_backend;

#[cfg(feature = "rusqlite-backend")]
pub use rusqlite_backend::RusqliteBackend;

/// Error type for backend operations
#[derive(Debug)]
pub struct BackendError {
    pub message: String,
}

impl BackendError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BackendError {}

/// Parameter values for SQL queries
#[derive(Debug, Clone)]
pub enum Value {
    /// NULL value
    Null,
    /// 64-bit integer
    Integer(i64),
    /// Text string
    Text(String),
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.into())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => Value::Null,
        }
    }
}

/// A single row returned from a query
#[derive(Debug, Clone)]
pub struct Row {
    /// Column values in order
    pub values: Vec<Option<String>>,
}

impl Row {
    /// Create a new row with the given values
    pub fn new(values: Vec<Option<String>>) -> Self {
        Self { values }
    }

    /// Get a value at the given column index as a string
    pub fn get_str(&self, idx: usize) -> Option<&str> {
        self.values.get(idx).and_then(|v| v.as_deref())
    }

    /// Get a value at the given column index as an i64
    pub fn get_i64(&self, idx: usize) -> Result<i64, BackendError> {
        let s = self
            .get_str(idx)
            .ok_or_else(|| BackendError::new("NULL value"))?;
        s.parse()
            .map_err(|_| BackendError::new("Failed to parse integer"))
    }

    /// Get a value at the given column index as an optional i64
    pub fn get_optional_i64(&self, idx: usize) -> Result<Option<i64>, BackendError> {
        match self.values.get(idx) {
            Some(Some(s)) => s
                .parse()
                .map(Some)
                .map_err(|_| BackendError::new("Failed to parse integer")),
            Some(None) | None => Ok(None),
        }
    }

    /// Get a value at the given column index as an optional string
    pub fn get_optional_str(&self, idx: usize) -> Option<&str> {
        self.values.get(idx).and_then(|v| v.as_deref())
    }

    /// Get a value at the given column index, returning None if NULL or missing
    pub fn get_string(&self, idx: usize) -> Option<String> {
        self.values.get(idx).and_then(|v| v.clone())
    }
}

/// Result of executing a SQL statement
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Column names
    pub columns: Vec<String>,
    /// Rows returned by the query
    pub rows: Vec<Row>,
    /// Number of rows affected (for INSERT/UPDATE/DELETE)
    pub changes: u64,
    /// Row ID of the last inserted row
    pub last_insert_rowid: i64,
}

impl QueryResult {
    /// Create an empty result (for statements that don't return rows)
    pub fn empty(changes: u64, last_insert_rowid: i64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            changes,
            last_insert_rowid,
        }
    }

    /// Create a result with rows
    pub fn with_rows(columns: Vec<String>, rows: Vec<Row>, last_insert_rowid: i64) -> Self {
        Self {
            columns,
            rows,
            changes: 0,
            last_insert_rowid,
        }
    }
}

/// Database backend trait
///
/// Implement this trait to provide a custom SQLite backend for chainlink.
///
/// # Example
///
/// ```ignore
/// use chainlink::backend::{DatabaseBackend, QueryResult, Value, BackendError};
///
/// struct MyBackend {
///     // your SQLite connection
/// }
///
/// impl DatabaseBackend for MyBackend {
///     fn open(path: &str) -> Result<Self, BackendError> {
///         // Open database at path
///         Ok(MyBackend { /* ... */ })
///     }
///
///     fn execute(&self, sql: &str, params: &[Value]) -> Result<QueryResult, BackendError> {
///         // Execute SQL with parameters and return results
///         todo!()
///     }
///
///     fn execute_batch(&self, sql: &str) -> Result<(), BackendError> {
///         // Execute multiple SQL statements
///         todo!()
///     }
///
///     fn last_insert_rowid(&self) -> i64 {
///         // Return the rowid of the last inserted row
///         todo!()
///     }
/// }
/// ```
pub trait DatabaseBackend: Sized {
    /// Open a database at the given path
    fn open(path: &str) -> Result<Self, BackendError>;

    /// Execute a SQL statement with parameters and return results
    ///
    /// The parameters use SQLite's ?1, ?2, etc. positional syntax.
    fn execute(&self, sql: &str, params: &[Value]) -> Result<QueryResult, BackendError>;

    /// Execute multiple SQL statements (no parameters, no results)
    ///
    /// Used for schema creation and migrations.
    fn execute_batch(&self, sql: &str) -> Result<(), BackendError>;

    /// Get the rowid of the last inserted row
    fn last_insert_rowid(&self) -> i64;

    /// Execute a function within a transaction
    ///
    /// If the function returns Ok, the transaction is committed.
    /// If the function returns Err, the transaction is rolled back.
    fn transaction<T, F>(&self, f: F) -> Result<T, BackendError>
    where
        F: FnOnce() -> Result<T, BackendError>,
    {
        self.execute("BEGIN TRANSACTION", &[])?;
        match f() {
            Ok(result) => {
                self.execute("COMMIT", &[])?;
                Ok(result)
            }
            Err(e) => {
                let _ = self.execute("ROLLBACK", &[]);
                Err(e)
            }
        }
    }
}
