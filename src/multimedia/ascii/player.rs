//! Shared playback state machine and types for ASCII video player

use std::time::{Instant, Duration};
use std::sync::mpsc;

use crate::multimedia::traits::{AsciiPlayOptions, AsciiColorMode};

/// Decoded frame ready for rendering, pre-converted to ASCII
#[derive(Debug, Clone)]
pub struct QueuedFrame {
    /// Pre-rendered ASCII lines
    pub ascii_lines: Vec<String>,
    /// Presentation timestamp in seconds from start
    pub pts: f64,
    /// Frame duration in seconds
    pub duration: f64,
    /// Original frame dimensions before ASCII conversion
    pub original_width: u32,
    pub original_height: u32,
}

/// Playback state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Actively playing
    Playing,
    /// Paused, holding current frame
    Paused,
    /// Seeking in progress
    Seeking,
    /// Playback complete
    Finished,
}

/// User input action from keyboard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    /// Quit playback
    Quit,
    /// Toggle pause/resume
    TogglePause,
    /// Seek backward by step
    SeekBackward,
    /// Seek forward by step
    SeekForward,
    /// Increase playback speed
    SpeedUp,
    /// Decrease playback speed
    SpeedDown,
    /// Terminal was resized
    Resize,
    /// No action
    None,
}

/// Default seek step in seconds
pub const DEFAULT_SEEK_STEP: f64 = 10.0;
/// Default speed change step
pub const DEFAULT_SPEED_STEP: f64 = 0.25;
/// Minimum playback speed
pub const MIN_SPEED: f64 = 0.25;
/// Maximum playback speed
pub const MAX_SPEED: f64 = 4.0;
/// Frame queue capacity - pre-buffer this many frames
pub const DEFAULT_FRAME_QUEUE_CAPACITY: usize = 15;

/// Playback context shared between decode thread and main thread
#[derive(Debug)]
pub struct PlaybackContext {
    /// Current playback state
    pub state: PlaybackState,
    /// Current playback position in seconds
    pub current_pts: f64,
    /// Last rendered frame PTS
    pub last_rendered_pts: f64,
    /// Total video duration in seconds
    pub total_duration: f64,
    /// Current playback speed multiplier (1.0 = normal)
    pub speed: f64,
    /// Current output dimensions (may change on resize)
    pub output_width: u32,
    pub output_height: u32,
    /// Original video dimensions
    pub original_width: u32,
    pub original_height: u32,
    /// Color mode for ASCII conversion
    pub color_mode: AsciiColorMode,
    /// Whether to show FPS
    pub show_fps: bool,
    /// Clock start time (adjusted for pauses/seeks)
    pub clock_start: Instant,
    /// Offset from clock start to actual playback position
    pub clock_offset: f64,
    /// Frames rendered
    pub frames_rendered: u64,
    /// Frames dropped due to drift
    pub frames_dropped: u64,
}

impl PlaybackContext {
    /// Create new playback context
    pub fn new(
        total_duration: f64,
        original_width: u32,
        original_height: u32,
        output_width: u32,
        output_height: u32,
        options: &AsciiPlayOptions,
    ) -> Self {
        let now = Instant::now();
        Self {
            state: PlaybackState::Playing,
            current_pts: 0.0,
            last_rendered_pts: -1.0,
            total_duration,
            speed: options.speed,
            output_width,
            output_height,
            original_width,
            original_height,
            color_mode: options.color_mode,
            show_fps: options.show_fps,
            clock_start: now,
            clock_offset: 0.0,
            frames_rendered: 0,
            frames_dropped: 0,
        }
    }

    /// Get current playback position based on clock and state
    pub fn get_current_pts(&self) -> f64 {
        match self.state {
            PlaybackState::Playing => {
                let elapsed = self.clock_start.elapsed().as_secs_f64() * self.speed;
                self.clock_offset + elapsed
            }
            PlaybackState::Paused | PlaybackState::Seeking | PlaybackState::Finished => {
                self.clock_offset
            }
        }
    }

    /// Toggle pause/resume
    pub fn toggle_pause(&mut self) {
        match self.state {
            PlaybackState::Playing => {
                // Pause - save current position to offset
                self.clock_offset = self.get_current_pts();
                self.state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                // Resume - adjust clock start to maintain position
                self.clock_start = Instant::now();
                self.state = PlaybackState::Playing;
            }
            _ => {}
        }
    }

    /// Seek by relative delta
    pub fn seek_relative(&mut self, delta: f64) -> f64 {
        let new_pts = (self.get_current_pts() + delta).clamp(0.0, self.total_duration);
        self.seek_to(new_pts);
        new_pts
    }

    /// Seek to absolute position
    pub fn seek_to(&mut self, pts: f64) {
        self.current_pts = pts;
        self.clock_offset = pts;
        self.clock_start = Instant::now();
        self.state = PlaybackState::Seeking;
    }

    /// Change speed by relative delta
    pub fn adjust_speed(&mut self, delta: f64) {
        let current_pts = self.get_current_pts();
        self.speed = (self.speed + delta).clamp(MIN_SPEED, MAX_SPEED);
        // Adjust clock to maintain current position
        self.clock_offset = current_pts;
        self.clock_start = Instant::now();
    }

    /// Check if we need to output dimensions update after resize
    pub fn handle_resize(&mut self, new_width: u32, new_height: u32) -> bool {
        if new_width != self.output_width || new_height != self.output_height {
            self.output_width = new_width;
            self.output_height = new_height;
            true
        } else {
            false
        }
    }

    /// Calculate current FPS over a window
    pub fn calculate_fps(&self, start_time: Instant) -> f64 {
        let elapsed = start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 && self.frames_rendered > 0 {
            self.frames_rendered as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Message from main thread to decode thread
#[derive(Debug, Clone)]
pub enum DecodeMessage {
    /// Continue decoding normally
    Continue,
    /// Seek to target position
    Seek(f64),
    /// Terminal resized - update output dimensions
    Resize(u32, u32),
    /// Stop decoding and exit
    Stop,
}

/// Create a new bounded frame queue
pub fn create_frame_queue() -> (mpsc::SyncSender<QueuedFrame>, mpsc::Receiver<QueuedFrame>) {
    mpsc::sync_channel(DEFAULT_FRAME_QUEUE_CAPACITY)
}

/// Create a new channel for decode control messages
pub fn create_decode_control() -> (mpsc::Sender<DecodeMessage>, mpsc::Receiver<DecodeMessage>) {
    mpsc::channel()
}
