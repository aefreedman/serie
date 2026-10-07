use std::{cell::RefCell, path::Path, process::Command};

use arboard::Clipboard;

use crate::config::ClipboardConfig;

const USER_COMMAND_MARKER_PREFIX: &str = "{{";
const USER_COMMAND_TARGET_HASH_MARKER: &str = "{{target_hash}}";
const USER_COMMAND_FIRST_PARENT_HASH_MARKER: &str = "{{first_parent_hash}}";
const USER_COMMAND_PARENT_HASHES_MARKER: &str = "{{parent_hashes}}";
const USER_COMMAND_REFS_MARKER: &str = "{{refs}}";
const USER_COMMAND_BRANCHES_MARKER: &str = "{{branches}}";
const USER_COMMAND_REMOTE_BRANCHES_MARKER: &str = "{{remote_branches}}";
const USER_COMMAND_TAGS_MARKER: &str = "{{tags}}";
const USER_COMMAND_STASH_MARKER: &str = "{{stash}}";
const USER_COMMAND_AREA_WIDTH_MARKER: &str = "{{area_width}}";
const USER_COMMAND_AREA_HEIGHT_MARKER: &str = "{{area_height}}";

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = const { RefCell::new(None) };
}

pub fn copy_to_clipboard(value: String, config: &ClipboardConfig) -> Result<(), String> {
    match config {
        ClipboardConfig::Auto => copy_to_clipboard_auto(value),
        ClipboardConfig::Custom { commands } => copy_to_clipboard_custom(value, commands),
    }
}

fn copy_to_clipboard_custom(value: String, commands: &[String]) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    if commands.first().is_none_or(|s| s.trim().is_empty()) {
        return Err("No clipboard command specified".to_string());
    }

    let mut child = Command::new(&commands[0])
        .args(&commands[1..])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run {}: {e}", commands[0]))?;

    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| format!("No stdin available for {}", commands[0]))
        .and_then(|mut stdin| {
            stdin
                .write_all(value.as_bytes())
                .map_err(|e| format!("Failed to write to {}: {e}", commands[0]))
        });

    // Close stdin and reap the helper even when writing fails.
    let status = child
        .wait()
        .map_err(|e| format!("{} failed: {e}", commands[0]));
    write_result?;
    let status = status?;
    if !status.success() {
        return Err(format!(
            "{} exited with non-zero status: {status}",
            commands[0]
        ));
    }

    Ok(())
}

fn copy_to_clipboard_auto(value: String) -> Result<(), String> {
    CLIPBOARD.with_borrow_mut(|clipboard| {
        if clipboard.is_none() {
            *clipboard = Clipboard::new()
                .map(Some)
                .map_err(|e| format!("Failed to create clipboard: {e:?}"))?;
        }

        clipboard
            .as_mut()
            .expect("The clipboard should have been initialized above")
            .set_text(value)
            .map_err(|e| format!("Failed to copy to clipboard: {e:?}"))
    })
}

pub struct ExternalCommandParameters<'a> {
    pub command: &'a [String],
    pub workspace: &'a Path,
    pub target_hash: String,
    pub primary_parent: Option<String>,
    pub parent_hashes: Vec<String>,
    pub all_refs: Vec<String>,
    pub branches: Vec<String>,
    pub remote_branches: Vec<String>,
    pub tags: Vec<String>,
    pub stash: Option<String>,
    pub area_width: u16,
    pub area_height: u16,
}

pub fn exec_user_command(params: ExternalCommandParameters) -> Result<String, String> {
    let command = build_user_command(&params)?;

    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(params.workspace)
        .output()
        .map_err(|e| format!("Failed to execute command: {e:?}"))?;

    if !output.status.success() {
        let msg = format!(
            "Command exited with non-zero status: {}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(msg);
    }

    Ok(String::from_utf8_lossy(&output.stdout).into())
}

pub fn exec_user_command_suspend(params: ExternalCommandParameters) -> Result<(), String> {
    let command = build_user_command(&params)?;

    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(params.workspace)
        .status()
        .map_err(|e| format!("Failed to execute command: {e:?}"))?;

    if !output.success() {
        let msg = format!("Command exited with non-zero status: {output}");
        return Err(msg);
    }

    Ok(())
}

fn build_user_command(params: &ExternalCommandParameters) -> Result<Vec<String>, String> {
    if params
        .command
        .first()
        .is_none_or(|s| replace_command_arg(s, params).trim().is_empty())
    {
        return Err("No user command executable specified".into());
    }
    let mut command = Vec::new();
    for arg in params.command {
        if !arg.contains(USER_COMMAND_MARKER_PREFIX) {
            command.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            // If the marker is used as a standalone argument, expand it into multiple arguments.
            // This allows the command to receive each item as a separate argument and correctly handle items that contain spaces.
            USER_COMMAND_BRANCHES_MARKER => command.extend(params.branches.clone()),
            USER_COMMAND_REMOTE_BRANCHES_MARKER => command.extend(params.remote_branches.clone()),
            USER_COMMAND_TAGS_MARKER | "{{labels}}" => command.extend(params.tags.clone()),
            USER_COMMAND_REFS_MARKER => command.extend(params.all_refs.clone()),
            USER_COMMAND_PARENT_HASHES_MARKER | "{{parents}}" => {
                command.extend(params.parent_hashes.clone())
            }
            // Otherwise, replace the marker within the single argument string.
            _ => command.push(replace_command_arg(arg, params)),
        }
    }
    Ok(command)
}

fn replace_command_arg(s: &str, params: &ExternalCommandParameters) -> String {
    let sep = " ";
    let target_hash = &params.target_hash;
    let first_parent_hash = params.primary_parent.as_deref().unwrap_or_default();
    let parent_hashes = &params.parent_hashes.join(sep);
    let all_refs = &params.all_refs.join(sep);
    let branches = &params.branches.join(sep);
    let remote_branches = &params.remote_branches.join(sep);
    let tags = &params.tags.join(sep);
    let stash = params.stash.as_deref().unwrap_or_default();
    let area_width = &params.area_width.to_string();
    let area_height = &params.area_height.to_string();

    // Scan the template once: producer names may themselves contain marker text.
    let mut result = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(USER_COMMAND_MARKER_PREFIX) {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find("}}") else {
            break;
        };
        let marker = &rest[..end + 2];
        let value = match marker {
            USER_COMMAND_TARGET_HASH_MARKER | "{{changeset}}" => target_hash,
            USER_COMMAND_FIRST_PARENT_HASH_MARKER | "{{primary_parent}}" => first_parent_hash,
            USER_COMMAND_PARENT_HASHES_MARKER | "{{parents}}" => parent_hashes,
            USER_COMMAND_REFS_MARKER => all_refs,
            USER_COMMAND_BRANCHES_MARKER => branches,
            USER_COMMAND_REMOTE_BRANCHES_MARKER => remote_branches,
            USER_COMMAND_TAGS_MARKER | "{{labels}}" => tags,
            USER_COMMAND_STASH_MARKER => stash,
            USER_COMMAND_AREA_WIDTH_MARKER => area_width,
            USER_COMMAND_AREA_HEIGHT_MARKER => area_height,
            _ => marker,
        };
        result.push_str(value);
        rest = &rest[end + 2..];
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only the subprocess invocation supplies an output path; normal test runs do nothing.
    #[test]
    fn benign_clipboard_helper() {
        use std::io::Read;
        let args: Vec<_> = std::env::args().collect();
        let Some(output) = args.windows(2).find_map(|pair| {
            (pair[0] == "--skip"
                && Path::new(&pair[1])
                    .file_name()
                    .is_some_and(|n| n == "clipboard stdin.txt"))
            .then(|| &pair[1])
        }) else {
            return;
        };
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        std::fs::write(output, bytes).unwrap();
        std::process::exit(if args.iter().any(|a| a == "clipboard-helper-failure") {
            7
        } else {
            0
        });
    }

    #[test]
    fn custom_clipboard_stdin_and_failures() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("clipboard helper with spaces 日本語");
        std::fs::create_dir(&dir).unwrap();
        let exe = dir.join(format!("benign helper{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let output = dir.join("clipboard stdin.txt");
        let mut commands = vec![
            exe.to_string_lossy().into_owned(),
            "--exact".into(),
            "external::tests::benign_clipboard_helper".into(),
            "--nocapture".into(),
            "--skip".into(),
            output.to_string_lossy().into_owned(),
        ];
        for selector in [
            "cs:17@rep:synthetic repo@repserver:example.invalid:8087",
            "br:/main/spaced 日本語 branch@rep:synthetic repo@repserver:example.invalid:8087",
            "lb:release λ ; $literal@rep:synthetic repo@repserver:example.invalid:8087",
            "",
        ] {
            let config = ClipboardConfig::Custom {
                commands: commands.clone(),
            };
            copy_to_clipboard(selector.into(), &config).unwrap();
            assert_eq!(std::fs::read(&output).unwrap(), selector.as_bytes());
        }
        commands.extend(["--skip".into(), "clipboard-helper-failure".into()]);
        let error = copy_to_clipboard(
            "controlled failure".into(),
            &ClipboardConfig::Custom { commands },
        )
        .unwrap_err();
        assert!(error.contains("non-zero status"), "{error}");
        assert_eq!(std::fs::read(&output).unwrap(), b"controlled failure");

        let missing = dir.join("missing helper").to_string_lossy().into_owned();
        for commands in [vec![], vec!["".into()], vec![" ".into()], vec![missing]] {
            assert!(
                copy_to_clipboard("unused".into(), &ClipboardConfig::Custom { commands }).is_err()
            );
        }
    }

    fn params<'a>(command: &'a [String], workspace: &'a Path) -> ExternalCommandParameters<'a> {
        ExternalCommandParameters {
            command,
            workspace,
            target_hash: "cs:17@rep:demo repo@repserver:server:8087".into(),
            primary_parent: None,
            parent_hashes: vec!["cs:14@rep:demo repo@repserver:server:8087".into()],
            branches: vec!["br:/main/spaced branch@rep:demo repo@repserver:server:8087".into()],
            tags: vec!["lb:release one@rep:demo repo@repserver:server:8087".into()],
            all_refs: vec![
                "br:/main/spaced branch@rep:demo repo@repserver:server:8087".into(),
                "lb:release one@rep:demo repo@repserver:server:8087".into(),
            ],
            remote_branches: vec![],
            stash: None,
            area_width: 80,
            area_height: 20,
        }
    }

    #[test]
    fn qualified_aliases_and_argv_expansion() {
        let command: Vec<String> = [
            "helper",
            "{{changeset}}",
            "{{target_hash}}",
            "{{primary_parent}}",
            "{{first_parent_hash}}",
            "{{parents}}",
            "{{parent_hashes}}",
            "{{branches}}",
            "{{labels}}",
            "{{tags}}",
            "{{refs}}",
            "refs={{refs}}",
            "{{remote_branches}}",
            "{{stash}}",
            "{{area_width}}x{{area_height}}",
            "literal ; $value",
        ]
        .map(str::to_owned)
        .into();
        let p = params(&command, Path::new("."));
        let argv = build_user_command(&p).unwrap();
        assert_eq!(argv[1], p.target_hash);
        assert_eq!(argv[1], argv[2]);
        assert_eq!(&argv[3..5], ["", ""]); // merge-only must not become primary
        assert_eq!(argv[5], argv[6]);
        assert_eq!(argv[7], p.branches[0]);
        assert_eq!(argv[8], argv[9]);
        assert_eq!(&argv[10..12], p.all_refs);
        assert_eq!(argv[12], format!("refs={}", p.all_refs.join(" ")));
        assert_eq!(&argv[13..], ["", "80x20", "literal ; $value"]);
        let mut names = params(&command, Path::new("."));
        names.tags = vec!["lb:{{area_width}} with spaces@rep:demo@repserver:server".into()];
        assert_eq!(
            replace_command_arg("label={{labels}}", &names),
            format!("label={}", names.tags[0])
        );
        let mut root = params(&command, Path::new("."));
        root.parent_hashes.clear();
        assert!(!build_user_command(&root)
            .unwrap()
            .iter()
            .any(|s| s.starts_with("cs:14")));
    }

    // The test executable is a benign temp helper: no shell, cm, Git or workspace mutations.
    #[test]
    fn benign_helper() {
        use std::io::Write;
        let cwd = std::env::current_dir().unwrap();
        if cwd
            .file_name()
            .is_none_or(|n| n != "command workspace with spaces")
        {
            return;
        }
        let args: Vec<_> = std::env::args().collect();
        assert!(args.contains(&"br:/main/spaced branch@rep:demo repo@repserver:server:8087".into()));
        assert!(args.contains(&"literal ; $value".into()));
        let mode = std::fs::read_to_string(cwd.join("mode")).unwrap();
        if mode == "failure" {
            eprint!("benign failure");
            std::io::stderr().flush().unwrap();
            std::process::exit(7);
        }
        if mode == "output" {
            print!("cwd={}\noutput with spaces\tand tabs", cwd.display());
        }
        std::io::stdout().flush().unwrap();
        std::process::exit(0);
    }

    #[test]
    fn execution_cwd_output_empty_and_failures_in_both_modes() {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().join("command workspace with spaces");
        std::fs::create_dir(&cwd).unwrap();
        let command = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--exact".into(),
            "external::tests::benign_helper".into(),
            "--nocapture".into(),
            "--skip".into(),
            "{{branches}}".into(),
            "--skip".into(),
            "literal ; $value".into(),
        ];
        std::fs::write(cwd.join("mode"), "output").unwrap();
        let output = exec_user_command(params(&command, &cwd)).unwrap();
        assert!(output.contains(&format!("cwd={}", cwd.display())));
        assert!(output.contains("output with spaces\tand tabs"));
        assert!(exec_user_command_suspend(params(&command, &cwd)).is_ok());
        std::fs::write(cwd.join("mode"), "empty").unwrap();
        // The helper exits before the harness prints results; harness prelude may remain.
        assert!(!exec_user_command(params(&command, &cwd))
            .unwrap()
            .contains("output with spaces"));
        std::fs::write(cwd.join("mode"), "failure").unwrap();
        assert!(exec_user_command(params(&command, &cwd))
            .unwrap_err()
            .contains("benign failure"));
        assert!(exec_user_command_suspend(params(&command, &cwd))
            .unwrap_err()
            .contains("non-zero"));
        let missing = vec![cwd.join("missing-helper").to_string_lossy().into_owned()];
        assert!(exec_user_command(params(&missing, &cwd)).is_err());
        assert!(exec_user_command_suspend(params(&missing, &cwd)).is_err());
        for command in [
            vec![],
            vec!["".into()],
            vec![" ".into()],
            vec!["{{primary_parent}}".into(), "fallback".into()],
        ] {
            assert!(exec_user_command(params(&command, &cwd)).is_err());
            assert!(exec_user_command_suspend(params(&command, &cwd)).is_err());
        }
    }
}
