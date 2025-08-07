use egui_term::TerminalBackend;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use crate::ime::korean::KoreanInputState;
use crate::ipc::DaemonClient;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ViewMode {
    Single,
    Grid { 
        rows: usize, 
        cols: usize,
        // Custom cell sizes: col_ratios[i] = width ratio for column i
        // row_ratios[i] = height ratio for row i
        col_ratios: Vec<f32>,
        row_ratios: Vec<f32>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum PanelContent {
    Terminal(u64), // terminal_id
    Split {
        direction: SplitDirection,
        first: Box<PanelContent>,
        second: Box<PanelContent>,
        ratio: f32, // 0.0 to 1.0
    },
}

#[derive(Debug, Clone)]
pub struct TerminalTab {
    pub id: u64,
    pub title: String,
}

/// Application state containing all terminal-related data
pub struct AppState {
    pub tabs: HashMap<u64, TerminalTab>,
    pub tab_order: Vec<u64>, // Maintain tab order
    pub active_tab_id: u64,
    pub next_tab_id: u64,
    pub terminals: HashMap<u64, TerminalBackend>, // All terminal backends
    pub next_terminal_id: u64,
    pub tab_layouts: HashMap<u64, PanelContent>, // Layout for each tab
    pub view_mode: ViewMode,
    pub focused_terminal: Option<u64>,
    
    // Broadcasting
    pub broadcast_mode: bool,
    pub selected_terminals: HashSet<u64>, // Terminals to broadcast to
    
    // Korean IME support
    pub korean_input_states: HashMap<u64, KoreanInputState>, // Per-terminal Korean input state
    
    // Communication
    pub pty_proxy_receiver: Receiver<(u64, egui_term::PtyEvent)>,
    pub pty_proxy_sender: Sender<(u64, egui_term::PtyEvent)>,
    pub egui_ctx: egui::Context,
    
    // PTY Daemon integration
    pub daemon_client: Option<Arc<TokioMutex<DaemonClient>>>,
    pub daemon_sessions: HashMap<u64, Uuid>, // terminal_id -> session_id mapping
}

impl AppState {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_proxy_sender, pty_proxy_receiver) = std::sync::mpsc::channel();
        let egui_ctx = cc.egui_ctx.clone();
        
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: 0,
            next_tab_id: 1,
            terminals: HashMap::new(),
            next_terminal_id: 1,
            tab_layouts: HashMap::new(),
            view_mode: ViewMode::Single,
            focused_terminal: None,
            broadcast_mode: false,
            selected_terminals: HashSet::new(),
            korean_input_states: HashMap::new(),
            pty_proxy_receiver,
            pty_proxy_sender,
            egui_ctx,
            daemon_client: None,
            daemon_sessions: HashMap::new(),
        }
    }
    
    pub fn create_terminal(&mut self) -> u64 {
        let system_shell = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string());
            
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        
        let terminal_backend = TerminalBackend::new(
            terminal_id,
            self.egui_ctx.clone(),
            self.pty_proxy_sender.clone(),
            egui_term::BackendSettings {
                shell: system_shell,
                ..Default::default()
            },
        )
        .unwrap();

        self.terminals.insert(terminal_id, terminal_backend);
        self.korean_input_states.insert(terminal_id, KoreanInputState::new());
        terminal_id
    }
    
    /// Connect to PTY daemon if not already connected
    pub async fn ensure_daemon_connection(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.daemon_client.is_none() {
            match DaemonClient::new().await {
                Ok(client) => {
                    self.daemon_client = Some(Arc::new(TokioMutex::new(client)));
                    log::info!("Successfully connected to PTY daemon");
                }
                Err(e) => {
                    log::warn!("Failed to connect to PTY daemon: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    
    /// Create a new terminal using the PTY daemon
    pub async fn create_daemon_terminal(&mut self, shell: Option<String>, working_directory: Option<String>) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Ensure daemon connection
        self.ensure_daemon_connection().await?;
        
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        
        if let Some(daemon_client) = &self.daemon_client {
            let mut client = daemon_client.lock().await;
            let shell = shell.unwrap_or_else(|| {
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
            });
            
            let session_id = client.create_session(shell, working_directory).await?;
            self.daemon_sessions.insert(terminal_id, session_id);
            
            log::info!("Created daemon terminal {} with session {:?}", terminal_id, session_id);
            Ok(terminal_id)
        } else {
            Err("Daemon client not available".into())
        }
    }
    
    /// Detach a terminal from current process and transfer to new window
    pub async fn detach_terminal_to_new_window(&mut self, terminal_id: u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(session_id) = self.daemon_sessions.get(&terminal_id) {
            let session_id = *session_id;
            
            // Create new window with the session
            let current_exe = std::env::current_exe()
                .map_err(|e| format!("Failed to get current executable: {}", e))?;
                
            let mut cmd = std::process::Command::new(&current_exe);
            cmd.arg("--attach-session")
               .arg(session_id.to_string());
               
            cmd.spawn()
                .map_err(|e| format!("Failed to spawn new window: {}", e))?;
            
            // Detach from current session
            if let Some(daemon_client) = &self.daemon_client {
                let mut client = daemon_client.lock().await;
                client.detach_from_session(session_id).await?;
            }
            
            // Remove terminal from current process
            self.terminals.remove(&terminal_id);
            self.daemon_sessions.remove(&terminal_id);
            self.korean_input_states.remove(&terminal_id);
            
            // Remove from tabs and layouts
            let tabs_to_check: Vec<u64> = self.tab_layouts.iter()
                .filter_map(|(&tab_id, layout)| {
                    if Self::layout_contains_terminal(layout, terminal_id) {
                        Some(tab_id)
                    } else {
                        None
                    }
                })
                .collect();
                
            for tab_id in tabs_to_check {
                // Remove the terminal from the layout
                if let Some(layout) = self.tab_layouts.get_mut(&tab_id) {
                    Self::remove_terminal_from_layout(layout, terminal_id);
                    
                    // If layout becomes empty, remove the tab
                    if Self::layout_is_empty(layout) {
                        self.remove_tab(tab_id);
                    }
                }
            }
            
            log::info!("Successfully detached terminal {} to new window", terminal_id);
            Ok(())
        } else {
            Err(format!("Terminal {} is not a daemon session", terminal_id).into())
        }
    }
    
    /// Attach to an existing daemon session (used when launching with --attach-session)
    pub async fn attach_to_daemon_session(&mut self, session_id: Uuid) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Ensure daemon connection
        self.ensure_daemon_connection().await?;
        
        if let Some(daemon_client) = &self.daemon_client {
            let mut client = daemon_client.lock().await;
            client.attach_to_session(session_id).await?;
            
            let terminal_id = self.next_terminal_id;
            self.next_terminal_id += 1;
            
            self.daemon_sessions.insert(terminal_id, session_id);
            
            log::info!("Attached to daemon session {:?} as terminal {}", session_id, terminal_id);
            Ok(terminal_id)
        } else {
            Err("Daemon client not available".into())
        }
    }
    
    /// Remove a tab and clean up associated resources
    fn remove_tab(&mut self, tab_id: u64) {
        if let Some(_tab) = self.tabs.remove(&tab_id) {
            // Remove from tab order
            self.tab_order.retain(|&id| id != tab_id);
            
            // Remove layout
            self.tab_layouts.remove(&tab_id);
            
            // Update active tab if necessary
            if self.active_tab_id == tab_id {
                self.active_tab_id = self.tab_order.first().copied().unwrap_or(0);
            }
            
            log::debug!("Removed tab {}", tab_id);
        }
    }
    
    /// Check if a layout contains a specific terminal
    fn layout_contains_terminal(layout: &PanelContent, terminal_id: u64) -> bool {
        match layout {
            PanelContent::Terminal(id) => *id == terminal_id,
            PanelContent::Split { first, second, .. } => {
                Self::layout_contains_terminal(first, terminal_id) ||
                Self::layout_contains_terminal(second, terminal_id)
            }
        }
    }
    
    /// Remove a terminal from a layout
    fn remove_terminal_from_layout(layout: &mut PanelContent, terminal_id: u64) {
        // This is a simplified implementation
        // In a real implementation, we'd need to handle split removal and reorganization
        match layout {
            PanelContent::Terminal(id) if *id == terminal_id => {
                // This case needs special handling at the caller level
            }
            PanelContent::Terminal(_) => {
                // Different terminal, do nothing
            }
            PanelContent::Split { first, second, .. } => {
                Self::remove_terminal_from_layout(first, terminal_id);
                Self::remove_terminal_from_layout(second, terminal_id);
            }
        }
    }
    
    /// Check if a layout is empty (contains no terminals)
    fn layout_is_empty(layout: &PanelContent) -> bool {
        match layout {
            PanelContent::Terminal(_) => false,
            PanelContent::Split { first, second, .. } => {
                Self::layout_is_empty(first) && Self::layout_is_empty(second)
            }
        }
    }
}
