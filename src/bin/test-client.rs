use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use uuid::Uuid;
use log::info;

use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Connecting to PTY daemon...");
    
    // Connect to daemon
    let mut stream = UnixStream::connect(SOCKET_PATH).await?;
    info!("Connected to daemon");
    
    let client_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Test: Register client
    let register_msg = ClientMessage::RegisterClient { client_id };
    let json = serde_json::to_string(&register_msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    info!("Sent register message");
    
    // Test: Create session
    let create_msg = ClientMessage::CreateSession {
        session_id,
        shell: "/bin/bash".to_string(),
        working_directory: Some("/tmp".to_string()),
    };
    let json = serde_json::to_string(&create_msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    info!("Sent create session message");
    
    // Test: List sessions
    tokio::time::sleep(Duration::from_millis(100)).await;
    let list_msg = ClientMessage::ListSessions;
    let json = serde_json::to_string(&list_msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    info!("Sent list sessions message");
    
    // Test: Disconnect
    tokio::time::sleep(Duration::from_millis(100)).await;
    let disconnect_msg = ClientMessage::Disconnect { client_id };
    let json = serde_json::to_string(&disconnect_msg)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    info!("Sent disconnect message");
    
    info!("Test completed");
    Ok(())
}
