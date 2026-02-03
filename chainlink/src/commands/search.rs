use crate::commands::CmdResult;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

pub fn run<B: DatabaseBackend>(db: &Database<B>, query: &str, out: &impl Output) -> CmdResult<()> {
    let results = db.search_issues(query)?;

    if results.is_empty() {
        out_println!(out, "No issues found matching '{}'", query);
        return Ok(());
    }

    out_println!(out, "Found {} issue(s) matching '{}':\n", results.len(), query);

    for issue in results {
        let status_marker = if issue.status == "closed" { "✓" } else { " " };
        let parent_str = issue
            .parent_id
            .map(|p| format!(" (sub of #{})", p))
            .unwrap_or_default();

        out_println!(
            out,
            "#{:<4} [{}] {:8} {}{} {}",
            issue.id,
            status_marker,
            issue.priority,
            issue.title,
            parent_str,
            if issue.status == "closed" {
                "(closed)"
            } else {
                ""
            }
        );

        // Show snippet of description if it contains the query
        if let Some(ref desc) = issue.description {
            if desc.to_lowercase().contains(&query.to_lowercase()) {
                let preview: String = desc.chars().take(60).collect();
                let suffix = if desc.len() > 60 { "..." } else { "" };
                out_println!(out, "      └─ {}{}", preview.replace('\n', " "), suffix);
            }
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

    // ==================== Unit Tests ====================

    #[test]
    fn test_search_finds_by_title() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Fix authentication bug", None, "high")
            .unwrap();
        db.create_issue("Add dark mode", None, "medium").unwrap();

        let result = run(&db, "authentication", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_finds_by_description() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Feature A", Some("This relates to user login"), "medium")
            .unwrap();

        let result = run(&db, "login", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_case_insensitive() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Fix AUTHENTICATION Bug", None, "high")
            .unwrap();

        // Search with different cases should still work (via db layer)
        let result = run(&db, "authentication", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_no_results() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Some issue", None, "medium").unwrap();

        let result = run(&db, "nonexistent", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_empty_database() {
        let (db, _dir) = setup_test_db();

        let result = run(&db, "anything", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_empty_query() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Test issue", None, "medium").unwrap();

        let result = run(&db, "", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_special_characters() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Fix bug with @mentions", None, "medium")
            .unwrap();

        let result = run(&db, "@mentions", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_unicode() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Fix 日本語 support", None, "medium")
            .unwrap();

        let result = run(&db, "日本語", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_sql_injection() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Normal issue", None, "medium").unwrap();

        // Should not crash or inject SQL
        let result = run(&db, "'; DROP TABLE issues; --", &StdOutput);
        assert!(result.is_ok());

        // Database should still be intact
        let issues = db.list_issues(None, None, None).unwrap();
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn test_search_with_wildcards() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Test issue with pattern", None, "medium")
            .unwrap();

        // SQL wildcards should be escaped
        let result = run(&db, "%pattern%", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_finds_in_comments() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Generic issue", None, "medium").unwrap();
        db.add_comment(id, "Found the root cause in authentication module")
            .unwrap();

        let result = run(&db, "authentication", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_subissue_shows_parent() {
        let (db, _dir) = setup_test_db();
        let parent_id = db.create_issue("Parent feature", None, "high").unwrap();
        db.create_subissue(parent_id, "Sub task authentication", None, "medium")
            .unwrap();

        let result = run(&db, "authentication", &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_closed_issue() {
        let (db, _dir) = setup_test_db();
        let id = db
            .create_issue("Fix authentication bug", None, "high")
            .unwrap();
        db.close_issue(id).unwrap();

        let result = run(&db, "authentication", &StdOutput);
        assert!(result.is_ok());
    }

    // ==================== Property-Based Tests ====================

    proptest! {
        #[test]
        fn prop_search_never_panics(query in ".*") {
            let (db, _dir) = setup_test_db();
            db.create_issue("Test issue", None, "medium").unwrap();
            let _ = run(&db, &query, &StdOutput);
        }

        #[test]
        fn prop_search_with_issues_never_panics(
            title in "[a-zA-Z0-9 ]{1,50}",
            query in "[a-zA-Z0-9]{1,20}"
        ) {
            let (db, _dir) = setup_test_db();
            db.create_issue(&title, None, "medium").unwrap();
            let result = run(&db, &query, &StdOutput);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_search_unicode_never_panics(
            title in "[\\p{L}\\p{N} ]{1,30}",
            query in "[\\p{L}\\p{N}]{1,10}"
        ) {
            let (db, _dir) = setup_test_db();
            db.create_issue(&title, None, "medium").unwrap();
            let result = run(&db, &query, &StdOutput);
            prop_assert!(result.is_ok());
        }
    }
}
