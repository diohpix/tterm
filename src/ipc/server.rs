use tokio::net::UnixListener;
use super::messages::{ClientMessage, DaemonMessage};

pub struct IpcServer {
    listener: Option<UnixListener>,
}

impl IpcServer {
    pub fn new() -> Self {
        Self {
            listener: None,
        }
    }

    pub async fn bind(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(super::SOCKET_PATH);
        
        let listener = UnixListener::bind(super::SOCKET_PATH)?;
        self.listener = Some(listener);
        Ok(())
    }

    // TODO: Implement server functionality
}
