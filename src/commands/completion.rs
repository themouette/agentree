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

# Dynamic flag value completion for agentree
_agentree_branches() {{
    if git rev-parse --git-dir > /dev/null 2>&1; then
        git branch --format='%(refname:short)' 2>/dev/null
    fi
}}

_agentree_agents() {{
    echo "{agents}"
}}

# Enhanced completion that adds dynamic flag values
_agentree_enhanced() {{
    local cur="${{COMP_WORDS[COMP_CWORD]}}"
    local prev="${{COMP_WORDS[COMP_CWORD-1]}}"

    # Handle flag value completions
    case "$prev" in
        --agent|-a)
            COMPREPLY=( $(compgen -W "$(_agentree_agents)" -- "$cur") )
            return 0
            ;;
        --backend)
            COMPREPLY=( $(compgen -W "{backends}" -- "$cur") )
            return 0
            ;;
        --base|-b)
            COMPREPLY=( $(compgen -W "$(_agentree_branches)" -- "$cur") )
            return 0
            ;;
    esac

    # For everything else, use static completion
    _agentree_static
}}

# Wrap the original completion function
if declare -F _agentree > /dev/null; then
    # Rename function by replacing only the function declaration line
    # Using sed to avoid replacing _agentree in function body
    eval "$(declare -f _agentree | sed '1s/_agentree/_agentree_static/')"

    _agentree() {{
        _agentree_enhanced
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

# Dynamic flag value completion for agentree (zsh)
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

# Add dynamic flag value completion
# This extends the generated completion
if (( $+functions[_agentree] )); then
    # Store original function
    functions[_agentree_static]=$functions[_agentree]

    # Override with dynamic version
    _agentree() {{
        local prev_word="${{words[$((CURRENT-1))]}}"

        # Handle flag value completions
        case "$prev_word" in
            --agent|-a)
                _agentree_agents
                return 0
                ;;
            --backend)
                _agentree_backends
                return 0
                ;;
            --base|-b)
                _agentree_branches
                return 0
                ;;
        esac

        # For everything else, use static completion
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

/// Generate dynamic completion helper for fish
fn write_fish_dynamic_completion() -> Result<()> {
    // Build agent and backend lists at runtime from constants
    let agents = DEFAULT_AGENTS.join("\n    echo ");
    let backends = BACKEND_NAMES.join("\n    echo ");

    let dynamic_completion = format!(
        r#"

# Dynamic flag value completion for agentree (fish)
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

# Add dynamic value completions for flags
complete -c agentree -l agent -f -a "(__agentree_agents)" -d "AI agent to use"
complete -c agentree -l backend -f -a "(__agentree_backends)" -d "Backend to use"
complete -c agentree -l base -f -a "(__agentree_branches)" -d "Base branch to create from"
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
