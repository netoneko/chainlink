use crate::commands::CmdResult;
use crate::db::DbError;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

pub fn add<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, related_id: i64, out: &impl Output) -> CmdResult<()> {
    db.require_issue(issue_id)?;
    db.require_issue(related_id)?;

    if db.add_relation(issue_id, related_id)? {
        out_println!(out, "Linked #{} ↔ #{}", issue_id, related_id);
    } else {
        out_println!(
            out,
            "Issues #{} and #{} are already related",
            issue_id, related_id
        );
    }

    Ok(())
}

pub fn remove<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, related_id: i64, out: &impl Output) -> CmdResult<()> {
    if db.remove_relation(issue_id, related_id)? {
        out_println!(out, "Unlinked #{} ↔ #{}", issue_id, related_id);
    } else {
        out_println!(
            out,
            "No relation found between #{} and #{}",
            issue_id, related_id
        );
    }

    Ok(())
}

pub fn list<B: DatabaseBackend>(db: &Database<B>, issue_id: i64, out: &impl Output) -> CmdResult<()> {
    db.require_issue(issue_id)?;

    let related = db.get_related_issues(issue_id)?;

    if related.is_empty() {
        out_println!(out, "No related issues for #{}", issue_id);
        return Ok(());
    }

    out_println!(out, "Related to #{}:", issue_id);
    for r in related {
        let status_marker = if r.status == "closed" { "✓" } else { " " };
        out_println!(
            out,
            "  #{:<4} [{}] {:8} {}",
            r.id, status_marker, r.priority, r.title
        );
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
    fn test_add_relation() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        let result = add(&db, id1, id2, &StdOutput);
        assert!(result.is_ok());

        let related = db.get_related_issues(id1).unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].id, id2);
    }

    #[test]
    fn test_add_relation_bidirectional() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        add(&db, id1, id2, &StdOutput).unwrap();

        let related1 = db.get_related_issues(id1).unwrap();
        let related2 = db.get_related_issues(id2).unwrap();
        assert_eq!(related1.len(), 1);
        assert_eq!(related2.len(), 1);
    }

    #[test]
    fn test_add_relation_nonexistent_issue() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Issue 1", None, "medium").unwrap();

        let result = add(&db, id, 99999, &StdOutput);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_add_duplicate_relation() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        add(&db, id1, id2, &StdOutput).unwrap();
        let result = add(&db, id1, id2, &StdOutput);
        assert!(result.is_ok());

        let related = db.get_related_issues(id1).unwrap();
        assert_eq!(related.len(), 1);
    }

    #[test]
    fn test_remove_relation() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        add(&db, id1, id2, &StdOutput).unwrap();
        let result = remove(&db, id1, id2, &StdOutput);
        assert!(result.is_ok());

        let related = db.get_related_issues(id1).unwrap();
        assert_eq!(related.len(), 0);
    }

    #[test]
    fn test_remove_nonexistent_relation() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();

        let result = remove(&db, id1, id2, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_relations() {
        let (db, _dir) = setup_test_db();
        let id1 = db.create_issue("Issue 1", None, "medium").unwrap();
        let id2 = db.create_issue("Issue 2", None, "medium").unwrap();
        let id3 = db.create_issue("Issue 3", None, "medium").unwrap();

        add(&db, id1, id2, &StdOutput).unwrap();
        add(&db, id1, id3, &StdOutput).unwrap();

        let result = list(&db, id1, &StdOutput);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_relations_nonexistent() {
        let (db, _dir) = setup_test_db();

        let result = list(&db, 99999, &StdOutput);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_no_relations() {
        let (db, _dir) = setup_test_db();
        let id = db.create_issue("Lonely issue", None, "medium").unwrap();

        let result = list(&db, id, &StdOutput);
        assert!(result.is_ok());
    }

    proptest! {
        #[test]
        fn prop_add_remove_roundtrip(a in 0i64..3, b in 0i64..3) {
            if a != b {
                let (db, _dir) = setup_test_db();
                let ids: Vec<i64> = (0..5).map(|i| db.create_issue(&format!("Issue {}", i), None, "medium").unwrap()).collect();

                let id1 = ids[a as usize % ids.len()];
                let id2 = ids[b as usize % ids.len()];

                add(&db, id1, id2, &StdOutput).unwrap();
                let related = db.get_related_issues(id1).unwrap();
                prop_assert!(!related.is_empty());

                remove(&db, id1, id2, &StdOutput).unwrap();
                let related = db.get_related_issues(id1).unwrap();
                prop_assert!(related.is_empty());
            }
        }
    }
}
