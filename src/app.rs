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
        Self::new_with_session(cc, None)
    }
    
    pub fn new_with_session(cc: &eframe::CreationContext<'_>, attach_session_id: Option<uuid::Uuid>) -> Self {
        log::info!("🚀 TTerminal app starting up...");
        
        // Load and configure Korean fonts
        log::info!("⚙️ Configuring Korean fonts...");
        Self::configure_korean_fonts(&cc.egui_ctx);
        
        log::info!("🏗️ Creating AppState...");
        let mut state = AppState::new(cc);
        
        if let Some(session_id) = attach_session_id {
            // Attempt to attach to existing session
            let egui_ctx = cc.egui_ctx.clone();
            tokio::spawn(async move {
                // This is a simplified approach - in practice, we'd need better state management
                log::info!("Attempting to attach to session: {:?}", session_id);
                egui_ctx.request_repaint();
            });
        } else {
            // Create the first tab for new instance
            log::info!("📑 Creating first tab...");
            TabManager::create_new_tab(&mut state);
        }
        
        log::info!("✅ TTerminal app created successfully");
        
        Self { state }
    }
    
    fn configure_korean_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        
        // Load D2Coding font data
        let d2coding_font_data = include_bytes!("../fonts/D2Coding.ttf");
        
        // Insert D2Coding font
        fonts.font_data.insert(
            "D2Coding".to_owned(),
            egui::FontData::from_static(d2coding_font_data).into(),
        );
        
        // Insert D2Coding at the front of monospace fonts
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap()
            .insert(0, "D2Coding".to_owned());
        
        // Also add to proportional for UI text that might contain Korean
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
            .insert(0, "D2Coding".to_owned());
        
        // Apply font configuration
        ctx.set_fonts(fonts);
    }
    
    /// Handle daemon terminal output
    fn handle_daemon_output(&mut self) {
        // For now, we'll implement a simple approach
        // TODO: Implement proper daemon output handling
        // This would require setting up a receiver for daemon output
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        
    
        // Initialize with first tab if none exist
        if self.state.tabs.is_empty() {
            log::info!("🆘 No tabs exist, creating first tab in update()");
            TabManager::create_new_tab(&mut self.state);
        }
        
        // Check if we should close
        if ctx.input(|i| i.viewport().close_requested()) {
            log::info!("🚪 Close requested by user");
        }
        
        // Check if any terminals have exited
        let terminal_count = self.state.terminal_manager.get_terminal_count();
        let connecting_count = self.state.connecting_terminals.len();
        
        // Only force close if there are tabs but no terminals AND no connecting terminals
        if terminal_count == 0 && connecting_count == 0 && !self.state.tabs.is_empty() {
            log::error!("💀 All terminals died but tabs still exist! Force closing.");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        
        // Handle pending tab creation (disabled for now)
        // if let Some(tab_id) = self.state.pending_tab_creation.take() {
        //     // Try to create daemon terminal, fallback to local
        //     let terminal_id = self.state.create_terminal();
        //     
        //     // Update tab title to remove loading state
        //     if let Some(tab) = self.state.tabs.get_mut(&tab_id) {
        //         tab.title = format!("Terminal {}", tab_id);
        //     }
        //     
        //     // Set up the layout with the created terminal
        //     let layout = crate::types::PanelContent::Terminal(terminal_id);
        //     self.state.tab_layouts.insert(tab_id, layout);
        //     self.state.focused_terminal = Some(terminal_id);
        //     
        //     // Update grid size after tab creation
        //     crate::grid_manager::GridManager::update_grid_size(&mut self.state);
        // }
        
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
        
        // Process daemon connection results
        self.state.process_daemon_connection_results();
        
        // Handle daemon output
        self.handle_daemon_output();
        
        // Handle PTY events
        while let Ok((terminal_id, event)) = self.state.pty_proxy_receiver.try_recv() {
            log::debug!("📥 PTY event received for terminal {}: {:?}", terminal_id, event);
            match event {
                PtyEvent::Exit => {
                    log::warn!("💀 Terminal {} exited!", terminal_id);
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
