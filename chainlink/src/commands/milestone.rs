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

pub fn create<B: DatabaseBackend>(db: &Database<B>, name: &str, description: Option<&str>, out: &impl Output) -> CmdResult<()> {
    let id = db.create_milestone(name, description)?;
    out_println!(out, "Created milestone #{}: {}", id, name);
    Ok(())
}

pub fn list<B: DatabaseBackend>(db: &Database<B>, status: Option<&str>, out: &impl Output) -> CmdResult<()> {
    let milestones = db.list_milestones(status)?;

    if milestones.is_empty() {
        out_println!(out, "No milestones found.");
        return Ok(());
    }

    for m in milestones {
        let issues = db.get_milestone_issues(m.id)?;
        let total = issues.len();
        let closed = issues.iter().filter(|i| i.status == "closed").count();
        let progress = if total > 0 {
            format!("{}/{}", closed, total)
        } else {
            "0/0".to_string()
        };

        let status_marker = if m.status == "closed" { "✓" } else { " " };
        out_println!(out, "#{:<3} [{}] {} ({})", m.id, status_marker, m.name, progress);
    }

    Ok(())
}

pub fn show<B: DatabaseBackend>(db: &Database<B>, id: i64, out: &impl Output) -> CmdResult<()> {
    let m = match db.get_milestone(id)? {
        Some(m) => m,
        None => return Err(DbError::Validation(format!("Milestone #{} not found", id)).into()),
    };
    out_println!(out, "Milestone #{}: {}", m.id, m.name);
    out_println!(out, "Status: {}", m.status);
    out_println!(out, "Created: {}", m.created_at.format("%Y-%m-%d %H:%M:%S"));

    if let Some(closed) = m.closed_at {
        out_println!(out, "Closed: {}", closed.format("%Y-%m-%d %H:%M:%S"));
    }

    if let Some(ref desc) = m.description {
        if !desc.is_empty() {
            out_println!(out, "\nDescription:");
            for line in desc.lines() {
                out_println!(out, "  {}", line);
            }
        }
    }

    let issues = db.get_milestone_issues(id)?;
    let total = issues.len();
    let closed = issues.iter().filter(|i| i.status == "closed").count();

    out_println!(out, "\nProgress: {}/{} issues closed", closed, total);

    if !issues.is_empty() {
        out_println!(out, "\nIssues:");
        for issue in issues {
            let status_marker = if issue.status == "closed" { "✓" } else { " " };
            out_println!(
                out,
                "  #{:<4} [{}] {:8} {}",
                issue.id, status_marker, issue.priority, issue.title
            );
        }
    }

    Ok(())
}

pub fn add<B: DatabaseBackend>(db: &Database<B>, milestone_id: i64, issue_ids: &[i64], out: &impl Output) -> CmdResult<()> {
    let milestone = db.get_milestone(milestone_id)?;
    if milestone.is_none() {
        return Err(DbError::Validation(format!("Milestone #{} not found", milestone_id)).into());
    }

    for &issue_id in issue_ids {
        if db.get_issue(issue_id)?.is_none() {
            out_println!(out, "Warning: Issue #{} not found, skipping", issue_id);
            continue;
        }

        if db.add_issue_to_milestone(milestone_id, issue_id)? {
            out_println!(out, "Added #{} to milestone #{}", issue_id, milestone_id);
        } else {
            out_println!(out, "Issue #{} already in milestone #{}", issue_id, milestone_id);
        }
    }

    Ok(())
}

pub fn remove<B: DatabaseBackend>(db: &Database<B>, milestone_id: i64, issue_id: i64, out: &impl Output) -> CmdResult<()> {
    if db.remove_issue_from_milestone(milestone_id, issue_id)? {
        out_println!(out, "Removed #{} from milestone #{}", issue_id, milestone_id);
    } else {
        out_println!(out, "Issue #{} not in milestone #{}", issue_id, milestone_id);
    }

    Ok(())
}

pub fn close<B: DatabaseBackend>(db: &Database<B>, id: i64, out: &impl Output) -> CmdResult<()> {
    if db.close_milestone(id)? {
        out_println!(out, "Closed milestone #{}", id);
    } else {
        out_println!(out, "Milestone #{} not found", id);
    }

    Ok(())
}

pub fn delete<B: DatabaseBackend>(db: &Database<B>, id: i64, out: &impl Output) -> CmdResult<()> {
    if db.delete_milestone(id)? {
        out_println!(out, "Deleted milestone #{}", id);
    } else {
        out_println!(out, "Milestone #{} not found", id);
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
    fn test_create_milestone() {
        let (db, _dir) = setup_test_db();
        let result = create(&db, "v1.0", None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_milestone_with_description() {
        let (db, _dir) = setup_test_db();
        let result = create(&db, "v1.0", Some("First release"), &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_milestones_empty() {
        let (db, _dir) = setup_test_db();
        let result = list(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_milestones() {
        let (db, _dir) = setup_test_db();
        db.create_milestone("v1.0", None).unwrap();
        db.create_milestone("v2.0", None).unwrap();
        let result = list(&db, None, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", Some("Description")).unwrap();
        let result = show(&db, id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_nonexistent_milestone() {
        let (db, _dir) = setup_test_db();
        let result = show(&db, 99999, &StdOutput);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_issue_to_milestone() {
        let (db, _dir) = setup_test_db();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        let result = add(&db, milestone_id, &[issue_id], &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_to_nonexistent_milestone() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        let result = add(&db, 99999, &[issue_id], &StdOutput);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_issue_from_milestone() {
        let (db, _dir) = setup_test_db();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        db.add_issue_to_milestone(milestone_id, issue_id).unwrap();
        let result = remove(&db, milestone_id, issue_id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_close_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", None).unwrap();
        let result = close(&db, id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", None).unwrap();
        let result = delete(&db, id, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_milestone_progress() {
        let (db, _dir) = setup_test_db();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        let issue1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let issue2 = db.create_issue("Issue 2", None, "medium").unwrap();
        db.add_issue_to_milestone(milestone_id, issue1).unwrap();
        db.add_issue_to_milestone(milestone_id, issue2).unwrap();
        db.close_issue(issue1).unwrap();
        let result = show(&db, milestone_id, &StdOutput);
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn prop_create_milestone_never_panics(name in "[a-zA-Z0-9 ]{1,30}") {
            let (db, _dir) = setup_test_db();
            let result = create(&db, &name, None, &StdOutput);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_list_never_panics(count in 0usize..5) {
            let (db, _dir) = setup_test_db();
            for i in 0..count {
                db.create_milestone(&format!("v{}.0", i), None).unwrap();
            }
            let result = list(&db, None, &StdOutput);
            prop_assert!(result.is_ok());
        }
    }
}
