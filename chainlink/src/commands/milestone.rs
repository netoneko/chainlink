use anyhow::{bail, Result};

use chainlink::backend::DatabaseBackend;
use chainlink::db::Database;

pub fn create<B: DatabaseBackend>(db: &Database<B>, name: &str, description: Option<&str>) -> Result<()> {
    let id = db.create_milestone(name, description)?;
    println!("Created milestone #{}: {}", id, name);
    Ok(())
}

pub fn list<B: DatabaseBackend>(db: &Database<B>, status: Option<&str>) -> Result<()> {
    let milestones = db.list_milestones(status)?;

    if milestones.is_empty() {
        println!("No milestones found.");
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
        println!("#{:<3} [{}] {} ({})", m.id, status_marker, m.name, progress);
    }

    Ok(())
}

pub fn show<B: DatabaseBackend>(db: &Database<B>, id: i64) -> Result<()> {
    let m = match db.get_milestone(id)? {
        Some(m) => m,
        None => bail!("Milestone #{} not found", id),
    };
    println!("Milestone #{}: {}", m.id, m.name);
    println!("Status: {}", m.status);
    println!("Created: {}", m.created_at.format("%Y-%m-%d %H:%M:%S"));

    if let Some(closed) = m.closed_at {
        println!("Closed: {}", closed.format("%Y-%m-%d %H:%M:%S"));
    }

    if let Some(ref desc) = m.description {
        if !desc.is_empty() {
            println!("\nDescription:");
            for line in desc.lines() {
                println!("  {}", line);
            }
        }
    }

    let issues = db.get_milestone_issues(id)?;
    let total = issues.len();
    let closed = issues.iter().filter(|i| i.status == "closed").count();

    println!("\nProgress: {}/{} issues closed", closed, total);

    if !issues.is_empty() {
        println!("\nIssues:");
        for issue in issues {
            let status_marker = if issue.status == "closed" { "✓" } else { " " };
            println!(
                "  #{:<4} [{}] {:8} {}",
                issue.id, status_marker, issue.priority, issue.title
            );
        }
    }

    Ok(())
}

pub fn add<B: DatabaseBackend>(db: &Database<B>, milestone_id: i64, issue_ids: &[i64]) -> Result<()> {
    let milestone = db.get_milestone(milestone_id)?;
    if milestone.is_none() {
        bail!("Milestone #{} not found", milestone_id);
    }

    for &issue_id in issue_ids {
        if db.get_issue(issue_id)?.is_none() {
            println!("Warning: Issue #{} not found, skipping", issue_id);
            continue;
        }

        if db.add_issue_to_milestone(milestone_id, issue_id)? {
            println!("Added #{} to milestone #{}", issue_id, milestone_id);
        } else {
            println!("Issue #{} already in milestone #{}", issue_id, milestone_id);
        }
    }

    Ok(())
}

pub fn remove<B: DatabaseBackend>(db: &Database<B>, milestone_id: i64, issue_id: i64) -> Result<()> {
    if db.remove_issue_from_milestone(milestone_id, issue_id)? {
        println!("Removed #{} from milestone #{}", issue_id, milestone_id);
    } else {
        println!("Issue #{} not in milestone #{}", issue_id, milestone_id);
    }

    Ok(())
}

pub fn close<B: DatabaseBackend>(db: &Database<B>, id: i64) -> Result<()> {
    if db.close_milestone(id)? {
        println!("Closed milestone #{}", id);
    } else {
        println!("Milestone #{} not found", id);
    }

    Ok(())
}

pub fn delete<B: DatabaseBackend>(db: &Database<B>, id: i64) -> Result<()> {
    if db.delete_milestone(id)? {
        println!("Deleted milestone #{}", id);
    } else {
        println!("Milestone #{} not found", id);
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

    #[test]
    fn test_create_milestone() {
        let (db, _dir) = setup_test_db();
        let result = create(&db, "v1.0", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_milestone_with_description() {
        let (db, _dir) = setup_test_db();
        let result = create(&db, "v1.0", Some("First release"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_milestones_empty() {
        let (db, _dir) = setup_test_db();
        let result = list(&db, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_milestones() {
        let (db, _dir) = setup_test_db();
        db.create_milestone("v1.0", None).unwrap();
        db.create_milestone("v2.0", None).unwrap();
        let result = list(&db, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", Some("Description")).unwrap();
        let result = show(&db, id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_nonexistent_milestone() {
        let (db, _dir) = setup_test_db();
        let result = show(&db, 99999);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_issue_to_milestone() {
        let (db, _dir) = setup_test_db();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        let result = add(&db, milestone_id, &[issue_id]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_to_nonexistent_milestone() {
        let (db, _dir) = setup_test_db();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        let result = add(&db, 99999, &[issue_id]);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_issue_from_milestone() {
        let (db, _dir) = setup_test_db();
        let milestone_id = db.create_milestone("v1.0", None).unwrap();
        let issue_id = db.create_issue("Test issue", None, "medium").unwrap();
        db.add_issue_to_milestone(milestone_id, issue_id).unwrap();
        let result = remove(&db, milestone_id, issue_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_close_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", None).unwrap();
        let result = close(&db, id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_milestone() {
        let (db, _dir) = setup_test_db();
        let id = db.create_milestone("v1.0", None).unwrap();
        let result = delete(&db, id);
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
        let result = show(&db, milestone_id);
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn prop_create_milestone_never_panics(name in "[a-zA-Z0-9 ]{1,30}") {
            let (db, _dir) = setup_test_db();
            let result = create(&db, &name, None);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn prop_list_never_panics(count in 0usize..5) {
            let (db, _dir) = setup_test_db();
            for i in 0..count {
                db.create_milestone(&format!("v{}.0", i), None).unwrap();
            }
            let result = list(&db, None);
            prop_assert!(result.is_ok());
        }
    }
}
