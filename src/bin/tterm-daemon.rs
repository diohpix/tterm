use std::collections::HashMap;
use std::time::SystemTime;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use log::{info, error, debug};

// Import from main crate
use full_screen::ipc::{ClientMessage, DaemonMessage, SessionInfo, SOCKET_PATH};

struct PtyDaemon {
    sessions: HashMap<Uuid, TerminalSession>,
    clients: HashMap<Uuid, ClientConnection>,
}

struct TerminalSession {
    id: Uuid,
    shell: String,
    working_directory: Option<String>,
    attached_clients: Vec<Uuid>,
    created_at: SystemTime,
    last_activity: SystemTime,
    // TODO: Add actual PTY management
}

struct ClientConnection {
    id: Uuid,
    stream: UnixStream,
}

impl PtyDaemon {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
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
                let session = TerminalSession {
                    id: session_id,
                    shell: shell.clone(),
                    working_directory: working_directory.clone(),
                    attached_clients: vec![client_id],
                    created_at: SystemTime::now(),
                    last_activity: SystemTime::now(),
                };
                self.sessions.insert(session_id, session);
                Some(DaemonMessage::SessionCreated { session_id })
            }
            ClientMessage::ListSessions => {
                debug!("Listing sessions for client: {:?}", client_id);
                let sessions: Vec<SessionInfo> = self.sessions.values().map(|session| {
                    SessionInfo {
                        id: session.id,
                        shell: session.shell.clone(),
                        working_directory: session.working_directory.clone(),
                        attached_clients: session.attached_clients.clone(),
                        created_at: session.created_at,
                        last_activity: session.last_activity,
                    }
                }).collect();
                Some(DaemonMessage::SessionList { sessions })
            }
            ClientMessage::Disconnect { client_id: disconnect_id } => {
                debug!("Client disconnecting: {:?}", disconnect_id);
                self.clients.remove(&disconnect_id);
                None
            }
            _ => {
                debug!("Unhandled message: {:?}", message);
                Some(DaemonMessage::Error { 
                    message: "Not implemented yet".to_string() 
                })
            }
        }
    }

    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(SOCKET_PATH);
        
        let listener = UnixListener::bind(SOCKET_PATH)?;
        info!("PTY Daemon listening on {}", SOCKET_PATH);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let client_id = Uuid::new_v4();
                    info!("New client connected: {:?}", client_id);
                    
                    // TODO: Handle client connection in separate task
                    // For now, just store the connection
                    // self.clients.insert(client_id, ClientConnection { id: client_id, stream });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
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
