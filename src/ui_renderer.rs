use crate::types::{AppState, ViewMode, PanelContent, SplitDirection};
use crate::tab_manager::TabManager;
use crate::grid_manager::GridManager;
use crate::broadcast_manager::BroadcastManager;
use crate::ime::cjk;
use egui::{Ui, Rect, Vec2, Pos2, FontId, Align2};
use egui_term::TerminalView;

pub struct UiRenderer;

impl UiRenderer {
    /// Render the tab bar
    pub fn render_tab_bar(state: &mut AppState, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let mut tab_to_close = None;
            let mut tab_to_activate = None;
            
            // Use tab_order to maintain consistent order
            for &tab_id in &state.tab_order {
                if let Some(tab) = state.tabs.get(&tab_id) {
                    let is_active = tab_id == state.active_tab_id;
                    
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let tab_response = ui.selectable_label(is_active, &tab.title);
                            
                            if tab_response.clicked() {
                                tab_to_activate = Some(tab_id);
                            }
                            
                            // Close button
                            if ui.small_button("×").clicked() && state.tabs.len() > 1 {
                                tab_to_close = Some(tab_id);
                            }
                        });
                    });
                }
            }
            
            // New tab button
            if ui.button("+").clicked() {
                TabManager::create_new_tab(state);
            }
            
            // Handle tab activation outside the closure
            if let Some(tab_id) = tab_to_activate {
                TabManager::switch_to_tab(state, tab_id);
            }
            
            if let Some(tab_id) = tab_to_close {
                TabManager::close_tab(state, tab_id);
            }
        });
    }
    
    /// Render the status bar
    pub fn render_status_bar(state: &AppState, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Broadcast status
            if BroadcastManager::is_broadcast_mode_active(state) {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "☀ BROADCAST");
                ui.label(format!("({} terminals)", BroadcastManager::selected_terminal_count(state)));
            } else {
                ui.label("Single input");
            }
            
            ui.separator();
            
            // View mode status
            match &state.view_mode {
                ViewMode::Single => ui.label("Single view"),
                ViewMode::Grid { rows, cols, .. } => ui.label(format!("Grid {}x{}", rows, cols)),
            };
            
            ui.separator();
            
            // Focused terminal
            if let Some(focused) = state.focused_terminal {
                ui.label(format!("Focus: Terminal {}", focused));
            }
        });
    }
    
    /// Render panel content (terminal or split)
    pub fn render_panel_content(state: &mut AppState, ui: &mut Ui, content: &mut PanelContent, available_rect: Rect) {
        match content {
            PanelContent::Terminal(terminal_id) => {
                Self::render_terminal_panel(state, ui, *terminal_id, available_rect);
            }
            PanelContent::Split { direction, first, second, ratio, .. } => {
                Self::render_split_panel(state, ui, direction, first, second, ratio, available_rect);
            }
        }
    }
    
    /// Render a terminal panel
    fn render_terminal_panel(state: &mut AppState, ui: &mut Ui, terminal_id: u64, available_rect: Rect) {
        let is_focused = state.focused_terminal == Some(terminal_id);
        let is_selected_for_broadcast = BroadcastManager::is_terminal_selected(state, terminal_id);
        
        if let Some(terminal_backend) = state.terminals.get_mut(&terminal_id) {
            // Add visual focus indicator and broadcast selection
            let border_color = if is_focused {
                egui::Color32::from_rgb(0, 150, 255) // Blue for focused
            } else if is_selected_for_broadcast {
                egui::Color32::from_rgb(255, 100, 100) // Red for broadcast selected
            } else {
                egui::Color32::GRAY // Gray for normal
            };
            
            ui.painter().rect_stroke(
                available_rect,
                2.0,
                egui::Stroke::new(2.0, border_color),
                egui::epaint::StrokeKind::Outside,
            );
            
            // Check if this terminal is composing Korean text to potentially adjust cursor rendering
            let _is_composing_korean = state.korean_input_states
                .get(&terminal_id)
                .map(|korean_state| korean_state.is_composing)
                .unwrap_or(false);
            
            let terminal = TerminalView::new(ui, terminal_backend)
                .set_focus(false) // Disable focus on TerminalView to prevent mouse dependency
                .set_size(Vec2::new(available_rect.width(), available_rect.height()));
            
            // Render terminal and check for clicks
            ui.scope_builder(egui::UiBuilder::new().max_rect(available_rect), |ui| {
                ui.add(terminal)
            });
            
            // Render CJK double-wide cursor overlay (includes Korean composition)
            Self::render_cjk_cursor_overlay(state, ui, terminal_id, available_rect);
            
            // Check if the terminal area was clicked
            if ui.input(|i| i.pointer.any_click()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if available_rect.contains(pos) {
                        state.focused_terminal = Some(terminal_id);
                    }
                }
            }
        }
    }
    
    /// Render a split panel
    fn render_split_panel(
        state: &mut AppState, 
        ui: &mut Ui, 
        direction: &SplitDirection,
        first: &mut Box<PanelContent>,
        second: &mut Box<PanelContent>,
        ratio: &mut f32,
        available_rect: Rect
    ) {
        match direction {
            SplitDirection::Horizontal => {
                Self::render_horizontal_split(state, ui, first, second, ratio, available_rect);
            }
            SplitDirection::Vertical => {
                Self::render_vertical_split(state, ui, first, second, ratio, available_rect);
            }
        }
    }
    
    /// Render horizontal split
    fn render_horizontal_split(
        state: &mut AppState,
        ui: &mut Ui,
        first: &mut Box<PanelContent>,
        second: &mut Box<PanelContent>,
        ratio: &mut f32,
        available_rect: Rect,
    ) {
        let split_y = available_rect.min.y + available_rect.height() * *ratio;
        
        let first_rect = Rect::from_min_max(
            available_rect.min,
            egui::pos2(available_rect.max.x, split_y - 2.0),
        );
        
        let second_rect = Rect::from_min_max(
            egui::pos2(available_rect.min.x, split_y + 2.0),
            available_rect.max,
        );
        
        // Create separator area for dragging
        let separator_rect = Rect::from_min_max(
            egui::pos2(available_rect.min.x, split_y - 2.0),
            egui::pos2(available_rect.max.x, split_y + 2.0),
        );
        
        Self::render_panel_content(state, ui, first, first_rect);
        
        // Handle separator dragging
        let separator_response = ui.allocate_rect(separator_rect, egui::Sense::drag());
        if separator_response.dragged() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let new_split_y = pointer_pos.y;
                let new_ratio = ((new_split_y - available_rect.min.y) / available_rect.height()).clamp(0.1, 0.9);
                *ratio = new_ratio;
            }
        }
        
        // Draw separator with hover effect
        let separator_color = if separator_response.hovered() {
            egui::Color32::from_rgb(100, 150, 255) // Blue when hovered
        } else {
            egui::Color32::GRAY
        };
        
        // Ensure separator is clipped to the available rect
        let clipped_separator_rect = separator_rect.intersect(available_rect);
        ui.painter().rect_filled(clipped_separator_rect, 0.0, separator_color);
        
        // Change cursor when hovering
        if separator_response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        
        Self::render_panel_content(state, ui, second, second_rect);
    }
    
    /// Render vertical split
    fn render_vertical_split(
        state: &mut AppState,
        ui: &mut Ui,
        first: &mut Box<PanelContent>,
        second: &mut Box<PanelContent>,
        ratio: &mut f32,
        available_rect: Rect,
    ) {
        let split_x = available_rect.min.x + available_rect.width() * *ratio;
        
        let first_rect = Rect::from_min_max(
            available_rect.min,
            egui::pos2(split_x - 2.0, available_rect.max.y),
        );
        
        let second_rect = Rect::from_min_max(
            egui::pos2(split_x + 2.0, available_rect.min.y),
            available_rect.max,
        );
        
        // Create separator area for dragging
        let separator_rect = Rect::from_min_max(
            egui::pos2(split_x - 2.0, available_rect.min.y),
            egui::pos2(split_x + 2.0, available_rect.max.y),
        );
        
        Self::render_panel_content(state, ui, first, first_rect);
        
        // Handle separator dragging
        let separator_response = ui.allocate_rect(separator_rect, egui::Sense::drag());
        if separator_response.dragged() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let new_split_x = pointer_pos.x;
                let new_ratio = ((new_split_x - available_rect.min.x) / available_rect.width()).clamp(0.1, 0.9);
                *ratio = new_ratio;
            }
        }
        
        // Draw separator with hover effect
        let separator_color = if separator_response.hovered() {
            egui::Color32::from_rgb(100, 150, 255) // Blue when hovered
        } else {
            egui::Color32::GRAY
        };
        
        // Ensure separator is clipped to the available rect
        let clipped_separator_rect = separator_rect.intersect(available_rect);
        ui.painter().rect_filled(clipped_separator_rect, 0.0, separator_color);
        
        // Change cursor when hovering
        if separator_response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        
        Self::render_panel_content(state, ui, second, second_rect);
    }
    
    /// Render grid view
    pub fn render_grid_view(state: &mut AppState, ui: &mut Ui, available_rect: Rect) {
        // Extract grid parameters to avoid borrow issues
        let grid_info = match &state.view_mode {
            ViewMode::Grid { rows, cols, col_ratios, row_ratios } => Some((*rows, *cols, col_ratios.clone(), row_ratios.clone())),
            _ => None,
        };
        
        if let Some((rows, cols, mut col_ratios, mut row_ratios)) = grid_info {
            let tab_count = state.tabs.len();
            
            // Get all tabs for grid view
            let mut tab_ids: Vec<_> = state.tab_order.clone();
            tab_ids.truncate(rows * cols); // Don't render more than grid capacity
            
            // Render grid cells
            for (idx, &tab_id) in tab_ids.iter().enumerate() {
                Self::render_grid_cell(state, ui, tab_id, idx, rows, cols, &col_ratios, &row_ratios, tab_count, available_rect);
            }
            
            // Render grid separators
            Self::render_grid_separators(ui, &mut col_ratios, &mut row_ratios, rows, cols, tab_count, available_rect);
            
            // Update the view mode with modified ratios
            state.view_mode = ViewMode::Grid { rows, cols, col_ratios, row_ratios };
        }
    }
    
    /// Render a single grid cell
    fn render_grid_cell(
        state: &mut AppState,
        ui: &mut Ui,
        tab_id: u64,
        tab_index: usize,
        rows: usize,
        cols: usize,
        col_ratios: &[f32],
        row_ratios: &[f32],
        tab_count: usize,
        available_rect: Rect,
    ) {
        let cell_rect = GridManager::calculate_cell_rect(
            available_rect, rows, cols, col_ratios, row_ratios, tab_count, tab_index
        );
        
        // Get tab layout and render it
        if let Some(tab) = state.tabs.get(&tab_id) {
            if let Some(layout) = state.tab_layouts.get(&tab_id).cloned() {
                // Draw tab border
                let is_active_tab = tab_id == state.active_tab_id;
                let border_color = if is_active_tab {
                    egui::Color32::from_rgb(0, 150, 255) // Blue for active tab
                } else {
                    egui::Color32::GRAY // Gray for normal
                };
                
                ui.painter().rect_stroke(
                    cell_rect,
                    2.0,
                    egui::Stroke::new(2.0, border_color),
                    egui::epaint::StrokeKind::Outside,
                );
                
                // Check for mouse click to activate tab
                if ui.input(|i| i.pointer.any_click()) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if cell_rect.contains(pos) {
                            state.active_tab_id = tab_id;
                            state.focused_terminal = TabManager::get_first_terminal_id(&layout);
                        }
                    }
                }
                
                // Calculate header height and content area
                let header_height = 25.0;
                let header_rect = Rect::from_min_size(
                    cell_rect.min,
                    egui::vec2(cell_rect.width(), header_height),
                );
                let content_rect = Rect::from_min_size(
                    egui::pos2(cell_rect.min.x, cell_rect.min.y + header_height),
                    egui::vec2(cell_rect.width(), cell_rect.height() - header_height),
                );
                
                // Draw header background
                ui.painter().rect_filled(
                    header_rect,
                    2.0,
                    egui::Color32::from_rgb(50, 50, 50), // Dark gray background
                );
                
                // Draw header text
                ui.painter().text(
                    header_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &tab.title,
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                );
                
                // Render the tab's layout in the content area
                let mut layout_copy = layout.clone();
                Self::render_panel_content_clipped(state, ui, &mut layout_copy, content_rect);
                
                // Update the layout if it was modified
                state.tab_layouts.insert(tab_id, layout_copy);
            }
        }
    }
    
    /// Render grid separators
    fn render_grid_separators(
        ui: &mut Ui,
        col_ratios: &mut [f32],
        row_ratios: &mut [f32],
        rows: usize,
        cols: usize,
        tab_count: usize,
        available_rect: Rect,
    ) {
        // Calculate positions
        let mut col_positions = vec![0.0];
        let mut current_x = 0.0;
        for &ratio in col_ratios.iter() {
            current_x += ratio * available_rect.width();
            col_positions.push(current_x);
        }
        
        let mut row_positions = vec![0.0];
        let mut current_y = 0.0;
        for &ratio in row_ratios.iter() {
            current_y += ratio * available_rect.height();
            row_positions.push(current_y);
        }
        
        // Render column separators
        for col in 0..cols-1 {
            let separator_rect = GridManager::calculate_column_separator_rect(
                available_rect, rows, cols, &col_positions, &row_positions, tab_count, col
            );
            
            let separator_response = ui.allocate_rect(separator_rect, egui::Sense::drag());
            if separator_response.dragged() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    GridManager::handle_column_separator_drag(
                        col_ratios, available_rect, pointer_pos.x, col, cols
                    );
                }
            }
            
            // Draw separator with hover effect
            let separator_color = if separator_response.hovered() {
                egui::Color32::from_rgb(100, 150, 255)
            } else {
                egui::Color32::DARK_GRAY
            };
            
            ui.painter().rect_filled(separator_rect, 0.0, separator_color);
            
            if separator_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
        
        // Render row separators
        for row in 0..rows-1 {
            let separator_rect = GridManager::calculate_row_separator_rect(
                available_rect, &row_positions, row
            );
            
            let separator_response = ui.allocate_rect(separator_rect, egui::Sense::drag());
            if separator_response.dragged() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                    GridManager::handle_row_separator_drag(
                        row_ratios, available_rect, pointer_pos.y, row, rows
                    );
                }
            }
            
            // Draw separator with hover effect
            let separator_color = if separator_response.hovered() {
                egui::Color32::from_rgb(100, 150, 255)
            } else {
                egui::Color32::DARK_GRAY
            };
            
            ui.painter().rect_filled(separator_rect, 0.0, separator_color);
            
            if separator_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }
    }
    
    /// Render panel content with clipping
    fn render_panel_content_clipped(state: &mut AppState, ui: &mut Ui, content: &mut PanelContent, available_rect: Rect) {
        // Use ui.allocate_ui_with_layout for proper clipping
        ui.allocate_ui_with_layout(
            available_rect.size(),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
                ui.set_clip_rect(available_rect);
                Self::render_panel_content(state, ui, content, available_rect);
            },
        );
    }
    
    /// Render CJK double-wide cursor overlay on terminal
    /// This handles both Korean composition and completed CJK characters
    fn render_cjk_cursor_overlay(
        state: &AppState,
        ui: &mut Ui,
        terminal_id: u64,
        terminal_rect: Rect,
    ) {
        if let Some(terminal) = state.terminals.get(&terminal_id) {
            // Calculate cursor position in egui coordinates
            let cursor_pos = Self::terminal_cursor_to_screen_pos(
                terminal,
                terminal_rect,
            );
            
            // Get the actual terminal font settings
            let content = terminal.last_content();
            let terminal_size = &content.terminal_size;
            let font_size = terminal_size.cell_height as f32;
            let single_char_width = terminal_size.cell_width as f32;
            let char_height = terminal_size.cell_height as f32;
            
            // Check for Korean composition first
            let korean_state = state.korean_input_states.get(&terminal_id);
            let composing_char = korean_state
                .filter(|state| state.is_composing)
                .and_then(|state| state.get_current_char());
            
            // Get character at cursor position from terminal content
            let cursor_point = content.grid.cursor.point;
            let char_at_cursor = content.grid.display_iter()
                .find(|indexed| indexed.point == cursor_point)
                .map(|indexed| indexed.c);
            
            // Determine if we should show double-wide cursor
            let should_show_double = cjk::should_show_double_cursor(composing_char, char_at_cursor);
            
            if should_show_double {
                let cursor_width = if composing_char.is_some() || 
                    char_at_cursor.map_or(false, cjk::is_double_width_char) {
                    single_char_width * 2.0
                } else {
                    single_char_width
                };
                
                // Use the monospace font family (which includes D2Coding from app.rs configuration)
                let font_id = FontId::new(font_size, egui::FontFamily::Monospace);
                
                // Define colors for better visibility
                let cursor_bg_color = egui::Color32::from_rgb(255, 255, 255); // White background for cursor
                let cursor_fg_color = egui::Color32::from_rgb(0, 0, 0); // Black text on white background
                
                // Draw double-wide cursor background
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        cursor_pos,
                        Vec2::new(cursor_width, char_height),
                    ),
                    egui::Rounding::ZERO,
                    cursor_bg_color,
                );
                
                // Draw the character with inverted colors
                if let Some(display_char) = composing_char.or(char_at_cursor) {
                    let text_pos = Pos2::new(
                        cursor_pos.x + cursor_width / 2.0,
                        cursor_pos.y,
                    );
                    
                    ui.painter().text(
                        text_pos,
                        Align2::CENTER_TOP,
                        display_char.to_string(),
                        font_id,
                        cursor_fg_color,
                    );
                }
                
                // Draw a subtle border around the cursor for better definition
                ui.painter().rect_stroke(
                    Rect::from_min_size(
                        cursor_pos,
                        Vec2::new(cursor_width, char_height),
                    ),
                    egui::Rounding::ZERO,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(128, 128, 128)),
                    egui::epaint::StrokeKind::Outside,
                );
            }
        }
    }
    
    /// Convert terminal cursor position to egui screen coordinates
    fn terminal_cursor_to_screen_pos(
        terminal: &egui_term::TerminalBackend,
        terminal_rect: Rect,
    ) -> Pos2 {
        let content = terminal.last_content();
        let cursor_point = content.grid.cursor.point;
        let terminal_size = &content.terminal_size;
        
        // Calculate character cell size
        let cell_width = terminal_size.cell_width;
        let cell_height = terminal_size.cell_height;
        
        // Convert terminal grid coordinates to screen coordinates
        let x = terminal_rect.min.x + (cursor_point.column.0 as f32 * cell_width as f32);
        let y = terminal_rect.min.y + (cursor_point.line.0 as f32 * cell_height as f32);
        
        Pos2::new(x, y)
    }
}
