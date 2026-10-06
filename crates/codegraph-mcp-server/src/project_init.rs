// ABOUTME: Project initialization chooses local hooks and merges agent instructions before indexing.
// ABOUTME: Setup is provider-independent, preserves unrelated content, and is repeatable.

use crate::agent_hooks::{self, Harness};
use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use std::{
    fs,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

pub const PROJECT_INSTRUCTIONS: &str = include_str!("prompts/project_instructions.md");
const BEGIN: &str = "<!-- codegraph:begin -->";
const END: &str = "<!-- codegraph:end -->";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HookSelection {
    Claude,
    Codex,
    Both,
    None,
}

impl HookSelection {
    fn harness(self) -> Option<Harness> {
        match self {
            Self::Claude => Some(Harness::Claude),
            Self::Codex => Some(Harness::Codex),
            Self::Both => Some(Harness::Both),
            Self::None => None,
        }
    }
}

struct GuidanceChange {
    path: PathBuf,
    contents: String,
    changed: bool,
}

/// Finish all project-local setup before the caller loads providers or starts indexing.
pub fn prepare(project: &Path, requested: Option<HookSelection>) -> Result<PathBuf> {
    let project = agent_hooks::project_directory(project)?;
    eprintln!("Step 1: Project-local Claude/Codex hooks");
    let selection = if let Some(selection) = requested {
        selection
    } else if [Harness::Claude, Harness::Codex]
        .into_iter()
        .any(|harness| {
            agent_hooks::install(&project, harness, true)
                .is_ok_and(|changes| changes.iter().all(|change| !change.changed))
        })
    {
        eprintln!("Existing CodeGraph hooks found; use --hooks to add another harness.");
        HookSelection::None
    } else {
        if !io::stdin().is_terminal() {
            bail!(
                "Select project hooks with --hooks claude|codex|both|none, or run init in an interactive terminal. No files were changed."
            );
        }
        prompt_selection(&mut io::stdin().lock(), &mut io::stderr().lock())?
    };

    // Validate every selected setting and both instruction files before any writes.
    let hooks = selection
        .harness()
        .map(|harness| agent_hooks::install(&project, harness, true))
        .transpose()?
        .unwrap_or_default();
    let guidance = guidance_changes(&project)?;
    agent_hooks::apply_changes(&hooks)?;
    for change in &hooks {
        eprintln!(
            "{} {}",
            if change.changed {
                "Updated"
            } else {
                "Already configured:"
            },
            change.path.display()
        );
    }
    eprintln!("Step 2: Project agent instructions");
    for change in guidance {
        if change.changed {
            agent_hooks::write_project_file(&change.path, change.contents.as_bytes())?;
        }
        eprintln!(
            "{} {}",
            if change.changed {
                "Updated"
            } else {
                "Already configured:"
            },
            change.path.display()
        );
    }
    Ok(project)
}

fn prompt_selection(input: &mut impl BufRead, output: &mut impl Write) -> Result<HookSelection> {
    writeln!(output, "Install guidance hooks in this project only:")?;
    writeln!(output, "  1) Claude Code   2) Codex   3) Both   4) None")?;
    loop {
        write!(output, "Choice [4]: ")?;
        output.flush()?;
        let mut choice = String::new();
        if input.read_line(&mut choice)? == 0 {
            bail!("Hook selection cancelled; pass --hooks claude|codex|both|none.");
        }
        match choice.trim().to_ascii_lowercase().as_str() {
            "1" | "claude" => return Ok(HookSelection::Claude),
            "2" | "codex" => return Ok(HookSelection::Codex),
            "3" | "both" => return Ok(HookSelection::Both),
            "" | "4" | "none" => return Ok(HookSelection::None),
            _ => writeln!(output, "Enter 1, 2, 3, 4, claude, codex, both or none.")?,
        }
    }
}

fn guidance_changes(project: &Path) -> Result<Vec<GuidanceChange>> {
    ["AGENTS.md", "CLAUDE.md"]
        .into_iter()
        .map(|name| {
            let path = project.join(name);
            agent_hooks::reject_symlink(&path)?;
            let original = match fs::read_to_string(&path) {
                Ok(contents) => contents,
                Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
                Err(error) => {
                    return Err(error).with_context(|| format!("Cannot read {}", path.display()));
                }
            };
            let contents = merge_guidance(&original)
                .with_context(|| format!("Cannot update {}", path.display()))?;
            Ok(GuidanceChange {
                path,
                changed: original != contents,
                contents,
            })
        })
        .collect()
}

/// Inspect real Markdown lines, ignoring headings and ownership markers inside fences.
fn visible_lines(contents: &str) -> Result<Vec<(usize, usize, &str)>> {
    let mut lines = Vec::new();
    let mut offset = 0;
    let mut fence: Option<(char, usize)> = None;
    for line in contents.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        let trimmed = text.trim_start();
        let first = trimmed.chars().next().unwrap_or(' ');
        let width = trimmed.chars().take_while(|ch| *ch == first).count();
        let indent = text.len() - trimmed.len();
        if let Some((character, length)) = fence {
            if indent <= 3
                && first == character
                && width >= length
                && trimmed[width..].trim().is_empty()
            {
                fence = None;
            }
        } else if indent <= 3 && matches!(first, '`' | '~') && width >= 3 {
            fence = Some((first, width));
        } else {
            lines.push((offset, offset + line.len(), text));
        }
        offset += line.len();
    }
    if fence.is_some() {
        bail!(
            "An unclosed Markdown code fence would hide the CodeGraph instructions; close it before retrying."
        );
    }
    Ok(lines)
}

fn managed_block(include_heading: bool, newline: &str) -> String {
    let instructions = if include_heading {
        PROJECT_INSTRUCTIONS
    } else {
        PROJECT_INSTRUCTIONS.strip_prefix("# codegraph\n").unwrap()
    };
    format!("{BEGIN}\n{instructions}{END}\n").replace('\n', newline)
}

fn merge_guidance(original: &str) -> Result<String> {
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let lines = visible_lines(original)?;
    let begins: Vec<_> = lines.iter().filter(|(_, _, text)| *text == BEGIN).collect();
    let ends: Vec<_> = lines.iter().filter(|(_, _, text)| *text == END).collect();
    match (begins.as_slice(), ends.as_slice()) {
        ([begin], [end]) if begin.0 < end.0 => {
            let include_heading = !lines.iter().any(|(offset, _, text)| {
                text.eq_ignore_ascii_case("# codegraph") && (*offset < begin.0 || *offset > end.0)
            });
            Ok(format!(
                "{}{}{}",
                &original[..begin.0],
                managed_block(include_heading, newline),
                &original[end.1..]
            ))
        }
        ([], []) => {
            if let Some((_, end, _)) = lines
                .iter()
                .find(|(_, _, text)| text.eq_ignore_ascii_case("# codegraph"))
            {
                let separator = if original[..*end].ends_with('\n') {
                    ""
                } else {
                    newline
                };
                Ok(format!(
                    "{}{separator}{}{}",
                    &original[..*end],
                    managed_block(false, newline),
                    &original[*end..]
                ))
            } else {
                let separator =
                    if original.is_empty() || original.ends_with(&format!("{newline}{newline}")) {
                        ""
                    } else if original.ends_with(newline) {
                        newline
                    } else if newline == "\r\n" {
                        "\r\n\r\n"
                    } else {
                        "\n\n"
                    };
                Ok(format!(
                    "{original}{separator}{}",
                    managed_block(true, newline)
                ))
            }
        }
        _ => bail!(
            "CodeGraph instruction markers are duplicated, reversed or incomplete; repair the managed block before retrying."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_offers_all_choices_and_retries_invalid_input() {
        for (input, expected) in [
            ("1\n", HookSelection::Claude),
            ("2\n", HookSelection::Codex),
            ("3\n", HookSelection::Both),
            ("4\n", HookSelection::None),
            ("\n", HookSelection::None),
            ("wrong\nboth\n", HookSelection::Both),
        ] {
            let mut output = Vec::new();
            let actual = prompt_selection(&mut io::Cursor::new(input), &mut output).unwrap();
            assert_eq!(actual, expected);
            let output = String::from_utf8(output).unwrap();
            for label in ["Claude Code", "Codex", "Both", "None"] {
                assert!(output.contains(label));
            }
        }
        assert!(prompt_selection(&mut io::Cursor::new(""), &mut Vec::new()).is_err());
    }

    #[test]
    fn guidance_preserves_custom_sections_and_is_idempotent() {
        let original = "# Team rules\nKeep these rules.\n\n# codegraph\nCustom graph policy.\n\n# More rules\nKeep these too.\n";
        let merged = merge_guidance(original).unwrap();
        assert_eq!(merged.matches("# codegraph").count(), 1);
        assert!(merged.starts_with("# Team rules\nKeep these rules.\n"));
        assert!(merged.ends_with("Custom graph policy.\n\n# More rules\nKeep these too.\n"));
        assert_eq!(merge_guidance(&merged).unwrap(), merged);
        let stale = merged.replace("start code exploration", "obsolete workflow");
        assert_eq!(merge_guidance(&stale).unwrap(), merged);
        let missing_heading = merged.replace("# codegraph\n", "");
        let repaired = merge_guidance(&missing_heading).unwrap();
        assert_eq!(repaired.matches("# codegraph").count(), 1);
        assert_eq!(merge_guidance(&repaired).unwrap(), repaired);
    }

    #[test]
    fn fenced_examples_are_ignored_and_crlf_is_preserved() {
        let original = "# Rules\r\n```md\r\n# codegraph\r\n<!-- codegraph:begin -->\r\n```\r\n";
        let merged = merge_guidance(original).unwrap();
        assert!(merged.starts_with(original));
        assert!(merged.ends_with("<!-- codegraph:end -->\r\n"));
        assert_eq!(
            visible_lines(&merged)
                .unwrap()
                .iter()
                .filter(|(_, _, text)| *text == "# codegraph")
                .count(),
            1
        );
        assert_eq!(merge_guidance(&merged).unwrap(), merged);
        assert!(merge_guidance("# Rules\n```md\n").is_err());
    }

    #[test]
    fn malformed_guidance_prevents_writing_hooks_or_the_other_file() {
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("AGENTS.md"), "Team instructions\n").unwrap();
        fs::write(project.path().join("CLAUDE.md"), BEGIN).unwrap();
        assert!(prepare(project.path(), Some(HookSelection::Both)).is_err());
        assert!(!project.path().join(".claude").exists());
        assert!(!project.path().join(".codex").exists());
        assert_eq!(
            fs::read_to_string(project.path().join("AGENTS.md")).unwrap(),
            "Team instructions\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn guidance_symlinks_cannot_modify_another_project() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("CLAUDE.md");
        fs::write(&target, "Outside instructions\n").unwrap();
        std::os::unix::fs::symlink(&target, project.path().join("CLAUDE.md")).unwrap();
        assert!(prepare(project.path(), Some(HookSelection::Both)).is_err());
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "Outside instructions\n"
        );
        assert!(!project.path().join(".claude").exists());
        assert!(!project.path().join("AGENTS.md").exists());
    }
}
