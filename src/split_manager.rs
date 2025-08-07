use crate::types::{AppState, PanelContent, SplitDirection};
use crate::tab_manager::TabManager;

pub struct SplitManager;

impl SplitManager {
    /// Split the focused panel in the specified direction
    pub fn split_focused_panel(state: &mut AppState, direction: SplitDirection) {
        if let Some(focused_terminal_id) = state.focused_terminal {
            let new_terminal_id = state.create_terminal();
            let active_tab_id = state.active_tab_id;
            
            if let Some(layout) = state.tab_layouts.get_mut(&active_tab_id) {
                Self::replace_terminal_with_split_static(layout, focused_terminal_id, direction, new_terminal_id);
                state.focused_terminal = Some(new_terminal_id);
            }
        }
    }
    
    /// Replace a terminal with a split containing the original and a new terminal
    fn replace_terminal_with_split_static(
        content: &mut PanelContent,
        target_terminal_id: u64,
        direction: SplitDirection,
        new_terminal_id: u64,
    ) -> bool {
        match content {
            PanelContent::Terminal(id) if *id == target_terminal_id => {
                let old_terminal = PanelContent::Terminal(*id);
                let new_terminal = PanelContent::Terminal(new_terminal_id);
                
                *content = PanelContent::Split {
                    direction,
                    first: Box::new(old_terminal),
                    second: Box::new(new_terminal),
                    ratio: 0.5,
                };
                true
            }
            PanelContent::Split { first, second, .. } => {
                Self::replace_terminal_with_split_static(first, target_terminal_id, direction, new_terminal_id)
                    || Self::replace_terminal_with_split_static(second, target_terminal_id, direction, new_terminal_id)
            }
            _ => false,
        }
    }
    
    /// Handle terminal exit and merge panels if necessary
    pub fn handle_terminal_exit(state: &mut AppState, terminal_id: u64, ctx: &egui::Context) {
        // Find which tab contains this terminal
        let mut tab_to_close = None;
        let mut terminal_found = false;
        
        for (&tab_id, layout) in &state.tab_layouts {
            if Self::contains_terminal(layout, terminal_id) {
                terminal_found = true;
                
                // Check if this is the only terminal in the tab
                let terminal_count = Self::count_terminals_in_layout(layout);
                
                if terminal_count == 1 {
                    // This is the only terminal in the tab
                    if state.tabs.len() == 1 {
                        // This is the last tab, close the application
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    } else {
                        // Close the entire tab
                        tab_to_close = Some(tab_id);
                    }
                } else {
                    // There are other terminals in the tab, find sibling BEFORE removing the terminal
                    let sibling_id = if state.focused_terminal == Some(terminal_id) {
                        Self::find_sibling_terminal_before_removal(&layout, terminal_id)
                    } else {
                        None
                    };
                    
                    // Now remove terminal and merge panels
                    Self::remove_terminal_and_merge_panels(state, tab_id, terminal_id);
                    
                    // Update focus if needed
                    if state.focused_terminal == Some(terminal_id) {
                        if let Some(sibling) = sibling_id {
                            state.focused_terminal = Some(sibling);
                        } else if let Some(updated_layout) = state.tab_layouts.get(&state.active_tab_id) {
                            state.focused_terminal = TabManager::get_first_terminal_id(updated_layout);
                        }
                    }
                }
                break;
            }
        }
        
        if !terminal_found {
            // Terminal not found in any tab layout, just remove from terminals map
            state.terminals.remove(&terminal_id);
            state.selected_terminals.remove(&terminal_id);
            return;
        }
        
        if let Some(tab_id) = tab_to_close {
            TabManager::close_tab(state, tab_id);
        }
        
        // Remove terminal from data structures
        state.terminals.remove(&terminal_id);
        state.selected_terminals.remove(&terminal_id);
    }
    
    /// Check if a layout contains a specific terminal
    pub fn contains_terminal(content: &PanelContent, terminal_id: u64) -> bool {
        match content {
            PanelContent::Terminal(id) => *id == terminal_id,
            PanelContent::Split { first, second, .. } => {
                Self::contains_terminal(first, terminal_id) || Self::contains_terminal(second, terminal_id)
            }
        }
    }
    
    /// Count terminals in a layout
    pub fn count_terminals_in_layout(content: &PanelContent) -> usize {
        match content {
            PanelContent::Terminal(_) => 1,
            PanelContent::Split { first, second, .. } => {
                Self::count_terminals_in_layout(first) + Self::count_terminals_in_layout(second)
            }
        }
    }
    
    /// Remove terminal and merge panels
    fn remove_terminal_and_merge_panels(state: &mut AppState, tab_id: u64, terminal_id: u64) {
        if let Some(layout) = state.tab_layouts.get(&tab_id).cloned() {
            if let Some(merged_layout) = Self::remove_terminal_from_layout(&layout, terminal_id) {
                state.tab_layouts.insert(tab_id, merged_layout);
            }
        }
    }
    
    /// Find sibling terminal before removal
    fn find_sibling_terminal_before_removal(content: &PanelContent, terminal_id: u64) -> Option<u64> {
        Self::find_direct_sibling(content, terminal_id)
    }
    
    /// Find the direct sibling terminal - the one that shares the same immediate parent split
    fn find_direct_sibling(content: &PanelContent, terminal_id: u64) -> Option<u64> {
        match content {
            PanelContent::Terminal(_) => None,
            PanelContent::Split { first, second, .. } => {
                // Check if the target terminal is a direct child of this split
                match (first.as_ref(), second.as_ref()) {
                    (PanelContent::Terminal(id), _) if *id == terminal_id => {
                        // Target is first child, sibling is second child
                        TabManager::get_first_terminal_id(second)
                    }
                    (_, PanelContent::Terminal(id)) if *id == terminal_id => {
                        // Target is second child, sibling is first child  
                        TabManager::get_first_terminal_id(first)
                    }
                    _ => {
                        // Target is not a direct child, search recursively in children
                        Self::find_direct_sibling(first, terminal_id)
                            .or_else(|| Self::find_direct_sibling(second, terminal_id))
                    }
                }
            }
        }
    }
    
    /// Remove terminal from layout and return merged layout
    fn remove_terminal_from_layout(content: &PanelContent, terminal_id: u64) -> Option<PanelContent> {
        match content {
            PanelContent::Terminal(id) if *id == terminal_id => {
                // This terminal should be removed, return None to indicate removal
                None
            }
            PanelContent::Terminal(_) => {
                // Different terminal, keep as is
                Some(content.clone())
            }
            PanelContent::Split { direction, first, second, ratio } => {
                let first_result = Self::remove_terminal_from_layout(first, terminal_id);
                let second_result = Self::remove_terminal_from_layout(second, terminal_id);
                
                match (first_result, second_result) {
                    (None, Some(second_content)) => {
                        // First panel was removed, return second panel content
                        Some(second_content)
                    }
                    (Some(first_content), None) => {
                        // Second panel was removed, return first panel content
                        Some(first_content)
                    }
                    (Some(first_content), Some(second_content)) => {
                        // Both panels remain, reconstruct split
                        Some(PanelContent::Split {
                            direction: *direction,
                            first: Box::new(first_content),
                            second: Box::new(second_content),
                            ratio: *ratio,
                        })
                    }
                    (None, None) => {
                        // Both panels were removed (shouldn't happen in normal cases)
                        None
                    }
                }
            }
        }
    }
    
    /// Navigate focus within splits
    pub fn navigate_focus_in_splits(state: &mut AppState) {
        if let Some(layout) = state.tab_layouts.get(&state.active_tab_id) {
            let terminal_ids = TabManager::collect_terminal_ids(layout);
            
            if terminal_ids.len() <= 1 {
                return; // No navigation needed with only one terminal
            }
            
            if let Some(current_focused) = state.focused_terminal {
                if let Some(current_idx) = terminal_ids.iter().position(|&id| id == current_focused) {
                    // Move to next terminal (cycling back to first if at end)
                    let next_idx = (current_idx + 1) % terminal_ids.len();
                    state.focused_terminal = Some(terminal_ids[next_idx]);
                } else {
                    // If current focus is not in this tab, focus on first terminal
                    state.focused_terminal = terminal_ids.first().copied();
                }
            } else {
                // No current focus, focus on first terminal
                state.focused_terminal = terminal_ids.first().copied();
            }
        }
    }
}
