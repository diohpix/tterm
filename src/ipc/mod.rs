pub mod messages;
pub mod client;
pub mod server;

pub use messages::*;
pub use client::DaemonClient;
pub use server::IpcServer;

/// Unix domain socket path for IPC communication
#[cfg(unix)]
pub const SOCKET_PATH: &str = "/tmp/tterm-daemon.sock";

#[cfg(windows)]
pub const SOCKET_PATH: &str = r"\\.\pipe\tterm-daemon";
