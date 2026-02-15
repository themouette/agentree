use crate::error::Result;
use clap::Parser;
use std::env;

#[derive(Parser, Debug)]
pub struct ShellInitArgs {
    /// Shell type to generate initialization for (auto-detected if not specified)
    #[arg(long)]
    shell: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Shell {
    Bash,
    Zsh,
    Fish,
}

pub fn execute(args: ShellInitArgs) -> Result<()> {
    let shell = if let Some(shell_name) = args.shell {
        parse_shell(&shell_name)?
    } else {
        detect_shell()?
    };

    match shell {
        Shell::Bash | Shell::Zsh => print_posix_function(),
        Shell::Fish => print_fish_function(),
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

fn print_posix_function() {
    print!(
        r#"agentree() {{
  if [ "$1" = "cd" ]; then
    eval "$(command agentree cd "$2")"
  else
    command agentree "$@"
  fi
}}
"#
    );
}

fn print_fish_function() {
    print!(
        r#"function agentree
  if test "$argv[1]" = "cd"
    eval (command agentree cd $argv[2])
  else
    command agentree $argv
  end
end
"#
    );
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
}
