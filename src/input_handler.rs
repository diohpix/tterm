use crate::types::{AppState, SplitDirection};
use crate::tab_manager::TabManager;
use crate::split_manager::SplitManager;
use crate::grid_manager::GridManager;
use crate::broadcast_manager::BroadcastManager;
use crate::ime::korean::{KoreanInputState, is_consonant, is_vowel};
use egui::{Key, Modifiers};
use egui_term::BackendCommand;

pub struct InputHandler;

impl InputHandler {
    /// Handle all keyboard shortcuts and input
    pub fn handle_input(state: &mut AppState, ctx: &egui::Context) -> bool {
        let mut handled_by_shortcuts = false;
        
        // Handle Tab key specifically for terminal when focused
        ctx.input(|i| {
            if i.key_pressed(Key::Tab) && state.focused_terminal.is_some() {
                // Send Tab directly to focused terminal, bypass UI focus system
                Self::send_tab_to_focused_terminal(state);
                handled_by_shortcuts = true;
            }
        });
        
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
                        } else {
                            // Handle Korean input composition first
                            let final_text = Self::handle_korean_input(state, focused_terminal_id, &text);
                            
                            // Then send to terminal
                            if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                terminal.process_command(BackendCommand::Write(final_text.as_bytes().to_vec()));
                            }
                        }
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        // Handle keys that should finalize Korean composition
                        match key {
                            Key::Enter => {
                                // Finalize any pending Korean composition before sending Enter
                                Self::finalize_korean_composition(state, focused_terminal_id);
                                if broadcast_mode {
                                    BroadcastManager::broadcast_input(state, "\n");
                                } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                    terminal.process_command(BackendCommand::Write(b"\n".to_vec()));
                                }
                            }
                            Key::Space => {
                                // Space is handled by Text event, don't handle it here to avoid duplication
                                // Just finalize composition if active
                                Self::finalize_korean_composition(state, focused_terminal_id);
                            }
                            Key::Backspace => {
                                // Handle backspace for Korean composition
                                if let Some(korean_state) = state.korean_input_states.get_mut(&focused_terminal_id) {
                                    if korean_state.is_composing {
                                        korean_state.handle_backspace();
                                        continue; // Don't send backspace to terminal, just update overlay
                                    }
                                }
                                
                                // If not handled by Korean IME, fall through to normal key processing
                                if let Some(bytes) = Self::key_to_bytes(&key, &modifiers) {
                                    if broadcast_mode {
                                        BroadcastManager::broadcast_input(state, &String::from_utf8_lossy(&bytes));
                                    } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                        terminal.process_command(BackendCommand::Write(bytes));
                                    }
                                }
                            }
                            Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight => {
                                // Arrow keys should finalize Korean composition
                                Self::finalize_korean_composition(state, focused_terminal_id);
                                if let Some(bytes) = Self::key_to_bytes(&key, &modifiers) {
                                    if broadcast_mode {
                                        BroadcastManager::broadcast_input(state, &String::from_utf8_lossy(&bytes));
                                    } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                        terminal.process_command(BackendCommand::Write(bytes));
                                    }
                                }
                            }
                            Key::Escape => {
                                // ESC should finalize Korean composition
                                Self::finalize_korean_composition(state, focused_terminal_id);
                                if let Some(bytes) = Self::key_to_bytes(&key, &modifiers) {
                                    if broadcast_mode {
                                        BroadcastManager::broadcast_input(state, &String::from_utf8_lossy(&bytes));
                                    } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                        terminal.process_command(BackendCommand::Write(bytes));
                                    }
                                }
                            }
                            _ => {
                                // For other keys, handle normally
                                if let Some(bytes) = Self::key_to_bytes(&key, &modifiers) {
                                    if broadcast_mode {
                                        BroadcastManager::broadcast_input(state, &String::from_utf8_lossy(&bytes));
                                    } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                                        terminal.process_command(BackendCommand::Write(bytes));
                                    }
                                }
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
            // Tab is handled separately to bypass UI focus system
            Key::Tab => None,
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
    
    /// Send Tab key directly to focused terminal
    fn send_tab_to_focused_terminal(state: &mut AppState) {
        if let Some(focused_terminal_id) = state.focused_terminal {
            let broadcast_mode = BroadcastManager::is_broadcast_mode_active(state);
            
            if broadcast_mode {
                BroadcastManager::broadcast_input(state, "\t");
            } else if let Some(terminal) = state.terminals.get_mut(&focused_terminal_id) {
                let tab_bytes = b"\t".to_vec();
                terminal.process_command(BackendCommand::Write(tab_bytes));
            }
        }
    }
    
    /// Handle Korean input composition and return the final text to send to terminal
    fn handle_korean_input(state: &mut AppState, terminal_id: u64, input_text: &str) -> String {
        // Get or create Korean input state for this terminal
        let korean_state = state.korean_input_states.get_mut(&terminal_id)
            .expect("Korean input state should exist for terminal");
        
        let mut result = String::new();
        
        for ch in input_text.chars() {
            // Check if this is a Korean jamo (consonant or vowel)
            if is_consonant(ch) || is_vowel(ch) {
                // Process the Korean character
                let completed_chars = Self::process_korean_char(korean_state, ch);
                
                // Only send completed characters to terminal
                // Composing characters will be shown via overlay
                result.push_str(&completed_chars);
            } else {
                // Non-Korean character - commit any pending composition and add the character
                if korean_state.is_composing {
                    if let Some(composed) = korean_state.get_current_char() {
                        result.push(composed);
                    }
                    korean_state.reset();
                }
                result.push(ch);
            }
        }
        
        result
    }
    
    /// Finalize any pending Korean composition and send to terminal
    fn finalize_korean_composition(state: &mut AppState, terminal_id: u64) {
        if let Some(korean_state) = state.korean_input_states.get_mut(&terminal_id) {
            if korean_state.is_composing {
                if let Some(terminal) = state.terminals.get_mut(&terminal_id) {
                    // Send the finalized character to terminal
                    if let Some(completed) = korean_state.get_current_char() {
                        terminal.process_command(BackendCommand::Write(completed.to_string().as_bytes().to_vec()));
                    }
                }
                korean_state.reset();
            }
        }
    }
    
    /// Process a single Korean character and return the result (improved wterm-style logic)
    fn process_korean_char(korean_state: &mut KoreanInputState, ch: char) -> String {
        use crate::ime::korean::*;
        
        let mut result = String::new();
        
        if is_consonant(ch) {
            if korean_state.chosung.is_none() {
                // First consonant - set as chosung, start composing
                korean_state.chosung = Some(ch);
                korean_state.is_composing = true;
            } else if korean_state.jungsung.is_some() && korean_state.jongsung.is_none() {
                // We have chosung + jungsung, this consonant becomes jongsung
                korean_state.jongsung = Some(ch);
            } else if let Some(existing_jong) = korean_state.jongsung {
                // Try to combine with existing jongsung (복합 자음)
                if let Some(combined) = combine_consonants(existing_jong, ch) {
                    korean_state.jongsung = Some(combined);
                } else {
                    // Can't combine - complete current syllable and start new one
                    if let Some(completed) = korean_state.get_current_char() {
                        result.push(completed);
                    }
                    korean_state.reset();
                    korean_state.chosung = Some(ch);
                    korean_state.is_composing = true;
                }
            } else {
                // Already have chosung but no jungsung - complete current and start new
                if let Some(completed) = korean_state.get_current_char() {
                    result.push(completed);
                }
                korean_state.reset();
                korean_state.chosung = Some(ch);
                korean_state.is_composing = true;
            }
        } else if is_vowel(ch) {
            if korean_state.chosung.is_some() && korean_state.jungsung.is_none() {
                // We have chosung, this vowel becomes jungsung
                korean_state.jungsung = Some(ch);
            } else if let Some(existing_jung) = korean_state.jungsung {
                // Check if we have jongsung - if so, we need to move it to new syllable
                if let Some(jong) = korean_state.jongsung {
                    // Complete current syllable without the jongsung (ㄱㅏㄴ->ㄱㅏ완성, ㄴㅏ시작)
                    let cho_idx = get_chosung_index(korean_state.chosung.unwrap()).unwrap();
                    let jung_idx = get_jungsung_index(existing_jung).unwrap();
                    let completed = compose_korean(cho_idx, jung_idx, 0); // No jongsung
                    result.push(completed);
                    
                    // Start new syllable with jongsung as chosung
                    korean_state.reset();
                    korean_state.chosung = Some(jong);
                    korean_state.jungsung = Some(ch);
                    korean_state.is_composing = true;
                } else {
                    // Try to combine with existing jungsung (복합 모음)
                    if let Some(combined) = combine_vowels(existing_jung, ch) {
                        korean_state.jungsung = Some(combined);
                    } else {
                        // Can't combine - complete current syllable
                        if let Some(completed) = korean_state.get_current_char() {
                            result.push(completed);
                        }
                        korean_state.reset();
                        // Vowel can't start a new syllable without consonant, so just send it
                        result.push(ch);
                    }
                }
            } else {
                // No chosung yet - vowel can't start syllable, just send it
                result.push(ch);
            }
        }
        
        result
    }
}
