use crate::commands::CmdResult;
use crate::db::DbError;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

pub fn run<B: DatabaseBackend>(db: &Database<B>, id: i64, out: &impl Output) -> CmdResult<()> {
    let issue = match db.get_issue(id)? {
        Some(i) => i,
        None => return Err(DbError::NotFound(format!("Issue #{} not found", id)).into()),
    };

    out_println!(out, "Issue #{}: {}", issue.id, issue.title);
    out_println!(out, "Status: {}", issue.status);
    out_println!(out, "Priority: {}", issue.priority);
    if let Some(parent_id) = issue.parent_id {
        out_println!(out, "Parent: #{}", parent_id);
    }
    out_println!(out, "Created: {}", issue.created_at.format("%Y-%m-%d %H:%M:%S"));
    out_println!(out, "Updated: {}", issue.updated_at.format("%Y-%m-%d %H:%M:%S"));

    if let Some(closed) = issue.closed_at {
        out_println!(out, "Closed: {}", closed.format("%Y-%m-%d %H:%M:%S"));
    }

    // Labels
    let labels = db.get_labels(id)?;
    if !labels.is_empty() {
        out_println!(out, "Labels: {}", labels.join(", "));
    }

    // Milestone
    if let Some(milestone) = db.get_issue_milestone(id)? {
        out_println!(out, "Milestone: #{} {}", milestone.id, milestone.name);
    }

    // Description
    if let Some(desc) = &issue.description {
        if !desc.is_empty() {
            out_println!(out, "\nDescription:");
            for line in desc.lines() {
                out_println!(out, "  {}", line);
            }
        }
    }

    // Comments
    let comments = db.get_comments(id)?;
    if !comments.is_empty() {
        out_println!(out, "\nComments:");
        for comment in comments {
            out_println!(
                out,
                "  [{}] {}",
                comment.created_at.format("%Y-%m-%d %H:%M"),
                comment.content
            );
        }
    }

    // Dependencies
    let blockers = db.get_blockers(id)?;
    let blocking = db.get_blocking(id)?;

    out_println!(out);
    if blockers.is_empty() {
        out_println!(out, "Blocked by: (none)");
    } else {
        let blocker_strs: Vec<String> = blockers.iter().map(|b| format!("#{}", b)).collect();
        out_println!(out, "Blocked by: {}", blocker_strs.join(", "));
    }

    if blocking.is_empty() {
        out_println!(out, "Blocking: (none)");
    } else {
        let blocking_strs: Vec<String> = blocking.iter().map(|b| format!("#{}", b)).collect();
        out_println!(out, "Blocking: {}", blocking_strs.join(", "));
    }

    // Subissues
    let subissues = db.get_subissues(id)?;
    if !subissues.is_empty() {
        out_println!(out, "\nSubissues:");
        for sub in subissues {
            out_println!(
                out,
                "  #{} [{}] {} - {}",
                sub.id, sub.status, sub.priority, sub.title
            );
        }
    }

    // Related issues
    let related = db.get_related_issues(id)?;
    if !related.is_empty() {
        out_println!(out, "\nRelated:");
        for rel in related {
            let status_marker = if rel.status == "closed" { "✓" } else { " " };
            out_println!(
                out,
                "  #{} [{}] {} - {}",
                rel.id, status_marker, rel.priority, rel.title
            );
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
    fn test_show_existing_issue() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_nonexistent_issue() {
        let (db, _dir) = setup_test_db();

        let result = run(&db, 99999, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_show_issue_with_description() {
        let (db, _dir) = setup_test_db();
        let issue_id = db
            .create_issue("Test issue", Some("A detailed description"), "high")
            .unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_labels() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        db.add_label(issue_id, "bug").unwrap();
        db.add_label(issue_id, "urgent").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_comments() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        db.add_comment(issue_id, "First comment").unwrap();
        db.add_comment(issue_id, "Second comment").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_blockers() {
        let (db, _dir) = setup_test_db();
        let blocker_id = db.create_issue("Blocker", None, "high").unwrap();
        let issue_id = db.create_issue("Blocked issue", None, "medium").unwrap();
        db.add_dependency(issue_id, blocker_id).unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_subissues() {
        let (db, _dir) = setup_test_db();
        let parent_id = db.create_issue("Parent", None, "high").unwrap();
        db.create_subissue(parent_id, "Child 1", None, "medium")
            .unwrap();
        db.create_subissue(parent_id, "Child 2", None, "low")
            .unwrap();

        let result = run(&db, parent_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_subissue_shows_parent() {
        let (db, _dir) = setup_test_db();
        let parent_id = db.create_issue("Parent", None, "high").unwrap();
        let child_id = db
            .create_subissue(parent_id, "Child", None, "medium")
            .unwrap();

        let result = run(&db, child_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_related() {
        let (db, _dir) = setup_test_db();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();
        db.add_relation(issue1, issue2).unwrap();

        let result = run(&db, issue1, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_closed_issue() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        db.close_issue(issue_id).unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_with_milestone() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        db.add_issue_to_milestone(milestone_id, issue_id).unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_unicode_content() {
        let (db, _dir) = setup_test_db();
        let issue_id = db
            .create_issue("测试问题 🐛", Some("描述 αβγ"), "medium")
            .unwrap();
        db.add_comment(issue_id, "评论 🎉").unwrap();
        db.add_label(issue_id, "バグ").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_multiline_description() {
        let (db, _dir) = setup_test_db();
        let desc = "Line 1\nLine 2\n\nLine 4 after blank";
        let issue_id = db.create_issue("Test", Some(desc), "medium").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_issue_empty_description() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test", Some(""), "medium").unwrap();

        let result = run(&db, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    // ==================== Property-Based Tests ====================

    proptest! {
        #[test]
        fn prop_show_never_panics(title in "[a-zA-Z0-9 ]{1,50}") {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue(&title, None, "medium").unwrap();
            let result = run(&db, issue_id, &StdOutput);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_show_nonexistent_always_fails(issue_id in 1000i64..10000) {
            let (db, _dir) = setup_test_db();
            let result = run(&db, issue_id, &StdOutput);
            prop_assert!(result.is_err());
        }

        #[test]
        fn prop_show_with_description_never_panics(
            title in "[a-zA-Z0-9 ]{1,30}",
            desc in "[a-zA-Z0-9 \n]{0,200}"
        ) {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue(&title, Some(&desc), "medium").unwrap();
            let result = run(&db, issue_id, &StdOutput);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_show_unicode_never_panics(
            title in "[\\p{L}\\p{N} ]{1,30}"
        ) {
            let (db, _dir) = setup_test_db();
            let issue_id = db.create_issue(&title, None, "medium").unwrap();
            let result = run(&db, issue_id, &StdOutput);
            prop_assert!(result.is_ok());
        }
    }
}
