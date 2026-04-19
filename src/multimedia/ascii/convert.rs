//! Convert RGB frames to ASCII art

use image::{DynamicImage, GenericImageView};
use crate::multimedia::traits::AsciiColorMode;

/// Standard ASCII gradient from darkest to lightest
/// More characters = smoother gradient
const ASCII_GRADIENT: &[u8] = b" .'`^\",:;Il!i~+_-=?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// Convert RGB frame buffer to ASCII lines
pub fn image_to_ascii(
    image: &DynamicImage,
    width: u32,
    height: u32,
    color_mode: AsciiColorMode,
) -> Vec<String> {
    // Resize image to target dimensions
    // Use faster filter for real-time playback (good enough quality for ASCII)
    let resized = image.resize_exact(width, height, image::imageops::FilterType::Triangle);

    let mut ascii_lines = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut line = String::with_capacity(width as usize);
        for x in 0..width {
            let px = resized.get_pixel(x, y);
            let r = px[0];
            let g = px[1];
            let b = px[2];

            // Calculate luminance using standard ITU-R BT.601 formula
            // Y = 0.299 R + 0.587 G + 0.114 B
            let luminance = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0;

            match color_mode {
                AsciiColorMode::None => {
                    // Map luminance to ASCII character
                    let idx = (luminance * (ASCII_GRADIENT.len() - 1) as f64) as usize;
                    line.push(ASCII_GRADIENT[idx] as char);
                }
                AsciiColorMode::Ansi256 => {
                    let idx = (luminance * (ASCII_GRADIENT.len() - 1) as f64) as usize;
                    let c = ASCII_GRADIENT[idx] as char;
                    let ansi = rgb_to_ansi256(r, g, b);
                    line.push_str(&format!("\x1b[38;5;{}m{}", ansi, c));
                }
                AsciiColorMode::TrueColor => {
                    let idx = (luminance * (ASCII_GRADIENT.len() - 1) as f64) as usize;
                    let c = ASCII_GRADIENT[idx] as char;
                    line.push_str(&format!("\x1b[38;2;{};{};{}m{}", r, g, b, c));
                }
            }
        }
        // Reset color at end of line if we used color
        if color_mode != AsciiColorMode::None {
            line.push_str("\x1b[0m");
        }
        ascii_lines.push(line);
    }

    ascii_lines
}

/// Convert RGB to 256-color ANSI code
fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // 6x6x6 color cube: 16 + 36*r + 6*g + b
    let r = ((r as f32 / 255.0) * 5.0).round() as u8;
    let g = ((g as f32 / 255.0) * 5.0).round() as u8;
    let b = ((b as f32 / 255.0) * 5.0).round() as u8;
    16 + 36 * r + 6 * g + b
}

/// Calculate output dimensions respecting aspect ratio with character aspect correction
/// Terminal characters are typically ~2x taller than wide, so we correct for that
pub fn calculate_dimensions(
    original_width: u32,
    original_height: u32,
    requested_width: Option<u32>,
    requested_height: Option<u32>,
    terminal_width: u32,
    terminal_height: u32,
    scale_mode: crate::multimedia::traits::AsciiScaleMode,
) -> (u32, u32) {
    const CHAR_ASPECT_RATIO: f64 = 0.5; // chars are ~2x taller than wide

    // If user explicitly specified width and/or height, use those regardless of scale mode
    match (requested_width, requested_height) {
        (Some(w), Some(h)) => return (w, h),
        (Some(w), None) => {
            let h = (w as f64 * original_height as f64 / original_width as f64) * CHAR_ASPECT_RATIO;
            return (w, h.round() as u32);
        }
        (None, Some(h)) => {
            let w = (h as f64 * original_width as f64 / original_height as f64) / CHAR_ASPECT_RATIO;
            return (w.round() as u32, h);
        }
        (None, None) => {} // fall through to auto-calculate based on scale mode
    }

    // Auto-calculate based on scaling mode
    match scale_mode {
        crate::multimedia::traits::AsciiScaleMode::NoScale => {
            // Use original dimensions, no scaling
            let w = original_width;
            let h = (original_height as f64 * CHAR_ASPECT_RATIO).round() as u32;
            (w, h)
        }
        crate::multimedia::traits::AsciiScaleMode::FitWindow => {
            // Fit exactly to terminal, may change aspect ratio
            (terminal_width, terminal_height)
        }
        crate::multimedia::traits::AsciiScaleMode::KeepAspect => {
            // Fit to terminal while preserving aspect ratio
            let w = terminal_width;
            let h = (w as f64 * original_height as f64 / original_width as f64) * CHAR_ASPECT_RATIO;
            let h = h.min(terminal_height as f64) as u32;
            (w, h)
        }
    }
}
