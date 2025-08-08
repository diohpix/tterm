use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Message protocol type identifiers
pub const MSG_TYPE_JSON: u8 = 0;
pub const MSG_TYPE_BYTES: u8 = 1;

/// Protocol message wrapper
#[derive(Debug, Clone)]
pub enum ProtocolMessage {
    /// JSON message (type 0)
    Json(JsonMessage),
    /// Raw bytes (type 1)  
    Bytes(Vec<u8>),
}

/// JSON message types for session management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JsonMessage {
    Client(ClientMessage),
    Daemon(DaemonMessage),
}

/// Messages sent from client to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Register a new client connection
    RegisterClient {
        client_id: Uuid,
    },
    /// Create a new terminal session
    RegisterAndCreateSession {
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
    /// Send input to session (moved to raw bytes protocol)
    SendInput {
        session_id: Uuid,
        data: Vec<u8>,
    },
    /// Resize terminal session
    ResizeSession {
        session_id: Uuid,
        cols: u16,
        rows: u16,
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

impl ProtocolMessage {
    /// Serialize protocol message to bytes with length prefix
    pub fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let payload = match self {
            ProtocolMessage::Json(json_msg) => {
                let mut payload = vec![MSG_TYPE_JSON];
                let json_str = serde_json::to_string(json_msg)?;
                payload.extend_from_slice(json_str.as_bytes());
                payload
            }
            ProtocolMessage::Bytes(data) => {
                let mut payload = vec![MSG_TYPE_BYTES];
                payload.extend_from_slice(data);
                payload
            }
        };
        
        // Add 4-byte length prefix
        let mut result = Vec::new();
        result.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        result.extend_from_slice(&payload);
        Ok(result)
    }
    
    /// Deserialize protocol message from bytes (without length prefix)
    pub fn from_bytes(data: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if data.is_empty() {
            return Err("Empty data".into());
        }
        
        let msg_type = data[0];
        let payload = &data[1..];
        
        match msg_type {
            MSG_TYPE_JSON => {
                let json_str = std::str::from_utf8(payload)?;
                let json_msg = serde_json::from_str::<JsonMessage>(json_str)?;
                Ok(ProtocolMessage::Json(json_msg))
            }
            MSG_TYPE_BYTES => {
                Ok(ProtocolMessage::Bytes(payload.to_vec()))
            }
            _ => Err(format!("Unknown message type: {}", msg_type).into())
        }
    }
    
    /// Read a complete protocol message from AsyncRead with length prefix
    pub async fn read_from_stream<R: tokio::io::AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Read 4-byte length prefix
        let mut length_bytes = [0u8; 4];
        reader.read_exact(&mut length_bytes).await?;
        let message_length = u32::from_be_bytes(length_bytes) as usize;
        
        if message_length > 1024 * 1024 { // 1MB limit
            return Err("Message too large".into());
        }
        
        // Read the actual message
        let mut message_bytes = vec![0u8; message_length];
        reader.read_exact(&mut message_bytes).await?;
        
        Self::from_bytes(&message_bytes)
    }
}

/// Terminal input/output message for raw bytes protocol (optimized)
#[derive(Debug, Clone)]
pub struct TerminalData {
    pub data: Vec<u8>,
}

impl TerminalData {
    /// Create new terminal data
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
    
    /// Serialize terminal data to protocol bytes with length prefix (optimized - no UUID)
    pub fn to_protocol_bytes(&self) -> Vec<u8> {
        let payload = {
            let mut payload = vec![MSG_TYPE_BYTES];
            payload.extend_from_slice(&self.data);
            payload
        };
        
        // Add 4-byte length prefix
        let mut result = Vec::new();
        result.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        result.extend_from_slice(&payload);
        result
    }
    
    /// Deserialize terminal data from protocol bytes (optimized)
    pub fn from_protocol_bytes(data: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if data.is_empty() {
            return Err("Empty terminal data".into());
        }
        
        if data[0] != MSG_TYPE_BYTES {
            return Err("Not a bytes message".into());
        }
        
        let payload = data[1..].to_vec();
        
        Ok(TerminalData {
            data: payload,
        })
    }
}
