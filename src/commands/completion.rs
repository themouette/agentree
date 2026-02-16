use crate::constants::{BACKEND_NAMES, DEFAULT_AGENTS};
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
    // Build agent and backend lists at runtime from constants
    let agents = DEFAULT_AGENTS.join(" ");
    let backends = BACKEND_NAMES.join(" ");

    let dynamic_completion = format!(
        r#"

# Dynamic branch and value completion for agentree
_agentree_branches() {{
    if git rev-parse --git-dir > /dev/null 2>&1; then
        git branch --format='%(refname:short)' 2>/dev/null
    fi
}}

# Agent completions for --agent flag
_agentree_agents() {{
    echo "{agents}"
}}

# Override completion for commands that take branch arguments
_agentree_branch_commands() {{
    local cur="${{COMP_WORDS[COMP_CWORD]}}"
    local prev="${{COMP_WORDS[COMP_CWORD-1]}}"
    local cmd="${{COMP_WORDS[1]}}"

    # Provide values for specific flags
    case "$prev" in
        --agent)
            # Provide agent completions
            COMPREPLY=( $(compgen -W "$(_agentree_agents)" -- "$cur") )
            return 0
            ;;
        --backend)
            # Provide backend completions
            COMPREPLY=( $(compgen -W "{backends}" -- "$cur") )
            return 0
            ;;
    esac

    # Commands that take branch as first argument
    case "$cmd" in
        shell|agent|exec|remove|cd|editor)
            # Check if previous word is the command itself or a flag that expects a value
            # This handles: agentree shell <branch> and agentree shell --flag value <branch>
            case "$prev" in
                "$cmd")
                    # Directly after command - suggest branches
                    COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
                    return 0
                    ;;
                *)
                    # Check if we're likely at a branch position (not a flag)
                    if [[ "$cur" != -* ]] && [[ "$prev" != -* ]]; then
                        COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
                        return 0
                    fi
                    ;;
            esac
            ;;
        create)
            # For create: <branch> [start_ref]
            # Context-aware: check what the previous word was instead of position
            case "$prev" in
                create)
                    # Directly after 'create' - new branch name (no completion)
                    COMPREPLY=()
                    return 0
                    ;;
                *)
                    # If previous word is not a flag and current word is not a flag,
                    # we're likely at the start_ref position - suggest branches
                    if [[ "$cur" != -* ]] && [[ "$prev" != -* ]] && [[ "$prev" != "$cmd" ]]; then
                        COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
                        return 0
                    fi
                    ;;
            esac
            ;;
    esac

    return 1
}}

# Wrap the original completion function
if declare -F _agentree > /dev/null; then
    # Rename function by replacing only the function declaration line
    # Using sed to avoid replacing _agentree in function body
    eval "$(declare -f _agentree | sed '1s/_agentree/_agentree_static/')"

    _agentree() {{
        # Try dynamic completion first
        _agentree_branch_commands && return 0
        # Fall back to static completion
        _agentree_static
    }}
fi
"#,
        agents = agents,
        backends = backends
    );

    print!("{}", dynamic_completion);
    Ok(())
}

/// Generate dynamic completion helper for zsh
fn write_zsh_dynamic_completion() -> Result<()> {
    // Build agent and backend lists at runtime from constants
    let agents = DEFAULT_AGENTS.join(" ");
    let backends = BACKEND_NAMES.join(" ");

    let dynamic_completion = format!(
        r#"

# Dynamic branch and value completion for agentree (zsh)
_agentree_branches() {{
    local branches
    if git rev-parse --git-dir > /dev/null 2>&1; then
        branches=(${{(f)"$(git branch --format='%(refname:short)' 2>/dev/null)"}})
        _describe 'branches' branches
    fi
}}

_agentree_agents() {{
    local agents
    agents=({agents})
    _describe 'agents' agents
}}

_agentree_backends() {{
    local backends
    backends=({backends})
    _describe 'backends' backends
}}

# Add branch and value completion to relevant commands
# This extends the generated completion
if (( $+functions[_agentree] )); then
    # Store original function
    functions[_agentree_static]=$functions[_agentree]

    # Override with dynamic version
    _agentree() {{
        local curcontext="$curcontext" state line
        typeset -A opt_args
        local cmd
        local current_word="${{words[CURRENT]}}"
        local prev_word="${{words[$((CURRENT-1))]}}"

        # Check if we're completing a flag value
        case "$prev_word" in
            --agent)
                _agentree_agents
                return 0
                ;;
            --backend)
                _agentree_backends
                return 0
                ;;
        esac

        # If current word starts with -, let static completion handle flags
        if [[ "$current_word" == -* ]]; then
            _agentree_static
            return 0
        fi

        # Parse the command line to get the subcommand
        local -a args
        args=(${{words[2,-1]}})

        # Find the subcommand (first non-flag argument after agentree)
        local subcmd=""
        for word in ${{words[2,-1]}}; do
            if [[ "$word" != -* ]]; then
                subcmd="$word"
                break
            fi
        done

        # For commands that take branch arguments, offer both branches and flags
        case "$subcmd" in
            shell|agent|exec|remove|cd|editor)
                # Offer both branches and let static completion add flags
                _agentree_branches
                _agentree_static
                return 0
                ;;
            create)
                # For create: first arg is branch name (no completion), second is start_ref (branches)
                # Count non-flag arguments
                local -a positional
                positional=()
                for word in ${{words[2,-1]}}; do
                    if [[ "$word" != -* ]] && [[ "$word" != "$subcmd" ]]; then
                        positional+=("$word")
                    fi
                done

                if [[ ${{#positional}} -eq 1 ]]; then
                    # Second positional arg - suggest branches for start_ref
                    _agentree_branches
                fi
                _agentree_static
                return 0
                ;;
            *)
                _agentree_static
                return 0
                ;;
        esac
    }}
fi
"#,
        agents = agents,
        backends = backends
    );

    print!("{}", dynamic_completion);
    Ok(())
}

/// Generate dynamic completion helper for fish
fn write_fish_dynamic_completion() -> Result<()> {
    // Build agent and backend lists at runtime from constants
    let agents = DEFAULT_AGENTS.join("\n    echo ");
    let backends = BACKEND_NAMES.join("\n    echo ");

    let dynamic_completion = format!(
        r#"

# Dynamic branch and value completion for agentree (fish)
function __agentree_branches
    if git rev-parse --git-dir >/dev/null 2>&1
        git branch --format='%(refname:short)' 2>/dev/null
    end
end

function __agentree_agents
    echo {agents}
end

function __agentree_backends
    echo {backends}
end

# Add value completions for flags
complete -c agentree -l agent -f -a "(__agentree_agents)" -d "AI agent to use"
complete -c agentree -l backend -f -a "(__agentree_backends)" -d "Backend to use"

# Add branch completion to commands that take branch arguments
complete -c agentree -n "__fish_seen_subcommand_from shell" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from agent" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from exec" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from remove" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from cd" -a "(__agentree_branches)" -d "Branch"
complete -c agentree -n "__fish_seen_subcommand_from editor" -a "(__agentree_branches)" -d "Branch"

# For create command, second argument is start_ref (suggest branches)
# First argument is new branch name (no suggestions)
complete -c agentree -n "__fish_seen_subcommand_from create" -a "(__agentree_branches)" -d "Start ref"
"#,
        agents = agents,
        backends = backends
    );

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
