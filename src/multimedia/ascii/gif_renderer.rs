//! ASCII grid to RGB image renderer using fontdue

use image::{ImageBuffer, Rgb, RgbImage};
use fontdue::{Font, FontSettings};

use crate::multimedia::error::MultimediaError;
use super::convert::AsciiCell;

/// Rendering style for ASCII GIF
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifStyle {
    /// Large font (16x32 cells), terminal screenshot appearance
    Terminal,
    /// Small font (4x8 cells), pixel-art appearance
    PixelArt,
}

/// Renders ASCII grids to RGB images using fontdue
pub struct GifRenderer {
    font: Font,
    cell_width: u32,
    cell_height: u32,
}

impl GifRenderer {
    /// Create a new renderer with the given style
    pub fn new(style: GifStyle) -> Result<Self, MultimediaError> {
        let font = Self::load_font()?;
        let (cell_width, cell_height) = match style {
            GifStyle::Terminal => (16, 32),
            GifStyle::PixelArt => (4, 8),
        };
        Ok(Self { font, cell_width, cell_height })
    }

    /// Try to load a system monospace font
    fn load_font() -> Result<Font, MultimediaError> {
        let paths = [
            // Windows
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\cour.ttf",
            // Linux
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
            // macOS
            "/System/Library/Fonts/Monaco.dfont",
            "/Library/Fonts/Andale Mono.ttf",
        ];

        for path in &paths {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
                    return Ok(font);
                }
            }
        }

        Err(MultimediaError::InvalidParameter(
            "No monospace font found. Please install a system monospace font.".to_string()
        ))
    }

    /// Render an ASCII grid to an RGB image
    pub fn render_frame(&self, grid: &[Vec<AsciiCell>]) -> RgbImage {
        let rows = grid.len() as u32;
        let cols = grid.first().map(|r| r.len() as u32).unwrap_or(0);
        let img_width = cols * self.cell_width;
        let img_height = rows * self.cell_height;

        let mut img = ImageBuffer::from_pixel(img_width, img_height, Rgb([0, 0, 0]));

        for (row_idx, row) in grid.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                let x = col_idx as u32 * self.cell_width;
                let y = row_idx as u32 * self.cell_height;
                self.render_cell(&mut img, x, y, cell);
            }
        }

        img
    }

    /// Render a single ASCII cell to the image
    fn render_cell(&self, img: &mut RgbImage, x: u32, y: u32, cell: &AsciiCell) {
        let (metrics, bitmap) = self.font.rasterize(cell.char, self.cell_height as f32);

        for (i, &alpha) in bitmap.iter().enumerate() {
            let bx = i % metrics.width;
            let by = i / metrics.width;
            let px = x + bx as u32;
            let py = y + by as u32;

            if px < img.width() && py < img.height() {
                let a = alpha as f32 / 255.0;
                let r = (cell.color[0] as f32 * a) as u8;
                let g = (cell.color[1] as f32 * a) as u8;
                let b = (cell.color[2] as f32 * a) as u8;
                img.put_pixel(px, py, Rgb([r, g, b]));
            }
        }
    }
}
