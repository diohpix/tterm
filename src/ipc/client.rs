use std::sync::Arc;
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use log::{info, error, debug, warn};

use super::messages::{ClientMessage, DaemonMessage};
use super::SOCKET_PATH;

/// Client for communicating with the PTY daemon
pub struct DaemonClient {
    client_id: Uuid,
    stream: Arc<Mutex<Option<BufReader<UnixStream>>>>,
    response_sender: mpsc::UnboundedSender<DaemonMessage>,
    response_receiver: mpsc::UnboundedReceiver<DaemonMessage>,
}

impl DaemonClient {
    /// Create a new daemon client and connect to the daemon
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client_id = Uuid::new_v4();
        let stream = UnixStream::connect(SOCKET_PATH).await?;
        info!("Connected to PTY daemon with client ID: {:?}", client_id);
        
        let reader = BufReader::new(stream);
        let stream_mutex = Arc::new(Mutex::new(Some(reader)));
        
        let (response_sender, response_receiver) = mpsc::unbounded_channel();
        
        let mut client = Self {
            client_id,
            stream: stream_mutex.clone(),
            response_sender: response_sender.clone(),
            response_receiver,
        };
        
        // Start the response reading task
        let stream_for_reader = stream_mutex.clone();
        let sender_for_reader = response_sender.clone();
        tokio::spawn(async move {
            Self::read_responses(stream_for_reader, sender_for_reader).await;
        });
        
        // Register with daemon
        client.register().await?;
        
        Ok(client)
    }
    
    /// Register this client with the daemon
    async fn register(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let register_msg = ClientMessage::RegisterClient {
            client_id: self.client_id,
        };
        
        self.send_message(register_msg).await?;
        
        // Wait for registration confirmation
        if let Some(DaemonMessage::ClientRegistered { .. }) = self.response_receiver.recv().await {
            info!("Successfully registered with daemon");
            Ok(())
        } else {
            Err("Failed to register with daemon".into())
        }
    }
    
    /// Send a message to the daemon
    async fn send_message(&self, message: ClientMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(&message)?;
        
        if let Some(mut stream_guard) = self.stream.lock().await.take() {
            stream_guard.get_mut().write_all(json.as_bytes()).await?;
            stream_guard.get_mut().write_all(b"\n").await?;
            self.stream.lock().await.replace(stream_guard);
        } else {
            return Err("Stream not available".into());
        }
        
        Ok(())
    }
    
    /// Create a new PTY session through the daemon
    pub async fn create_session(&mut self, shell: String, working_directory: Option<String>) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let session_id = Uuid::new_v4();
        let create_msg = ClientMessage::CreateSession {
            session_id,
            shell,
            working_directory,
        };
        
        self.send_message(create_msg).await?;
        
        // Wait for session creation confirmation
        if let Some(DaemonMessage::SessionCreated { session_id: created_id }) = self.response_receiver.recv().await {
            if created_id == session_id {
                info!("Successfully created session: {:?}", session_id);
                Ok(session_id)
            } else {
                Err("Session ID mismatch".into())
            }
        } else {
            Err("Failed to create session".into())
        }
    }
    
    /// Attach to an existing session
    pub async fn attach_to_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let attach_msg = ClientMessage::AttachToSession {
            session_id,
            client_id: self.client_id,
        };
        
        self.send_message(attach_msg).await?;
        Ok(())
    }
    
    /// Detach from a session
    pub async fn detach_from_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let detach_msg = ClientMessage::DetachFromSession {
            session_id,
            client_id: self.client_id,
        };
        
        self.send_message(detach_msg).await?;
        Ok(())
    }
    
    /// Send input to a session
    pub async fn send_input(&mut self, session_id: Uuid, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let input_msg = ClientMessage::SendInput {
            session_id,
            data,
        };
        
        self.send_message(input_msg).await?;
        Ok(())
    }
    
    /// Read output from a session
    pub async fn read_output(&mut self, session_id: Uuid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let output_msg = ClientMessage::ReadOutput { session_id };
        
        self.send_message(output_msg).await?;
        
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
        
        self.send_message(list_msg).await?;
        
        // Wait for session list response
        if let Some(DaemonMessage::SessionList { sessions }) = self.response_receiver.recv().await {
            Ok(sessions)
        } else {
            Err("Failed to get session list".into())
        }
    }
    
    /// Terminate a session
    pub async fn terminate_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let terminate_msg = ClientMessage::TerminateSession { session_id };
        
        self.send_message(terminate_msg).await?;
        Ok(())
    }
    
    /// Background task to read responses from the daemon
    async fn read_responses(
        stream: Arc<Mutex<Option<BufReader<UnixStream>>>>,
        sender: mpsc::UnboundedSender<DaemonMessage>,
    ) {
        let mut line = String::new();
        
        loop {
            line.clear();
            
            let read_result = {
                if let Some(mut stream_guard) = stream.lock().await.take() {
                    let result = stream_guard.read_line(&mut line).await;
                    stream.lock().await.replace(stream_guard);
                    result
                } else {
                    break;
                }
            };
            
            match read_result {
                Ok(0) => {
                    debug!("Daemon connection closed");
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    
                    match serde_json::from_str::<DaemonMessage>(trimmed) {
                        Ok(message) => {
                            debug!("Received message from daemon: {:?}", message);
                            if sender.send(message).is_err() {
                                debug!("Response receiver closed");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse daemon message: {}, raw: {}", e, trimmed);
                        }
                    }
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
        
        self.send_message(disconnect_msg).await?;
        
        // Close the stream
        self.stream.lock().await.take();
        
        info!("Disconnected from daemon");
        Ok(())
    }
}

impl Drop for DaemonClient {
    fn drop(&mut self) {
        debug!("DaemonClient dropped for client: {:?}", self.client_id);
    }
}