use anyhow::Result;

use chainlink::backend::DatabaseBackend;
use chainlink::db::Database;
use chainlink::utils::truncate;

pub fn run<B: DatabaseBackend>(
    db: &Database<B>,
    status: Option<&str>,
    label: Option<&str>,
    priority: Option<&str>,
) -> Result<()> {
    let issues = db.list_issues(status, label, priority)?;

    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    for issue in issues {
        let status_display = format!("[{}]", issue.status);
        let date = issue.created_at.format("%Y-%m-%d");
        println!(
            "#{:<4} {:8} {:<40} {:8} {}",
            issue.id,
            status_display,
            truncate(&issue.title, 40),
            issue.priority,
            date
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chainlink::backend::RusqliteBackend;
    use proptest::prelude::*;
    use tempfile::tempdir;

    fn setup_test_db() -> (Database<RusqliteBackend>, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        (db, dir)
    }

    // Truncate function tests
    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_unicode() {
        // Multi-byte UTF-8 characters
        assert_eq!(truncate("← → ↑ ↓", 10), "← → ↑ ↓");
        let result = truncate("←←←←←←←←←←←←", 5);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_truncate_emoji() {
        // Emoji are multi-byte
        let result = truncate("🎉🎊🎈🎁🎂🎄🎃🎇🎆", 6);
        assert!(result.ends_with("..."));
        assert_eq!(result.chars().count(), 6);
    }

    #[test]
    fn test_truncate_mixed_unicode() {
        let result = truncate("Hello 世界! 🌍", 8);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_edge_cases() {
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("ab", 3), "ab");
        assert_eq!(truncate("abcd", 3), "...");
    }

    // Run function tests
    #[test]
    fn test_run_empty() {
        let (db, _dir) = setup_test_db();
        let result = run(&db, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_issues() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Issue 1", None, "high").unwrap();
        db.create_issue("Issue 2", None, "medium").unwrap();
        db.create_issue("Issue 3", None, "low").unwrap();

        let result = run(&db, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_status_filter_open() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Open issue", None, "medium").unwrap();
        let id2 = db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(id2).unwrap();

        let issues = db.list_issues(Some("open"), None, None).unwrap();
        assert!(issues.iter().any(|i| i.id == id1));
        assert!(!issues.iter().any(|i| i.id == id2));

        let result = run(&db, Some("open"), None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_status_filter_closed() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Open issue", None, "medium").unwrap();
        let id2 = db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(id2).unwrap();

        let issues = db.list_issues(Some("closed"), None, None).unwrap();
        assert!(!issues.iter().any(|i| i.id == id1));
        assert!(issues.iter().any(|i| i.id == id2));

        let result = run(&db, Some("closed"), None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_status_filter_all() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Open issue", None, "medium").unwrap();
        let id2 = db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(id2).unwrap();

        let result = run(&db, Some("all"), None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_label_filter() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Bug issue", None, "high").unwrap();
        let id2 = db.create_issue("Feature issue", None, "medium").unwrap();
        db.add_label(id1, "bug").unwrap();
        db.add_label(id2, "feature").unwrap();

        let issues = db.list_issues(None, Some("bug"), None).unwrap();
        assert!(issues.iter().any(|i| i.id == id1));
        assert!(!issues.iter().any(|i| i.id == id2));

        let result = run(&db, None, Some("bug"), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_priority_filter() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("High priority", None, "high").unwrap();
        let id2 = db.create_issue("Low priority", None, "low").unwrap();

        let issues = db.list_issues(None, None, Some("high")).unwrap();
        assert!(issues.iter().any(|i| i.id == id1));
        assert!(!issues.iter().any(|i| i.id == id2));

        let result = run(&db, None, None, Some("high"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_combined_filters() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("High bug", None, "high").unwrap();
        let id2 = db.create_issue("Low bug", None, "low").unwrap();
        let id3 = db.create_issue("High feature", None, "high").unwrap();
        db.add_label(id1, "bug").unwrap();
        db.add_label(id2, "bug").unwrap();
        db.add_label(id3, "feature").unwrap();

        let issues = db
            .list_issues(Some("open"), Some("bug"), Some("high"))
            .unwrap();
        assert!(issues.iter().any(|i| i.id == id1));
        assert!(!issues.iter().any(|i| i.id == id2));
        assert!(!issues.iter().any(|i| i.id == id3));

        let result = run(&db, Some("open"), Some("bug"), Some("high"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_long_title_truncation() {
        let (db, _dir) = setup_test_db();
        let long_title = "A".repeat(100);
        db.create_issue(&long_title, None, "medium").unwrap();

        let result = run(&db, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_unicode_title() {
        let (db, _dir) = setup_test_db();
        db.create_issue("日本語タイトル 🎉", None, "medium")
            .unwrap();

        let result = run(&db, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_no_matching_filter() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Issue", None, "medium").unwrap();

        let result = run(&db, None, Some("nonexistent-label"), None);
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn truncate_never_panics(s in ".*", max_chars in 1usize..200) {
            let _ = truncate(&s, max_chars);
        }

        #[test]
        fn truncate_result_is_valid_utf8(s in ".*", max_chars in 4usize..100) {
            let result = truncate(&s, max_chars);
            // If we can iterate chars, it's valid UTF-8
            let _ = result.chars().count();
        }

        #[test]
        fn truncate_respects_max_chars(s in ".{10,100}", max_chars in 5usize..50) {
            let result = truncate(&s, max_chars);
            assert!(result.chars().count() <= max_chars);
        }

        #[test]
        fn truncate_preserves_short_strings(s in ".{0,10}") {
            let result = truncate(&s, 20);
            assert_eq!(result, s);
        }

        #[test]
        fn truncate_adds_ellipsis_for_long_strings(s in ".{20,50}", max_chars in 5usize..15) {
            let result = truncate(&s, max_chars);
            if s.chars().count() > max_chars {
                assert!(result.ends_with("..."));
            }
        }

        #[test]
        fn prop_run_never_panics(count in 0usize..5) {
            let (db, _dir) = setup_test_db();
            for i in 0..count {
                db.create_issue(&format!("Issue {}", i), None, "medium").unwrap();
            }
            let result = run(&db, None, None, None);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_run_with_filters(status in "open|closed|all", priority in "low|medium|high|critical") {
            let (db, _dir) = setup_test_db();
            db.create_issue("Test", None, &priority).unwrap();
            let result = run(&db, Some(&status), None, Some(&priority));
            prop_assert!(result.is_ok());
        }
    }
}
