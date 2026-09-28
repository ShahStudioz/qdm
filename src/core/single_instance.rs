//! Cross-platform Single-Instance Enforcement and Local IPC.
//!
//! Ensures only one instance of QDM runs per user session.
//! If a second instance is launched:
//! - It forwards any command-line argument (or a `Focus` command) to the running instance via local IPC.
//! - On Windows: calls `AllowSetForegroundWindow` so the running instance can claim foreground focus.
//! - The secondary process exits immediately with code 0.
//! - The primary instance restores from tray/minimized state and focuses its window.

use std::sync::OnceLock;
use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleInstanceCommand {
    Focus,
    Open(String),
}

static IPC_SENDER: OnceLock<broadcast::Sender<SingleInstanceCommand>> = OnceLock::new();
static INITIAL_ARG: OnceLock<String> = OnceLock::new();
static STARTED_MINIMIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn set_started_minimized(val: bool) {
    STARTED_MINIMIZED.store(val, std::sync::atomic::Ordering::SeqCst);
}

pub fn take_started_minimized() -> bool {
    STARTED_MINIMIZED.swap(false, std::sync::atomic::Ordering::SeqCst)
}

fn get_sender() -> &'static broadcast::Sender<SingleInstanceCommand> {
    IPC_SENDER.get_or_init(|| {
        let (tx, _) = broadcast::channel(16);
        tx
    })
}

/// Returns a receiver to listen for commands sent by secondary instances.
pub fn subscribe() -> broadcast::Receiver<SingleInstanceCommand> {
    get_sender().subscribe()
}

/// Retrieves and consumes any initial CLI argument that was passed on first app startup.
pub fn take_initial_arg() -> Option<String> {
    INITIAL_ARG.get().cloned()
}

/// Parses an IPC line into a `SingleInstanceCommand`.
pub fn parse_command(line: &str) -> SingleInstanceCommand {
    let trimmed = line.trim();
    if let Some(url) = trimmed.strip_prefix("OPEN:") {
        SingleInstanceCommand::Open(url.to_string())
    } else {
        SingleInstanceCommand::Focus
    }
}

// ===========================================================================
// Windows Named Pipe Implementation
// ===========================================================================
#[cfg(windows)]
mod platform {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
    use tokio::net::windows::named_pipe::ServerOptions;

    fn get_pipe_name() -> String {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
        format!(r"\\.\pipe\qdm-single-instance-{}", user)
    }

    /// Attempts to notify an existing instance. Returns `true` if an existing instance
    /// was found and notified, or `false` if this process is the primary instance.
    pub fn notify_existing_or_acquire(cli_arg: Option<String>) -> bool {
        let pipe_name = get_pipe_name();

        // Try connecting to existing pipe
        let mut stream_opt = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .ok();

        // If pipe was busy, give it a short retry
        if stream_opt.is_none() {
            for _ in 0..3 {
                std::thread::sleep(Duration::from_millis(50));
                if let Ok(file) = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&pipe_name)
                {
                    stream_opt = Some(file);
                    break;
                }
            }
        }

        if let Some(mut stream) = stream_opt {
            // Allow existing instance to take foreground focus
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(0xFFFFFFFF);
            }

            let msg = match cli_arg {
                Some(arg) if !arg.trim().is_empty() => format!("OPEN:{}\n", arg.trim()),
                _ => "FOCUS\n".to_string(),
            };

            let _ = stream.write_all(msg.as_bytes());
            let _ = stream.flush();

            // Optional: read a brief ack
            let mut reader = BufReader::new(stream);
            let mut ack = String::new();
            let _ = reader.read_line(&mut ack);

            return true;
        }

        // We are the primary instance. Save initial arg if any.
        if let Some(arg) = cli_arg {
            if !arg.trim().is_empty() {
                let _ = INITIAL_ARG.set(arg.trim().to_string());
            }
        }

        // Start listening on named pipe in background
        start_pipe_server(&pipe_name);
        false
    }

    fn start_pipe_server(pipe_name: &str) {
        let name = pipe_name.to_string();
        let tx = get_sender().clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[QDM SingleInstance] Failed to build runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let mut server = match ServerOptions::new().first_pipe_instance(true).create(&name)
                {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "[QDM SingleInstance] Failed to create first pipe instance: {}",
                            e
                        );
                        return;
                    }
                };

                loop {
                    if server.connect().await.is_ok() {
                        let connected_server = server;
                        server = match ServerOptions::new().create(&name) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!(
                                    "[QDM SingleInstance] Failed to create next pipe instance: {}",
                                    e
                                );
                                break;
                            }
                        };

                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            let mut reader = TokioBufReader::new(connected_server);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok() {
                                let cmd = parse_command(&line);
                                let _ = tx_clone.send(cmd);
                                let mut inner = reader.into_inner();
                                let _ = inner.write_all(b"OK\n").await;
                                let _ = inner.flush().await;
                            }
                        });
                    }
                }
            });
        });
    }
}

// ===========================================================================
// Unix Domain Socket Implementation (Linux / macOS)
// ===========================================================================
#[cfg(unix)]
mod platform {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
    use tokio::net::UnixListener;

    fn get_socket_path() -> PathBuf {
        crate::core::utils::paths::get_qdm_dir().join("qdm.sock")
    }

    pub fn notify_existing_or_acquire(cli_arg: Option<String>) -> bool {
        let socket_path = get_socket_path();

        if let Ok(mut stream) = UnixStream::connect(&socket_path) {
            let msg = match cli_arg {
                Some(arg) if !arg.trim().is_empty() => format!("OPEN:{}\n", arg.trim()),
                _ => "FOCUS\n".to_string(),
            };

            let _ = stream.write_all(msg.as_bytes());
            let _ = stream.flush();

            let mut reader = BufReader::new(stream);
            let mut ack = String::new();
            let _ = reader.read_line(&mut ack);

            return true;
        }

        // Clean up stale socket file if any
        let _ = std::fs::remove_file(&socket_path);

        if let Some(arg) = cli_arg {
            if !arg.trim().is_empty() {
                let _ = INITIAL_ARG.set(arg.trim().to_string());
            }
        }

        start_unix_server(socket_path);
        false
    }

    fn start_unix_server(socket_path: PathBuf) {
        let tx = get_sender().clone();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[QDM SingleInstance] Failed to build runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let listener = match UnixListener::bind(&socket_path) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[QDM SingleInstance] Failed to bind Unix socket: {}", e);
                        return;
                    }
                };

                loop {
                    if let Ok((stream, _)) = listener.accept().await {
                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            let mut reader = TokioBufReader::new(stream);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok() {
                                let cmd = parse_command(&line);
                                let _ = tx_clone.send(cmd);
                                let mut inner = reader.into_inner();
                                let _ = inner.write_all(b"OK\n").await;
                                let _ = inner.flush().await;
                            }
                        });
                    }
                }
            });
        });
    }
}

pub use platform::notify_existing_or_acquire;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_focus() {
        assert_eq!(parse_command("FOCUS\n"), SingleInstanceCommand::Focus);
        assert_eq!(parse_command("  FOCUS  "), SingleInstanceCommand::Focus);
        assert_eq!(parse_command(""), SingleInstanceCommand::Focus);
    }

    #[test]
    fn test_parse_command_open() {
        assert_eq!(
            parse_command("OPEN:magnet:?xt=urn:btih:abc\n"),
            SingleInstanceCommand::Open("magnet:?xt=urn:btih:abc".to_string())
        );
        assert_eq!(
            parse_command("OPEN:https://example.com/file.zip"),
            SingleInstanceCommand::Open("https://example.com/file.zip".to_string())
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_named_pipe_ipc_communication() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

        let pipe_name = r"\\.\pipe\qdm-test-single-instance-pipe";
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(pipe_name)
            .unwrap();

        let (tx, mut rx) = broadcast::channel(4);

        tokio::spawn(async move {
            let server = server;
            if server.connect().await.is_ok() {
                let mut reader = TokioBufReader::new(server);
                let mut line = String::new();
                if reader.read_line(&mut line).await.is_ok() {
                    let cmd = parse_command(&line);
                    let _ = tx.send(cmd);
                }
            }
        });

        // Client connects and sends OPEN command
        let mut client = ClientOptions::new().open(pipe_name).unwrap();
        client
            .write_all(b"OPEN:https://example.com/test.zip\n")
            .await
            .unwrap();
        client.flush().await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(
            received,
            SingleInstanceCommand::Open("https://example.com/test.zip".to_string())
        );
    }
}
