use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use uuid::Uuid;
use log::info;

use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("Connecting to PTY daemon for interactive test...");
    
    // Connect to daemon
    let stream = UnixStream::connect(SOCKET_PATH).await?;
    info!("Connected to daemon");
    
    let mut reader = BufReader::new(stream);
    let client_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    
    // Helper function to send message and read response
    async fn send_and_receive(reader: &mut BufReader<UnixStream>, message: ClientMessage) -> Result<Option<DaemonMessage>, Box<dyn std::error::Error>> {
        let json = serde_json::to_string(&message)?;
        reader.get_mut().write_all(json.as_bytes()).await?;
        reader.get_mut().write_all(b"\n").await?;
        
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;
        
        if !response_line.trim().is_empty() {
            match serde_json::from_str::<DaemonMessage>(response_line.trim()) {
                Ok(response) => {
                    info!("Response: {:?}", response);
                    Ok(Some(response))
                }
                Err(e) => {
                    info!("Failed to parse response: {}, raw: {}", e, response_line.trim());
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
    
    // Test: Register client
    info!("Registering client...");
    let register_msg = ClientMessage::RegisterClient { client_id };
    send_and_receive(&mut reader, register_msg).await?;
    
    // Test: Create session
    info!("Creating session...");
    let create_msg = ClientMessage::CreateSession {
        session_id,
        shell: "/bin/bash".to_string(),
        working_directory: Some("/tmp".to_string()),
    };
    send_and_receive(&mut reader, create_msg).await?;
    
    // Wait a bit for shell to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Test: Send a simple command
    info!("Sending 'echo Hello PTY!' command...");
    let input_msg = ClientMessage::SendInput {
        session_id,
        data: b"echo 'Hello PTY!'\n".to_vec(),
    };
    send_and_receive(&mut reader, input_msg).await?;
    
    // Test: Read output multiple times
    for i in 0..5 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        info!("Reading output attempt {}...", i + 1);
        let output_msg = ClientMessage::ReadOutput { session_id };
        if let Some(DaemonMessage::SessionOutput { data, .. }) = send_and_receive(&mut reader, output_msg).await? {
            let output_str = String::from_utf8_lossy(&data);
            info!("Received output: {:?}", output_str);
        }
    }
    
    // Test: Send another command
    info!("Sending 'pwd' command...");
    let pwd_msg = ClientMessage::SendInput {
        session_id,
        data: b"pwd\n".to_vec(),
    };
    send_and_receive(&mut reader, pwd_msg).await?;
    
    // Read output again
    for i in 0..3 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        info!("Reading pwd output attempt {}...", i + 1);
        let output_msg = ClientMessage::ReadOutput { session_id };
        if let Some(DaemonMessage::SessionOutput { data, .. }) = send_and_receive(&mut reader, output_msg).await? {
            let output_str = String::from_utf8_lossy(&data);
            info!("Received pwd output: {:?}", output_str);
        }
    }
    
    // Test: List sessions
    info!("Listing sessions...");
    let list_msg = ClientMessage::ListSessions;
    send_and_receive(&mut reader, list_msg).await?;
    
    // Test: Disconnect
    info!("Disconnecting...");
    let disconnect_msg = ClientMessage::Disconnect { client_id };
    send_and_receive(&mut reader, disconnect_msg).await?;
    
    info!("Interactive test completed");
    Ok(())
}
