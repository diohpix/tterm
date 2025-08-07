use crate::types::{AppState, ViewMode};
use egui::Rect;

pub struct GridManager;

impl GridManager {
    /// Calculate optimal grid size based on tab count
    pub fn calculate_optimal_grid_size(tab_count: usize) -> (usize, usize) {
        if tab_count <= 1 {
            return (1, 1);
        }
        
        match tab_count {
            2 => (1, 2), // 1x2 for 2 tabs (horizontal)
            3 => (2, 2), // 2x2 for 3 tabs (3rd tab spans full width on bottom)
            4 => (2, 2), // 2x2 for 4 tabs (perfect fit)
            5..=6 => (2, 3), // 2x3 for 5-6 tabs
            7..=9 => (3, 3), // 3x3 for 7-9 tabs
            _ => {
                // For larger numbers, use sqrt calculation
                let cols = (tab_count as f32).sqrt().ceil() as usize;
                let rows = (tab_count as f32 / cols as f32).ceil() as usize;
                (rows.max(1), cols.max(1))
            }
        }
    }
    
    /// Update grid size based on current tab count
    pub fn update_grid_size(state: &mut AppState) {
        if let ViewMode::Grid { .. } = state.view_mode {
            let tab_count = state.tabs.len();
            if tab_count <= 1 {
                // If only one tab left, switch to single view to fill the screen
                state.view_mode = ViewMode::Single;
            } else {
                let (rows, cols) = Self::calculate_optimal_grid_size(tab_count);
                // Create uniform ratios for new grid size
                let col_ratios = vec![1.0 / cols as f32; cols];
                let row_ratios = vec![1.0 / rows as f32; rows];
                state.view_mode = ViewMode::Grid { rows, cols, col_ratios, row_ratios };
            }
        }
    }
    
    /// Toggle between single and grid view
    pub fn toggle_grid_view(state: &mut AppState) {
        state.view_mode = match state.view_mode {
            ViewMode::Single => {
                // Don't switch to grid view if only one tab exists
                if state.tabs.len() <= 1 {
                    return; // Stay in single view
                }
                
                let (rows, cols) = Self::calculate_optimal_grid_size(state.tabs.len());
                let col_ratios = vec![1.0 / cols as f32; cols];
                let row_ratios = vec![1.0 / rows as f32; rows];
                ViewMode::Grid { rows, cols, col_ratios, row_ratios }
            }
            ViewMode::Grid { .. } => ViewMode::Single,
        };
    }
    
    /// Calculate grid cell rectangle for a given tab index
    pub fn calculate_cell_rect(
        available_rect: Rect,
        rows: usize,
        cols: usize,
        col_ratios: &[f32],
        row_ratios: &[f32],
        tab_count: usize,
        tab_index: usize,
    ) -> Rect {
        let row = tab_index / cols;
        let col = tab_index % cols;
        
        // Calculate cumulative positions for grid cells
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
        
        // Calculate cell position using ratios
        let cell_x = available_rect.min.x + col_positions[col];
        let cell_y = available_rect.min.y + row_positions[row];
        let cell_width = col_positions[col + 1] - col_positions[col];
        let cell_height = row_positions[row + 1] - row_positions[row];
        
        // Special handling for 3 tabs in 2x2 grid: 3rd tab spans full width
        if tab_count == 3 && rows == 2 && cols == 2 && tab_index == 2 {
            // Third tab spans full width on bottom row
            Rect::from_min_size(
                egui::pos2(available_rect.min.x, available_rect.min.y + row_positions[1]),
                egui::vec2(available_rect.width() - 4.0, row_positions[2] - row_positions[1] - 4.0),
            )
        } else {
            // Normal grid cell with separator gaps
            Rect::from_min_size(
                egui::pos2(cell_x + 2.0, cell_y + 2.0),
                egui::vec2(cell_width - 4.0, cell_height - 4.0),
            )
        }
    }
    
    /// Calculate column separator rectangle
    pub fn calculate_column_separator_rect(
        available_rect: Rect,
        rows: usize,
        cols: usize,
        col_positions: &[f32],
        row_positions: &[f32],
        tab_count: usize,
        col: usize,
    ) -> Rect {
        let sep_x = available_rect.min.x + col_positions[col + 1];
        
        // Special handling for 3 or fewer tabs: don't draw column separator in bottom row where tab spans full width
        if tab_count <= 3 && cols == 2 && rows == 2 {
            // For 3 tabs in 2x2, only draw separator in top row
            let bottom_row_y = available_rect.min.y + row_positions[1];
            Rect::from_min_max(
                egui::pos2(sep_x - 2.0, available_rect.min.y),
                egui::pos2(sep_x + 2.0, bottom_row_y),
            )
        } else {
            Rect::from_min_max(
                egui::pos2(sep_x - 2.0, available_rect.min.y),
                egui::pos2(sep_x + 2.0, available_rect.max.y),
            )
        }
    }
    
    /// Calculate row separator rectangle
    pub fn calculate_row_separator_rect(
        available_rect: Rect,
        row_positions: &[f32],
        row: usize,
    ) -> Rect {
        let sep_y = available_rect.min.y + row_positions[row + 1];
        Rect::from_min_max(
            egui::pos2(available_rect.min.x, sep_y - 2.0),
            egui::pos2(available_rect.max.x, sep_y + 2.0),
        )
    }
    
    /// Handle column separator drag
    pub fn handle_column_separator_drag(
        col_ratios: &mut [f32],
        available_rect: Rect,
        pointer_x: f32,
        col: usize,
        cols: usize,
    ) {
        let new_x = pointer_x - available_rect.min.x;
        let total_width = available_rect.width();
        
        // Calculate new ratios
        let min_ratio = 0.05; // Minimum 5% width
        let left_ratio = (new_x / total_width).clamp(min_ratio, 1.0 - min_ratio * (cols - col - 1) as f32);
        
        // Adjust ratios
        let old_left_total: f32 = col_ratios.iter().take(col + 1).sum();
        let old_right_total: f32 = col_ratios.iter().skip(col + 1).sum();
        
        if old_left_total > 0.0 && old_right_total > 0.0 {
            let new_right_total = 1.0 - left_ratio;
            
            // Scale left ratios
            for i in 0..=col {
                col_ratios[i] = col_ratios[i] * left_ratio / old_left_total;
            }
            
            // Scale right ratios
            for i in (col + 1)..cols {
                col_ratios[i] = col_ratios[i] * new_right_total / old_right_total;
            }
        }
    }
    
    /// Handle row separator drag
    pub fn handle_row_separator_drag(
        row_ratios: &mut [f32],
        available_rect: Rect,
        pointer_y: f32,
        row: usize,
        rows: usize,
    ) {
        let new_y = pointer_y - available_rect.min.y;
        let total_height = available_rect.height();
        
        // Calculate new ratios
        let min_ratio = 0.05; // Minimum 5% height
        let top_ratio = (new_y / total_height).clamp(min_ratio, 1.0 - min_ratio * (rows - row - 1) as f32);
        
        // Adjust ratios
        let old_top_total: f32 = row_ratios.iter().take(row + 1).sum();
        let old_bottom_total: f32 = row_ratios.iter().skip(row + 1).sum();
        
        if old_top_total > 0.0 && old_bottom_total > 0.0 {
            let new_bottom_total = 1.0 - top_ratio;
            
            // Scale top ratios
            for i in 0..=row {
                row_ratios[i] = row_ratios[i] * top_ratio / old_top_total;
            }
            
            // Scale bottom ratios
            for i in (row + 1)..rows {
                row_ratios[i] = row_ratios[i] * new_bottom_total / old_bottom_total;
            }
        }
    }
}
