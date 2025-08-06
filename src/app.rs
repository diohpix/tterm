use egui::{Vec2, Ui, Rect};
use egui_term::{PtyEvent, TerminalBackend, TerminalView};
use std::sync::mpsc::{Receiver, Sender};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub enum ViewMode {
    Single,
    Grid { rows: usize, cols: usize },
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

pub struct TerminalTab {
    pub id: u64,
    pub title: String,
}

pub struct App {
    tabs: HashMap<u64, TerminalTab>,
    tab_order: Vec<u64>, // Maintain tab order
    active_tab_id: u64,
    next_tab_id: u64,
    terminals: HashMap<u64, TerminalBackend>, // All terminal backends
    next_terminal_id: u64,
    tab_layouts: HashMap<u64, PanelContent>, // Layout for each tab
    view_mode: ViewMode,
    focused_terminal: Option<u64>,
    // Broadcasting
    broadcast_mode: bool,
    selected_terminals: std::collections::HashSet<u64>, // Terminals to broadcast to
    pty_proxy_receiver: Receiver<(u64, egui_term::PtyEvent)>,
    pty_proxy_sender: Sender<(u64, egui_term::PtyEvent)>,
    egui_ctx: egui::Context,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (pty_proxy_sender, pty_proxy_receiver) = std::sync::mpsc::channel();
        let egui_ctx = cc.egui_ctx.clone();
        
        let mut app = Self {
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
            pty_proxy_receiver,
            pty_proxy_sender,
            egui_ctx,
        };
        
        // Create the first tab
        app.create_new_tab();
        
        app
    }
    
    fn create_new_tab(&mut self) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        
        // Create a terminal for this tab
        let terminal_id = self.create_terminal();
        
        let tab = TerminalTab {
            id: tab_id,
            title: format!("Terminal {}", tab_id),
        };
        
        // Set up the layout with a single terminal
        let layout = PanelContent::Terminal(terminal_id);
        
        self.tabs.insert(tab_id, tab);
        self.tab_order.push(tab_id); // Maintain order
        self.tab_layouts.insert(tab_id, layout);
        self.active_tab_id = tab_id;
        self.focused_terminal = Some(terminal_id);
    }
    
    fn create_terminal(&mut self) -> u64 {
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
        terminal_id
    }
    
    fn close_tab(&mut self, tab_id: u64) {
        if self.tabs.len() > 1 {
            // Remove associated terminals
            if let Some(layout) = self.tab_layouts.remove(&tab_id) {
                self.collect_terminal_ids(&layout).into_iter().for_each(|tid| {
                    self.terminals.remove(&tid);
                });
            }
            
            self.tabs.remove(&tab_id);
            self.tab_order.retain(|&id| id != tab_id); // Remove from order
            
            // If we closed the active tab, switch to another one
            if self.active_tab_id == tab_id {
                if let Some(&next_tab_id) = self.tab_order.first() {
                    self.active_tab_id = next_tab_id;
                    // Update focused terminal
                    if let Some(layout) = self.tab_layouts.get(&self.active_tab_id) {
                        self.focused_terminal = self.get_first_terminal_id(layout);
                    }
                }
            }
        }
    }
    
    fn collect_terminal_ids(&self, content: &PanelContent) -> Vec<u64> {
        match content {
            PanelContent::Terminal(id) => vec![*id],
            PanelContent::Split { first, second, .. } => {
                let mut ids = self.collect_terminal_ids(first);
                ids.extend(self.collect_terminal_ids(second));
                ids
            }
        }
    }
    
    fn get_first_terminal_id(&self, content: &PanelContent) -> Option<u64> {
        match content {
            PanelContent::Terminal(id) => Some(*id),
            PanelContent::Split { first, .. } => self.get_first_terminal_id(first),
        }
    }
    
    fn render_tab_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let mut tab_to_close = None;
            let mut tab_to_activate = None;
            
            // Use tab_order to maintain consistent order
            for &tab_id in &self.tab_order {
                if let Some(tab) = self.tabs.get(&tab_id) {
                    let is_active = tab_id == self.active_tab_id;
                    
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let tab_response = ui.selectable_label(is_active, &tab.title);
                            
                            if tab_response.clicked() {
                                tab_to_activate = Some(tab_id);
                            }
                            
                            // Close button
                            if ui.small_button("×").clicked() && self.tabs.len() > 1 {
                                tab_to_close = Some(tab_id);
                            }
                        });
                    });
                }
            }
            
            // New tab button
            if ui.button("+").clicked() {
                self.create_new_tab();
            }
            
            // Handle tab activation outside the closure
            if let Some(tab_id) = tab_to_activate {
                self.active_tab_id = tab_id;
                // Update focus to the first terminal in this tab
                if let Some(layout) = self.tab_layouts.get(&tab_id) {
                    self.focused_terminal = self.get_first_terminal_id(layout);
                }
            }
            
            if let Some(tab_id) = tab_to_close {
                self.close_tab(tab_id);
            }
        });
    }
    
    fn split_focused_panel(&mut self, direction: SplitDirection) {
        if let Some(focused_terminal_id) = self.focused_terminal {
            let new_terminal_id = self.create_terminal();
            let active_tab_id = self.active_tab_id;
            
            if let Some(layout) = self.tab_layouts.get_mut(&active_tab_id) {
                Self::replace_terminal_with_split_static(layout, focused_terminal_id, direction, new_terminal_id);
                self.focused_terminal = Some(new_terminal_id);
            }
        }
    }
    
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
    
    fn render_panel_content(&mut self, ui: &mut Ui, content: &PanelContent, available_rect: Rect) {
        match content {
            PanelContent::Terminal(terminal_id) => {
                if let Some(terminal_backend) = self.terminals.get_mut(terminal_id) {
                    let is_focused = self.focused_terminal == Some(*terminal_id);
                    
                    // Add visual focus indicator and broadcast selection
                    let border_color = if is_focused {
                        egui::Color32::from_rgb(0, 150, 255) // Blue for focused
                    } else if self.broadcast_mode && self.selected_terminals.contains(terminal_id) {
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
                    
                    let terminal = TerminalView::new(ui, terminal_backend)
                        .set_focus(is_focused)
                        .set_size(Vec2::new(available_rect.width(), available_rect.height()));
                    
                    // Render terminal and check for clicks
                    ui.scope_builder(egui::UiBuilder::new().max_rect(available_rect), |ui| {
                        ui.add(terminal)
                    });
                    
                    // Check if the terminal area was clicked
                    if ui.input(|i| i.pointer.any_click()) {
                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            if available_rect.contains(pos) {
                                self.focused_terminal = Some(*terminal_id);
                            }
                        }
                    }
                }
            }
            PanelContent::Split { direction, first, second, ratio, .. } => {
                match direction {
                    SplitDirection::Horizontal => {
                        let split_y = available_rect.min.y + available_rect.height() * ratio;
                        
                        let first_rect = Rect::from_min_max(
                            available_rect.min,
                            egui::pos2(available_rect.max.x, split_y - 1.0),
                        );
                        
                        let second_rect = Rect::from_min_max(
                            egui::pos2(available_rect.min.x, split_y + 1.0),
                            available_rect.max,
                        );
                        
                        self.render_panel_content(ui, first, first_rect);
                        
                        // Draw separator
                        ui.painter().line_segment(
                            [egui::pos2(available_rect.min.x, split_y), egui::pos2(available_rect.max.x, split_y)],
                            egui::Stroke::new(1.0, egui::Color32::GRAY),
                        );
                        
                        self.render_panel_content(ui, second, second_rect);
                    }
                    SplitDirection::Vertical => {
                        let split_x = available_rect.min.x + available_rect.width() * ratio;
                        
                        let first_rect = Rect::from_min_max(
                            available_rect.min,
                            egui::pos2(split_x - 1.0, available_rect.max.y),
                        );
                        
                        let second_rect = Rect::from_min_max(
                            egui::pos2(split_x + 1.0, available_rect.min.y),
                            available_rect.max,
                        );
                        
                        self.render_panel_content(ui, first, first_rect);
                        
                        // Draw separator
                        ui.painter().line_segment(
                            [egui::pos2(split_x, available_rect.min.y), egui::pos2(split_x, available_rect.max.y)],
                            egui::Stroke::new(1.0, egui::Color32::GRAY),
                        );
                        
                        self.render_panel_content(ui, second, second_rect);
                    }
                }
            }
        }
    }
    
    fn toggle_grid_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Single => {
                // Don't switch to grid view if only one tab exists
                if self.tabs.len() <= 1 {
                    return; // Stay in single view
                }
                
                // Calculate optimal grid size based on number of tabs
                let tab_count = self.tabs.len();
                let cols = (tab_count as f32).sqrt().ceil() as usize;
                let rows = (tab_count as f32 / cols as f32).ceil() as usize;
                ViewMode::Grid { rows: rows.max(1), cols: cols.max(1) }
            }
            ViewMode::Grid { .. } => ViewMode::Single,
        };
    }
    
    fn render_grid_view(&mut self, ui: &mut Ui, available_rect: Rect) {
        if let ViewMode::Grid { rows, cols } = self.view_mode {
            let cell_width = available_rect.width() / cols as f32;
            let cell_height = available_rect.height() / rows as f32;
            
            // Get all tabs for grid view (maintain split layouts per tab)
            let mut tab_ids: Vec<_> = self.tab_order.clone();
            tab_ids.truncate(rows * cols); // Don't render more than grid capacity
            
            for (idx, &tab_id) in tab_ids.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                
                let cell_rect = Rect::from_min_size(
                    egui::pos2(
                        available_rect.min.x + col as f32 * cell_width,
                        available_rect.min.y + row as f32 * cell_height,
                    ),
                    egui::vec2(cell_width - 2.0, cell_height - 2.0), // Small gap between cells
                );
                
                // Get tab layout and render it
                if let Some(tab) = self.tabs.get(&tab_id) {
                    if let Some(layout) = self.tab_layouts.get(&tab_id).cloned() {
                        // Draw tab border
                        let is_active_tab = tab_id == self.active_tab_id;
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
                                    self.active_tab_id = tab_id;
                                    self.focused_terminal = self.get_first_terminal_id(&layout);
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
                        
                        // Render the tab's layout (including splits) in the content area
                        self.render_panel_content(ui, &layout, content_rect);
                    }
                }
            }
        }
    }
    
    fn toggle_broadcast_mode(&mut self) {
        self.broadcast_mode = !self.broadcast_mode;
        
        if self.broadcast_mode {
            // Start with all terminals selected
            self.selected_terminals = self.terminals.keys().cloned().collect();
        } else {
            self.selected_terminals.clear();
        }
    }
    
    fn toggle_terminal_selection(&mut self, terminal_id: u64) {
        if self.selected_terminals.contains(&terminal_id) {
            self.selected_terminals.remove(&terminal_id);
        } else {
            self.selected_terminals.insert(terminal_id);
        }
    }
    
    fn broadcast_input(&mut self, input: &str) {
        if self.broadcast_mode {
            for &terminal_id in &self.selected_terminals {
                if let Some(terminal) = self.terminals.get_mut(&terminal_id) {
                    // Send input to terminal
                    terminal.process_command(egui_term::BackendCommand::Write(input.as_bytes().to_vec()));
                }
            }
        }
    }
    
    fn navigate_focus_in_splits(&mut self) {
        if let Some(layout) = self.tab_layouts.get(&self.active_tab_id) {
            let terminal_ids = self.collect_terminal_ids(layout);
            
            if terminal_ids.len() <= 1 {
                return; // No navigation needed with only one terminal
            }
            
            if let Some(current_focused) = self.focused_terminal {
                if let Some(current_idx) = terminal_ids.iter().position(|&id| id == current_focused) {
                    // Move to next terminal (cycling back to first if at end)
                    let next_idx = (current_idx + 1) % terminal_ids.len();
                    self.focused_terminal = Some(terminal_ids[next_idx]);
                } else {
                    // If current focus is not in this tab, focus on first terminal
                    self.focused_terminal = terminal_ids.first().copied();
                }
            } else {
                // No current focus, focus on first terminal
                self.focused_terminal = terminal_ids.first().copied();
            }
        }
    }
    
    fn render_status_bar(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Broadcast status
            if self.broadcast_mode {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "☀ BROADCAST");
                ui.label(format!("({} terminals)", self.selected_terminals.len()));
            } else {
                ui.label("Single input");
            }
            
            ui.separator();
            
            // View mode status
            match self.view_mode {
                ViewMode::Single => ui.label("Single view"),
                ViewMode::Grid { rows, cols } => ui.label(format!("Grid {}x{}", rows, cols)),
            };
            
            ui.separator();
            
            // Focused terminal
            if let Some(focused) = self.focused_terminal {
                ui.label(format!("Focus: Terminal {}", focused));
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle PTY events
        while let Ok((tab_id, event)) = self.pty_proxy_receiver.try_recv() {
            match event {
                PtyEvent::Exit => {
                    if self.tabs.len() == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
                    } else {
                        self.close_tab(tab_id);
                    }
                }
                _ => {}
            }
        }
        
        // Handle keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.mac_cmd {
                if i.key_pressed(egui::Key::T) {
                    self.create_new_tab();
                }
                if i.key_pressed(egui::Key::W) && self.tabs.len() > 1 {
                    self.close_tab(self.active_tab_id);
                }
                // Tab switching with Ctrl+1-9
                for (idx, &key) in [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9].iter().enumerate() {
                    if i.key_pressed(key) {
                        if idx < self.tab_order.len() {
                            self.active_tab_id = self.tab_order[idx];
                            // Update focus to the first terminal in this tab
                            if let Some(layout) = self.tab_layouts.get(&self.active_tab_id) {
                                self.focused_terminal = self.get_first_terminal_id(layout);
                            }
                        }
                    }
                }
                
                // Split shortcuts
                if i.modifiers.shift {
                    
                    if i.key_pressed(egui::Key::D) {
                        self.split_focused_panel(SplitDirection::Horizontal);
                    }
                }else{
                    if i.key_pressed(egui::Key::D) {
                        self.split_focused_panel(SplitDirection::Vertical);
                    }
                }
                
                // Grid view toggle (Ctrl+S as per PRD)
                if i.key_pressed(egui::Key::S) {
                    self.toggle_grid_view();
                }
                
                // Broadcast shortcuts
                if i.modifiers.shift {
                    if i.key_pressed(egui::Key::B) {
                        self.toggle_broadcast_mode();
                    }
                    if i.key_pressed(egui::Key::A) {
                        // Toggle all terminals selection
                        if self.broadcast_mode {
                            if self.selected_terminals.len() == self.terminals.len() {
                                self.selected_terminals.clear();
                            } else {
                                self.selected_terminals = self.terminals.keys().cloned().collect();
                            }
                        }
                    }
                }
            }
            
            // Panel navigation with Alt+Arrow keys
            if i.modifiers.alt {
                if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight) ||
                   i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowDown) {
                    self.navigate_focus_in_splits();
                }
            }
        });

        // Top panel for tabs (only show in single mode)
        if matches!(self.view_mode, ViewMode::Single) {
            egui::TopBottomPanel::top("tab_panel").show(ctx, |ui| {
                self.render_tab_bar(ui);
            });
        }
        
        // Bottom panel for status
        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // Main terminal area
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();
            
            match self.view_mode {
                ViewMode::Single => {
                    if let Some(layout) = self.tab_layouts.get(&self.active_tab_id).cloned() {
                        self.render_panel_content(ui, &layout, available_rect);
                    }
                }
                ViewMode::Grid { .. } => {
                    self.render_grid_view(ui, available_rect);
                }
            }
        });
    }
}
