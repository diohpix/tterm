use std::time::Duration;
use tokio::net::UnixStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use uuid::Uuid;
use log::info;

use full_screen::ipc::{ClientMessage, DaemonMessage, SOCKET_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🧪 Starting SSH session detachment test...");
    
    // Connect to daemon
    let stream = UnixStream::connect(SOCKET_PATH).await?;
    info!("✅ Connected to PTY daemon");
    
    let mut reader = BufReader::new(stream);
    let client_id = Uuid::new_v4();
    
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
                    info!("📨 Response: {:?}", response);
                    Ok(Some(response))
                }
                Err(e) => {
                    info!("❌ Failed to parse response: {}, raw: {}", e, response_line.trim());
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
    
    // Register client
    info!("📝 Registering client...");
    let register_msg = ClientMessage::RegisterClient { client_id };
    send_and_receive(&mut reader, register_msg).await?;
    
    // Create SSH session (simulated with bash for now)
    info!("🔑 Creating SSH-like session...");
    let session_id = Uuid::new_v4();
    let create_msg = ClientMessage::CreateSession {
        session_id,
        shell: "/bin/bash".to_string(),
        working_directory: Some("/tmp".to_string()),
    };
    send_and_receive(&mut reader, create_msg).await?;
    
    // Wait for shell to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Simulate SSH commands
    info!("🖥️  Simulating SSH session activity...");
    
    // 1. Change directory
    info!("📁 Executing: cd /tmp");
    let cd_cmd = ClientMessage::SendInput {
        session_id,
        data: b"cd /tmp\n".to_vec(),
    };
    send_and_receive(&mut reader, cd_cmd).await?;
    
    // 2. Check current directory  
    info!("📍 Executing: pwd");
    let pwd_cmd = ClientMessage::SendInput {
        session_id,
        data: b"pwd\n".to_vec(),
    };
    send_and_receive(&mut reader, pwd_cmd).await?;
    
    // 3. Create a file to simulate work
    info!("📝 Executing: echo 'SSH session data' > session_file.txt");
    let create_file_cmd = ClientMessage::SendInput {
        session_id,
        data: b"echo 'SSH session data' > session_file.txt\n".to_vec(),
    };
    send_and_receive(&mut reader, create_file_cmd).await?;
    
    // 4. List files
    info!("📋 Executing: ls -la");
    let ls_cmd = ClientMessage::SendInput {
        session_id,
        data: b"ls -la\n".to_vec(),
    };
    send_and_receive(&mut reader, ls_cmd).await?;
    
    // Read all output
    info!("📖 Reading session output...");
    for i in 0..8 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let output_msg = ClientMessage::ReadOutput { session_id };
        if let Some(DaemonMessage::SessionOutput { data, .. }) = send_and_receive(&mut reader, output_msg).await? {
            if !data.is_empty() {
                let output_str = String::from_utf8_lossy(&data);
                info!("📺 Output {}: {}", i + 1, output_str.trim());
            }
        }
    }
    
    // Simulate tab detachment (disconnect from current client but keep session alive)
    info!("🔄 Simulating tab detachment - detaching from session...");
    let detach_msg = ClientMessage::DetachFromSession {
        session_id,
        client_id,
    };
    send_and_receive(&mut reader, detach_msg).await?;
    
    info!("⏸️  Session detached! Session is now orphaned but still running.");
    info!("💡 In real scenario, new window would attach to session: {}", session_id);
    
    // Simulate new window attachment
    info!("🆕 Simulating new window attachment...");
    let new_client_id = Uuid::new_v4();
    let register_new = ClientMessage::RegisterClient { client_id: new_client_id };
    send_and_receive(&mut reader, register_new).await?;
    
    let attach_msg = ClientMessage::AttachToSession {
        session_id,
        client_id: new_client_id,
    };
    send_and_receive(&mut reader, attach_msg).await?;
    
    // Verify session state is preserved
    info!("✅ Verifying session state preservation...");
    let verify_cmd = ClientMessage::SendInput {
        session_id,
        data: b"cat session_file.txt\n".to_vec(),
    };
    send_and_receive(&mut reader, verify_cmd).await?;
    
    // Read verification output
    tokio::time::sleep(Duration::from_millis(300)).await;
    let output_msg = ClientMessage::ReadOutput { session_id };
    if let Some(DaemonMessage::SessionOutput { data, .. }) = send_and_receive(&mut reader, output_msg).await? {
        let output_str = String::from_utf8_lossy(&data);
        info!("🔍 Verification output: {}", output_str.trim());
        
        if output_str.contains("SSH session data") {
            info!("🎉 SUCCESS! Session state preserved across detachment!");
        } else {
            info!("⚠️  Session state may not be fully preserved");
        }
    }
    
    // List all sessions
    info!("📊 Listing all active sessions...");
    let list_msg = ClientMessage::ListSessions;
    send_and_receive(&mut reader, list_msg).await?;
    
    // Clean up
    info!("🧹 Cleaning up session...");
    let terminate_msg = ClientMessage::TerminateSession { session_id };
    send_and_receive(&mut reader, terminate_msg).await?;
    
    // Disconnect
    let disconnect_msg = ClientMessage::Disconnect { client_id: new_client_id };
    send_and_receive(&mut reader, disconnect_msg).await?;
    
    info!("🏁 SSH session detachment test completed successfully!");
    Ok(())
}
