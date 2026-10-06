// ABOUTME: Exercises project init hook choices, repeatable guidance and setup-before-index ordering.
// ABOUTME: Runs the shipped CLI in temporary projects without real providers or language servers.

use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output, Stdio},
};

fn run(arguments: &[&str], directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codegraph"))
        .args(arguments)
        .current_dir(directory)
        .env(
            "CODEGRAPH_CONFIG_PATH",
            directory.join("missing-config.toml"),
        )
        .env("CODEGRAPH_DEBUG", "0")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn all_hook_choices_install_only_selected_harnesses_and_merge_both_guides() {
    for choice in ["claude", "codex", "both", "none"] {
        let project = tempfile::tempdir().unwrap();
        std::fs::write(
            project.path().join("AGENTS.md"),
            "# Team rules\nKeep team rules.\n",
        )
        .unwrap();
        std::fs::write(
            project.path().join("CLAUDE.md"),
            "# Claude rules\nKeep Claude rules.\n",
        )
        .unwrap();
        let claude = matches!(choice, "claude" | "both");
        let codex = matches!(choice, "codex" | "both");
        if claude {
            std::fs::create_dir(project.path().join(".claude")).unwrap();
            std::fs::write(
                project.path().join(".claude/settings.json"),
                r#"{"permissions":{"deny":["Bash(rm:*)"]}}"#,
            )
            .unwrap();
        }
        let output = run(&["init", "--hooks", choice, "--no-index"], project.path());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(!project.path().join(".codegraph/db").exists());
        assert_eq!(
            project.path().join(".claude/settings.json").exists(),
            claude
        );
        assert_eq!(project.path().join(".codex/hooks.json").exists(), codex);
        if claude {
            let settings: Value = serde_json::from_slice(
                &std::fs::read(project.path().join(".claude/settings.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(settings["permissions"]["deny"][0], "Bash(rm:*)");
        }
        let before: Vec<_> = ["AGENTS.md", "CLAUDE.md"]
            .map(|name| {
                let text = std::fs::read_to_string(project.path().join(name)).unwrap();
                assert!(text.contains("Keep "));
                assert_eq!(text.matches("# codegraph").count(), 1);
                for tool in ["context", "impact", "architecture", "quality"] {
                    assert!(text.contains(&format!("codegraph agent {tool}")));
                }
                text
            })
            .into();
        let arguments = if choice == "none" {
            vec!["init", "--hooks", "none", "--no-index"]
        } else {
            vec!["init", "--no-index"]
        };
        let output = run(&arguments, project.path());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for (name, contents) in ["AGENTS.md", "CLAUDE.md"].iter().zip(before) {
            assert_eq!(
                std::fs::read_to_string(project.path().join(name)).unwrap(),
                contents
            );
        }
    }
}

#[test]
fn existing_inline_codex_hooks_skip_prompt_and_adding_claude_preserves_them() {
    let project = tempfile::tempdir().unwrap();
    let output = run(&["hooks", "install", "--harness", "codex"], project.path());
    assert!(output.status.success());
    let hooks_path = project.path().join(".codex/hooks.json");
    let mut settings: Value = serde_json::from_slice(&std::fs::read(&hooks_path).unwrap()).unwrap();
    settings["model"] = "existing-model".into();
    let inline = toml::to_string(&settings).unwrap();
    let config_path = project.path().join(".codex/config.toml");
    std::fs::write(&config_path, &inline).unwrap();
    std::fs::remove_file(&hooks_path).unwrap();

    let output = run(&["init", "--no-index"], project.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Existing CodeGraph hooks found"));
    assert!(!hooks_path.exists());
    assert!(!project.path().join(".claude").exists());

    let output = run(&["init", "--hooks", "both", "--no-index"], project.path());
    assert!(output.status.success());
    assert!(project.path().join(".claude/settings.json").exists());
    assert!(!hooks_path.exists());
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), inline);
    let claude = std::fs::read(project.path().join(".claude/settings.json")).unwrap();
    let output = run(&["init", "--hooks", "none", "--no-index"], project.path());
    assert!(output.status.success());
    assert_eq!(
        std::fs::read(project.path().join(".claude/settings.json")).unwrap(),
        claude
    );
    assert_eq!(std::fs::read_to_string(config_path).unwrap(), inline);
}

#[test]
fn missing_choice_in_noninteractive_init_changes_nothing() {
    let project = tempfile::tempdir().unwrap();
    let output = run(&["init", "--no-index"], project.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--hooks claude|codex|both|none"));
    assert_eq!(std::fs::read_dir(project.path()).unwrap().count(), 0);
}

#[test]
fn setup_finishes_before_indexing_configuration_is_loaded() {
    let project = tempfile::tempdir().unwrap();
    let output = run(&["init", "--hooks", "both"], project.path());
    assert_eq!(output.status.code(), Some(1));
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.find("Step 1:").unwrap() < diagnostics.find("Step 2:").unwrap());
    assert!(diagnostics.find("Step 2:").unwrap() < diagnostics.find("Step 3:").unwrap());
    for name in [
        "AGENTS.md",
        "CLAUDE.md",
        ".claude/settings.json",
        ".codex/hooks.json",
    ] {
        assert!(project.path().join(name).exists());
    }
    assert!(!project.path().join(".codegraph/db").exists());
}

#[test]
fn init_indexes_selected_project_using_its_env_after_guidance_is_written() {
    let invoking = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let config = invoking.path().join("providers.toml");
    codegraph_core::config_manager::ConfigManager::create_default_config(&config).unwrap();
    std::fs::write(project.path().join("a.rs"), "pub fn sample() {}\n").unwrap();
    // Persistent embedded stores apply the bundled schema; mem:// intentionally starts empty.
    std::fs::write(
        project.path().join(".env"),
        format!(
            "CODEGRAPH_SURREALDB_URL=surrealkv://{}\nCODEGRAPH_EMBEDDING_POLICY=off\nCODEGRAPH_SEMANTIC_RESOLUTION=off\nCODEGRAPH_ANALYZERS=0\nCODEGRAPH_VECTOR_INDEX_MODE=off\nCODEGRAPH_PROJECT_ID=init-offline\nCODEGRAPH_NO_PROGRESS=1\n",
            project.path().join(".codegraph/db").display()
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codegraph"))
        .args([
            "--config",
            "providers.toml",
            "init",
            project.path().to_str().unwrap(),
            "--hooks",
            "both",
        ])
        .current_dir(invoking.path())
        .env("CODEGRAPH_DEBUG", "0")
        .env_remove("CODEGRAPH_CONFIG_PATH")
        .env_remove("CODEGRAPH_SURREALDB_URL")
        .env_remove("CODEGRAPH_EMBEDDING_POLICY")
        .env_remove("CODEGRAPH_SEMANTIC_RESOLUTION")
        .env_remove("CODEGRAPH_PROJECT_ID")
        .env_remove("CODEGRAPH_ANALYZERS")
        .env_remove("CODEGRAPH_VECTOR_INDEX_MODE")
        .env_remove("CODEGRAPH_SCHEMA")
        .env_remove("CODEGRAPH_USE_GRAPH_SCHEMA")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let catalog = codegraph_core::artifact_cache::ArtifactCache::new(
        project.path().join(".codegraph/index-cache"),
        "project-catalog-v1",
    );
    let state: Value = catalog.get("init-offline").unwrap();
    assert_eq!(state["stats"]["files"], 1);
    assert_eq!(state["stats"]["complete"], true);
    assert_eq!(state["stats"]["inference_texts"], 0);
    assert!(project.path().join("AGENTS.md").exists());
    assert!(!invoking.path().join("AGENTS.md").exists());
}
