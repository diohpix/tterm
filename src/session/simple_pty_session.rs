use log::{debug, error, info};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct SimplePtySession {
    pub id: Uuid,
    pub session_id: Uuid, // Keep for compatibility
    pub shell: String,
    pub working_directory: String,
    pub attached_clients: HashSet<Uuid>,
    pub created_at: u64,
    pub last_activity: u64,
    pty_master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    output_rx: Arc<Mutex<mpsc::UnboundedReceiver<Vec<u8>>>>,
}

impl SimplePtySession {
    pub fn new(session_id: Uuid, shell: &str, working_directory: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Creating simple PTY session: {:?}", session_id);

        // Create PTY system
        let pty_system = native_pty_system();
        let pty_pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let pty_master = pty_pair.master;
        let pty_slave = pty_pair.slave;

        // Build command
        let mut cmd = CommandBuilder::new(shell);
        cmd.args(&["-i", "-l"]); // Interactive login shell
        cmd.cwd(working_directory);
        
        // Set environment variables
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLUMNS", "80");
        cmd.env("LINES", "24");
        cmd.env("LC_ALL", "en_US.UTF-8");
        cmd.env("LANG", "en_US.UTF-8");

        // Spawn the command
        let mut child = pty_slave.spawn_command(cmd)?;
        
        // Drop slave to avoid deadlock (critical!)
        drop(pty_slave);
        
        info!("✅ PTY process spawned for session: {:?}", session_id);

        let pty_master = Arc::new(Mutex::new(pty_master));

        // Create channels
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let output_rx = Arc::new(Mutex::new(output_rx));

        // Input handling task (stdin)
        let pty_for_input = pty_master.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                while let Some(data) = input_rx.blocking_recv() {
                    if let Ok(pty) = pty_for_input.lock() {
                        if let Ok(mut writer) = pty.take_writer() {
                            if writer.write_all(&data).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }).await;
            
            if let Err(e) = result {
                debug!("Input task error for session: {:?} - {}", session_id, e);
            }
        });

        // Output handling task (stdout) - Fixed non-blocking version
        let pty_for_output = pty_master.clone();
        tokio::spawn(async move {
            debug!("🚀 Starting output task for session: {:?}", session_id);
            
            let mut buf = [0u8; 1024];
            let mut continue_reading = true;
            
            while continue_reading {
                if let Ok(pty) = pty_for_output.lock() {
                    if let Ok(mut reader) = pty.try_clone_reader() {
                        // Use a shorter timeout to prevent infinite blocking
                        drop(pty); // Release lock before potentially blocking operation
                        
                        match reader.read(&mut buf) {
                            Ok(n) if n > 0 => {
                                debug!("📖 PTY read {} bytes for session: {:?}", n, session_id);
                                let data = buf[..n].to_vec();
                                
                                match output_tx.send(data.clone()) {
                                    Ok(()) => {
                                        debug!("✅ PTY data sent to channel for session: {:?} - first 50 chars: {:?}", session_id, String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 50)]));
                                    }
                                    Err(e) => {
                                        debug!("❌ Output channel send failed for session: {:?} - {:?}", session_id, e);
                                        continue_reading = false;
                                    }
                                }
                            }
                            Ok(0) => {
                                debug!("PTY EOF for session: {:?}", session_id);
                                continue_reading = false;
                            }
                            Ok(_) => {
                                // Handle other sizes - should not happen but continue anyway
                                debug!("PTY read unexpected size for session: {:?}", session_id);
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // Non-blocking read with no data available - this is normal
                                debug!("PTY read would block for session: {:?} (normal)", session_id);
                            }
                            Err(e) => {
                                debug!("PTY read error for session: {:?} - {}", session_id, e);
                                continue_reading = false;
                            }
                        }
                    } else {
                        error!("Failed to clone PTY reader for session: {:?}", session_id);
                        continue_reading = false;
                    }
                } else {
                    error!("Failed to lock PTY master for session: {:?}", session_id);
                    continue_reading = false;
                }
                
                // Small delay to prevent CPU spinning
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            
            debug!("🛑 Output task ended for session: {:?}", session_id);
        });

        // Process monitoring task
        tokio::spawn(async move {
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        info!("PTY process exited for session: {:?}, status: {:?}", session_id, status);
                        break;
                    }
                    Ok(None) => {
                        // Still running
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        error!("Error waiting for PTY process: {:?}", e);
                        break;
                    }
                }
            }
        });

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(SimplePtySession {
            id: session_id,
            session_id,
            shell: shell.to_string(),
            working_directory: working_directory.to_string(),
            attached_clients: HashSet::new(),
            created_at: now,
            last_activity: now,
            pty_master,
            input_tx,
            output_rx,
        })
    }



    pub fn read_output(&self) -> Option<Vec<u8>> {
        debug!("🔍 SimplePtySession::read_output called for session: {:?}", self.session_id);
        if let Ok(mut receiver) = self.output_rx.lock() {
            match receiver.try_recv() {
                Ok(data) => {
                    debug!("✅ SimplePtySession read {} bytes from channel for session: {:?}", data.len(), self.session_id);
                    Some(data)
                }
                Err(e) => {
                    debug!("⚠️ SimplePtySession channel error for session: {:?} - {:?}", self.session_id, e);
                    None
                }
            }
        } else {
            debug!("❌ SimplePtySession failed to lock output receiver for session: {:?}", self.session_id);
            None
        }
    }

    pub fn attach_client(&mut self, client_id: Uuid) {
        self.attached_clients.insert(client_id);
        self.update_last_activity();
    }

    pub fn detach_client(&mut self, client_id: Uuid) {
        self.attached_clients.remove(&client_id);
        self.update_last_activity();
    }

    pub fn should_cleanup(&self, orphan_timeout_seconds: u64) -> bool {
        if !self.attached_clients.is_empty() {
            return false; // Has attached clients
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        (now - self.last_activity) > orphan_timeout_seconds
    }

    fn update_last_activity(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn send_input(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("📝 simple pty Sending {} bytes to PTY session: {:?}", data.len(), self.session_id);
        self.update_last_activity();
        self.input_tx.send(data.to_vec())?;
        Ok(())
    }
}
