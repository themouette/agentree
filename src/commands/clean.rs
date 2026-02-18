use crate::commands::common::WorkspaceContext;
use crate::error::Result;
use crate::worktree::recovery;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct CleanArgs {
    // No arguments needed for now
}

pub fn execute(_args: CleanArgs) -> Result<()> {
    let _ctx = WorkspaceContext::init(None, None, None, None)?;
    recovery::try_repair()?;
    recovery::prune()?;
    println!("Cleanup complete.");
    Ok(())
}
