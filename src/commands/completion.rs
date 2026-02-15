use crate::error::Result;
use clap_complete::{generate, Shell};
use std::io;

#[derive(clap::Args, Debug)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

pub fn execute(args: CompletionArgs, cmd: &mut clap::Command) -> Result<()> {
    let bin_name = "agentree";

    generate(args.shell, cmd, bin_name, &mut io::stdout());

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
}
