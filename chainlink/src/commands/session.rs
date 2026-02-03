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

pub fn start<B: DatabaseBackend>(db: &Database<B>, out: &impl Output) -> CmdResult<()> {
    // Check if there's already an active session
    if let Some(current) = db.get_current_session()? {
        out_println!(
            out,
            "Session #{} is already active (started {})",
            current.id,
            current.started_at.format("%Y-%m-%d %H:%M")
        );
        return Ok(());
    }

    // Show previous session's handoff notes
    if let Some(last) = db.get_last_session()? {
        if let Some(ended) = last.ended_at {
            out_println!(out, "Previous session ended: {}", ended.format("%Y-%m-%d %H:%M"));
        }
        if let Some(notes) = &last.handoff_notes {
            if !notes.is_empty() {
                out_println!(out, "Handoff notes:");
                for line in notes.lines() {
                    out_println!(out, "  {}", line);
                }
                out_println!(out);
            }
        }
    }

    let id = db.start_session()?;
    out_println!(out, "Session #{} started.", id);
    Ok(())
}

pub fn end<B: DatabaseBackend>(db: &Database<B>, notes: Option<&str>, out: &impl Output) -> CmdResult<()> {
    let session = match db.get_current_session()? {
        Some(s) => s,
        None => return Err(DbError::Validation("No active session".into()).into()),
    };

    db.end_session(session.id, notes)?;
    out_println!(out, "Session #{} ended.", session.id);
    if notes.is_some() {
        out_println!(out, "Handoff notes saved.");
    }
    Ok(())
}

pub fn status<B: DatabaseBackend>(db: &Database<B>, out: &impl Output) -> CmdResult<()> {
    let session = match db.get_current_session()? {
        Some(s) => s,
        None => {
            out_println!(out, "No active session. Use 'chainlink session start' to begin.");
            return Ok(());
        }
    };

    #[cfg(feature = "std")]
    let minutes = {
        let duration = Utc::now() - session.started_at;
        duration.num_minutes()
    };
    #[cfg(not(feature = "std"))]
    let minutes = 0i64;

    out_println!(
        out,
        "Session #{} (started {})",
        session.id,
        session.started_at.format("%Y-%m-%d %H:%M")
    );

    if let Some(issue_id) = session.active_issue_id {
        if let Some(issue) = db.get_issue(issue_id)? {
            out_println!(out, "Working on: #{} {}", issue.id, issue.title);
        } else {
            out_println!(out, "Working on: #{} (issue not found)", issue_id);
        }
    } else {
        out_println!(out, "Working on: (none)");
    }

    out_println!(out, "Duration: {} minutes", minutes);
    Ok(())
}

pub fn work<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, out: &impl Output) -> CmdResult<()> {
    let session = match db.get_current_session()? {
        Some(s) => s,
        None => return Err(DbError::Validation("No active session. Use 'chainlink session start' first.".into()).into()),
    };

    let issue = match db.get_issue(issue_id)? {
        Some(i) => i,
        None => return Err(DbError::Validation(format!("Issue #{} not found", issue_id)).into()),
    };

    db.set_session_issue(session.id, issue_id)?;
    out_println!(out, "Now working on: #{} {}", issue.id, issue.title);
    Ok(())
}

pub fn last_handoff<B: DatabaseBackend>(db: &Database<B>, out: &impl Output) -> CmdResult<()> {
    match db.get_last_session()? {
        Some(session) => {
            if let Some(notes) = &session.handoff_notes {
                if !notes.is_empty() {
                    out_println!(out, "{}", notes);
                    return Ok(());
                }
            }
            out_println!(out, "No previous handoff notes.");
        }
        None => {
            out_println!(out, "No previous session found.");
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

    // ==================== Start Tests ====================

    #[test]
    fn test_start_session() {
        let (db, _dir) = setup_test_db();

        let result = start(&db, &StdOutput);
        assert!(result.is_ok());

        let session = db.get_current_session().unwrap();
        assert!(session.is_some());
    }

    #[test]
    fn test_start_already_active() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        let first_session = db.get_current_session().unwrap().unwrap();

        // Starting again should not create new session
        let result = start(&db, &StdOutput);
        assert!(result.is_ok());

        let current = db.get_current_session().unwrap().unwrap();
        assert_eq!(current.id, first_session.id);
    }

    // ==================== End Tests ====================

    #[test]
    fn test_end_session() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        let result = end(&db, None, &StdOutput);
        assert!(result.is_ok());

        let session = db.get_current_session().unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_end_session_with_notes() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        let result = end(&db, Some("Completed auth feature"), &StdOutput);
        assert!(result.is_ok());

        let last = db.get_last_session().unwrap().unwrap();
        assert_eq!(
            last.handoff_notes,
            Some("Completed auth feature".to_string())
        );
    }

    #[test]
    fn test_end_no_active_session() {
        let (db, _dir) = setup_test_db();

        let result = end(&db, None, &StdOutput);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No active session"));
    }

    // ==================== Status Tests ====================

    #[test]
    fn test_status_no_session() {
        let (db, _dir) = setup_test_db();

        let result = status(&db, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_with_session() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        let result = status(&db, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_with_active_issue() {
        let (db, _dir) = setup_test_db();

        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        start(&db, &StdOutput).unwrap();
        work(&db, issue_id, &StdOutput).unwrap();

        let result = status(&db, &StdOutput);
        assert!(result.is_ok());
    }

    // ==================== Work Tests ====================

    #[test]
    fn test_work_sets_active_issue() {
        let (db, _dir) = setup_test_db();

        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        start(&db, &StdOutput).unwrap();

        let result = work(&db, issue_id, &StdOutput);
        assert!(result.is_ok());

        let session = db.get_current_session().unwrap().unwrap();
        assert_eq!(session.active_issue_id, Some(issue_id));
    }

    #[test]
    fn test_work_no_session() {
        let (db, _dir) = setup_test_db();

        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();

        let result = work(&db, issue_id, &StdOutput);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No active session"));
    }

    #[test]
    fn test_work_nonexistent_issue() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();

        let result = work(&db, 99999, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_work_change_active_issue() {
        let (db, _dir) = setup_test_db();

        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();
        start(&db, &StdOutput).unwrap();

        work(&db, issue1, &StdOutput).unwrap();
        let session = db.get_current_session().unwrap().unwrap();
        assert_eq!(session.active_issue_id, Some(issue1));

        work(&db, issue2, &StdOutput).unwrap();
        let session = db.get_current_session().unwrap().unwrap();
        assert_eq!(session.active_issue_id, Some(issue2));
    }

    // ==================== Last Handoff Tests ====================

    #[test]
    fn test_last_handoff_no_sessions() {
        let (db, _dir) = setup_test_db();

        let result = last_handoff(&db, &StdOutput);
        assert!(result.is_ok());
        // Should handle gracefully when no sessions exist
    }

    #[test]
    fn test_last_handoff_no_notes() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        end(&db, None, &StdOutput).unwrap();

        let result = last_handoff(&db, &StdOutput);
        assert!(result.is_ok());
        // Should handle gracefully when last session has no notes
    }

    #[test]
    fn test_last_handoff_with_notes() {
        let (db, _dir) = setup_test_db();

        start(&db, &StdOutput).unwrap();
        end(&db, Some("Important handoff notes"), &StdOutput).unwrap();

        let result = last_handoff(&db, &StdOutput);
        assert!(result.is_ok());
        // Notes should be retrievable
        let last = db.get_last_session().unwrap().unwrap();
        assert_eq!(
            last.handoff_notes,
            Some("Important handoff notes".to_string())
        );
    }

    // ==================== Full Workflow Tests ====================

    #[test]
    fn test_full_session_workflow() {
        let (db, _dir) = setup_test_db();

        // Start session
        start(&db, &StdOutput).unwrap();
        assert!(db.get_current_session().unwrap().is_some());

        // Create and work on issue
        let issue_id = db.create_issue("Feature", None, "high").unwrap();
        work(&db, issue_id, &StdOutput).unwrap();

        // Check status
        status(&db, &StdOutput).unwrap();

        // End with notes
        end(&db, Some("Made progress on feature"), &StdOutput).unwrap();
        assert!(db.get_current_session().unwrap().is_none());

        // Start new session
        start(&db, &StdOutput).unwrap();
        let last = db.get_last_session().unwrap().unwrap();
        assert_eq!(
            last.handoff_notes,
            Some("Made progress on feature".to_string())
        );
    }

    // ==================== Property-Based Tests ====================

    proptest! {
        #[test]
        fn prop_start_end_cycle(iterations in 1usize..5) {
            let (db, _dir) = setup_test_db();

            for _ in 0..iterations {
                start(&db, &StdOutput).unwrap();
                prop_assert!(db.get_current_session().unwrap().is_some());
                end(&db, None, &StdOutput).unwrap();
                prop_assert!(db.get_current_session().unwrap().is_none());
            }
        }

        #[test]
        fn prop_handoff_notes_roundtrip(notes in "[a-zA-Z0-9 ]{0,100}") {
            let (db, _dir) = setup_test_db();

            start(&db, &StdOutput).unwrap();
            end(&db, Some(&notes), &StdOutput).unwrap();

            let last = db.get_last_session().unwrap().unwrap();
            prop_assert_eq!(last.handoff_notes, Some(notes));
        }

        #[test]
        fn prop_work_nonexistent_fails(issue_id in 1000i64..10000) {
            let (db, _dir) = setup_test_db();

            start(&db, &StdOutput).unwrap();
            let result = work(&db, issue_id, &StdOutput);
            prop_assert!(result.is_err());
        }
    }
}
