// ABOUTME: Opt-in project-local Claude Code and Codex lifecycle guidance.
// ABOUTME: Emits context without configuration loading, indexing, or provider calls.

use anyhow::{bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const CLI_INSTRUCTIONS: &str = include_str!("prompts/cli_instructions.md");
const HOOK_COMMAND: &str =
    "if command -v codegraph >/dev/null 2>&1; then codegraph hooks emit || true; fi";
const MAX_HOOK_INPUT_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Subcommand)]
pub enum HookCommand {
    /// Merge guidance hooks into project settings; never modify user-level settings.
    Install {
        #[arg(long, value_enum, default_value_t = Harness::Both)]
        harness: Harness,
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Print proposed settings as JSON without creating or changing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Read a harness event from stdin and return non-blocking context JSON.
    Emit,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Harness {
    Claude,
    Codex,
    Both,
}

#[derive(Deserialize)]
struct HookInput {
    hook_event_name: String,
    cwd: Option<PathBuf>,
}

pub fn run(command: &HookCommand) -> Result<()> {
    match command {
        HookCommand::Install {
            harness,
            project,
            dry_run,
        } => {
            let changes = install(project, *harness, *dry_run)?;
            if *dry_run {
                println!("{}", serde_json::to_string_pretty(&changes)?);
            } else {
                for change in changes {
                    println!(
                        "{} {}",
                        if change.changed {
                            "Updated"
                        } else {
                            "Already configured:"
                        },
                        change.path.display()
                    );
                }
            }
            Ok(())
        }
        HookCommand::Emit => {
            // Hooks fail open on invalid/oversized input and unsupported events.
            let mut input = String::new();
            let response = if io::stdin()
                .take(MAX_HOOK_INPUT_BYTES + 1)
                .read_to_string(&mut input)
                .is_ok()
                && input.len() as u64 <= MAX_HOOK_INPUT_BYTES
            {
                hook_response(&input, cfg!(feature = "ai-enhanced"))
            } else {
                json!({})
            };
            println!("{response}");
            Ok(())
        }
    }
}

/// Produce only guidance; never evaluate prompts, tool arguments, or transcript paths.
pub fn hook_response(input: &str, agentic_available: bool) -> Value {
    if !agentic_available {
        return json!({});
    }
    let Ok(input) = serde_json::from_str::<HookInput>(input) else {
        return json!({});
    };
    if !matches!(
        input.hook_event_name.as_str(),
        "SessionStart" | "SubagentStart"
    ) {
        return json!({});
    }
    let mut instructions = CLI_INSTRUCTIONS.to_string();
    if let Some(cwd) = input.cwd.as_deref() {
        if let Some(root) = project_root(cwd) {
            // Quote for the POSIX shell used by the installed hook command.
            let quoted_root = format!("'{}'", root.to_string_lossy().replace('\'', "'\\''"));
            instructions.push_str(&format!(
                "\nSession project root: {}. When calling from a subdirectory, append `--project {quoted_root}` to each agentic command.\n",
                root.display()
            ));
        }
    }
    json!({"hookSpecificOutput": {
        "hookEventName": input.hook_event_name, "additionalContext": instructions
    }})
}

fn project_root(cwd: &Path) -> Option<PathBuf> {
    let cwd = cwd.canonicalize().ok()?;
    if !cwd.is_dir() {
        return None;
    }
    cwd.ancestors()
        .find(|path| path.join(".git").exists())
        .map(Path::to_path_buf)
        .or(Some(cwd))
}

#[derive(serde::Serialize)]
pub struct HookChange {
    path: PathBuf,
    changed: bool,
    settings: Value,
}

/// Read and validate both configurations before writing either one.
pub fn install(project: &Path, harness: Harness, dry_run: bool) -> Result<Vec<HookChange>> {
    let project = project
        .canonicalize()
        .context("Cannot resolve hook project directory")?;
    if !project.is_dir() {
        bail!("Hook project must be a directory");
    }
    if dirs::home_dir()
        .and_then(|home| home.canonicalize().ok())
        .as_ref()
        == Some(&project)
    {
        bail!("Hooks must be installed in a project directory, not the user home directory");
    }
    let paths: &[(&str, &str)] = match harness {
        Harness::Claude => &[(".claude", "settings.json")],
        Harness::Codex => &[(".codex", "hooks.json")],
        Harness::Both => &[(".claude", "settings.json"), (".codex", "hooks.json")],
    };
    let mut changes = Vec::new();
    for (directory, filename) in paths {
        let directory = project.join(directory);
        let path = directory.join(filename);
        reject_symlink(&directory)?;
        reject_symlink(&path)?;
        let original = match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content)
                .with_context(|| format!("Invalid JSON in {}", path.display()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => json!({}),
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot read {}", path.display()))
            }
        };
        let settings = merge_settings(original.clone())
            .with_context(|| format!("Cannot merge {}", path.display()))?;
        changes.push(HookChange {
            path,
            changed: original != settings,
            settings,
        });
    }
    if !dry_run {
        for change in &changes {
            if change.changed {
                write_settings(&change.path, &change.settings)?;
            }
        }
    }
    Ok(changes)
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "Refusing to write project hooks through symlink {}",
                path.display()
            )
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn merge_settings(mut settings: Value) -> Result<Value> {
    let object = settings
        .as_object_mut()
        .context("Settings must be a JSON object")?;
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("hooks must be a JSON object")?;
    for event in ["SessionStart", "SubagentStart"] {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .with_context(|| format!("{event} must be an array"))?;
        let mut already_installed = false;
        for group in groups.iter() {
            let handlers = group
                .get("hooks")
                .and_then(Value::as_array)
                .with_context(|| format!("{event} matcher group must have a hooks array"))?;
            already_installed |= handlers.iter().any(|handler| {
                handler.get("command").and_then(Value::as_str) == Some(HOOK_COMMAND)
            });
        }
        if !already_installed {
            let mut group = json!({"hooks": [{
                "type": "command", "command": HOOK_COMMAND, "timeout": 5
            }]});
            if event == "SessionStart" {
                group["matcher"] = json!("^(startup|resume|clear|compact)$");
            }
            groups.push(group);
        }
    }
    Ok(settings)
}

fn write_settings(path: &Path, settings: &Value) -> Result<()> {
    let parent = path.parent().context("Settings have no parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".codegraph-hooks-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        serde_json::to_writer_pretty(&mut file, settings)?;
        writeln!(file)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("Cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_is_restored_for_sessions_and_subagents_without_blocking() {
        for event in ["SessionStart", "SubagentStart"] {
            let output = hook_response(&json!({"hook_event_name": event}).to_string(), true);
            assert_eq!(output["hookSpecificOutput"]["hookEventName"], event);
            let context = output["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap();
            for command in ["context", "impact", "architecture", "quality"] {
                assert!(context.contains(&format!("codegraph agent {command}")));
            }
            assert!(output.get("decision").is_none());
            assert!(output.get("continue").is_none());
        }
        for input in ["not json", "{}", r#"{"hook_event_name":"PreToolUse"}"#] {
            assert_eq!(hook_response(input, true), json!({}));
        }
        assert_eq!(
            hook_response(r#"{"hook_event_name":"SessionStart"}"#, false),
            json!({})
        );
    }

    #[test]
    fn merge_preserves_permissions_and_existing_hooks_and_is_idempotent() {
        let existing = json!({
            "permissions": {"allow": ["Bash(cargo test:*)"]},
            "env": {"CUSTOM_SETTING": "keep"},
            "hooks": {"SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "echo existing"}]}]}
        });
        let merged = merge_settings(existing.clone()).unwrap();
        assert_eq!(merged["permissions"], existing["permissions"]);
        assert_eq!(merged["env"], existing["env"]);
        assert_eq!(
            merged["hooks"]["SessionStart"][0],
            existing["hooks"]["SessionStart"][0]
        );
        assert_eq!(
            merged["hooks"]["SessionStart"][1]["matcher"],
            "^(startup|resume|clear|compact)$"
        );
        assert_eq!(merge_settings(merged.clone()).unwrap(), merged);
    }

    #[test]
    fn dry_run_creates_nothing_and_invalid_second_config_changes_neither() {
        let project = tempfile::tempdir().unwrap();
        let changes = install(project.path(), Harness::Both, true).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(!project.path().join(".claude").exists());
        assert!(!project.path().join(".codex").exists());
        fs::create_dir(project.path().join(".codex")).unwrap();
        fs::write(project.path().join(".codex/hooks.json"), "invalid json").unwrap();
        assert!(install(project.path(), Harness::Both, false).is_err());
        assert!(!project.path().join(".claude").exists());
        assert_eq!(
            fs::read_to_string(project.path().join(".codex/hooks.json")).unwrap(),
            "invalid json"
        );
    }

    #[test]
    fn install_is_repeatable_and_guidance_finds_root_from_subdirectory() {
        let project = tempfile::tempdir().unwrap();
        fs::create_dir(project.path().join(".git")).unwrap();
        fs::create_dir(project.path().join("nested")).unwrap();
        let first = install(project.path(), Harness::Both, false).unwrap();
        assert!(first.iter().all(|change| change.changed));
        let before = fs::read(project.path().join(".codex/hooks.json")).unwrap();
        let second = install(project.path(), Harness::Both, false).unwrap();
        assert!(second.iter().all(|change| !change.changed));
        assert_eq!(
            fs::read(project.path().join(".codex/hooks.json")).unwrap(),
            before
        );
        let output = hook_response(
            &json!({"hook_event_name": "SessionStart", "cwd": project.path().join("nested")})
                .to_string(),
            true,
        );
        let root = project.path().canonicalize().unwrap();
        assert!(output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(&format!("--project '{}'", root.display())));
    }

    #[cfg(unix)]
    #[test]
    fn installed_shell_command_fails_open_for_missing_or_old_binaries() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        let directory = tempfile::tempdir().unwrap();
        let invoke = || {
            Command::new("/bin/sh")
                .args(["-c", HOOK_COMMAND])
                .env("PATH", directory.path())
                .output()
                .unwrap()
        };
        let missing = invoke();
        assert!(missing.status.success());
        assert!(missing.stdout.is_empty());
        let binary = directory.path().join("codegraph");
        fs::write(&binary, "#!/bin/sh\nexit 2\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(invoke().status.success());
    }

    #[cfg(unix)]
    #[test]
    fn installer_rejects_symlinks_to_other_configuration() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), project.path().join(".claude")).unwrap();
        assert!(install(project.path(), Harness::Claude, false).is_err());
        assert!(!outside.path().join("settings.json").exists());
    }
}
