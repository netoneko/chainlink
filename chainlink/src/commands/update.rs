use crate::commands::CmdResult;
use crate::db::DbError;

#[cfg(not(feature = "std"))]
use alloc::format;

use crate::backend::DatabaseBackend;
use crate::commands::create::validate_priority;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

pub fn run<B: DatabaseBackend>(
    db: &Database<B>,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    out: &impl Output,
) -> CmdResult<()> {
    if title.is_none() && description.is_none() && priority.is_none() {
        return Err(DbError::Validation("Nothing to update. Use --title, --description, or --priority".into()).into());
    }

    if let Some(p) = priority {
        if !validate_priority(p) {
            return Err(DbError::Validation(
                format!("Invalid priority '{}'. Must be one of: low, medium, high, critical", p)
            ).into());
        }
    }

    if db.update_issue(id, title, description, priority)? {
        out_println!(out, "Updated issue #{}", id);
    } else {
        return Err(DbError::Validation(format!("Issue #{} not found", id)).into());
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

    // ==================== Unit Tests ====================

    #[test]
    fn test_update_title() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Original title", None, "medium").unwrap();

        let result = run(&db, issue_id, Some("New title"), None, None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, "New title");
    }

    #[test]
    fn test_update_description() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", None, "medium").unwrap();

        let result = run(&db, issue_id, None, Some("New description"), None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.description, Some("New description".to_string()));
    }

    #[test]
    fn test_update_priority() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", None, "medium").unwrap();

        let result = run(&db, issue_id, None, None, Some("critical"), &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.priority, "critical");
    }

    #[test]
    fn test_update_all_fields() {
        let (db, _dir) = setup_test_db();
        let issue_id = db
            .create_issue("Original", Some("Old desc"), "low")
            .unwrap();

        let result = run(
            &db,
            issue_id,
            Some("New title"),
            Some("New description"),
            Some("high"),
            &StdOutput,
        );
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, "New title");
        assert_eq!(issue.description, Some("New description".to_string()));
        assert_eq!(issue.priority, "high");
    }

    #[test]
    fn test_update_nothing_fails() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", None, "medium").unwrap();

        let result = run(&db, issue_id, None, None, None, &StdOutput);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Nothing to update"));
    }

    #[test]
    fn test_update_nonexistent_issue() {
        let (db, _dir) = setup_test_db();

        let result = run(&db, 99999, Some("New title"), None, None, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_update_invalid_priority() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", None, "medium").unwrap();

        let result = run(&db, issue_id, None, None, Some("urgent"), &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid priority"));
    }

    #[test]
    fn test_update_preserves_unchanged_fields() {
        let (db, _dir) = setup_test_db();
        let issue_id = db
            .create_issue("Original title", Some("Original desc"), "high")
            .unwrap();

        // Only update title
        run(&db, issue_id, Some("New title"), None, None, &StdOutput).unwrap();

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, "New title");
        assert_eq!(issue.description, Some("Original desc".to_string()));
        assert_eq!(issue.priority, "high");
    }

    #[test]
    fn test_update_unicode_title() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Original", None, "medium").unwrap();

        let result = run(&db, issue_id, Some("新しいタイトル 🎉"), None, None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, "新しいタイトル 🎉");
    }

    #[test]
    fn test_update_empty_description() {
        let (db, _dir) = setup_test_db();
        let issue_id = db
            .create_issue("Test", Some("Has description"), "medium")
            .unwrap();

        let result = run(&db, issue_id, None, Some(""), None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.description, Some("".to_string()));
    }

    #[test]
    fn test_update_sql_injection() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Original", None, "medium").unwrap();

        let malicious = "'; DROP TABLE issues; --";
        let result = run(&db, issue_id, Some(malicious), None, None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, malicious);

        // Verify database is intact
        let issues = db.list_issues(None, None, None).unwrap();
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_update_closed_issue() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", None, "medium").unwrap();
        db.close_issue(issue_id).unwrap();

        let result = run(&db, issue_id, Some("Updated closed issue"), None, None, &StdOutput);
        assert!(result.is_ok());

        let issue = db.get_issue(issue_id).unwrap().unwrap();
        assert_eq!(issue.title, "Updated closed issue");
        assert_eq!(issue.status, "closed"); // Status should remain closed
    }

    // ==================== Property-Based Tests ====================

    proptest! {
        #[test]
        fn prop_update_title_roundtrip(
            original in "[a-zA-Z0-9 ]{1,30}",
            new_title in "[a-zA-Z0-9 ]{1,30}"
        ) {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue(&original, None, "medium").unwrap();

            run(&db, issue_id, Some(&new_title), None, None, &StdOutput).unwrap();

            let issue = db.get_issue(issue_id).unwrap().unwrap();
            prop_assert_eq!(issue.title, new_title);
        }

        #[test]
        fn prop_update_priority_valid(priority in "low|medium|high|critical") {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue("Test", None, "medium").unwrap();

            let result = run(&db, issue_id, None, None, Some(&priority), &StdOutput);
            prop_assert!(result.is_ok());

            let issue = db.get_issue(issue_id).unwrap().unwrap();
            prop_assert_eq!(issue.priority, priority);
        }

        #[test]
        fn prop_update_priority_invalid(
            priority in "[a-zA-Z]{1,10}"
                .prop_filter("Exclude valid priorities", |s| {
                    !["low", "medium", "high", "critical"].contains(&s.as_str())
                })
        ) {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue("Test", None, "medium").unwrap();

            let result = run(&db, issue_id, None, None, Some(&priority), &StdOutput);
            prop_assert!(result.is_err());
        }

        #[test]
        fn prop_nonexistent_issue_fails(issue_id in 1000i64..10000) {
            let (db, _dir) = setup_test_db();

            let result = run(&db, issue_id, Some("New title"), None, None, &StdOutput);
            prop_assert!(result.is_err());
        }

        #[test]
        fn prop_unicode_description_roundtrip(desc in "[\\p{L}\\p{N} ]{1,100}") {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue("Test", None, "medium").unwrap();

            run(&db, issue_id, None, Some(&desc), None, &StdOutput).unwrap();

            let issue = db.get_issue(issue_id).unwrap().unwrap();
            prop_assert_eq!(issue.description, Some(desc));
        }
    }
}
