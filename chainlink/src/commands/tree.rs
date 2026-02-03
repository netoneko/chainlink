#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::commands::CmdResult;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::models::Issue;
use crate::out_println;
use crate::output::Output;

fn status_icon(status: &str) -> &'static str {
    match status {
        "open" => " ",
        "closed" => "x",
        _ => "?",
    }
}

fn print_issue(out: &impl Output, issue: &Issue, indent: usize) {
    let prefix = "  ".repeat(indent);
    let icon = status_icon(&issue.status);
    out_println!(
        out,
        "{}[{}] #{} {} - {}",
        prefix, icon, issue.id, issue.priority, issue.title
    );
}

fn print_tree_recursive<B: DatabaseBackend>(out: &impl Output, db: &Database<B>, parent_id: i64, indent: usize) -> CmdResult<()> {
    let subissues = db.get_subissues(parent_id)?;
    for sub in subissues {
        print_issue(out, &sub, indent);
        print_tree_recursive(out, db, sub.id, indent + 1)?;
    }
    Ok(())
}

pub fn run<B: DatabaseBackend>(db: &Database<B>, status_filter: Option<&str>, out: &impl Output) -> CmdResult<()> {
    // Get all top-level issues (no parent)
    let all_issues = db.list_issues(status_filter, None, None)?;
    let top_level: Vec<_> = all_issues
        .into_iter()
        .filter(|i| i.parent_id.is_none())
        .collect();

    if top_level.is_empty() {
        out_println!(out, "No issues found.");
        return Ok(());
    }

    for issue in top_level {
        print_issue(out, &issue, 0);
        print_tree_recursive(out, db, issue.id, 1)?;
    }

    // Legend
    out_println!(out, "");
    out_println!(out, "Legend: [ ] open, [x] closed");

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
    fn test_status_icon_open() {
        assert_eq!(status_icon("open"), " ");
    }

    #[test]
    fn test_status_icon_closed() {
        assert_eq!(status_icon("closed"), "x");
    }

    #[test]
    fn test_status_icon_unknown() {
        assert_eq!(status_icon("archived"), "?");
    }

    #[test]
    fn test_run_empty() {
        let (db, _dir) = setup_test_db();
        let result = run(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_single_issue() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Test issue", None, "medium").unwrap();
        let result = run(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_hierarchy() {
        let (db, _dir) = setup_test_db();
        let parent = db.create_issue("Parent", None, "high").unwrap();
        db.create_subissue(parent, "Child 1", None, "medium")
            .unwrap();
        db.create_subissue(parent, "Child 2", None, "low").unwrap();
        let result = run(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_nested_hierarchy() {
        let (db, _dir) = setup_test_db();
        let parent = db.create_issue("Grandparent", None, "high").unwrap();
        let child = db
            .create_subissue(parent, "Parent", None, "medium")
            .unwrap();
        db.create_subissue(child, "Child", None, "low").unwrap();
        let result = run(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_status_filter() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Open issue", None, "medium").unwrap();
        db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(id).unwrap();
        let result = run(&db, Some("open"), &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_closed_filter() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Issue", None, "medium").unwrap();
        db.close_issue(id).unwrap();
        let result = run(&db, Some("closed"), &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_all_filter() {
        let (db, _dir) = setup_test_db();
        db.create_issue("Open issue", None, "medium").unwrap();
        let id = db.create_issue("Closed issue", None, "medium").unwrap();
        db.close_issue(id).unwrap();
        let result = run(&db, Some("all"), &StdOutput);
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn prop_run_never_panics(count in 0usize..5) {
            let (db, _dir) = setup_test_db();
            for i in 0..count {
                db.create_issue(&format!("Issue {}", i), None, "medium").unwrap();
            }
            let result = run(&db, None, &StdOutput);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_hierarchy_never_panics(depth in 1usize..4) {
            let (db, _dir) = setup_test_db();
            let mut parent_id = db.create_issue("Root", None, "high").unwrap();
            for i in 0..depth {
                parent_id = db.create_subissue(parent_id, &format!("Child {}", i), None, "medium").unwrap();
            }
            let result = run(&db, None, &StdOutput);
            prop_assert!(result.is_ok());
        }
    }
}
