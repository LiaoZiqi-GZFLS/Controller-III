//! Convert RGB frames to ASCII art

use crate::multimedia::traits::AsciiColorMode;
use image::{DynamicImage, GenericImageView, Rgb};

/// An ASCII cell with its display character and RGB color
#[derive(Debug, Clone)]
pub struct AsciiCell {
    pub char: char,
    pub color: Rgb<u8>,
}

/// Standard ASCII gradient from darkest to lightest
/// More characters = smoother gradient
const ASCII_GRADIENT: &[u8] =
    b" .'`^\",:;Il!i~+_-=?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// Convert RGB frame to a grid of ASCII cells with per-cell color
pub fn frame_to_ascii_grid(
    image: &DynamicImage,
    width: u32,
    height: u32,
    color_mode: AsciiColorMode,
) -> Vec<Vec<AsciiCell>> {
    let resized = image.resize_exact(width, height, image::imageops::FilterType::Triangle);
    let mut grid = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut row = Vec::with_capacity(width as usize);
        for x in 0..width {
            let px = resized.get_pixel(x, y);
            let r = px[0];
            let g = px[1];
            let b = px[2];

            let luminance = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0;
            let idx = (luminance * (ASCII_GRADIENT.len() - 1) as f64) as usize;
            let ch = ASCII_GRADIENT[idx] as char;

            let color = match color_mode {
                AsciiColorMode::None => Rgb([255, 255, 255]),
                _ => Rgb([r, g, b]),
            };

            row.push(AsciiCell { char: ch, color });
        }
        grid.push(row);
    }
    grid
}

/// Convert RGB frame buffer to ASCII lines
pub fn image_to_ascii(
    image: &DynamicImage,
    width: u32,
    height: u32,
    color_mode: AsciiColorMode,
) -> Vec<String> {
    let grid = frame_to_ascii_grid(image, width, height, color_mode);
    let mut lines = Vec::with_capacity(grid.len());

    for row in grid {
        let mut line = String::with_capacity(row.len());
        for cell in row {
            match color_mode {
                AsciiColorMode::None => line.push(cell.char),
                AsciiColorMode::Ansi256 => {
                    let ansi = rgb_to_ansi256(cell.color[0], cell.color[1], cell.color[2]);
                    line.push_str(&format!("\x1b[38;5;{}m{}", ansi, cell.char));
                }
                AsciiColorMode::TrueColor => {
                    line.push_str(&format!(
                        "\x1b[38;2;{};{};{}m{}",
                        cell.color[0], cell.color[1], cell.color[2], cell.char
                    ));
                }
            }
        }
        if color_mode != AsciiColorMode::None {
            line.push_str("\x1b[0m");
        }
        lines.push(line);
    }
    lines
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
