# TTerminal

A modern terminal emulator built with Rust and egui, featuring advanced productivity features like tabs, grid view, split panes, and input broadcasting.

## Features

- 🗂️ **Multi-tab Support**: Unlimited terminal tabs with easy switching
- 🔳 **Grid View**: Display multiple terminals simultaneously in a grid layout
- ✂️ **Split Panes**: Recursive horizontal and vertical terminal splitting
- 📢 **Input Broadcasting**: Send input to multiple terminals at once
- ⚡ **High Performance**: Built with Rust for speed and reliability
- 🎨 **Modern UI**: Clean interface powered by egui
- 🔧 **Highly Configurable**: Customizable appearance and behavior

## Quick Start

### Prerequisites

- Rust 1.70 or later
- Git

### Building from Source

```bash
git clone https://github.com/yourusername/tterm.git
cd tterm
cargo build --release
```

### Running

```bash
cargo run
```

## Keyboard Shortcuts

### Tab Management

- `Ctrl+T` - New tab
- `Ctrl+W` - Close current tab
- `Ctrl+Tab` - Next tab
- `Ctrl+Shift+Tab` - Previous tab
- `Ctrl+1-9` - Switch to tab by number

### Split Management

- `Ctrl+Shift+V` - Split vertically
- `Ctrl+Shift+H` - Split horizontally
- `Ctrl+Shift+X` - Close current pane
- `Alt+Arrow` - Navigate between panes

### View Management

- `F11` - Toggle fullscreen
- `Ctrl+Shift+G` - Toggle grid view
- `Ctrl+Plus` - Increase font size
- `Ctrl+Minus` - Decrease font size

### Broadcast Mode

- `Ctrl+Shift+B` - Toggle broadcast mode
- `Ctrl+Shift+A` - Select/deselect all terminals

## Configuration

TTerminal stores its configuration in a platform-specific location:

- **macOS**: `~/Library/Application Support/tterm/config.toml`
- **Linux**: `~/.config/tterm/config.toml`
- **Windows**: `%APPDATA%/tterm/config.toml`

### Example Configuration

```toml
[appearance]
font_family = "JetBrains Mono"
font_size = 14.0
theme = "dark"
opacity = 0.95

[behavior]
scrollback_lines = 10000
close_tab_on_exit = true
confirm_quit = true

[keyboard]
new_tab = "Ctrl+T"
close_tab = "Ctrl+W"
split_vertical = "Ctrl+Shift+V"
```

## Development

### Project Structure

```
src/
├── main.rs           # Application entry point
├── app.rs            # Main application logic
├── config.rs         # Configuration management
├── terminal/         # Terminal backend
│   ├── mod.rs
│   ├── session.rs    # Terminal session management
│   ├── pty.rs        # PTY handling
│   └── renderer.rs   # Terminal rendering
├── ui/               # User interface components
│   ├── mod.rs
│   ├── tab_bar.rs    # Tab bar component
│   ├── terminal_view.rs # Terminal display
│   ├── status_bar.rs # Status bar
│   └── menu_bar.rs   # Menu bar
└── utils.rs          # Utility functions
```

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests if applicable
5. Commit your changes (`git commit -m 'Add some amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

### Testing

```bash
# Run tests
cargo test

# Run with logging
RUST_LOG=info cargo run

# Performance benchmarks
cargo bench
```

## Architecture

TTerminal is built with a modular architecture:

- **Core**: Application state management and coordination
- **Terminal Backend**: PTY management and terminal emulation via alacritty_terminal
- **UI Layer**: egui-based user interface components
- **Configuration**: TOML-based configuration system

## Roadmap

### Current Status

- ✅ Basic project structure
- ✅ egui application framework
- ✅ Configuration system
- ✅ Basic UI components
- 🚧 Terminal session management
- 🚧 PTY integration

### Upcoming Features

- [ ] PTY integration with alacritty_terminal
- [ ] Split pane functionality
- [ ] Input broadcasting
- [ ] Grid view implementation
- [ ] Session save/restore
- [ ] SSH integration
- [ ] Plugin system

## License

This project is licensed under the MIT OR Apache-2.0 license.

## Acknowledgments

- [alacritty](https://github.com/alacritty/alacritty) - For the excellent terminal backend
- [egui](https://github.com/emilk/egui) - For the immediate mode GUI framework
- [Rust](https://rust-lang.org/) - For the amazing programming language

## Support

If you encounter any issues or have questions, please open an issue on GitHub.
