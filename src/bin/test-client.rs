
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use uuid::Uuid;
use log::info;

use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Connecting to PTY daemon...");
    
    // Connect to daemon
    let stream = UnixStream::connect(SOCKET_PATH).await?;
    info!("Connected to daemon");
    
    let mut reader = BufReader::new(stream);
    let client_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Helper function to send message and read response
    async fn send_and_receive(reader: &mut BufReader<UnixStream>, message: ClientMessage) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&message)?;
        reader.get_mut().write_all(json.as_bytes()).await?;
        reader.get_mut().write_all(b"\n").await?;
        
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;
        
        if !response_line.trim().is_empty() {
            match serde_json::from_str::<DaemonMessage>(response_line.trim()) {
                Ok(response) => info!("Response: {:?}", response),
                Err(e) => info!("Failed to parse response: {}, raw: {}", e, response_line.trim()),
            }
        }
        
        Ok(())
    }
    
    // Test: Register client
    info!("Registering client...");
    let register_msg = ClientMessage::RegisterClient { client_id };
    send_and_receive(&mut reader, register_msg).await?;
    
    // Test: Create session
    info!("Creating session...");
    let create_msg = ClientMessage::RegisterAndCreateSession {
        session_id,
        shell: "/bin/bash".to_string(),
        working_directory: Some("/tmp".to_string()),
    };
    send_and_receive(&mut reader, create_msg).await?;
    
    // Test: List sessions
    info!("Listing sessions...");
    let list_msg = ClientMessage::ListSessions;
    send_and_receive(&mut reader, list_msg).await?;
    
    // Test: Disconnect
    info!("Disconnecting...");
    let disconnect_msg = ClientMessage::Disconnect { client_id };
    send_and_receive(&mut reader, disconnect_msg).await?;
    
    info!("Test completed");
    Ok(())
}
