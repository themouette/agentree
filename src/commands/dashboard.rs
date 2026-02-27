use crate::dashboard;
use crate::error::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DashboardArgs {
    /// Run as TUI process inside the left tmux pane (internal use, hidden from help)
    #[arg(long, hide = true)]
    pub tui: bool,
}

pub fn execute(args: DashboardArgs) -> Result<()> {
    dashboard::execute(args.tui)
}
