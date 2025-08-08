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
    pub daemon_terminals: HashSet<u64>, // Track which terminals are daemon-backed
    
    // Async terminal creation
    pub pending_tab_creation: Option<u64>, // tab_id that's waiting for terminal creation
    pub connecting_terminals: std::collections::HashMap<u64, String>, // terminal_id -> connection status
    pub daemon_connection_receiver: Option<std::sync::mpsc::Receiver<(u64, Result<(crate::ipc::DaemonClient, uuid::Uuid), Box<dyn std::error::Error + Send + Sync>>)>>,
    pub daemon_connection_sender: Option<std::sync::mpsc::Sender<(u64, Result<(crate::ipc::DaemonClient, uuid::Uuid), Box<dyn std::error::Error + Send + Sync>>)>>,
}

impl AppState {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_proxy_sender, pty_proxy_receiver) = std::sync::mpsc::channel();
        
        // Create daemon connection channel
        let (daemon_conn_sender, daemon_conn_receiver) = std::sync::mpsc::channel();
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
            daemon_terminals: HashSet::new(),
            pending_tab_creation: None,
            connecting_terminals: HashMap::new(),
            daemon_connection_receiver: Some(daemon_conn_receiver),
            daemon_connection_sender: Some(daemon_conn_sender),
        }
    }
    
    pub fn create_terminal(&mut self) -> u64 {
        // Use daemon if available, fallback to local PTY
        self.create_terminal_with_daemon_attempt()
    }
    
    /// Create terminal without daemon check (for testing)
    pub fn create_terminal_basic(&mut self) -> u64 {
        let system_shell = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string());
        let working_directory = std::env::current_dir()
            .ok()
            .map(|p| p);
            
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        
        log::info!("Creating local PTY terminal {}", terminal_id);
        
        let terminal_backend = match TerminalBackend::new(
            terminal_id,
            self.egui_ctx.clone(),
            self.pty_proxy_sender.clone(),
            egui_term::BackendSettings {
                shell: system_shell.clone(),
                args: vec!["-l".to_string()], // Login shell args
                working_directory: working_directory.clone(),
                env: std::collections::HashMap::new(),
            },
        ) {
            Ok(backend) => {
                log::info!("✅ Terminal backend created successfully for terminal {}", terminal_id);
                backend
            }
            Err(e) => {
                log::error!("❌ Failed to create terminal backend for terminal {}: {}", terminal_id, e);
                panic!("Failed to create terminal backend: {}", e);
            }
        };

        self.terminals.insert(terminal_id, terminal_backend);
        self.korean_input_states.insert(terminal_id, KoreanInputState::new());
        terminal_id
    }
    
    /// Create terminal with daemon support - actual connection attempt
    pub fn create_terminal_with_daemon_attempt(&mut self) -> u64 {
        let system_shell = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string());
        let working_directory = std::env::current_dir()
            .ok();
            
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        
        // Check if daemon is available and try to connect
        if self.daemon_client.is_some() {
            log::info!("PTY daemon client already available for terminal {}", terminal_id);
            // Try to create daemon terminal immediately
            return self.create_daemon_terminal_sync(terminal_id, system_shell, working_directory);
        } else if std::path::Path::new("/tmp/tterm-daemon.sock").exists() {
            log::info!("✅ PTY daemon detected, attempting connection for terminal {}", terminal_id);
            
            // Mark this terminal as connecting
            self.connecting_terminals.insert(terminal_id, "Connecting to daemon...".to_string());
            
            // Spawn background task to connect to daemon
            self.spawn_daemon_connection_task(terminal_id, system_shell.clone(), working_directory.clone());
            
            // Return the terminal ID immediately (UI will show connecting state)
            return terminal_id;
        } else {
            log::debug!("PTY daemon not available, using local PTY for terminal {}", terminal_id);
        }
        
        // Fallback to local PTY terminal
        self.create_local_pty_terminal(terminal_id, system_shell, working_directory)
    }
    
    /// Create a local PTY terminal
    fn create_local_pty_terminal(&mut self, terminal_id: u64, system_shell: String, working_directory: Option<std::path::PathBuf>) -> u64 {
        let working_dir = working_directory.map(|p| p);
        let terminal_backend = TerminalBackend::new(
            terminal_id,
            self.egui_ctx.clone(),
            self.pty_proxy_sender.clone(),
            egui_term::BackendSettings {
                shell: system_shell,
                args: vec!["-l".to_string()], // Login shell
                working_directory: working_dir,
                env: std::collections::HashMap::new(),
            },
        )
        .unwrap();

        self.terminals.insert(terminal_id, terminal_backend);
        self.korean_input_states.insert(terminal_id, KoreanInputState::new());
        
        log::info!("Created local PTY terminal {}", terminal_id);
        terminal_id
    }
    
    /// Spawn a background task to connect to daemon
    fn spawn_daemon_connection_task(&mut self, terminal_id: u64, system_shell: String, working_directory: Option<std::path::PathBuf>) {
        let ctx = self.egui_ctx.clone();
        
        // Use the existing sender channel
        let tx = match self.daemon_connection_sender.clone() {
            Some(sender) => sender,
            None => {
                log::error!("No daemon connection sender available");
                return;
            }
        };
        
        std::thread::spawn(move || {
            log::info!("Background: Thread started for terminal {}", terminal_id);
            
            // Create a new tokio runtime for this thread
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => {
                    log::info!("Background: Tokio runtime created for terminal {}", terminal_id);
                    runtime
                }
                Err(e) => {
                    log::error!("Background: Failed to create tokio runtime for terminal {}: {}", terminal_id, e);
                    return;
                }
            };
            
            rt.block_on(async {
                log::info!("Background: Entering async block for terminal {}", terminal_id);
                log::info!("Background: Attempting to connect to daemon for terminal {}", terminal_id);
                
                match crate::ipc::DaemonClient::new().await {
                    Ok(mut client) => {
                        log::info!("Background: Successfully connected to daemon for terminal {}", terminal_id);
                        
                        // Create daemon session
                        log::info!("Background: About to create session for terminal {}", terminal_id);
                        log::info!("Background: Shell: {}, Working dir: {:?}", system_shell, working_directory.as_ref().map(|p| p.to_string_lossy()));
                        
                        let session_result = client.create_session(system_shell.clone(), working_directory.map(|p| p.to_string_lossy().to_string())).await;
                        log::info!("Background: create_session returned for terminal {}", terminal_id);
                        
                        match session_result {
                            Ok(session_id) => {
                                log::info!("Background: Created daemon session {:?} for terminal {}", session_id, terminal_id);
                                log::info!("Background: Sending result to main thread for terminal {}", terminal_id);
                                let send_result = tx.send((terminal_id, Ok((client, session_id))));
                                log::info!("Background: Send result: {:?} for terminal {}", send_result, terminal_id);
                            }
                            Err(e) => {
                                log::warn!("Background: Failed to create daemon session: {}", e);
                                let send_result = tx.send((terminal_id, Err(e)));
                                log::warn!("Background: Send error result: {:?} for terminal {}", send_result, terminal_id);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Background: Failed to connect to daemon: {}", e);
                        let _ = tx.send((terminal_id, Err(e)));
                    }
                }
                
                // Trigger UI repaint
                ctx.request_repaint();
            });
        });
    }
    
    /// Process daemon connection results from background tasks
    pub fn process_daemon_connection_results(&mut self) {
        let mut results = Vec::new();
        
        // Collect all results first to avoid borrow checker issues
        if let Some(ref receiver) = self.daemon_connection_receiver {
            while let Ok(result) = receiver.try_recv() {
                log::debug!("📥 Received daemon connection result");
                results.push(result);
            }
        }
        
        // Process the collected results
        for (terminal_id, result) in results {
            log::info!("🔄 Processing connection result for terminal {}", terminal_id);
            match result {
                Ok((client, session_id)) => {
                    log::info!("✅ Daemon connection completed for terminal {}, session: {:?}", terminal_id, session_id);
                    
                    // Store the daemon client and session
                    self.daemon_client = Some(Arc::new(TokioMutex::new(client)));
                    self.daemon_sessions.insert(terminal_id, session_id);
                    
                    // Remove from connecting terminals
                    self.connecting_terminals.remove(&terminal_id);
                    
                    // Create a daemon terminal instead of local PTY
                    self.create_daemon_terminal_ui(terminal_id, session_id);
                    
                    log::info!("Created daemon-backed terminal {} with daemon UI", terminal_id);
                }
                Err(e) => {
                    log::warn!("❌ Daemon connection failed for terminal {}: {}", terminal_id, e);
                    
                    // Remove from connecting terminals and fallback to local PTY
                    self.connecting_terminals.remove(&terminal_id);
                    
                    let working_dir = std::env::current_dir().ok();
                    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                    self.create_local_pty_terminal(terminal_id, shell, working_dir);
                    
                    log::info!("Created fallback local PTY for terminal {}", terminal_id);
                }
            }
        }
    }
    
    /// Create a daemon-backed terminal UI
    fn create_daemon_terminal_ui(&mut self, terminal_id: u64, session_id: Uuid) {
        log::info!("Creating daemon terminal UI for terminal {} with session {:?}", terminal_id, session_id);
        
        // Mark this terminal as daemon-backed
        self.daemon_terminals.insert(terminal_id);
        
        // For now, create a dummy terminal that will be handled differently in the UI
        // The actual I/O will be handled through the daemon client
        
        // Create a local terminal but mark it as daemon-controlled
        let working_dir = std::env::current_dir().ok();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        self.create_local_pty_terminal(terminal_id, shell, working_dir);
        
        log::info!("Daemon terminal UI created for terminal {}", terminal_id);
    }
    
    /// Create daemon terminal synchronously (when daemon client already exists)
    fn create_daemon_terminal_sync(&mut self, terminal_id: u64, system_shell: String, working_directory: Option<std::path::PathBuf>) -> u64 {
        // For now, fallback to local PTY since we can't do async in sync context
        log::warn!("Daemon client exists but sync creation not implemented, falling back to local PTY");
        self.create_local_pty_terminal(terminal_id, system_shell, working_directory)
    }
    
    /// Create a terminal with daemon fallback to local PTY
    pub async fn create_terminal_with_daemon_fallback(&mut self) -> u64 {
        let system_shell = std::env::var("SHELL")
            .unwrap_or_else(|_| "/bin/bash".to_string());
        let working_directory = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()));
            
        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        
        // Try to create daemon terminal first
        match self.create_daemon_terminal(Some(system_shell.clone()), working_directory.clone()).await {
            Ok(daemon_terminal_id) => {
                log::info!("✅ Created daemon terminal {} for UI terminal {}", daemon_terminal_id, terminal_id);
                
                // For daemon terminals, we still need a minimal backend for UI
                // but it will communicate through the daemon
                let working_dir = working_directory.map(|s| std::path::PathBuf::from(s));
                let terminal_backend = TerminalBackend::new(
                    terminal_id,
                    self.egui_ctx.clone(),
                    self.pty_proxy_sender.clone(),
                    egui_term::BackendSettings {
                        args: vec![system_shell],
                        working_directory: working_dir,
                        ..Default::default()
                    },
                )
                .unwrap();
                
                self.terminals.insert(terminal_id, terminal_backend);
                self.korean_input_states.insert(terminal_id, KoreanInputState::new());
                terminal_id
            }
            Err(e) => {
                log::warn!("❌ Daemon terminal creation failed: {}, falling back to local PTY", e);
                
                // Fallback to local PTY
                let working_dir = working_directory.map(|s| std::path::PathBuf::from(s));
                let terminal_backend = TerminalBackend::new(
                    terminal_id,
                    self.egui_ctx.clone(),
                    self.pty_proxy_sender.clone(),
                    egui_term::BackendSettings {
                        args: vec![system_shell],
                        working_directory: working_dir,
                        ..Default::default()
                    },
                )
                .unwrap();
                
                self.terminals.insert(terminal_id, terminal_backend);
                self.korean_input_states.insert(terminal_id, KoreanInputState::new());
                terminal_id
            }
        }
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
    
    /// Create a UI-only terminal backend for daemon terminals
    fn create_daemon_ui_terminal(&mut self, daemon_terminal_id: u64) -> u64 {
        // For daemon terminals, we don't create a real PTY locally
        // We just create the UI state and connect it to daemon communication
        
        // Create a dummy terminal backend that will be handled by daemon communication
        // For now, we'll create a minimal backend without actual PTY
        self.korean_input_states.insert(daemon_terminal_id, KoreanInputState::new());
        
        log::info!("Created UI terminal {} for daemon session", daemon_terminal_id);
        daemon_terminal_id
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
