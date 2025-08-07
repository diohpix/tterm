use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use std::time::SystemTime;
use uuid::Uuid;
use log::{info, error, debug};

use egui_term::{TerminalBackend, BackendSettings, BackendCommand};

/// Represents a single PTY session managed by the daemon
pub struct PtySession {
    pub id: Uuid,
    pub shell: String,
    pub working_directory: Option<String>,
    pub attached_clients: Vec<Uuid>,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    
    // The actual terminal backend
    terminal_backend: Option<TerminalBackend>,
    
    // Channel for sending commands to the terminal
    command_sender: Option<mpsc::Sender<BackendCommand>>,
}

impl PtySession {
    pub fn new(
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
            terminal_backend: None,
            command_sender: None,
        };
        
        // Initialize the actual PTY
        session.initialize_pty()?;
        
        Ok(session)
    }
    
    fn initialize_pty(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Initializing PTY for session: {:?}", self.id);
        
        // Create a dummy egui context for the terminal backend
        // This is a limitation - we need to find a better way to handle this
        // For now, we'll create the terminal backend when a client connects
        
        // TODO: Implement proper PTY initialization without egui dependency
        // This might require modifying egui_term or creating our own PTY wrapper
        
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
        
        if let Some(sender) = &self.command_sender {
            sender.send(BackendCommand::Write(data.to_vec()))?;
            self.last_activity = SystemTime::now();
        }
        
        Ok(())
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
