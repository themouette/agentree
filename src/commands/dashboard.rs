use crate::dashboard;
use crate::error::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct DashboardArgs {
    /// Run as TUI process inside the left tmux pane (internal use, hidden from help)
    #[arg(long, hide = true)]
    pub tui: bool,

    /// Kill the dashboard session and stop the daemon
    #[arg(long)]
    pub kill: bool,
}

pub fn execute(args: DashboardArgs) -> Result<()> {
    if args.kill {
        return dashboard::kill_dashboard();
    }
    dashboard::execute(args.tui)
}
