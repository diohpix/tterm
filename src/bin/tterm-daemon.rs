use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::time::{timeout, Duration};
use tokio::sync::Mutex;
use uuid::Uuid;
use log::{info, error, debug, warn};

// Import from main crate
use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};
use full_screen::session::SessionManager;

type SharedDaemon = Arc<Mutex<PtyDaemon>>;

struct PtyDaemon {
    session_manager: SessionManager,
    clients: HashMap<Uuid, ClientConnection>,
}

struct ClientConnection {
    id: Uuid,
    stream: UnixStream,
}

impl PtyDaemon {
    fn new() -> Self {
        Self {
            session_manager: SessionManager::new(),
            clients: HashMap::new(),
        }
    }

    async fn handle_client_message(&mut self, client_id: Uuid, message: ClientMessage) -> Option<DaemonMessage> {
        match message {
            ClientMessage::RegisterClient { client_id } => {
                debug!("Registering client: {:?}", client_id);
                Some(DaemonMessage::ClientRegistered { client_id })
            }
            ClientMessage::CreateSession { session_id, shell, working_directory } => {
                debug!("Creating session: {:?}", session_id);
                match self.session_manager.create_session(session_id, shell, working_directory).await {
                    Ok(()) => {
                        // Automatically attach the creating client to the session
                        if let Err(e) = self.session_manager.attach_client_to_session(session_id, client_id) {
                            error!("Failed to attach client to new session: {}", e);
                        }
                        Some(DaemonMessage::SessionCreated { session_id })
                    }
                    Err(e) => {
                        error!("Failed to create session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to create session: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::AttachToSession { session_id, client_id } => {
                debug!("Attaching client {:?} to session {:?}", client_id, session_id);
                match self.session_manager.attach_client_to_session(session_id, client_id) {
                    Ok(()) => None, // Silent success
                    Err(e) => {
                        error!("Failed to attach to session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to attach to session: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::DetachFromSession { session_id, client_id } => {
                debug!("Detaching client {:?} from session {:?}", client_id, session_id);
                match self.session_manager.detach_client_from_session(session_id, client_id) {
                    Ok(()) => None, // Silent success
                    Err(e) => {
                        error!("Failed to detach from session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to detach from session: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::SendInput { session_id, data } => {
                match self.session_manager.send_input_to_session(session_id, &data) {
                    Ok(()) => None, // Silent success
                    Err(e) => {
                        error!("Failed to send input to session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to send input: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::ReadOutput { session_id } => {
                match self.session_manager.read_output_from_session(session_id) {
                    Ok(Some(data)) => Some(DaemonMessage::SessionOutput { session_id, data }),
                    Ok(None) => None, // No output available
                    Err(e) => {
                        error!("Failed to read output from session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to read output: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::ListSessions => {
                debug!("Listing sessions for client: {:?}", client_id);
                let sessions = self.session_manager.list_sessions();
                Some(DaemonMessage::SessionList { sessions })
            }
            ClientMessage::TerminateSession { session_id } => {
                debug!("Terminating session: {:?}", session_id);
                match self.session_manager.terminate_session(session_id) {
                    Ok(()) => Some(DaemonMessage::SessionTerminated { session_id }),
                    Err(e) => {
                        error!("Failed to terminate session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to terminate session: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::Disconnect { client_id: disconnect_id } => {
                debug!("Client disconnecting: {:?}", disconnect_id);
                self.clients.remove(&disconnect_id);
                
                // Detach the client from all sessions
                for session_info in self.session_manager.list_sessions() {
                    if session_info.attached_clients.contains(&disconnect_id) {
                        if let Err(e) = self.session_manager.detach_client_from_session(session_info.id, disconnect_id) {
                            warn!("Failed to detach disconnecting client from session: {}", e);
                        }
                    }
                }
                None
            }
        }
    }
    


    async fn run(daemon: SharedDaemon) -> Result<(), Box<dyn std::error::Error>> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(SOCKET_PATH);
        
        let listener = UnixListener::bind(SOCKET_PATH)?;
        info!("PTY Daemon listening on {}", SOCKET_PATH);

        // Start cleanup task
        let daemon_for_cleanup = daemon.clone();
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                cleanup_interval.tick().await;
                debug!("Running periodic cleanup...");
                
                let mut daemon_guard = daemon_for_cleanup.lock().await;
                daemon_guard.session_manager.cleanup_orphaned_sessions();
                
                let session_count = daemon_guard.session_manager.session_count();
                if session_count > 0 {
                    info!("Active sessions: {}", session_count);
                }
            }
        });
        
        // Handle incoming connections
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let daemon_clone = daemon.clone();
                    tokio::spawn(async move {
                        handle_client_connection(daemon_clone, stream).await;
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

/// Handle a single client connection
async fn handle_client_connection(daemon: SharedDaemon, stream: UnixStream) {
    let client_id = Uuid::new_v4();
    info!("Handling client connection: {:?}", client_id);
    
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    
    loop {
        line.clear();
        
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Client {:?} disconnected", client_id);
                break;
            }
            Ok(_) => {
                // Parse the message
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<ClientMessage>(trimmed) {
                    Ok(message) => {
                        debug!("Received message from client {:?}: {:?}", client_id, message);
                        
                        let response = {
                            let mut daemon_guard = daemon.lock().await;
                            daemon_guard.handle_client_message(client_id, message).await
                        };
                        
                        if let Some(response) = response {
                            // Send response back to client
                            let response_json = match serde_json::to_string(&response) {
                                Ok(json) => json,
                                Err(e) => {
                                    error!("Failed to serialize response: {}", e);
                                    continue;
                                }
                            };
                            
                            let mut stream = reader.get_mut();
                            if let Err(e) = stream.write_all(response_json.as_bytes()).await {
                                error!("Failed to send response to client: {}", e);
                                break;
                            }
                            if let Err(e) = stream.write_all(b"\n").await {
                                error!("Failed to send newline to client: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse message from client {:?}: {}", client_id, e);
                        // Send error response
                        let error_response = DaemonMessage::Error {
                            message: format!("Invalid message format: {}", e),
                        };
                        let response_json = serde_json::to_string(&error_response).unwrap_or_default();
                        let mut stream = reader.get_mut();
                        let _ = stream.write_all(response_json.as_bytes()).await;
                        let _ = stream.write_all(b"\n").await;
                    }
                }
            }
            Err(e) => {
                error!("Error reading from client {:?}: {}", client_id, e);
                break;
            }
        }
    }
    
    // Clean up when client disconnects
    let disconnect_msg = ClientMessage::Disconnect { client_id };
    let mut daemon_guard = daemon.lock().await;
    daemon_guard.handle_client_message(client_id, disconnect_msg).await;
    info!("Client {:?} connection closed", client_id);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Starting TTerminal PTY Daemon");
    
    let daemon = PtyDaemon::new();
    let shared_daemon = Arc::new(Mutex::new(daemon));
    
    PtyDaemon::run(shared_daemon).await?;
    
    Ok(())
}
