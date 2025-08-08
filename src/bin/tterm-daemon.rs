use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;
use log::{info, error, debug, warn};

// Import from main crate
use full_screen::ipc::{ClientMessage, DaemonMessage, ProtocolMessage, JsonMessage, TerminalData, SOCKET_PATH};
use full_screen::session::SessionManager;

type SharedDaemon = Arc<Mutex<PtyDaemon>>;

struct PtyDaemon {
    session_manager: SessionManager,
    clients: HashMap<Uuid, ClientConnection>,
    // Map client_id to current session_id for efficient bytes routing
    client_sessions: HashMap<Uuid, Uuid>,
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
            client_sessions: HashMap::new(),
        }
    }

    async fn handle_client_message(&mut self, client_id: Uuid, message: ClientMessage) -> Option<DaemonMessage> {
        match message {
            ClientMessage::RegisterClient { client_id } => {
                debug!("Registering client: {:?}", client_id);
                Some(DaemonMessage::ClientRegistered { client_id })
            }
            ClientMessage::RegisterAndCreateSession { session_id, shell, working_directory } => {
                debug!("Creating session: {:?}", session_id);
                match self.session_manager.create_session(session_id, shell, working_directory).await {
                    Ok(()) => {
                        // Automatically attach the creating client to the session
                        if let Err(e) = self.session_manager.attach_client_to_session(session_id, client_id) {
                            error!("Failed to attach client to new session: {}", e);
                        } else {
                            // Map client to session for efficient bytes routing
                            self.client_sessions.insert(client_id, session_id);
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
                    Ok(()) => {
                        // Map client to session for efficient bytes routing
                        self.client_sessions.insert(client_id, session_id);
                        None // Silent success
                    }
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
                    Ok(()) => {
                        // Remove client-session mapping
                        self.client_sessions.remove(&client_id);
                        None // Silent success
                    }
                    Err(e) => {
                        error!("Failed to detach from session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to detach from session: {}", e) 
                        })
                    }
                }
            }
            ClientMessage::SendInput { session_id, data } => {
                // Note: SendInput is deprecated in favor of raw bytes protocol
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
            ClientMessage::ResizeSession { session_id, cols, rows } => {
                debug!("Resizing session {:?} to {}x{}", session_id, cols, rows);
                match self.session_manager.resize_session(session_id, cols, rows) {
                    Ok(()) => None, // Silent success
                    Err(e) => {
                        error!("Failed to resize session: {}", e);
                        Some(DaemonMessage::Error { 
                            message: format!("Failed to resize session: {}", e) 
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
                
                // Remove client-session mapping
                self.client_sessions.remove(&disconnect_id);
                
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
    
    let (mut read_stream, write_stream) = stream.into_split();
    let write_stream = Arc::new(Mutex::new(write_stream));
    
    // Spawn task to handle PTY output pushing
    let daemon_for_output = daemon.clone();
    let write_stream_for_output = write_stream.clone();
    let output_push_task = tokio::spawn(async move {
        let mut last_session_id: Option<Uuid> = None;
        
        loop {
            // Check if client has an active session
            let session_id = {
                let daemon_guard = daemon_for_output.lock().await;
                daemon_guard.client_sessions.get(&client_id).cloned()
            };
            
            if let Some(session_id) = session_id {
                if last_session_id != Some(session_id) {
                    debug!("Client {:?} now monitoring session {:?} for output", client_id, session_id);
                    last_session_id = Some(session_id);
                }
                
                // Try to read output from session
                let output_data = {
                    let mut daemon_guard = daemon_for_output.lock().await;
                    daemon_guard.session_manager.read_output_from_session(session_id).ok().flatten()
                };
                
                if let Some(data) = output_data {
                    debug!("Pushing PTY output to client {:?}: {} bytes", client_id, data.len());
                    
                    // Send output to client using raw bytes protocol
                    let protocol_msg = ProtocolMessage::Bytes(data);
                    match protocol_msg.to_bytes() {
                        Ok(response_bytes) => {
                            let mut stream = write_stream_for_output.lock().await;
                            if let Err(e) = stream.write_all(&response_bytes).await {
                                error!("Failed to push output to client {:?}: {}", client_id, e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize output for client {:?}: {}", client_id, e);
                        }
                    }
                } else {
                    // No output available, sleep briefly
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            } else {
                // No active session, sleep longer
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    });
    
    // Handle client messages
    loop {
        match ProtocolMessage::read_from_stream(&mut read_stream).await {
            Ok(ProtocolMessage::Json(JsonMessage::Client(client_msg))) => {
                debug!("Received JSON message from client {:?}: {:?}", client_id, client_msg);
                
                let response = {
                    let mut daemon_guard = daemon.lock().await;
                    daemon_guard.handle_client_message(client_id, client_msg).await
                };
                
                if let Some(response) = response {
                    // Send response back to client using new protocol
                    let protocol_response = ProtocolMessage::Json(JsonMessage::Daemon(response));
                    match protocol_response.to_bytes() {
                        Ok(response_bytes) => {
                            let mut stream = write_stream.lock().await;
                            if let Err(e) = stream.write_all(&response_bytes).await {
                                error!("Failed to send response to client: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize response: {}", e);
                            continue;
                        }
                    }
                }
            }
            Ok(ProtocolMessage::Bytes(raw_data)) => {
                // Handle raw terminal data (optimized - no UUID in protocol)
                let terminal_data = TerminalData::new(raw_data);
                let data_str = String::from_utf8_lossy(&terminal_data.data);
                debug!("Received terminal data from client {:?}: {} bytes, content: {:?}", 
                       client_id, terminal_data.data.len(), data_str);
                
                // Find session for this client
                let mut daemon_guard = daemon.lock().await;
                if let Some(&session_id) = daemon_guard.client_sessions.get(&client_id) {
                    if let Err(e) = daemon_guard.session_manager.send_input_to_session(
                        session_id, 
                        &terminal_data.data
                    ) {
                        error!("Failed to send terminal input to session {:?}: {}", session_id, e);
                    }
                } else {
                    warn!("No session found for client {:?}, ignoring terminal data", client_id);
                }
            }
            Ok(ProtocolMessage::Json(JsonMessage::Daemon(_))) => {
                warn!("Received unexpected daemon message from client {:?}", client_id);
            }
            Err(e) => {
                error!("Error reading from client {:?}: {}", client_id, e);
                break;
            }
        }
    }
    
    // Clean up when client disconnects
    output_push_task.abort(); // Stop the output push task
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
