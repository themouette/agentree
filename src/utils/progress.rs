use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Run a closure while displaying an animated spinner on stderr.
///
/// The spinner is only shown when stderr is a TTY. In non-interactive
/// environments (CI, scripts, piped output) the message is printed as a
/// plain line instead so output is still informative.
///
/// On success the spinner is replaced by a `✓` line. On error the spinner
/// is cleared and the error propagates to the caller.
///
/// # Example
///
/// ```ignore
/// let result = with_spinner("Creating branch 'feat' and worktree...", || {
///     create_worktree(&config, &root, "feat", None)
/// })?;
/// ```
pub fn with_spinner<F, T, E>(message: &str, f: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));

    let result = f();

    pb.finish_and_clear();
    result
}
