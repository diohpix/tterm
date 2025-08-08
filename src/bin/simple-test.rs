use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::UnixStream;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start daemon first
    println!("Starting daemon in background...");
    let mut daemon_process = tokio::process::Command::new("./target/debug/tterm-daemon")
        .env("RUST_LOG", "debug")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    
    // Wait for daemon to start
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    println!("🔌 Connecting to daemon...");
    let mut stream = UnixStream::connect("/tmp/tterm-daemon.sock").await?;
    println!("✅ Connected");
    
    // Register
    let client_uuid = Uuid::new_v4();
    let register_msg = format!(r#"{{"RegisterClient":{{"client_id":"{}"}}}}"#, client_uuid);
    stream.write_all(register_msg.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    println!("📝 Sent register");
    
    // Read register response
    let mut buf = [0; 1024];
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);
    println!("📥 Register response: {}", response.trim());
    
    // Create session
    let session_id = Uuid::new_v4();
    let create_msg = format!(r#"{{"CreateSession":{{"session_id":"{}","shell":"/bin/zsh","cwd":"/Users/xiphoid/git/tterm"}}}}"#, session_id);
    stream.write_all(create_msg.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    println!("🔧 Sent create session");
    
    // Read create response
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);
    println!("📥 Create response: {}", response.trim());
    
    // Wait for session to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    println!("⏰ Waited 2 seconds");
    
    // Try to read initial output first
    for i in 1..=3 {
        let read_msg = format!(r#"{{"ReadOutput":{{"session_id":"{}"}}}}"#, session_id);
        stream.write_all(read_msg.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        println!("📖 Sent read output #{}", i);
        
        // Try to read with timeout
        match tokio::time::timeout(tokio::time::Duration::from_millis(500), stream.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => {
                let response = String::from_utf8_lossy(&buf[..n]);
                println!("📥 Got response: {}", response.trim());
                
                // Parse response
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.trim()) {
                    if let Some(data_str) = parsed.get("data").and_then(|v| v.as_str()) {
                        use base64::Engine;
                        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(data_str) {
                            let text = String::from_utf8_lossy(&decoded);
                            println!("🎯 DECODED OUTPUT:\n{}", text);
                        }
                    }
                }
            }
            _ => println!("⏰ No response #{}", i),
        }
    }
    
    // Send ls -l command
    let input_msg = format!(r#"{{"SendInput":{{"session_id":"{}","data":"ls -l\n"}}}}"#, session_id);
    stream.write_all(input_msg.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    println!("⌨️  Sent 'ls -l'");
    
    // Wait for command to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // Read command output
    for i in 1..=5 {
        let read_msg = format!(r#"{{"ReadOutput":{{"session_id":"{}"}}}}"#, session_id);
        stream.write_all(read_msg.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        println!("📖 Reading ls output #{}", i);
        
        match tokio::time::timeout(tokio::time::Duration::from_millis(500), stream.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => {
                let response = String::from_utf8_lossy(&buf[..n]);
                println!("📥 Got response: {}", response.trim());
                
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.trim()) {
                    if let Some(data_str) = parsed.get("data").and_then(|v| v.as_str()) {
                        use base64::Engine;
                        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(data_str) {
                            let text = String::from_utf8_lossy(&decoded);
                            if !text.trim().is_empty() {
                                println!("🎯 LS OUTPUT:\n{}", text);
                                break; // Found output, stop looking
                            }
                        }
                    }
                }
            }
            _ => println!("⏰ No response #{}", i),
        }
    }
    
    // Clean up
    let _ = daemon_process.kill().await;
    println!("✅ Test completed");
    Ok(())
}
