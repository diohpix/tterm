use tokio::net::UnixStream;
use uuid::Uuid;
use super::messages::{ClientMessage, DaemonMessage};

pub struct IpcClient {
    client_id: Uuid,
    stream: Option<UnixStream>,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            client_id: Uuid::new_v4(),
            stream: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stream = UnixStream::connect(super::SOCKET_PATH).await?;
        self.stream = Some(stream);
        Ok(())
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id
    }

    // TODO: Implement message sending/receiving
    pub async fn send_message(&mut self, _message: ClientMessage) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Serialize and send message
        Ok(())
    }

    pub async fn receive_message(&mut self) -> Result<DaemonMessage, Box<dyn std::error::Error>> {
        // TODO: Receive and deserialize message
        Err("Not implemented".into())
    }
}
