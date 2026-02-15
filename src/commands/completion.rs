use crate::error::Result;
use clap_complete::{generate, Shell};
use std::io::{self, Write};

#[derive(clap::Args, Debug)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn execute(args: CompletionArgs, cmd: &mut clap::Command) -> Result<()> {
    let bin_name = "agentree";

    // Generate base completions from clap
    let mut buffer = Vec::new();
    generate(args.shell, cmd, bin_name, &mut buffer);

    // Write base completions
    io::stdout().write_all(&buffer)?;

    // Add dynamic branch completion helpers
    match args.shell {
        Shell::Bash => write_bash_dynamic_completion()?,
        Shell::Zsh => write_zsh_dynamic_completion()?,
        Shell::Fish => write_fish_dynamic_completion()?,
        _ => {
            // Other shells get static completions only for now
        }
    }

    Ok(())
}

/// Generate dynamic completion helper for bash
fn write_bash_dynamic_completion() -> Result<()> {
    let dynamic_completion = r#"

# Dynamic branch completion for agentree
_agentree_branches() {
    if git rev-parse --git-dir > /dev/null 2>&1; then
        git branch --format='%(refname:short)' 2>/dev/null
    fi
}

# Override completion for commands that take branch arguments
_agentree_branch_commands() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local prev="${COMP_WORDS[COMP_CWORD-1]}"
    local cmd="${COMP_WORDS[1]}"

    # Commands that take branch as first argument
    case "$cmd" in
        shell|agent|exec|remove|cd)
            # If we're at the position for branch name (after the command)
            if [[ $COMP_CWORD -eq 2 ]] || [[ "$prev" == "$cmd" ]]; then
                COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
                return 0
            fi
            ;;
        create)
            # For create: <branch> [start_ref]
            # First arg (position 2) is new branch name - no completion
            # Second arg (position 3) is start_ref - suggest existing branches
            if [[ $COMP_CWORD -eq 2 ]]; then
                # New branch name - no completion
                COMPREPLY=()
                return 0
            elif [[ $COMP_CWORD -eq 3 ]]; then
                # Start ref - suggest branches
                COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
                return 0
            fi
            ;;
    esac

    return 1
}

# Wrap the original completion function
if declare -F _agentree > /dev/null; then
    # Rename function by replacing only the function declaration line
    # Using sed to avoid replacing _agentree in function body
    eval "$(declare -f _agentree | sed '1s/_agentree/_agentree_static/')"

    _agentree() {
        # Try dynamic completion first
        _agentree_branch_commands && return 0
        # Fall back to static completion
        _agentree_static
    }
fi
"#;

    print!("{}", dynamic_completion);
    Ok(())
}

/// Generate dynamic completion helper for zsh
fn write_zsh_dynamic_completion() -> Result<()> {
    let dynamic_completion = r#"

# Dynamic branch completion for agentree (zsh)
_agentree_branches() {
    local branches
    if git rev-parse --git-dir > /dev/null 2>&1; then
        branches=(${(f)"$(git branch --format='%(refname:short)' 2>/dev/null)"})
        _describe 'branches' branches
    fi
}

# Add branch completion to relevant commands
# This extends the generated completion
if (( $+functions[_agentree] )); then
    # Store original function
    functions[_agentree_static]=$functions[_agentree]

    # Override with dynamic version
    _agentree() {
        local curcontext="$curcontext" state line
        local cmd

        _arguments -C \
            '1: :->command' \
            '*:: :->args' && return 0

        case $state in
            command)
                _agentree_static
                ;;
            args)
                cmd="${line[1]}"
                case "$cmd" in
                    shell|agent|exec|remove|cd)
                        _agentree_branches
                        ;;
                    create)
                        # For create: <branch> [start_ref]
                        # Second argument is start_ref - suggest branches
                        if [[ $#line -eq 2 ]]; then
                            _agentree_branches
                        fi
                        ;;
                    *)
                        _agentree_static
                        ;;
                esac
                ;;
        esac
    }
fi
"#;

    print!("{}", dynamic_completion);
    Ok(())
}

/// Generate dynamic completion helper for fish
fn write_fish_dynamic_completion() -> Result<()> {
    let dynamic_completion = r#"

# Dynamic branch completion for agentree (fish)
function __agentree_branches
    if git rev-parse --git-dir >/dev/null 2>&1
        git branch --format='%(refname:short)' 2>/dev/null
    end
end

# Add branch completion to commands that take branch arguments
complete -c agentree -n "__fish_seen_subcommand_from shell" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from agent" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from exec" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from remove" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from cd" -a "(__agentree_branches)" -d "Branch"

# For create command, second argument is start_ref (suggest branches)
# First argument is new branch name (no suggestions)
complete -c agentree -n "__fish_seen_subcommand_from create" -a "(__agentree_branches)" -d "Start ref"
"#;

    print!("{}", dynamic_completion);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_args_parsing() {
        // Test that we can parse shell types
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            args: CompletionArgs,
        }

        // Test bash
        let cli = TestCli::parse_from(["test", "bash"]);
        assert!(matches!(cli.args.shell, Shell::Bash));

        // Test zsh
        let cli = TestCli::parse_from(["test", "zsh"]);
        assert!(matches!(cli.args.shell, Shell::Zsh));

        // Test fish
        let cli = TestCli::parse_from(["test", "fish"]);
        assert!(matches!(cli.args.shell, Shell::Fish));
    }

    #[test]
    fn test_dynamic_completion_functions_exist() {
        // Test that our dynamic completion functions are defined
        // Just verify they don't panic
        write_bash_dynamic_completion().unwrap();
        write_zsh_dynamic_completion().unwrap();
        write_fish_dynamic_completion().unwrap();
    }
}
