use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use chainlink::backend::RusqliteBackend;
use chainlink::db::Database;

// Embed hook files at compile time
// Path: chainlink/src/commands/init.rs -> ../../../.claude/
const SETTINGS_JSON: &str = include_str!("../../../.claude/settings.json");
const PROMPT_GUARD_PY: &str = include_str!("../../../.claude/hooks/prompt-guard.py");
const POST_EDIT_CHECK_PY: &str = include_str!("../../../.claude/hooks/post-edit-check.py");
const SESSION_START_PY: &str = include_str!("../../../.claude/hooks/session-start.py");
const PRE_WEB_CHECK_PY: &str = include_str!("../../../.claude/hooks/pre-web-check.py");

// Embed MCP server for safe web fetching
const SAFE_FETCH_SERVER_PY: &str = include_str!("../../../.claude/mcp/safe-fetch-server.py");
const MCP_JSON: &str = include_str!("../../../.mcp.json");

// Embed sanitization patterns
const SANITIZE_PATTERNS: &str = include_str!("../../../.chainlink/rules/sanitize-patterns.txt");

// Embed rule files at compile time
// Path: chainlink/src/commands/init.rs -> ../../../.chainlink/rules/
const RULE_GLOBAL: &str = include_str!("../../../.chainlink/rules/global.md");
const RULE_PROJECT: &str = include_str!("../../../.chainlink/rules/project.md");
const RULE_RUST: &str = include_str!("../../../.chainlink/rules/rust.md");
const RULE_PYTHON: &str = include_str!("../../../.chainlink/rules/python.md");
const RULE_JAVASCRIPT: &str = include_str!("../../../.chainlink/rules/javascript.md");
const RULE_TYPESCRIPT: &str = include_str!("../../../.chainlink/rules/typescript.md");
const RULE_TYPESCRIPT_REACT: &str = include_str!("../../../.chainlink/rules/typescript-react.md");
const RULE_JAVASCRIPT_REACT: &str = include_str!("../../../.chainlink/rules/javascript-react.md");
const RULE_GO: &str = include_str!("../../../.chainlink/rules/go.md");
const RULE_JAVA: &str = include_str!("../../../.chainlink/rules/java.md");
const RULE_C: &str = include_str!("../../../.chainlink/rules/c.md");
const RULE_CPP: &str = include_str!("../../../.chainlink/rules/cpp.md");
const RULE_CSHARP: &str = include_str!("../../../.chainlink/rules/csharp.md");
const RULE_RUBY: &str = include_str!("../../../.chainlink/rules/ruby.md");
const RULE_PHP: &str = include_str!("../../../.chainlink/rules/php.md");
const RULE_SWIFT: &str = include_str!("../../../.chainlink/rules/swift.md");
const RULE_KOTLIN: &str = include_str!("../../../.chainlink/rules/kotlin.md");
const RULE_SCALA: &str = include_str!("../../../.chainlink/rules/scala.md");
const RULE_ZIG: &str = include_str!("../../../.chainlink/rules/zig.md");
const RULE_ODIN: &str = include_str!("../../../.chainlink/rules/odin.md");
const RULE_ELIXIR: &str = include_str!("../../../.chainlink/rules/elixir.md");
const RULE_ELIXIR_PHOENIX: &str = include_str!("../../../.chainlink/rules/elixir-phoenix.md");
const RULE_WEB: &str = include_str!("../../../.chainlink/rules/web.md");

/// All rule files to deploy
const RULE_FILES: &[(&str, &str)] = &[
    ("global.md", RULE_GLOBAL),
    ("project.md", RULE_PROJECT),
    ("rust.md", RULE_RUST),
    ("python.md", RULE_PYTHON),
    ("javascript.md", RULE_JAVASCRIPT),
    ("typescript.md", RULE_TYPESCRIPT),
    ("typescript-react.md", RULE_TYPESCRIPT_REACT),
    ("javascript-react.md", RULE_JAVASCRIPT_REACT),
    ("go.md", RULE_GO),
    ("java.md", RULE_JAVA),
    ("c.md", RULE_C),
    ("cpp.md", RULE_CPP),
    ("csharp.md", RULE_CSHARP),
    ("ruby.md", RULE_RUBY),
    ("php.md", RULE_PHP),
    ("swift.md", RULE_SWIFT),
    ("kotlin.md", RULE_KOTLIN),
    ("scala.md", RULE_SCALA),
    ("zig.md", RULE_ZIG),
    ("odin.md", RULE_ODIN),
    ("elixir.md", RULE_ELIXIR),
    ("elixir-phoenix.md", RULE_ELIXIR_PHOENIX),
    ("web.md", RULE_WEB),
    ("sanitize-patterns.txt", SANITIZE_PATTERNS),
];

pub fn run(path: &Path, force: bool) -> Result<()> {
    let chainlink_dir = path.join(".chainlink");
    let claude_dir = path.join(".claude");
    let hooks_dir = claude_dir.join("hooks");

    // Check if already initialized
    let chainlink_exists = chainlink_dir.exists();
    let claude_exists = claude_dir.exists();

    if chainlink_exists && claude_exists && !force {
        println!("Already initialized at {}", path.display());
        println!("Use --force to update hooks to latest version.");
        return Ok(());
    }

    let rules_dir = chainlink_dir.join("rules");

    // Create .chainlink directory and database
    if !chainlink_exists {
        fs::create_dir_all(&chainlink_dir).context("Failed to create .chainlink directory")?;

        let db_path = chainlink_dir.join("issues.db");
        Database::<RusqliteBackend>::open(db_path.to_str().unwrap_or(""))?;
        println!("Created {}", chainlink_dir.display());
    }

    // Create or update rules directory
    let rules_exist = rules_dir.exists();
    if !rules_exist || force {
        fs::create_dir_all(&rules_dir).context("Failed to create .chainlink/rules directory")?;

        for (filename, content) in RULE_FILES {
            fs::write(rules_dir.join(filename), content)
                .with_context(|| format!("Failed to write {}", filename))?;
        }

        if force && rules_exist {
            println!("Updated {} with latest rules", rules_dir.display());
        } else {
            println!("Created {} with default rules", rules_dir.display());
        }
    }

    // Create .claude directory and hooks (or update if force)
    if !claude_exists || force {
        fs::create_dir_all(&hooks_dir).context("Failed to create .claude/hooks directory")?;

        // Write settings.json
        fs::write(claude_dir.join("settings.json"), SETTINGS_JSON)
            .context("Failed to write settings.json")?;

        // Write hook scripts
        fs::write(hooks_dir.join("prompt-guard.py"), PROMPT_GUARD_PY)
            .context("Failed to write prompt-guard.py")?;

        fs::write(hooks_dir.join("post-edit-check.py"), POST_EDIT_CHECK_PY)
            .context("Failed to write post-edit-check.py")?;

        fs::write(hooks_dir.join("session-start.py"), SESSION_START_PY)
            .context("Failed to write session-start.py")?;

        fs::write(hooks_dir.join("pre-web-check.py"), PRE_WEB_CHECK_PY)
            .context("Failed to write pre-web-check.py")?;

        // Create MCP server directory and write safe-fetch server
        let mcp_dir = claude_dir.join("mcp");
        fs::create_dir_all(&mcp_dir).context("Failed to create .claude/mcp directory")?;
        fs::write(mcp_dir.join("safe-fetch-server.py"), SAFE_FETCH_SERVER_PY)
            .context("Failed to write safe-fetch-server.py")?;

        // Write .mcp.json to project root
        fs::write(path.join(".mcp.json"), MCP_JSON).context("Failed to write .mcp.json")?;

        if force && claude_exists {
            println!("Updated {} with latest hooks", claude_dir.display());
        } else {
            println!("Created {} with Claude Code hooks", claude_dir.display());
        }
    }

    println!("Chainlink initialized successfully!");
    println!("\nNext steps:");
    println!("  chainlink session start     # Start a session");
    println!("  chainlink create \"Task\"     # Create an issue");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chainlink::backend::RusqliteBackend;
    use tempfile::tempdir;

    #[test]
    fn test_run_fresh_init() {
        let dir = tempdir().unwrap();
        let result = run(dir.path(), false);
        assert!(result.is_ok());

        // Verify directories created
        assert!(dir.path().join(".chainlink").exists());
        assert!(dir.path().join(".chainlink/rules").exists());
        assert!(dir.path().join(".chainlink/issues.db").exists());
        assert!(dir.path().join(".claude").exists());
        assert!(dir.path().join(".claude/hooks").exists());
        assert!(dir.path().join(".claude/mcp").exists());
    }

    #[test]
    fn test_run_creates_hook_files() {
        let dir = tempdir().unwrap();
        run(dir.path(), false).unwrap();

        // Verify hook files
        assert!(dir.path().join(".claude/settings.json").exists());
        assert!(dir.path().join(".claude/hooks/prompt-guard.py").exists());
        assert!(dir.path().join(".claude/hooks/post-edit-check.py").exists());
        assert!(dir.path().join(".claude/hooks/session-start.py").exists());
        assert!(dir.path().join(".claude/hooks/pre-web-check.py").exists());
        assert!(dir.path().join(".claude/mcp/safe-fetch-server.py").exists());
        assert!(dir.path().join(".mcp.json").exists());
    }

    #[test]
    fn test_run_creates_rule_files() {
        let dir = tempdir().unwrap();
        run(dir.path(), false).unwrap();

        let rules_dir = dir.path().join(".chainlink/rules");
        assert!(rules_dir.join("global.md").exists());
        assert!(rules_dir.join("project.md").exists());
        assert!(rules_dir.join("rust.md").exists());
        assert!(rules_dir.join("python.md").exists());
        assert!(rules_dir.join("javascript.md").exists());
        assert!(rules_dir.join("typescript.md").exists());
    }

    #[test]
    fn test_run_already_initialized_no_force() {
        let dir = tempdir().unwrap();

        // First init
        run(dir.path(), false).unwrap();

        // Second init without force - should succeed but not recreate
        let result = run(dir.path(), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_force_update() {
        let dir = tempdir().unwrap();

        // First init
        run(dir.path(), false).unwrap();

        // Modify a hook file
        let hook_path = dir.path().join(".claude/hooks/prompt-guard.py");
        fs::write(&hook_path, "# modified").unwrap();

        // Force update
        run(dir.path(), true).unwrap();

        // Verify file was restored
        let content = fs::read_to_string(&hook_path).unwrap();
        assert_ne!(content, "# modified");
        assert!(content.contains("python") || content.contains("def") || content.len() > 20);
    }

    #[test]
    fn test_run_partial_init_chainlink_only() {
        let dir = tempdir().unwrap();

        // Create only .chainlink directory
        fs::create_dir_all(dir.path().join(".chainlink")).unwrap();

        let result = run(dir.path(), false);
        assert!(result.is_ok());

        // .claude should now exist
        assert!(dir.path().join(".claude").exists());
    }

    #[test]
    fn test_run_partial_init_claude_only() {
        let dir = tempdir().unwrap();

        // Create only .claude directory
        fs::create_dir_all(dir.path().join(".claude")).unwrap();

        let result = run(dir.path(), false);
        assert!(result.is_ok());

        // .chainlink should now exist
        assert!(dir.path().join(".chainlink").exists());
    }

    #[test]
    fn test_run_database_usable() {
        let dir = tempdir().unwrap();
        run(dir.path(), false).unwrap();

        // Open the created database and verify it works
        let db_path = dir.path().join(".chainlink/issues.db");
        let db = Database::<RusqliteBackend>::open(db_path.to_str().unwrap()).unwrap();

        // Should be able to create an issue
        let id = db.create_issue("Test issue", None, "medium").unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_run_rule_files_not_empty() {
        let dir = tempdir().unwrap();
        run(dir.path(), false).unwrap();

        let rules_dir = dir.path().join(".chainlink/rules");

        // Verify rule files have content
        let global = fs::read_to_string(rules_dir.join("global.md")).unwrap();
        assert!(!global.is_empty());

        let rust = fs::read_to_string(rules_dir.join("rust.md")).unwrap();
        assert!(!rust.is_empty());
    }

    #[test]
    fn test_run_force_updates_rules() {
        let dir = tempdir().unwrap();
        run(dir.path(), false).unwrap();

        // Modify a rule file
        let rule_path = dir.path().join(".chainlink/rules/global.md");
        fs::write(&rule_path, "# modified rule").unwrap();

        // Force update
        run(dir.path(), true).unwrap();

        // Verify file was restored
        let content = fs::read_to_string(&rule_path).unwrap();
        assert_ne!(content, "# modified rule");
    }

    #[test]
    fn test_run_idempotent_with_force() {
        let dir = tempdir().unwrap();

        // Multiple force runs should all succeed
        for _ in 0..3 {
            let result = run(dir.path(), true);
            assert!(result.is_ok());
        }

        // All files should still exist
        assert!(dir.path().join(".chainlink/issues.db").exists());
        assert!(dir.path().join(".claude/settings.json").exists());
    }

    #[test]
    fn test_embedded_constants_not_empty() {
        // Verify all embedded constants have content
        assert!(!SETTINGS_JSON.is_empty());
        assert!(!PROMPT_GUARD_PY.is_empty());
        assert!(!POST_EDIT_CHECK_PY.is_empty());
        assert!(!SESSION_START_PY.is_empty());
        assert!(!PRE_WEB_CHECK_PY.is_empty());
        assert!(!SAFE_FETCH_SERVER_PY.is_empty());
        assert!(!MCP_JSON.is_empty());
        assert!(!SANITIZE_PATTERNS.is_empty());
        assert!(!RULE_GLOBAL.is_empty());
        assert!(!RULE_RUST.is_empty());
    }

    #[test]
    fn test_rule_files_count() {
        // Verify we have the expected number of rule files
        assert!(RULE_FILES.len() >= 20);

        // All should have content
        for (name, content) in RULE_FILES {
            assert!(!name.is_empty(), "Rule file name should not be empty");
            assert!(
                !content.is_empty(),
                "Rule file {} should not be empty",
                name
            );
        }
    }
}
