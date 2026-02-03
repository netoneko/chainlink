use crate::commands::CmdResult;
use crate::db::DbError;

use crate::backend::DatabaseBackend;
use crate::db::Database;
use crate::out_println;
use crate::output::Output;

#[cfg(not(feature = "std"))]
use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

const VALID_PRIORITIES: [&str; 4] = ["low", "medium", "high", "critical"];

/// Built-in issue templates
pub struct Template {
    pub name: &'static str,
    pub priority: &'static str,
    pub label: &'static str,
    pub description_prefix: Option<&'static str>,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "bug",
        priority: "high",
        label: "bug",
        description_prefix: Some("Steps to reproduce:\n1. \n\nExpected: \nActual: "),
    },
    Template {
        name: "feature",
        priority: "medium",
        label: "feature",
        description_prefix: Some("Goal: \n\nAcceptance criteria:\n- "),
    },
    Template {
        name: "refactor",
        priority: "low",
        label: "refactor",
        description_prefix: Some("Current state: \n\nDesired state: \n\nReason: "),
    },
    Template {
        name: "research",
        priority: "low",
        label: "research",
        description_prefix: Some("Question: \n\nContext: \n\nFindings: "),
    },
];

pub fn get_template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

pub fn list_templates() -> Vec<&'static str> {
    TEMPLATES.iter().map(|t| t.name).collect()
}

pub fn validate_priority(priority: &str) -> bool {
    VALID_PRIORITIES.contains(&priority)
}

pub fn run<B: DatabaseBackend>(
    db: &Database<B>,
    title: &str,
    description: Option<&str>,
    priority: &str,
    template: Option<&str>,
    out: &impl Output,
) -> CmdResult<()> {
    // Apply template if specified
    let (final_priority, final_description, label) = if let Some(tmpl_name) = template {
        let tmpl = get_template(tmpl_name).ok_or_else(|| {
            DbError::Validation(format!(
                "Unknown template '{}'. Available: {}",
                tmpl_name,
                list_templates().join(", ")
            ))
        })?;

        // Template priority is default, user can override
        let priority = if priority != "medium" {
            priority
        } else {
            tmpl.priority
        };

        // Combine template description prefix with user description
        let desc = match (tmpl.description_prefix, description) {
            (Some(prefix), Some(user_desc)) => Some(format!("{}\n\n{}", prefix, user_desc)),
            (Some(prefix), None) => Some(prefix.to_string()),
            (None, user_desc) => user_desc.map(|s| s.to_string()),
        };

        (priority.to_string(), desc, Some(tmpl.label))
    } else {
        (
            priority.to_string(),
            description.map(|s| s.to_string()),
            None,
        )
    };

    if !validate_priority(&final_priority) {
        return Err(DbError::Validation(format!(
            "Invalid priority '{}'. Must be one of: {}",
            final_priority,
            VALID_PRIORITIES.join(", ")
        )).into());
    }

    let id = db.create_issue(title, final_description.as_deref(), &final_priority)?;

    // Auto-add label from template
    if let Some(lbl) = label {
        db.add_label(id, lbl)?;
    }

    out_println!(out, "Created issue #{}", id);
    if let Some(tmpl) = template {
        out_println!(out, "  Applied template: {}", tmpl);
    }
    Ok(())
}

pub fn run_subissue<B: DatabaseBackend>(
    db: &Database<B>,
    parent_id: i64,
    title: &str,
    description: Option<&str>,
    priority: &str,
    out: &impl Output,
) -> CmdResult<()> {
    if !validate_priority(priority) {
        return Err(DbError::Validation(format!(
            "Invalid priority '{}'. Must be one of: {}",
            priority,
            VALID_PRIORITIES.join(", ")
        )).into());
    }

    // Verify parent exists
    let parent = db.get_issue(parent_id)?;
    if parent.is_none() {
        return Err(DbError::Validation(format!("Parent issue #{} not found", parent_id)).into());
    }

    let id = db.create_subissue(parent_id, title, description, priority)?;
    out_println!(out, "Created subissue #{} under #{}", id, parent_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ==================== Unit Tests ====================

    #[test]
    fn test_validate_priority_valid() {
        assert!(validate_priority("low"));
        assert!(validate_priority("medium"));
        assert!(validate_priority("high"));
        assert!(validate_priority("critical"));
    }

    #[test]
    fn test_validate_priority_invalid() {
        assert!(!validate_priority(""));
        assert!(!validate_priority("urgent"));
        assert!(!validate_priority("LOW")); // Case sensitive
        assert!(!validate_priority("MEDIUM"));
        assert!(!validate_priority("High"));
        assert!(!validate_priority("CRITICAL"));
        assert!(!validate_priority(" medium"));
        assert!(!validate_priority("medium "));
        assert!(!validate_priority("medium\n"));
    }

    #[test]
    fn test_validate_priority_malicious() {
        // Security: ensure no injection vectors
        assert!(!validate_priority("'; DROP TABLE issues; --"));
        assert!(!validate_priority("high\0medium"));
        assert!(!validate_priority("medium; DELETE FROM issues"));
        assert!(!validate_priority("<script>alert('xss')</script>"));
    }

    #[test]
    fn test_get_template_exists() {
        let bug = get_template("bug");
        assert!(bug.is_some());
        let template = bug.unwrap();
        assert_eq!(template.name, "bug");
        assert_eq!(template.priority, "high");
        assert_eq!(template.label, "bug");
        assert!(template.description_prefix.is_some());
    }

    #[test]
    fn test_get_template_all_templates() {
        for name in ["bug", "feature", "refactor", "research"] {
            let template = get_template(name);
            assert!(template.is_some(), "Template '{}' should exist", name);
        }
    }

    #[test]
    fn test_get_template_not_found() {
        assert!(get_template("nonexistent").is_none());
        assert!(get_template("").is_none());
        assert!(get_template("Bug").is_none()); // Case sensitive
        assert!(get_template("BUG").is_none());
    }

    #[test]
    fn test_list_templates() {
        let templates = list_templates();
        assert!(templates.contains(&"bug"));
        assert!(templates.contains(&"feature"));
        assert!(templates.contains(&"refactor"));
        assert!(templates.contains(&"research"));
        assert_eq!(templates.len(), 4);
    }

    #[test]
    fn test_template_fields() {
        // Verify all templates have required fields
        for template in TEMPLATES {
            assert!(!template.name.is_empty());
            assert!(validate_priority(template.priority));
            assert!(!template.label.is_empty());
        }
    }

    #[test]
    fn test_template_bug_description_prefix() {
        let template = get_template("bug").unwrap();
        let prefix = template.description_prefix.unwrap();
        assert!(prefix.contains("Steps to reproduce"));
        assert!(prefix.contains("Expected"));
        assert!(prefix.contains("Actual"));
    }

    #[test]
    fn test_template_feature_description_prefix() {
        let template = get_template("feature").unwrap();
        let prefix = template.description_prefix.unwrap();
        assert!(prefix.contains("Goal"));
        assert!(prefix.contains("Acceptance criteria"));
    }

    // ==================== Property-Based Tests ====================

    proptest! {
        #[test]
        fn prop_validate_priority_never_panics(s in ".*") {
            let _ = validate_priority(&s);
        }

        #[test]
        fn prop_get_template_never_panics(s in ".*") {
            let _ = get_template(&s);
        }

        #[test]
        fn prop_valid_priorities_always_validate(priority in "low|medium|high|critical") {
            prop_assert!(validate_priority(&priority));
        }

        #[test]
        fn prop_invalid_priorities_never_validate(
            priority in "[a-zA-Z]{1,20}"
                .prop_filter("Exclude valid priorities", |s| {
                    !["low", "medium", "high", "critical"].contains(&s.as_str())
                })
        ) {
            prop_assert!(!validate_priority(&priority));
        }

        #[test]
        fn prop_template_names_consistent(name in "bug|feature|refactor|research") {
            let template = get_template(&name);
            prop_assert!(template.is_some());
            prop_assert_eq!(template.unwrap().name, name.as_str());
        }
    }
}
