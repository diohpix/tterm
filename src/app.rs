use crate::types::{AppState, ViewMode};
use crate::tab_manager::TabManager;
use crate::split_manager::SplitManager;
use crate::input_handler::InputHandler;
use crate::ui_renderer::UiRenderer;
use egui_term::PtyEvent;

pub struct App {
    state: AppState,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut state = AppState::new(cc);
        
        // Create the first tab
        TabManager::create_new_tab(&mut state);
        
        Self { state }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Consume Tab key events before egui can process them for UI focus
        // This ensures Tab keys go to terminals, not UI navigation
        if self.state.focused_terminal.is_some() {
            ctx.input(|i| {
                // This consumes the Tab key event, preventing default UI focus behavior
                if i.key_pressed(egui::Key::Tab) {
                    // Event is consumed, no action needed here as it's handled in InputHandler
                }
            });
        }
        
        // Handle PTY events
        while let Ok((terminal_id, event)) = self.state.pty_proxy_receiver.try_recv() {
            match event {
                PtyEvent::Exit => {
                    SplitManager::handle_terminal_exit(&mut self.state, terminal_id, ctx);
                }
                _ => {}
            }
        }
        
        // Handle keyboard shortcuts and input
        InputHandler::handle_input(&mut self.state, ctx);

        // Top panel for tabs (only show in single mode)
        if matches!(self.state.view_mode, ViewMode::Single) {
            egui::TopBottomPanel::top("tab_panel").show(ctx, |ui| {
                UiRenderer::render_tab_bar(&mut self.state, ui);
            });
        }
        
        // Bottom panel for status
        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            UiRenderer::render_status_bar(&self.state, ui);
        });

        // Main terminal area
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();
            
            match self.state.view_mode {
                ViewMode::Single => {
                    let active_tab_id = self.state.active_tab_id;
                    if let Some(layout) = self.state.tab_layouts.get(&active_tab_id).cloned() {
                        let mut layout_copy = layout;
                        UiRenderer::render_panel_content(&mut self.state, ui, &mut layout_copy, available_rect);
                        // Update the layout if it was modified (for resize)
                        self.state.tab_layouts.insert(active_tab_id, layout_copy);
                    }
                }
                ViewMode::Grid { .. } => {
                    UiRenderer::render_grid_view(&mut self.state, ui, available_rect);
                }
            }
        });
    }
}
