use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use log::{info, error, debug, warn};

use super::messages::{ClientMessage, DaemonMessage, ProtocolMessage, JsonMessage, TerminalData};
use super::SOCKET_PATH;

/// Client for communicating with the PTY daemon
pub struct DaemonClient {
    client_id: Uuid,
    write_stream: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    response_sender: mpsc::UnboundedSender<DaemonMessage>,
    response_receiver: mpsc::UnboundedReceiver<DaemonMessage>,
}

impl DaemonClient {
    /// Create a new daemon client and connect to the daemon
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client_id = Uuid::new_v4();
        let stream = UnixStream::connect(SOCKET_PATH).await?;
        info!("Connected to PTY daemon with client ID: {:?}", client_id);
        
        // Split stream into read and write halves
        let (read_stream, write_stream) = stream.into_split();
        let write_stream_mutex = Arc::new(Mutex::new(write_stream));
        
        let (response_sender, response_receiver) = mpsc::unbounded_channel();
        
        let client = Self {
            client_id,
            write_stream: write_stream_mutex.clone(),
            response_sender: response_sender.clone(),
            response_receiver,
        };
        
        // Start the response reading task
        let sender_for_reader = response_sender.clone();
        tokio::spawn(async move {
            Self::read_responses(read_stream, sender_for_reader).await;
        });
        
        // Register with daemon
        // client.register().await?; // TODO: Fix register method, skip for now
        
        Ok(client)
    }
    
    /// Register this client with the daemon
    async fn register(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let register_msg = ClientMessage::RegisterClient {
            client_id: self.client_id,
        };
        
        self.send_json_message(register_msg).await?;
        
        // Wait for registration confirmation
        if let Some(DaemonMessage::ClientRegistered { .. }) = self.response_receiver.recv().await {
            info!("Successfully registered with daemon");
            Ok(())
        } else {
            Err("Failed to register with daemon".into())
        }
    }
    
    /// Send a JSON message to the daemon
    async fn send_json_message(&self, message: ClientMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Sending JSON message: {:?}", message);
        
        let protocol_msg = ProtocolMessage::Json(JsonMessage::Client(message));
        let data = protocol_msg.to_bytes()?;
        info!("Message serialized, size: {} bytes", data.len());
        
        {
            info!("Write stream available, sending data...");
            let mut write_stream = self.write_stream.lock().await;
            write_stream.write_all(&data).await?;
            info!("Data sent successfully");
        }
        
        info!("JSON message sent successfully");
        Ok(())
    }
    
    /// Send raw terminal data to the daemon (optimized - no session_id needed)
    async fn send_terminal_data(&self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let terminal_data = TerminalData::new(data);
        let protocol_bytes = terminal_data.to_protocol_bytes();
        
        {
            let mut write_stream = self.write_stream.lock().await;
            write_stream.write_all(&protocol_bytes).await?;
        }
        
        Ok(())
    }
    
    /// Create a new PTY session through the daemon
    pub async fn create_session(&mut self, shell: String, working_directory: Option<String>) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let session_id = Uuid::new_v4();
        let create_msg = ClientMessage::RegisterAndCreateSession {
            session_id,
            shell,
            working_directory,
        };
        
        self.send_json_message(create_msg).await?;
        
        // TODO: Fix response handling - for now assume success
        info!("Successfully sent session creation request: {:?}", session_id);
        Ok(session_id)
        
        // // Wait for session creation confirmation
        // if let Some(DaemonMessage::SessionCreated { session_id: created_id }) = self.response_receiver.recv().await {
        //     if created_id == session_id {
        //         info!("Successfully created session: {:?}", session_id);
        //         Ok(session_id)
        //     } else {
        //         Err("Session ID mismatch".into())
        //     }
        // } else {
        //     Err("Failed to create session".into())
        // }
    }
    
    /// Attach to an existing session
    pub async fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let attach_msg = ClientMessage::AttachToSession {
            session_id,
            client_id: self.client_id,
        };
        
        self.send_json_message(attach_msg).await?;
        Ok(())
    }
    
    /// Detach from a session
    pub async fn detach_from_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let detach_msg = ClientMessage::DetachFromSession {
            session_id,
            client_id: self.client_id,
        };
        
        self.send_json_message(detach_msg).await?;
        Ok(())
    }
    
    /// Send input to a session
    pub async fn send_input(&mut self, _session_id: Uuid, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Use new raw bytes protocol for terminal input (session_id not needed in protocol)
        self.send_terminal_data(data).await
    }
    
    /// Read output from a session
    pub async fn read_output(&mut self, session_id: Uuid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let output_msg = ClientMessage::ReadOutput { session_id };
        
        self.send_json_message(output_msg).await?;
        
        // Wait for output response
        if let Some(DaemonMessage::SessionOutput { data, .. }) = self.response_receiver.recv().await {
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }
    
    /// List all active sessions
    pub async fn list_sessions(&mut self) -> Result<Vec<super::SessionInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let list_msg = ClientMessage::ListSessions;
        
        self.send_json_message(list_msg).await?;
        
        // Wait for session list response
        if let Some(DaemonMessage::SessionList { sessions }) = self.response_receiver.recv().await {
            Ok(sessions)
        } else {
            Err("Failed to get session list".into())
        }
    }
    
    /// Resize a session
    pub async fn resize_session(&mut self, session_id: Uuid, cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resize_msg = ClientMessage::ResizeSession { session_id, cols, rows };
        
        self.send_json_message(resize_msg).await?;
        Ok(())
    }
    
    /// Terminate a session
    pub async fn terminate_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let terminate_msg = ClientMessage::TerminateSession { session_id };
        
        self.send_json_message(terminate_msg).await?;
        Ok(())
    }
    
    /// Background task to read responses from the daemon
    async fn read_responses(
        mut read_stream: tokio::net::unix::OwnedReadHalf,
        sender: mpsc::UnboundedSender<DaemonMessage>,
    ) {
        loop {
            let protocol_msg = ProtocolMessage::read_from_stream(&mut read_stream).await;
            
            match protocol_msg {
                Ok(ProtocolMessage::Json(JsonMessage::Daemon(daemon_msg))) => {
                    debug!("Received JSON message from daemon: {:?}", daemon_msg);
                    if sender.send(daemon_msg).is_err() {
                        debug!("Response receiver closed");
                        break;
                    }
                }
                Ok(ProtocolMessage::Json(JsonMessage::Client(_))) => {
                    warn!("Received unexpected client message from daemon");
                }
                Ok(ProtocolMessage::Bytes(bytes)) => {
                    // For bytes messages, we might need to handle terminal output differently
                    debug!("Received {} bytes from daemon", bytes.len());
                    // TODO: Handle terminal output bytes if needed
                }
                Err(e) => {
                    error!("Error reading from daemon: {}", e);
                    break;
                }
            }
        }
        
        debug!("Daemon response reader task ended");
    }
    
    /// Get client ID
    pub fn client_id(&self) -> Uuid {
        self.client_id
    }
    
    /// Disconnect from daemon
    pub async fn disconnect(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let disconnect_msg = ClientMessage::Disconnect {
            client_id: self.client_id,
        };
        
        self.send_json_message(disconnect_msg).await?;
        
        // Write stream will be closed when dropped
        
        info!("Disconnected from daemon");
        Ok(())
    }
}

impl Drop for DaemonClient {
    fn drop(&mut self) {
        debug!("DaemonClient dropped for client: {:?}", self.client_id);
    }
}