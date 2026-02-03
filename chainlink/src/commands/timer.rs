use crate::commands::CmdResult;
use crate::db::DbError;
#[cfg(feature = "std")]
use chrono::Utc;
#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

pub fn start<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, out: &impl Output) -> CmdResult<()> {
    // Verify issue exists
    let issue = match db.get_issue(issue_id)? {
        Some(i) => i,
        None => return Err(DbError::Validation(format!("Issue #{} not found", issue_id)).into()),
    };

    // Check if there's already an active timer
    if let Some((active_id, _)) = db.get_active_timer()? {
        if active_id == issue_id {
            return Err(DbError::Validation(format!("Timer already running for issue #{}", issue_id)).into());
        } else {
            return Err(DbError::Validation(format!(
                "Timer already running for issue #{}. Stop it first with 'chainlink stop'.",
                active_id
            )).into());
        }
    }

    db.start_timer(issue_id)?;
    out_println!(out, "Started timer for #{}: {}", issue_id, issue.title);
    out_println!(out, "Run 'chainlink stop' when done.");

    Ok(())
}

pub fn stop<B: DatabaseBackend>(db: &Database<B>, out: &impl Output) -> CmdResult<()> {
    let (issue_id, started_at) = match db.get_active_timer()? {
        Some(a) => a,
        None => return Err(DbError::Validation("No timer running. Start one with 'chainlink start <id>'.".into()).into()),
    };
    #[cfg(feature = "std")]
    let duration = Utc::now().signed_duration_since(started_at);
    #[cfg(not(feature = "std"))]
    let duration = {
        // In no_std mode, we can't calculate duration accurately
        // Use a zero duration as fallback
        chrono::Duration::seconds(0)
    };

    db.stop_timer(issue_id)?;

    let issue = db.get_issue(issue_id)?;
    let title = issue
        .map(|i| i.title)
        .unwrap_or_else(|| "(deleted)".to_string());

    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    out_println!(out, "Stopped timer for #{}: {}", issue_id, title);
    out_println!(out, "Time spent: {}h {}m {}s", hours, minutes, seconds);

    // Show total time for this issue
    let total = db.get_total_time(issue_id)?;
    let total_hours = total / 3600;
    let total_minutes = (total % 3600) / 60;
    out_println!(
        out,
        "Total time on this issue: {}h {}m",
        total_hours,
        total_minutes
    );

    Ok(())
}

pub fn status<B: DatabaseBackend>(db: &Database<B>, out: &impl Output) -> CmdResult<()> {
    let active = db.get_active_timer()?;

    match active {
        Some((issue_id, started_at)) => {
            #[cfg(feature = "std")]
            let duration = Utc::now().signed_duration_since(started_at);
            #[cfg(not(feature = "std"))]
            let duration = {
                // In no_std mode, we can't calculate duration accurately
                // Use a zero duration as fallback
                chrono::Duration::seconds(0)
            };
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            let seconds = duration.num_seconds() % 60;

            let issue = db.get_issue(issue_id)?;
            let title = issue
                .map(|i| i.title)
                .unwrap_or_else(|| "(deleted)".to_string());

            out_println!(out, "Timer running: #{} {}", issue_id, title);
            out_println!(out, "Elapsed: {}h {}m {}s", hours, minutes, seconds);
        }
        None => {
            out_println!(out, "No timer running.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::RusqliteBackend;
    use crate::output::StdOutput;
    use proptest::prelude::*;
    use tempfile::tempdir;

    fn setup_test_db() -> (Database<RusqliteBackend>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        (db, dir)
    }

    #[test]
    fn test_start_timer() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Test issue", None, "medium").unwrap();

        let result = start(&db, id, &StdOutput);
        assert!(result.is_ok());

        let active = db.get_active_timer().unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().0, id);
    }

    #[test]
    fn test_start_nonexistent_issue() {
        let (db, _dir) = setup_test_db();

        let result = start(&db, 99999, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_start_timer_already_running() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Test issue", None, "medium").unwrap();

        start(&db, id, &StdOutput).unwrap();
        let result = start(&db, id, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already running"));
    }

    #[test]
    fn test_start_timer_different_issue_running() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        start(&db, id1, &StdOutput).unwrap();
        let result = start(&db, id2, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Stop it first"));
    }

    #[test]
    fn test_stop_timer() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Test issue", None, "medium").unwrap();

        start(&db, id, &StdOutput).unwrap();
        let result = stop(&db, &StdOutput);
        assert!(result.is_ok());

        let active = db.get_active_timer().unwrap();
        assert!(active.is_none());
    }

    #[test]
    fn test_stop_no_timer() {
        let (db, _dir) = setup_test_db();

        let result = stop(&db, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No timer running"));
    }

    #[test]
    fn test_status_no_timer() {
        let (db, _dir) = setup_test_db();

        let result = status(&db, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_with_timer() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Test issue", None, "medium").unwrap();

        start(&db, id, &StdOutput).unwrap();
        let result = status(&db, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_timer_workflow() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Test issue", None, "medium").unwrap();

        start(&db, id, &StdOutput).unwrap();
        status(&db, &StdOutput).unwrap();
        stop(&db, &StdOutput).unwrap();

        let active = db.get_active_timer().unwrap();
        assert!(active.is_none());
    }

    proptest! {
        #[test]
        fn prop_start_stop_roundtrip(idx in 0usize..5) {
            let (db, _dir) = setup_test_db();
            let ids: Vec<i64> = (0..5).map(|i| db.create_issue(&format!("Issue {}", i), None, "medium").unwrap()).collect();
            let id = ids[idx];

            start(&db, id, &StdOutput).unwrap();
            prop_assert!(db.get_active_timer().unwrap().is_some());

            stop(&db, &StdOutput).unwrap();
            prop_assert!(db.get_active_timer().unwrap().is_none());
        }
    }
}
