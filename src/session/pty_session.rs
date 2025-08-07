use std::time::SystemTime;
use std::sync::{Arc, Mutex};
use std::io::{Write, Read};
use uuid::Uuid;
use log::{info, error, debug};
use tokio::sync::mpsc;
use portable_pty::{native_pty_system, PtySize, CommandBuilder, Child, MasterPty};

/// Represents a single PTY session managed by the daemon
pub struct PtySession {
    pub id: Uuid,
    pub shell: String,
    pub working_directory: Option<String>,
    pub attached_clients: Vec<Uuid>,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    
    // PTY master and child process
    pty_master: Option<Arc<Mutex<Box<dyn MasterPty + Send>>>>,
    child_process: Option<Box<dyn Child + Send + Sync>>,
    
    // Channel for sending input to the PTY
    input_sender: Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
    
    // Channel for receiving output from the PTY
    output_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>>,
}

impl PtySession {
    pub async fn new(
        id: Uuid,
        shell: String,
        working_directory: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Creating new PTY session: {:?}", id);
        
        let mut session = Self {
            id,
            shell: shell.clone(),
            working_directory: working_directory.clone(),
            attached_clients: Vec::new(),
            created_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            pty_master: None,
            child_process: None,
            input_sender: None,
            output_receiver: None,
        };
        
        // Initialize the actual PTY
        session.initialize_pty().await?;
        
        Ok(session)
    }
    
    async fn initialize_pty(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Initializing PTY for session: {:?}", self.id);
        
        // Get the native PTY system
        let pty_system = native_pty_system();
        
        // Create a PTY pair
        let pty_pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        
        // Build command
        let mut cmd = CommandBuilder::new(&self.shell);
        if let Some(ref cwd) = self.working_directory {
            cmd.cwd(cwd);
        }
        
        // Spawn the child process
        let child = pty_pair.slave.spawn_command(cmd)?;
        debug!("Created PTY process for session: {:?} with PID: {:?}", self.id, child.process_id());
        
        // Store the child process
        self.child_process = Some(child);
        
        // Store the master PTY
        let master_arc = Arc::new(Mutex::new(pty_pair.master));
        self.pty_master = Some(master_arc.clone());
        
        // Set up channels for async I/O
        let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        
        self.input_sender = Some(input_tx);
        self.output_receiver = Some(output_rx);
        
        let session_id = self.id;
        
        // Spawn task to handle input from clients to PTY
        let master_for_input = master_arc.clone();
        tokio::spawn(async move {
            debug!("Starting PTY input task for session: {:?}", session_id);
            while let Some(data) = input_rx.recv().await {
                let result = tokio::task::spawn_blocking({
                    let master = master_for_input.clone();
                    move || {
                        if let Ok(mut pty_master) = master.lock() {
                            let mut writer = pty_master.take_writer().ok()?;
                            writer.write_all(&data).ok()?;
                            writer.flush().ok()
                        } else {
                            None
                        }
                    }
                }).await;
                
                if result.is_err() {
                    error!("Failed to write to PTY for session: {:?}", session_id);
                    break;
                }
            }
            debug!("PTY input task completed for session: {:?}", session_id);
        });
        
        // Spawn task to handle output from PTY to clients
        let master_for_output = master_arc.clone();
        tokio::spawn(async move {
            debug!("Starting PTY output task for session: {:?}", session_id);
            
            loop {
                let result = tokio::task::spawn_blocking({
                    let master = master_for_output.clone();
                    move || {
                        if let Ok(mut pty_master) = master.lock() {
                            let mut reader = pty_master.try_clone_reader().ok()?;
                            let mut buffer = [0u8; 4096];
                            match reader.read(&mut buffer) {
                                Ok(0) => Some(Vec::new()), // EOF
                                Ok(n) => Some(buffer[..n].to_vec()),
                                Err(_) => None,
                            }
                        } else {
                            None
                        }
                    }
                }).await;
                
                match result {
                    Ok(Some(data)) if !data.is_empty() => {
                        if output_tx.send(data).is_err() {
                            debug!("Output channel closed for session: {:?}", session_id);
                            break;
                        }
                    }
                    Ok(Some(_)) => {
                        // Empty data means EOF
                        debug!("PTY closed for session: {:?}", session_id);
                        break;
                    }
                    Ok(None) | Err(_) => {
                        error!("Failed to read from PTY for session: {:?}", session_id);
                        break;
                    }
                }
                
                // Small delay to avoid busy looping
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            debug!("PTY output task completed for session: {:?}", session_id);
        });
        
        Ok(())
    }
    
    pub fn attach_client(&mut self, client_id: Uuid) {
        debug!("Attaching client {:?} to session {:?}", client_id, self.id);
        
        if !self.attached_clients.contains(&client_id) {
            self.attached_clients.push(client_id);
            self.last_activity = SystemTime::now();
        }
    }
    
    pub fn detach_client(&mut self, client_id: Uuid) {
        debug!("Detaching client {:?} from session {:?}", client_id, self.id);
        
        self.attached_clients.retain(|&id| id != client_id);
        self.last_activity = SystemTime::now();
    }
    
    pub fn send_input(&mut self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Sending input to session {:?}: {} bytes", self.id, data.len());
        
        if let Some(sender) = &self.input_sender {
            sender.send(data.to_vec())?;
            self.last_activity = SystemTime::now();
        }
        
        Ok(())
    }
    
    pub fn read_output(&mut self) -> Option<Vec<u8>> {
        if let Some(receiver) = &mut self.output_receiver {
            if let Ok(data) = receiver.try_recv() {
                self.last_activity = SystemTime::now();
                return Some(data);
            }
        }
        None
    }
    
    pub fn is_orphaned(&self) -> bool {
        self.attached_clients.is_empty()
    }
    
    pub fn should_cleanup(&self, timeout_seconds: u64) -> bool {
        if !self.is_orphaned() {
            return false;
        }
        
        if let Ok(duration) = SystemTime::now().duration_since(self.last_activity) {
            duration.as_secs() > timeout_seconds
        } else {
            false
        }
    }
}
