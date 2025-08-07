use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Messages sent from client to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Register a new client connection
    RegisterClient {
        client_id: Uuid,
    },
    /// Create a new terminal session
    CreateSession {
        session_id: Uuid,
        shell: String,
        working_directory: Option<String>,
    },
    /// Attach client to existing session
    AttachToSession {
        session_id: Uuid,
        client_id: Uuid,
    },
    /// Detach client from session
    DetachFromSession {
        session_id: Uuid,
        client_id: Uuid,
    },
    /// Send input to session
    SendInput {
        session_id: Uuid,
        data: Vec<u8>,
    },
    /// Request to read output from session
    ReadOutput {
        session_id: Uuid,
    },
    /// Request session list
    ListSessions,
    /// Terminate session
    TerminateSession {
        session_id: Uuid,
    },
    /// Client disconnecting
    Disconnect {
        client_id: Uuid,
    },
}

/// Messages sent from daemon to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    /// Acknowledge client registration
    ClientRegistered {
        client_id: Uuid,
    },
    /// Session created successfully
    SessionCreated {
        session_id: Uuid,
    },
    /// Session output data
    SessionOutput {
        session_id: Uuid,
        data: Vec<u8>,
    },
    /// Session terminated
    SessionTerminated {
        session_id: Uuid,
    },
    /// List of active sessions
    SessionList {
        sessions: Vec<SessionInfo>,
    },
    /// Error occurred
    Error {
        message: String,
    },
}

/// Information about a terminal session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub shell: String,
    pub working_directory: Option<String>,
    pub attached_clients: Vec<Uuid>,
    pub created_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
}
