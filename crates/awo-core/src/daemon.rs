//! Daemon server infrastructure for `awod`.
//!
//! Provides a synchronous JSON-RPC 2.0 server that listens on a Unix
//! Domain Socket and dispatches commands through the [`Dispatcher`] trait.
//! The daemon holds an exclusive file lock to guarantee single-instance
//! operation and single-writer safety for the SQLite database.

use crate::app::AppPaths;
use crate::dispatch::{Dispatcher, RpcResponse, dispatch_rpc, parse_rpc_request};
use crate::error::{AwoError, AwoResult};
use fs2::FileExt;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Daemon state: manages the socket, lock file, and shutdown signal.
pub struct DaemonServer {
    socket_path: PathBuf,
    lock_path: PathBuf,
    _lock_file: fs::File,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Options for starting the daemon.
pub struct DaemonOptions {
    pub socket_path: PathBuf,
    pub lock_path: PathBuf,
}

impl DaemonOptions {
    pub fn from_paths(paths: &AppPaths) -> Self {
        Self {
            socket_path: paths.daemon_socket_path(),
            lock_path: paths.daemon_lock_path(),
        }
    }
}

impl DaemonServer {
    /// Acquire the daemon lock and prepare to listen.
    ///
    /// Returns an error if another daemon instance already holds the lock.
    pub fn acquire(options: DaemonOptions) -> AwoResult<Self> {
        if let Some(parent) = options.lock_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| AwoError::io("create daemon lock dir", parent, source))?;
        }

        let lock_file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&options.lock_path)
            .map_err(|source| AwoError::io("open daemon lock file", &options.lock_path, source))?;

        lock_file
            .try_lock_exclusive()
            .map_err(|source| AwoError::file_lock("exclusive", &options.lock_path, source))?;

        // Clean up any stale socket file from a previous crash
        if options.socket_path.exists() {
            let _ = fs::remove_file(&options.socket_path);
        }

        Ok(Self {
            socket_path: options.socket_path,
            lock_path: options.lock_path,
            _lock_file: lock_file,
            shutdown: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Returns a handle that can be used to request a graceful shutdown.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            flag: self.shutdown.clone(),
        }
    }

    /// Run the daemon event loop, accepting connections and dispatching
    /// JSON-RPC requests through the given dispatcher.
    ///
    /// This function blocks until [`ShutdownHandle::request_shutdown`] is
    /// called or an unrecoverable error occurs.
    #[cfg(unix)]
    pub fn run(&self, dispatcher: &mut dyn Dispatcher) -> AwoResult<()> {
        use std::os::unix::net::UnixListener;

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|source| AwoError::io("bind daemon socket", &self.socket_path, source))?;

        // Set a short accept timeout so we can check the shutdown flag
        listener
            .set_nonblocking(true)
            .map_err(|source| AwoError::io("set socket nonblocking", &self.socket_path, source))?;

        tracing::info!(socket = %self.socket_path.display(), "awod listening");

        while !self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    // Set blocking for the accepted connection
                    stream.set_nonblocking(false).map_err(|source| {
                        AwoError::io("set connection blocking", &self.socket_path, source)
                    })?;
                    if let Err(error) = handle_connection(stream, dispatcher) {
                        tracing::warn!(%error, "connection handler error");
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connection; sleep briefly and retry
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(source) => {
                    return Err(AwoError::io(
                        "accept daemon connection",
                        &self.socket_path,
                        source,
                    ));
                }
            }
        }

        tracing::info!("awod shutting down");
        self.cleanup();
        Ok(())
    }

    /// Stub for non-Unix platforms.
    #[cfg(not(unix))]
    pub fn run(&self, _dispatcher: &mut dyn Dispatcher) -> AwoResult<()> {
        Err(AwoError::supervisor(
            "daemon mode is not yet supported on this platform",
        ))
    }

    fn cleanup(&self) {
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl Drop for DaemonServer {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// A handle for requesting a graceful daemon shutdown.
#[derive(Clone)]
pub struct ShutdownHandle {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ShutdownHandle {
    pub fn request_shutdown(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Handle a single connection: read line-delimited JSON-RPC requests,
/// dispatch each one, and write back the response.
#[cfg(unix)]
fn handle_connection(
    stream: std::os::unix::net::UnixStream,
    dispatcher: &mut dyn Dispatcher,
) -> AwoResult<()> {
    let reader = BufReader::new(&stream);
    let mut writer = &stream;

    for line in reader.lines() {
        let line =
            line.map_err(|source| AwoError::io("read from socket", Path::new("<socket>"), source))?;
        if line.is_empty() {
            continue;
        }

        let response = match parse_rpc_request(line.as_bytes()) {
            Ok(request) => dispatch_rpc(dispatcher, &request),
            Err(error_response) => *error_response,
        };

        let response_bytes = serde_json::to_vec(&response)
            .map_err(|e| AwoError::supervisor(format!("failed to serialize RPC response: {e}")))?;
        writer
            .write_all(&response_bytes)
            .map_err(|source| AwoError::io("write to socket", Path::new("<socket>"), source))?;
        writer.write_all(b"\n").map_err(|source| {
            AwoError::io("write newline to socket", Path::new("<socket>"), source)
        })?;
        writer
            .flush()
            .map_err(|source| AwoError::io("flush socket", Path::new("<socket>"), source))?;
    }

    Ok(())
}

/// Check whether a daemon is currently running and reachable.
pub fn daemon_is_running(paths: &AppPaths) -> bool {
    let socket_path = paths.daemon_socket_path();
    if !socket_path.exists() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        UnixStream::connect(&socket_path).is_ok()
    }

    #[cfg(not(unix))]
    {
        false
    }
}

// ---------------------------------------------------------------------------
// Client: connect to a running daemon
// ---------------------------------------------------------------------------

/// A client that connects to a running `awod` daemon and dispatches
/// commands over the JSON-RPC socket.
#[cfg(unix)]
pub struct DaemonClient {
    reader: BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
    next_id: u64,
}

#[cfg(unix)]
impl DaemonClient {
    /// Connect to the daemon at the given socket path.
    pub fn connect(socket_path: &Path) -> AwoResult<Self> {
        use std::os::unix::net::UnixStream;
        let stream = UnixStream::connect(socket_path)
            .map_err(|source| AwoError::io("connect to daemon socket", socket_path, source))?;
        let reader = BufReader::new(
            stream
                .try_clone()
                .map_err(|source| AwoError::io("clone daemon socket", socket_path, source))?,
        );
        Ok(Self {
            reader,
            writer: stream,
            next_id: 1,
        })
    }

    /// Send a command and wait for the JSON-RPC response.
    pub fn call(&mut self, command: &crate::commands::Command) -> AwoResult<RpcResponse> {
        let id = serde_json::Value::Number(self.next_id.into());
        self.next_id += 1;

        let request = crate::dispatch::RpcRequest::from_command(command, id)
            .map_err(|e| AwoError::supervisor(format!("failed to build RPC request: {e}")))?;
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| AwoError::supervisor(format!("failed to serialize RPC request: {e}")))?;

        self.writer.write_all(&request_bytes).map_err(|source| {
            AwoError::io("write to daemon socket", Path::new("<socket>"), source)
        })?;
        self.writer.write_all(b"\n").map_err(|source| {
            AwoError::io(
                "write newline to daemon socket",
                Path::new("<socket>"),
                source,
            )
        })?;
        self.writer
            .flush()
            .map_err(|source| AwoError::io("flush daemon socket", Path::new("<socket>"), source))?;

        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(|source| {
            AwoError::io("read from daemon socket", Path::new("<socket>"), source)
        })?;

        serde_json::from_str::<RpcResponse>(&line)
            .map_err(|e| AwoError::supervisor(format!("malformed daemon response: {e}")))
    }
}

#[cfg(unix)]
impl crate::dispatch::Dispatcher for DaemonClient {
    fn dispatch(
        &mut self,
        command: crate::commands::Command,
    ) -> AwoResult<crate::commands::CommandOutcome> {
        let response = self.call(&command)?;
        if let Some(error) = response.error {
            return Err(AwoError::supervisor(error.message));
        }
        match response.result {
            Some(result) => Ok(crate::commands::CommandOutcome {
                summary: result.summary,
                events: result.events,
            }),
            None => Err(AwoError::supervisor("empty daemon response")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_options_from_paths() {
        let paths = AppPaths {
            config_dir: PathBuf::from("/tmp/config"),
            data_dir: PathBuf::from("/tmp/data"),
            state_db_path: PathBuf::from("/tmp/state.sqlite3"),
            logs_dir: PathBuf::from("/tmp/logs"),
            repos_dir: PathBuf::from("/tmp/repos"),
            clones_dir: PathBuf::from("/tmp/clones"),
            teams_dir: PathBuf::from("/tmp/teams"),
        };
        let options = DaemonOptions::from_paths(&paths);
        assert_eq!(options.socket_path, PathBuf::from("/tmp/data/awod.sock"));
        assert_eq!(options.lock_path, PathBuf::from("/tmp/data/awod.lock"));
    }

    #[test]
    fn daemon_is_not_running_when_no_socket_exists() {
        let paths = AppPaths {
            config_dir: PathBuf::from("/nonexistent"),
            data_dir: PathBuf::from("/nonexistent"),
            state_db_path: PathBuf::from("/nonexistent/state.sqlite3"),
            logs_dir: PathBuf::from("/nonexistent/logs"),
            repos_dir: PathBuf::from("/nonexistent/repos"),
            clones_dir: PathBuf::from("/nonexistent/clones"),
            teams_dir: PathBuf::from("/nonexistent/teams"),
        };
        assert!(!daemon_is_running(&paths));
    }

    #[cfg(unix)]
    #[test]
    fn daemon_acquire_and_double_lock_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let options = DaemonOptions {
            socket_path: temp_dir.path().join("test.sock"),
            lock_path: temp_dir.path().join("test.lock"),
        };
        let _server = DaemonServer::acquire(options).unwrap();

        // Second acquisition should fail because the lock is held
        let options2 = DaemonOptions {
            socket_path: temp_dir.path().join("test.sock"),
            lock_path: temp_dir.path().join("test.lock"),
        };
        let result = DaemonServer::acquire(options2);
        assert!(result.is_err(), "expected lock conflict");
    }

    #[cfg(unix)]
    #[test]
    fn daemon_server_end_to_end() {
        use crate::commands::{Command, CommandOutcome};
        use crate::dispatch::Dispatcher;
        use crate::error::AwoResult;

        struct EchoDispatcher;
        impl Dispatcher for EchoDispatcher {
            fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
                Ok(CommandOutcome {
                    summary: format!("echoed: {}", command.method_name()),
                    events: vec![],
                })
            }
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let socket_path = temp_dir.path().join("e2e.sock");
        let lock_path = temp_dir.path().join("e2e.lock");

        let options = DaemonOptions {
            socket_path: socket_path.clone(),
            lock_path,
        };
        let server = DaemonServer::acquire(options).unwrap();
        let shutdown = server.shutdown_handle();

        // Run the server in a background thread
        let server_thread = std::thread::spawn(move || {
            let mut dispatcher = EchoDispatcher;
            server.run(&mut dispatcher)
        });

        // Give the server a moment to bind
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Connect as client and send a request
        let mut client = DaemonClient::connect(&socket_path).unwrap();
        let command = Command::NoOp {
            label: "e2e-test".to_string(),
        };
        let response = client.call(&command).unwrap();
        assert!(response.error.is_none(), "expected success: {response:?}");
        let result = response.result.unwrap();
        assert!(result.summary.contains("echoed"));

        // Drop the client so the server's handle_connection() returns,
        // allowing the accept loop to check the shutdown flag.
        drop(client);

        // Shut down
        shutdown.request_shutdown();
        server_thread.join().unwrap().unwrap();
    }
}
