use crate::daemon::protocol::{Request, Response, WorkspaceInfo};
use crate::error::{AgentreeError, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Synchronous client that sends one-shot JSON requests to the daemon socket.
pub struct DaemonClient {
    sock_path: PathBuf,
}

impl DaemonClient {
    pub fn connect(sock_path: &Path) -> Result<Self> {
        // Just store the path; we open a new connection per request (one-shot protocol)
        Ok(DaemonClient {
            sock_path: sock_path.to_path_buf(),
        })
    }

    /// Send a request and receive a response (one-shot connection)
    fn send(&self, request: &Request) -> Result<Response> {
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixStream;

            let mut stream = UnixStream::connect(&self.sock_path)
                .map_err(|e| AgentreeError::Git(format!("Cannot connect to daemon: {}", e)))?;

            let json = serde_json::to_string(request).map_err(AgentreeError::Json)?;
            stream
                .write_all(json.as_bytes())
                .map_err(AgentreeError::Io)?;
            stream.write_all(b"\n").map_err(AgentreeError::Io)?;
            stream.flush().map_err(AgentreeError::Io)?;

            let reader = BufReader::new(&stream);
            let line = reader
                .lines()
                .next()
                .ok_or_else(|| AgentreeError::Git("Daemon sent empty response".to_string()))?
                .map_err(AgentreeError::Io)?;

            serde_json::from_str(&line).map_err(AgentreeError::Json)
        }

        #[cfg(not(unix))]
        Err(AgentreeError::DaemonNotRunning)
    }

    /// Request the list of all known workspaces from the daemon
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>> {
        match self.send(&Request::List)? {
            Response::Workspaces(ws) => Ok(ws),
            Response::Err(e) => Err(AgentreeError::Git(format!("Daemon error: {}", e))),
            _ => Err(AgentreeError::Git(
                "Unexpected daemon response".to_string(),
            )),
        }
    }

    /// Tell the daemon to clear the attention flag for a branch
    pub fn clear_attention(&self, branch: &str) -> Result<()> {
        match self.send(&Request::ClearAttention {
            branch: branch.to_string(),
        })? {
            Response::Ok => Ok(()),
            Response::Err(e) => Err(AgentreeError::Git(format!("Daemon error: {}", e))),
            _ => Err(AgentreeError::Git(
                "Unexpected daemon response".to_string(),
            )),
        }
    }
}

/// Try to connect to the daemon socket without returning an error on failure.
/// Used to check whether the daemon is already running.
pub fn try_connect(sock_path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        UnixStream::connect(sock_path).is_ok()
    }
    #[cfg(not(unix))]
    false
}
