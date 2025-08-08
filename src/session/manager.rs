use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
use log::{info, warn, debug};
use tokio::time::{interval, Interval};

use super::pty_session::PtySession;
use crate::ipc::SessionInfo;

/// Manages all PTY sessions for the daemon
pub struct SessionManager {
    sessions: HashMap<Uuid, PtySession>,
    cleanup_interval: Interval,
    orphan_timeout_seconds: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            cleanup_interval: interval(Duration::from_secs(60)), // Cleanup every minute
            orphan_timeout_seconds: 300, // 5 minutes
        }
    }
    
    pub async fn create_session(
        &mut self,
        session_id: Uuid,
        shell: String,
        working_directory: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Creating session: {:?}", session_id);
        
        if self.sessions.contains_key(&session_id) {
            return Err(format!("Session {:?} already exists", session_id).into());
        }
        
        let session = PtySession::new(session_id, shell, working_directory).await?;
        self.sessions.insert(session_id, session);
        
        info!("Session created successfully: {:?}", session_id);
        Ok(())
    }
    
    pub fn terminate_session(&mut self, session_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        info!("Terminating session: {:?}", session_id);
        
        if let Some(_session) = self.sessions.remove(&session_id) {
            info!("Session terminated: {:?}", session_id);
            Ok(())
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn attach_client_to_session(&mut self, session_id: Uuid, client_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Attaching client {:?} to session {:?}", client_id, session_id);
        
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.attach_client(client_id);
            Ok(())
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn detach_client_from_session(&mut self, session_id: Uuid, client_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Detaching client {:?} from session {:?}", client_id, session_id);
        
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.detach_client(client_id);
            Ok(())
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn send_input_to_session(&mut self, session_id: Uuid, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.send_input(data)
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn read_output_from_session(&mut self, session_id: Uuid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            Ok(session.read_output())
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn resize_session(&mut self, session_id: Uuid, cols: u16, rows: u16) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.resize(cols, rows)
        } else {
            Err(format!("Session {:?} not found", session_id).into())
        }
    }
    
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.values().map(|session| {
            SessionInfo {
                id: session.id,
                shell: session.shell.clone(),
                working_directory: session.working_directory.clone(),
                attached_clients: session.attached_clients.clone(),
                created_at: session.created_at,
                last_activity: session.last_activity,
            }
        }).collect()
    }
    
    pub fn cleanup_orphaned_sessions(&mut self) {
        let before_count = self.sessions.len();
        
        self.sessions.retain(|session_id, session| {
            if session.should_cleanup(self.orphan_timeout_seconds) {
                warn!("Cleaning up orphaned session: {:?}", session_id);
                false
            } else {
                true
            }
        });
        
        let after_count = self.sessions.len();
        let cleaned_count = before_count - after_count;
        
        if cleaned_count > 0 {
            info!("Cleaned up {} orphaned sessions", cleaned_count);
        }
    }
    
    pub async fn cleanup_tick(&mut self) {
        self.cleanup_interval.tick().await;
        self.cleanup_orphaned_sessions();
    }
    
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
    
    pub fn has_session(&self, session_id: Uuid) -> bool {
        self.sessions.contains_key(&session_id)
    }
}
