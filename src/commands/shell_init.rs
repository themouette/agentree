use crate::error::Result;
use clap::Parser;
use std::env;

#[derive(Parser, Debug)]
pub struct ShellInitArgs {
    /// Shell type to generate initialization for (auto-detected if not specified)
    #[arg(long)]
    shell: Option<String>,

    /// Include shell completion in output
    #[arg(long)]
    pub with_completion: bool,
}

#[derive(Debug, PartialEq)]
enum Shell {
    Bash,
    Zsh,
    Fish,
}

pub fn execute(args: ShellInitArgs, cmd: Option<&mut clap::Command>) -> Result<()> {
    let shell = if let Some(shell_name) = args.shell {
        parse_shell(&shell_name)?
    } else {
        detect_shell()?
    };

    // Print shell wrapper function
    match shell {
        Shell::Bash | Shell::Zsh => print_posix_function(),
        Shell::Fish => print_fish_function(),
    }

    // Optionally include completion
    if args.with_completion {
        if let Some(cmd) = cmd {
            println!(); // Blank line separator

            let shell_type = match shell {
                Shell::Bash => clap_complete::Shell::Bash,
                Shell::Zsh => clap_complete::Shell::Zsh,
                Shell::Fish => clap_complete::Shell::Fish,
            };

            // Generate completion using the completion module
            let completion_args = super::completion::CompletionArgs { shell: shell_type };
            super::completion::execute(completion_args, cmd)?;
        }
    }

    Ok(())
}

fn detect_shell() -> Result<Shell> {
    // Try to detect from parent process or $SHELL
    // First check SHELL env var
    if let Ok(shell_path) = env::var("SHELL") {
        if let Some(shell_name) = shell_path.split('/').next_back() {
            if let Ok(shell) = parse_shell(shell_name) {
                return Ok(shell);
            }
        }
    }

    // Default to bash/zsh (POSIX-compatible)
    Ok(Shell::Bash)
}

fn parse_shell(name: &str) -> Result<Shell> {
    match name.to_lowercase().as_str() {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        "fish" => Ok(Shell::Fish),
        _ => {
            // Default to POSIX-compatible (bash/zsh) for unknown shells
            eprintln!(
                "Warning: Unknown shell '{}'. Outputting POSIX-compatible initialization.",
                name
            );
            eprintln!("Supported shells: bash, zsh, fish");
            Ok(Shell::Bash)
        }
    }
}

fn posix_function() -> &'static str {
    r#"agentree() {
  if [ "$1" = "cd" ]; then
    eval "$(command agentree cd "${@:2}")"
  else
    command agentree "$@"
  fi
}
"#
}

fn fish_function() -> &'static str {
    r#"function agentree
  if test "$argv[1]" = "cd"
    eval (command agentree cd $argv[2..])
  else
    command agentree $argv
  end
end
"#
}

fn print_posix_function() {
    print!("{}", posix_function());
}

fn print_fish_function() {
    print!("{}", fish_function());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shell() {
        assert_eq!(parse_shell("bash").unwrap(), Shell::Bash);
        assert_eq!(parse_shell("zsh").unwrap(), Shell::Zsh);
        assert_eq!(parse_shell("fish").unwrap(), Shell::Fish);
        assert_eq!(parse_shell("BASH").unwrap(), Shell::Bash);
        // Unknown shells default to POSIX-compatible (bash)
        assert_eq!(parse_shell("unknown").unwrap(), Shell::Bash);
    }

    #[test]
    fn test_posix_function_output() {
        // Just verify it doesn't panic
        print_posix_function();
    }

    #[test]
    fn test_fish_function_output() {
        // Just verify it doesn't panic
        print_fish_function();
    }

    #[test]
    fn test_posix_cd_passes_all_remaining_args() {
        // The wrapper must use "${@:2}" so that:
        //   agentree cd              -> command agentree cd   (no args -> main repo)
        //   agentree cd feature      -> command agentree cd feature
        //   agentree cd feat -b main -> command agentree cd feat -b main
        let wrapper = posix_function();
        assert!(
            wrapper.contains(r#"cd "${@:2}")"#),
            "POSIX wrapper should pass all args after 'cd' using ${{@:2}}"
        );
    }

    #[test]
    fn test_fish_cd_passes_all_remaining_args() {
        // Fish: $argv[2..] passes all args from the second onwards,
        // so `agentree cd` with no extra args works (main repo navigation).
        let wrapper = fish_function();
        assert!(
            wrapper.contains("cd $argv[2..]"),
            "Fish wrapper should pass all args after 'cd' using $argv[2..]"
        );
    }
}
