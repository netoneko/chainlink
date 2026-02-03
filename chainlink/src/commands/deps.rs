use anyhow::{bail, Result};

use chainlink::backend::DatabaseBackend;
use chainlink::db::Database;
use chainlink::utils::truncate;

pub fn block<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, blocker_id: i64) -> Result<()> {
    // Check if both issues exist
    db.require_issue(issue_id)?;
    db.require_issue(blocker_id)?;

    if issue_id == blocker_id {
        bail!("An issue cannot block itself");
    }

    if db.add_dependency(issue_id, blocker_id)? {
        println!("Issue #{} is now blocked by #{}", issue_id, blocker_id);
    } else {
        println!("Dependency already exists");
    }
    Ok(())
}

pub fn unblock<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, blocker_id: i64) -> Result<()> {
    if db.remove_dependency(issue_id, blocker_id)? {
        println!(
            "Removed: #{} no longer blocked by #{}",
            issue_id, blocker_id
        );
    } else {
        println!("No such dependency found");
    }
    Ok(())
}

pub fn list_blocked<B: DatabaseBackend>(db: &Database<B>) -> Result<()> {
    let issues = db.list_blocked_issues()?;

    if issues.is_empty() {
        println!("No blocked issues.");
        return Ok(());
    }

    println!("Blocked issues:");
    for issue in issues {
        let blockers = db.get_blockers(issue.id)?;
        let blocker_strs: Vec<String> = blockers.iter().map(|b| format!("#{}", b)).collect();
        println!(
            "  #{:<4} {} (blocked by: {})",
            issue.id,
            truncate(&issue.title, 40),
            blocker_strs.join(", ")
        );
    }

    Ok(())
}

pub fn list_ready<B: DatabaseBackend>(db: &Database<B>) -> Result<()> {
    let issues = db.list_ready_issues()?;

    if issues.is_empty() {
        println!("No ready issues.");
        return Ok(());
    }

    println!("Ready issues (no blockers):");
    for issue in issues {
        println!("  #{:<4} {:8} {}", issue.id, issue.priority, issue.title);
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

    // Block function tests
    #[test]
    fn test_block_success() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();

        let result = block(&db, issue1, issue2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_nonexistent_issue() {
        let (db, _dir) = setup_test_db();
        let issue = db.create_issue("Issue", None, "medium").unwrap();

        let result = block(&db, 99999, issue);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_block_nonexistent_blocker() {
        let (db, _dir) = setup_test_db();
        let issue = db.create_issue("Issue", None, "medium").unwrap();

        let result = block(&db, issue, 99999);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_block_self() {
        let (db, _dir) = setup_test_db();
        let issue = db.create_issue("Issue", None, "medium").unwrap();

        let result = block(&db, issue, issue);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot block itself"));
    }

    #[test]
    fn test_block_duplicate() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();

        block(&db, issue1, issue2).unwrap();
        let result = block(&db, issue1, issue2);
        assert!(result.is_ok()); // Should succeed but print "already exists"
    }

    // Unblock function tests
    #[test]
    fn test_unblock_success() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();
        db.add_dependency(issue1, issue2).unwrap();

        let result = unblock(&db, issue1, issue2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unblock_nonexistent_dependency() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();

        let result = unblock(&db, issue1, issue2);
        assert!(result.is_ok()); // Should succeed but print "no such dependency"
    }

    // List blocked tests
    #[test]
    fn test_list_blocked_empty() {
        let (db, _dir) = setup_test_db();

        let result = list_blocked(&db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_blocked_with_issues() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Blocked issue", None, "medium").unwrap();
        let issue2 = db.create_issue("Blocker", None, "medium").unwrap();
        db.add_dependency(issue1, issue2).unwrap();

        let result = list_blocked(&db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_blocked_multiple_blockers() {
        let (db, _dir) = setup_test_db();
        let blocked = db.create_issue("Blocked", None, "medium").unwrap();
        let blocker1 = db.create_issue("Blocker 1", None, "medium").unwrap();
        let blocker2 = db.create_issue("Blocker 2", None, "medium").unwrap();
        db.add_dependency(blocked, blocker1).unwrap();
        db.add_dependency(blocked, blocker2).unwrap();

        let result = list_blocked(&db);
        assert!(result.is_ok());
    }

    // List ready tests
    #[test]
    fn test_list_ready_empty() {
        let (db, _dir) = setup_test_db();

        let result = list_ready(&db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_ready_with_issues() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Ready issue", None, "medium").unwrap();

        let result = list_ready(&db);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_ready_excludes_blocked() {
        let (db, _dir) = setup_test_db();
        let blocked = db.create_issue("Blocked", None, "high").unwrap();
        let blocker = db.create_issue("Blocker", None, "medium").unwrap();
        db.add_dependency(blocked, blocker).unwrap();

        let ready = db.list_ready_issues().unwrap();
        assert!(!ready.iter().any(|i| i.id == blocked));
        assert!(ready.iter().any(|i| i.id == blocker));
    }

    #[test]
    fn test_list_ready_excludes_closed() {
        let (db, _dir) = setup_test_db();
        let issue = db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(issue).unwrap();

        let ready = db.list_ready_issues().unwrap();
        assert!(!ready.iter().any(|i| i.id == issue));
    }

    // Integration tests
    #[test]
    fn test_block_unblock_roundtrip() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();

        block(&db, issue1, issue2).unwrap();
        let blocked = db.list_blocked_issues().unwrap();
        assert!(blocked.iter().any(|i| i.id == issue1));

        unblock(&db, issue1, issue2).unwrap();
        let blocked = db.list_blocked_issues().unwrap();
        assert!(!blocked.iter().any(|i| i.id == issue1));
    }

    #[test]
    fn test_closing_blocker_unblocks() {
        let (db, _dir) = setup_test_db();
        let blocked = db.create_issue("Blocked", None, "high").unwrap();
        let blocker = db.create_issue("Blocker", None, "medium").unwrap();
        db.add_dependency(blocked, blocker).unwrap();

        // Blocked issue should not be ready
        let ready = db.list_ready_issues().unwrap();
        assert!(!ready.iter().any(|i| i.id == blocked));

        // Close the blocker
        db.close_issue(blocker).unwrap();

        // Now blocked issue should be ready
        let ready = db.list_ready_issues().unwrap();
        assert!(ready.iter().any(|i| i.id == blocked));
    }

    proptest! {
        #[test]
        fn truncate_never_panics(s in ".*", max_chars in 1usize..200) {
            let _ = truncate(&s, max_chars);
        }

        #[test]
        fn truncate_result_valid_utf8(s in ".*", max_chars in 4usize..100) {
            let result = truncate(&s, max_chars);
            let _ = result.chars().count();
        }

        #[test]
        fn truncate_respects_limit(s in ".{10,100}", max_chars in 5usize..50) {
            let result = truncate(&s, max_chars);
            assert!(result.chars().count() <= max_chars);
        }

        #[test]
        fn prop_block_with_valid_issues(title1 in "[a-zA-Z ]{1,20}", title2 in "[a-zA-Z ]{1,20}") {
            let (db, _dir) = setup_test_db();
            let issue1 = db.create_issue(&title1, None, "medium").unwrap();
            let issue2 = db.create_issue(&title2, None, "medium").unwrap();

            let result = block(&db, issue1, issue2);
            prop_assert!(result.is_ok());
        }
    }
}
