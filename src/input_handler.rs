use crate::types::{AppState, SplitDirection};
use crate::tab_manager::TabManager;
use crate::split_manager::SplitManager;
use crate::grid_manager::GridManager;
use crate::broadcast_manager::BroadcastManager;
use egui::{Key, Modifiers};
use egui_term::BackendCommand;

pub struct InputHandler;

impl InputHandler {
    /// Handle all keyboard shortcuts and input
    pub fn handle_input(state: &mut AppState, ctx: &egui::Context) -> bool {
        let mut handled_by_shortcuts = false;
        
        ctx.input(|i| {
            if i.modifiers.command {
                // Tab management shortcuts
                if i.key_pressed(Key::T) {
                    TabManager::create_new_tab(state);
                    handled_by_shortcuts = true;
                }
                if i.key_pressed(Key::W) && state.tabs.len() > 1 {
                    TabManager::close_tab(state, state.active_tab_id);
                    handled_by_shortcuts = true;
                }
                
                // Tab switching with Ctrl+1-9
                for (idx, &key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9].iter().enumerate() {
                    if i.key_pressed(key) {
                        TabManager::switch_to_tab_by_index(state, idx);
                        handled_by_shortcuts = true;
                    }
                }
                
                // Split shortcuts
                if i.modifiers.shift {
                    if i.key_pressed(Key::D) {
                        SplitManager::split_focused_panel(state, SplitDirection::Horizontal);
                        handled_by_shortcuts = true;
                    }
                } else {
                    if i.key_pressed(Key::D) {
                        SplitManager::split_focused_panel(state, SplitDirection::Vertical);
                        handled_by_shortcuts = true;
                    }
                }
                
                // Grid view toggle (Ctrl+S as per PRD)
                if i.key_pressed(Key::S) {
                    GridManager::toggle_grid_view(state);
                    handled_by_shortcuts = true;
                }
                
                // Broadcast shortcuts
                if i.modifiers.shift {
                    if i.key_pressed(Key::B) {
                        BroadcastManager::toggle_broadcast_mode(state);
                        handled_by_shortcuts = true;
                    }
                    if i.key_pressed(Key::A) {
                        BroadcastManager::toggle_all_terminals_selection(state);
                        handled_by_shortcuts = true;
                    }
                }
            }
            
            // Panel navigation with Alt+Arrow keys
            if i.modifiers.alt {
                if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowRight) ||
                   i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown) {
                    SplitManager::navigate_focus_in_splits(state);
                    handled_by_shortcuts = true;
                }
            }
        });
        
        // Handle direct keyboard input to focused terminal (if not handled by shortcuts)
        if !handled_by_shortcuts {
            Self::handle_direct_input_to_focused_terminal(state, ctx);
        }
        
        handled_by_shortcuts
    }
    
    /// Handle direct keyboard input to the focused terminal
    fn handle_direct_input_to_focused_terminal(state: &mut AppState, ctx: &egui::Context) {
        if let Some(focused_terminal_id) = state.focused_terminal {
            let broadcast_mode = state.broadcast_mode;
            
            // First collect the events we need to process
            let events = ctx.input(|i| i.events.clone());
            
            for event in events {
                match event {
                    egui::Event::Text(text) => {
                        if broadcast_mode {
                            BroadcastManager::broadcast_input(state, &text);
                        } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                            terminal.process_command(BackendCommand::Write(text.as_bytes().to_vec()));
                        }
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        if let Some(bytes) = Self::key_to_bytes(&key, &modifiers) {
                            if broadcast_mode {
                                BroadcastManager::broadcast_input(state, &String::from_utf8_lossy(&bytes));
                            } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                terminal.process_command(BackendCommand::Write(bytes));
                            }
                        }
                    }
                    egui::Event::Paste(text) => {
                        if broadcast_mode {
                            BroadcastManager::broadcast_input(state, &text);
                        } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                            terminal.process_command(BackendCommand::Write(text.as_bytes().to_vec()));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    /// Convert key and modifiers to terminal bytes
    fn key_to_bytes(key: &Key, modifiers: &Modifiers) -> Option<Vec<u8>> {
        match key {
            // Special keys
            Key::Enter => Some(b"\r".to_vec()),
            Key::Tab => Some(b"\t".to_vec()),
            Key::Backspace => Some(b"\x7f".to_vec()),
            Key::Delete => Some(b"\x1b[3~".to_vec()),
            Key::ArrowUp => Some(b"\x1b[A".to_vec()),
            Key::ArrowDown => Some(b"\x1b[B".to_vec()),
            Key::ArrowRight => Some(b"\x1b[C".to_vec()),
            Key::ArrowLeft => Some(b"\x1b[D".to_vec()),
            Key::Home => Some(b"\x1b[H".to_vec()),
            Key::End => Some(b"\x1b[F".to_vec()),
            Key::PageUp => Some(b"\x1b[5~".to_vec()),
            Key::PageDown => Some(b"\x1b[6~".to_vec()),
            Key::Escape => Some(b"\x1b".to_vec()),
            _ => {
                // Handle Ctrl combinations (but skip application shortcuts)
                if modifiers.ctrl || modifiers.command {
                    Self::ctrl_key_to_bytes(key, modifiers)
                } else {
                    None
                }
            }
        }
    }
    
    /// Convert Ctrl key combinations to terminal bytes
    fn ctrl_key_to_bytes(key: &Key, modifiers: &Modifiers) -> Option<Vec<u8>> {
        // Skip shortcuts that are handled by application 
        match key {
            // Skip these as they are application shortcuts
            Key::T | Key::W | Key::S 
            | Key::Num1 | Key::Num2 | Key::Num3 
            | Key::Num4 | Key::Num5 | Key::Num6 
            | Key::Num7 | Key::Num8 | Key::Num9 => None,
            Key::D if modifiers.shift => None, // Skip horizontal split
            Key::D => None, // Skip vertical split
            Key::B if modifiers.shift => None, // Skip broadcast toggle
            Key::A if modifiers.shift => None, // Skip select all toggle
            // Process regular Ctrl combinations for terminal
            Key::A => Some(b"\x01".to_vec()),
            Key::B => Some(b"\x02".to_vec()),
            Key::C => Some(b"\x03".to_vec()),
            Key::E => Some(b"\x05".to_vec()),
            Key::F => Some(b"\x06".to_vec()),
            Key::G => Some(b"\x07".to_vec()),
            Key::H => Some(b"\x08".to_vec()),
            Key::I => Some(b"\x09".to_vec()),
            Key::J => Some(b"\x0a".to_vec()),
            Key::K => Some(b"\x0b".to_vec()),
            Key::L => Some(b"\x0c".to_vec()),
            Key::M => Some(b"\x0d".to_vec()),
            Key::N => Some(b"\x0e".to_vec()),
            Key::O => Some(b"\x0f".to_vec()),
            Key::P => Some(b"\x10".to_vec()),
            Key::Q => Some(b"\x11".to_vec()),
            Key::R => Some(b"\x12".to_vec()),
            Key::U => Some(b"\x15".to_vec()),
            Key::V => Some(b"\x16".to_vec()),
            Key::X => Some(b"\x18".to_vec()),
            Key::Y => Some(b"\x19".to_vec()),
            Key::Z => Some(b"\x1a".to_vec()),
            _ => None
        }
    }
}
