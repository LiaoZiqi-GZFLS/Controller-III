//! Terminal handling for ASCII playback - setup, cleanup, rendering, event handling

use std::io::{self, Write};
use std::time::Duration;
use crossterm::{
    ExecutableCommand,
    terminal::{self, size, LeaveAlternateScreen, EnterAlternateScreen},
    cursor::{Hide, Show, MoveTo},
    event::{self, Event, KeyCode, KeyModifiers},
};

use super::player::UserAction;

/// Terminal guard - handles setup and guarantees cleanup via Drop
pub struct TerminalGuard {
    cleaned_up: bool,
    last_size: (u32, u32),
}

impl TerminalGuard {
    /// Initialize terminal for ASCII playback:
    /// - Enter alternate screen
    /// - Hide cursor
    /// - Clear screen
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();

        // Enter alternate screen (preserves original terminal content)
        stdout.execute(EnterAlternateScreen)?;
        // Hide cursor during playback
        stdout.execute(Hide)?;
        // Enable raw mode for key handling (so Ctrl+C works correctly)
        terminal::enable_raw_mode()?;

        let last_size = Self::get_size()?;

        Ok(Self { cleaned_up: false, last_size })
    }

    /// Get current terminal size in characters (columns, rows)
    pub fn get_size() -> io::Result<(u32, u32)> {
        let (cols, rows) = size()?;
        Ok((cols as u32, rows as u32))
    }

    /// Render ASCII frame to terminal
    /// Moves cursor to top-left and prints each line
    /// Minimizes flicker by not clearing screen every frame
    pub fn render_frame(ascii_lines: &[String], fps: Option<f64>, speed: f64, current_pts: f64, duration: f64) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Move cursor to top-left corner
        stdout.execute(MoveTo(0, 0))?;

        // Print each ASCII line
        for line in ascii_lines {
            writeln!(stdout, "{}", line)?;
        }

        // Print status bar (FPS, speed, progress)
        let mut status = String::new();
        if let Some(fps) = fps {
            status.push_str(&format!("FPS: {:.1} ", fps));
        }
        if (speed - 1.0).abs() > 0.01 {
            status.push_str(&format!("Speed: {:.2}x ", speed));
        }
        // Progress bar
        let progress = current_pts / duration;
        let progress_pct = (progress * 100.0).round() as u8;
        status.push_str(&format!("[{}%] {}", progress_pct, Self::format_time(current_pts)));
        if duration.is_finite() {
            status.push_str(&format!("/{}", Self::format_time(duration)));
        }

        if !status.is_empty() {
            writeln!(stdout, "\x1b[0m{}", status.trim_end())?;
        }

        // Flush to ensure output is displayed immediately
        stdout.flush()?;

        Ok(())
    }

    /// Format time in seconds to MM:SS format
    fn format_time(seconds: f64) -> String {
        let total_seconds = seconds.floor() as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }

    /// Poll for user input with timeout. Returns user action if any.
    /// Also checks for terminal resize.
    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<UserAction> {
        if !event::poll(timeout)? {
            // Timeout - check if terminal was resized
            if let Ok((new_width, new_height)) = Self::get_size() {
                if new_width != self.last_size.0 || new_height != self.last_size.1 {
                    self.last_size = (new_width, new_height);
                    return Ok(UserAction::Resize);
                }
            }
            return Ok(UserAction::None);
        }

        let event = event::read()?;
        self.process_event(event)
    }

    /// Process a terminal event into a UserAction
    fn process_event(&mut self, event: Event) -> io::Result<UserAction> {
        match event {
            Event::Key(key) => {
                // Check for quit combinations
                if (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL)
                    || key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Esc
                {
                    return Ok(UserAction::Quit);
                }

                // Check for other control keys
                match key.code {
                    KeyCode::Char(' ') => Ok(UserAction::TogglePause),
                    KeyCode::Right => Ok(UserAction::SeekForward),
                    KeyCode::Left => Ok(UserAction::SeekBackward),
                    KeyCode::Up => Ok(UserAction::SpeedUp),
                    KeyCode::Down => Ok(UserAction::SpeedDown),
                    _ => Ok(UserAction::None),
                }
            }
            Event::Resize(_, _) => {
                // Terminal was resized
                if let Ok(new_size) = Self::get_size() {
                    self.last_size = new_size;
                }
                Ok(UserAction::Resize)
            }
            _ => Ok(UserAction::None),
        }
    }

    /// Check if user pressed Ctrl+C or q to quit (legacy, for compatibility)
    pub fn check_quit() -> io::Result<bool> {
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL)
                    || key.code == KeyCode::Char('q')
                    || key.code == KeyCode::Esc
                {
                    return Ok(true);
                }
            }
            let _ = event::read();
        }
        Ok(false)
    }

    /// Drain all pending events from input buffer
    /// Used to clear events from dialoguer before playback starts
    pub fn drain_all_events() -> io::Result<()> {
        while event::poll(Duration::from_millis(0))? {
            let _ = event::read();
        }
        Ok(())
    }

    /// Cleanup and restore terminal
    fn cleanup(&mut self) -> io::Result<()> {
        if !self.cleaned_up {
            let mut stdout = io::stdout();

            terminal::disable_raw_mode()?;
            stdout.execute(Show)?;
            stdout.execute(LeaveAlternateScreen)?;
            stdout.flush()?;

            self.cleaned_up = true;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    /// Guaranteed cleanup even on panic or Ctrl+C
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
