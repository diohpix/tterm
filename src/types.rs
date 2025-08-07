use egui_term::TerminalBackend;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use crate::ime::korean::KoreanInputState;

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
}
