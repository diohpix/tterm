use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::UnixStream;
use uuid::Uuid;

// Protocol constants
const MSG_TYPE_JSON: u8 = 0;
const MSG_TYPE_BYTES: u8 = 1;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Connecting to daemon...");
    
    // Connect to daemon
    let stream = UnixStream::connect("/tmp/tterm-daemon.sock").await?;
    println!("✅ Connected to daemon");
    
    let (mut rstream, mut wstream) = stream.into_split();
    tokio::spawn(async move {
        let mut read_count: usize = 0;
        loop {
            // Read length-prefixed protocol messages
            let mut len_buf = [0u8; 4];
            match rstream.read_exact(&mut len_buf).await {
                Ok(_) => {
                    let message_len = u32::from_be_bytes(len_buf) as usize;
                    println!("📏 Message length: {}", message_len);
                    
                    let mut message_buf = vec![0u8; message_len];
                    match rstream.read_exact(&mut message_buf).await {
                        Ok(_) => {
                            read_count += 1;
                            
                            // Check message type (first byte)
                            if message_buf.is_empty() {
                                continue;
                            }
                            
                            let msg_type = message_buf[0];
                            let payload = &message_buf[1..];
                            
                            match msg_type {
                                MSG_TYPE_JSON => {
                                    let response = String::from_utf8_lossy(payload);
                                    println!("📥 JSON #{}: {}", read_count, response.trim());
                                }
                                MSG_TYPE_BYTES => {
                                    println!("📥 Raw bytes #{}: {} bytes", read_count, payload.len());
                                    let text = String::from_utf8_lossy(payload);
                                    println!("    Content: {:?}", text);
                                }
                                _ => {
                                    println!("❓ Unknown message type: {}", msg_type);
                                }
                            }
                        }
                        Err(e) => {
                            println!("❌ Read message error: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Read length error: {}", e);
                    break;
                }
            }
        }
    });
    
    // Register client and create session in one step
    let client_uuid = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let register_and_create_msg = format!(
        r#"{{"RegisterAndCreateSession":{{"session_id":"{}","shell":"/bin/zsh","working_directory":"/Users/xiphoid/git/tterm"}}}}"#, 
        session_id
    );
    
    // Send JSON message with length-prefixed protocol
    let json_msg = serde_json::json!({
        "Client": {
            "RegisterAndCreateSession": {
                "session_id": session_id,
                "shell": "/bin/zsh",
                "working_directory": "/Users/xiphoid/git/tterm"
            }
        }
    });
    let protocol_msg = vec![MSG_TYPE_JSON];
    let mut payload = protocol_msg;
    payload.extend_from_slice(serde_json::to_string(&json_msg)?.as_bytes());
    
    // Add length prefix
    let length = payload.len() as u32;
    let mut final_msg = length.to_be_bytes().to_vec();
    final_msg.extend_from_slice(&payload);
    
    wstream.write_all(&final_msg).await?;
    println!("📝 Sent register and create session message {} {}",client_uuid, session_id);
    
    // Wait a moment for session to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    println!("⏰ Waited 1 second for session initialization");
    
    // Send ls -l command first
    println!("⌨️  Sending 'ls -l' command...");
    
    // Send terminal data with length-prefixed protocol (type 1 + data, no UUID needed)
    let command_data = "ls -l\n".as_bytes();
    let mut payload = vec![MSG_TYPE_BYTES];
    payload.extend_from_slice(command_data);  // Just the data, no UUID
    
    // Add length prefix
    let length = payload.len() as u32;
    let mut final_msg = length.to_be_bytes().to_vec();
    final_msg.extend_from_slice(&payload);
    
    wstream.write_all(&final_msg).await?;
    println!("✅ Sent 'ls -l' command with optimized protocol header (no UUID)");
    
    // Wait for automatic PTY output push from daemon
    println!("⏳ Waiting for automatic PTY output push from daemon...");
    tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
    
    println!("✅ Test completed - PTY output should have been automatically pushed!");
    Ok(())
}
