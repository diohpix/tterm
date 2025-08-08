use crate::types::{AppState, TerminalTab, PanelContent};

pub struct TabManager;

impl TabManager {
    /// Create a new terminal tab with daemon fallback
    pub fn create_new_tab_with_daemon(state: &mut AppState) {
        // If already creating a tab, ignore
        if state.pending_tab_creation.is_some() {
            return;
        }
        
        let tab_id = state.next_tab_id;
        state.next_tab_id += 1;
        
        // Mark this tab as pending creation
        state.pending_tab_creation = Some(tab_id);
        
        // Create temporary tab with loading state
        let tab = TerminalTab {
            id: tab_id,
            title: format!("Terminal {} (connecting...)", tab_id),
        };
        
        state.tabs.insert(tab_id, tab);
        state.tab_order.push(tab_id);
        state.active_tab_id = tab_id;
        
        // Terminal creation is now synchronous
        // No need for async triggers
    }
    
    /// Complete tab creation after async terminal is ready
    pub async fn complete_pending_tab_creation(state: &mut AppState) {
        if let Some(tab_id) = state.pending_tab_creation.take() {
            // Create terminal with daemon fallback
            let terminal_id = state.create_terminal_with_daemon_fallback().await;
            
            // Update tab title to remove loading state
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                tab.title = format!("Terminal {}", tab_id);
            }
            
            // Set up the layout with the created terminal
            let layout = PanelContent::Terminal(terminal_id);
            state.tab_layouts.insert(tab_id, layout);
            state.focused_terminal = Some(terminal_id);
            
            // Update grid size after tab creation
            crate::grid_manager::GridManager::update_grid_size(state);
        }
    }
    
    /// Create a new terminal tab (original synchronous method for fallback)
    pub fn create_new_tab(state: &mut AppState) {
        let tab_id = state.next_tab_id;
        state.next_tab_id += 1;
        
        // Create a terminal for this tab
        let terminal_id = state.create_terminal();
        
        let tab = TerminalTab {
            id: tab_id,
            title: format!("Terminal {}", tab_id),
        };
        
        // Set up the layout with a single terminal
        let layout = PanelContent::Terminal(terminal_id);
        
        state.tabs.insert(tab_id, tab);
        state.tab_order.push(tab_id); // Maintain order
        state.tab_layouts.insert(tab_id, layout);
        state.active_tab_id = tab_id;
        state.focused_terminal = Some(terminal_id);
        
        // Update grid size after tab creation
        crate::grid_manager::GridManager::update_grid_size(state);
    }
    
    /// Close a terminal tab
    pub fn close_tab(state: &mut AppState, tab_id: u64) {
        if state.tabs.len() > 1 {
            // Find the index of the tab to be closed BEFORE removing it
            let closed_tab_index = state.tab_order.iter().position(|&id| id == tab_id);
            
            // Remove associated terminals
            if let Some(layout) = state.tab_layouts.remove(&tab_id) {
                Self::collect_terminal_ids(&layout).into_iter().for_each(|tid| {
                    state.terminals.remove(&tid);
                });
            }
            
            state.tabs.remove(&tab_id);
            state.tab_order.retain(|&id| id != tab_id); // Remove from order
            
            // Update grid size after tab removal
            crate::grid_manager::GridManager::update_grid_size(state);
            
            // If we closed the active tab, switch to a smart previous/next tab
            if state.active_tab_id == tab_id {
                if let Some(closed_index) = closed_tab_index {
                    // Try to go to the previous tab (left), or next tab (right) if it was the first
                    let next_tab_id = if closed_index > 0 {
                        // Go to previous tab (index - 1)
                        state.tab_order.get(closed_index - 1).copied()
                    } else {
                        // This was the first tab, go to the new first tab
                        state.tab_order.first().copied()
                    };
                    
                    if let Some(tab_id) = next_tab_id {
                        state.active_tab_id = tab_id;
                        // Update focused terminal
                        if let Some(layout) = state.tab_layouts.get(&state.active_tab_id) {
                            state.focused_terminal = Self::get_first_terminal_id(layout);
                        }
                    }
                } else {
                    // Fallback: go to first tab
                    if let Some(&next_tab_id) = state.tab_order.first() {
                        state.active_tab_id = next_tab_id;
                        if let Some(layout) = state.tab_layouts.get(&state.active_tab_id) {
                            state.focused_terminal = Self::get_first_terminal_id(layout);
                        }
                    }
                }
            }
        }
    }
    
    /// Switch to a specific tab by ID
    pub fn switch_to_tab(state: &mut AppState, tab_id: u64) {
        if state.tabs.contains_key(&tab_id) {
            state.active_tab_id = tab_id;
            // Update focus to the first terminal in this tab
            if let Some(layout) = state.tab_layouts.get(&tab_id) {
                state.focused_terminal = Self::get_first_terminal_id(layout);
            }
        }
    }
    
    /// Switch to tab by index (0-based)
    pub fn switch_to_tab_by_index(state: &mut AppState, index: usize) {
        if index < state.tab_order.len() {
            let tab_id = state.tab_order[index];
            Self::switch_to_tab(state, tab_id);
        }
    }
    
    /// Get all terminal IDs in a layout
    pub fn collect_terminal_ids(content: &PanelContent) -> Vec<u64> {
        match content {
            PanelContent::Terminal(id) => vec![*id],
            PanelContent::Split { first, second, .. } => {
                let mut ids = Self::collect_terminal_ids(first);
                ids.extend(Self::collect_terminal_ids(second));
                ids
            }
        }
    }
    
    /// Get the first terminal ID in a layout
    pub fn get_first_terminal_id(content: &PanelContent) -> Option<u64> {
        match content {
            PanelContent::Terminal(id) => Some(*id),
            PanelContent::Split { first, .. } => Self::get_first_terminal_id(first),
        }
    }
}
