use crate::types::AppState;
use egui_term::BackendCommand;

pub struct BroadcastManager;

impl BroadcastManager {
    /// Toggle broadcast mode on/off
    pub fn toggle_broadcast_mode(state: &mut AppState) {
        state.broadcast_mode = !state.broadcast_mode;
        
        if state.broadcast_mode {
            // Start with all terminals selected
            state.selected_terminals = state.terminals.keys().cloned().collect();
        } else {
            state.selected_terminals.clear();
        }
    }
    
    /// Toggle selection of a specific terminal for broadcasting
    pub fn toggle_terminal_selection(state: &mut AppState, terminal_id: u64) {
        if state.selected_terminals.contains(&terminal_id) {
            state.selected_terminals.remove(&terminal_id);
        } else {
            state.selected_terminals.insert(terminal_id);
        }
    }
    
    /// Toggle selection of all terminals
    pub fn toggle_all_terminals_selection(state: &mut AppState) {
        if state.broadcast_mode {
            if state.selected_terminals.len() == state.terminals.len() {
                state.selected_terminals.clear();
            } else {
                state.selected_terminals = state.terminals.keys().cloned().collect();
            }
        }
    }
    
    /// Broadcast input to all selected terminals
    pub fn broadcast_input(state: &mut AppState, input: &str) {
        if state.broadcast_mode {
            for &terminal_id in &state.selected_terminals {
                if let Some(terminal) = state.terminals.get_mut(&terminal_id) {
                    // Send input to terminal
                    terminal.process_command(BackendCommand::Write(input.as_bytes().to_vec()));
                }
            }
        }
    }
    
    /// Check if a terminal is selected for broadcasting
    pub fn is_terminal_selected(state: &AppState, terminal_id: u64) -> bool {
        state.broadcast_mode && state.selected_terminals.contains(&terminal_id)
    }
    
    /// Get the number of selected terminals
    pub fn selected_terminal_count(state: &AppState) -> usize {
        state.selected_terminals.len()
    }
    
    /// Check if broadcast mode is active
    pub fn is_broadcast_mode_active(state: &AppState) -> bool {
        state.broadcast_mode
    }
}
