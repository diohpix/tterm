use std::collections::HashMap;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use log::{info, error, debug, warn};

// Import from main crate
use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};
use full_screen::session::SessionManager;

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
                match self.session_manager.create_session(session_id, shell, working_directory) {
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

    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(SOCKET_PATH);
        
        let listener = UnixListener::bind(SOCKET_PATH)?;
        info!("PTY Daemon listening on {}", SOCKET_PATH);

        // Start cleanup task
        let mut cleanup_interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            tokio::select! {
                // Handle new connections
                result = listener.accept() => {
                    match result {
                        Ok((_stream, _)) => {
                            let client_id = Uuid::new_v4();
                            info!("New client connected: {:?}", client_id);
                            
                            // TODO: Handle client connection in separate task
                            // For now, just log the connection
                            info!("Current sessions: {}", self.session_manager.session_count());
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                
                // Periodic cleanup
                _ = cleanup_interval.tick() => {
                    debug!("Running periodic cleanup...");
                    self.session_manager.cleanup_orphaned_sessions();
                    
                    let session_count = self.session_manager.session_count();
                    if session_count > 0 {
                        info!("Active sessions: {}", session_count);
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Starting TTerminal PTY Daemon");
    
    let mut daemon = PtyDaemon::new();
    daemon.run().await?;
    
    Ok(())
}
