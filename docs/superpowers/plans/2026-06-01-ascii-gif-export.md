# ASCII GIF 导出功能实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增 `multimedia ascii-gif` 子命令，将视频转换为 ASCII 艺术风格的动画 GIF。

**Architecture:** 复用现有 FFmpeg/Native 解码和 `image_to_ascii` 转换逻辑，新增 `GifRenderer`（fontdue 字体栅格化）和 `GifExporter`（image crate GIF 编码）两层。先重构 `convert.rs` 提取 `frame_to_ascii_grid` 和 `AsciiCell`，再依次构建渲染器、导出器、CLI 集成。

**Tech Stack:** Rust, ffmpeg-the-third, image 0.25 (with gif), fontdue 1.0

---

## 文件映射

| 文件 | 职责 |
|------|------|
| `src/multimedia/ascii/convert.rs` | 重构：提取 `frame_to_ascii_grid()`，新增 `AsciiCell` |
| `src/multimedia/ascii/gif_renderer.rs` | 新增：ASCII 网格 → RGB 图像渲染（fontdue） |
| `src/multimedia/ascii/gif_export.rs` | 新增：GIF 导出主逻辑（解码、采样、编码） |
| `src/multimedia/ascii/mod.rs` | 修改：导出 gif_renderer 和 gif_export 模块 |
| `src/multimedia/mod.rs` | 修改：导出 `export_ascii_gif`, `GifExportOptions`, `GifStyle` |
| `src/cli/args.rs` | 修改：添加 `AsciiGif` 子命令 |
| `src/main.rs` | 修改：处理 `AsciiGif` 命令 |
| `Cargo.toml` | 修改：添加 `gif` feature 到 image，添加 fontdue 依赖 |

---

### Task 1: 重构 convert.rs — 提取 AsciiCell 和 frame_to_ascii_grid

**Files:**
- Modify: `src/multimedia/ascii/convert.rs`

**Context:** 现有的 `image_to_ascii` 直接返回带 ANSI 转义码的字符串，无法复用颜色数据进行图片渲染。需要将"字符+颜色"的生成逻辑提取为独立的 `frame_to_ascii_grid`，`image_to_ascii` 在此基础上格式化为 ANSI 字符串。

- [ ] **Step 1: 添加 AsciiCell 结构体和导入**

在文件顶部 `use image::{DynamicImage, GenericImageView};` 下方添加：

```rust
use image::Rgb;

/// An ASCII cell with its display character and RGB color
#[derive(Debug, Clone)]
pub struct AsciiCell {
    pub char: char,
    pub color: Rgb<u8>,
}
```

- [ ] **Step 2: 提取 frame_to_ascii_grid 函数**

在 `image_to_ascii` 函数之前插入：

```rust
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
```

- [ ] **Step 3: 修改 image_to_ascii 复用 frame_to_ascii_grid**

将 `image_to_ascii` 的函数体完全替换为：

```rust
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
```

- [ ] **Step 4: 编译检查**

Run: `cargo check`
Expected: 编译通过，无错误。

- [ ] **Step 5: Commit**

```bash
git add src/multimedia/ascii/convert.rs
git commit -m "refactor: extract frame_to_ascii_grid and AsciiCell from image_to_ascii"
```

---

### Task 2: 添加依赖

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 为 image crate 添加 gif feature**

将：
```toml
image = { version = "0.25", features = ["jpeg", "png"] }
```

替换为：
```toml
image = { version = "0.25", features = ["jpeg", "png", "gif"] }
```

- [ ] **Step 2: 添加 fontdue 依赖**

在 `[dependencies]` 节任意位置添加：
```toml
fontdue = "1.0"
```

- [ ] **Step 3: 编译检查**

Run: `cargo check`
Expected: 编译通过（会下载 fontdue，首次可能较慢）。

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add fontdue and gif feature for image crate"
```

---

### Task 3: 创建 GifRenderer

**Files:**
- Create: `src/multimedia/ascii/gif_renderer.rs`

**Context:** `GifRenderer` 负责将 `Vec<Vec<AsciiCell>>` 栅格化为 RGB 图像。使用 `fontdue` 加载系统等宽字体，将每个字符渲染为带颜色的位图。

- [ ] **Step 1: 创建文件**

```rust
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
```

- [ ] **Step 2: 编译检查**

Run: `cargo check`
Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/multimedia/ascii/gif_renderer.rs
git commit -m "feat: add GifRenderer for ASCII grid to image rendering"
```

---

### Task 4: 创建 GifExporter

**Files:**
- Create: `src/multimedia/ascii/gif_export.rs`

**Context:** `GifExporter` 负责完整的导出流程：打开视频、按 FPS 采样帧、解码为 RGB、转换为 ASCII 网格、渲染为图片、编码为 GIF。使用 `ffmpeg-the-third` 解码，`image::codecs::gif::GifEncoder` 编码。

- [ ] **Step 1: 创建文件**

```rust
//! ASCII GIF export - decode video, convert to ASCII, render frames to GIF

use std::path::Path;
use std::fs::File;

use ffmpeg_the_third as ffmpeg;
use image::{ImageBuffer, Rgb, Frame, Delay};
use image::codecs::gif::GifEncoder;

use crate::multimedia::{
    error::MultimediaError,
    traits::{AsciiColorMode, AsciiScaleMode},
    ascii::{
        calculate_dimensions, frame_to_ascii_grid,
        gif_renderer::{GifRenderer, GifStyle},
    },
};

/// Options for ASCII GIF export
#[derive(Debug, Clone)]
pub struct GifExportOptions {
    pub fps: f64,
    pub width: u32,
    pub height: Option<u32>,
    pub style: GifStyle,
    pub color_mode: AsciiColorMode,
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

/// Export video to ASCII art animated GIF
pub fn export_ascii_gif(
    input: &Path,
    output: &Path,
    options: GifExportOptions,
) -> Result<(), MultimediaError> {
    ffmpeg::init()?;

    let mut input_ctx = ffmpeg::format::input(input)?;

    // Find video stream
    let mut video_stream_index = 0usize;
    let mut original_width = 0u32;
    let mut original_height = 0u32;
    let mut video_time_base = ffmpeg::Rational::new(1, 1);

    for (i, s) in input_ctx.streams().enumerate() {
        if s.parameters().medium() == ffmpeg::media::Type::Video {
            video_stream_index = i;
            video_time_base = s.time_base();
            if let Ok(context) = ffmpeg::codec::context::Context::from_parameters(s.parameters()) {
                if let Ok(decoder) = context.decoder().video() {
                    original_width = decoder.width();
                    original_height = decoder.height();
                }
            }
        }
    }

    if original_width == 0 || original_height == 0 {
        return Err(MultimediaError::NoVideoStream);
    }

    // Calculate ASCII grid dimensions
    let default_height = options.width * original_height / original_width;
    let (output_width, output_height) = calculate_dimensions(
        original_width,
        original_height,
        Some(options.width),
        options.height,
        options.width,
        default_height,
        AsciiScaleMode::KeepAspect,
    );

    // Setup renderer
    let renderer = GifRenderer::new(options.style)?;

    // Setup GIF encoder
    let output_file = File::create(output)?;
    let mut gif_encoder = GifEncoder::new(output_file);

    // Setup decoder
    let stream = &input_ctx.streams()[video_stream_index];
    let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = context.decoder().video()?;

    // Create scaler for RGB conversion
    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        decoder.height(),
        decoder.width(),
        ffmpeg::format::Pixel::RGB24,
        decoder.height(),
        decoder.width(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;

    // Calculate time range
    let start_time = options.start.unwrap_or(0.0);
    let end_time = options.duration.map(|d| start_time + d).unwrap_or_else(|| {
        input_ctx.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    });

    let dt = 1.0 / options.fps;
    let frame_delay = Delay::from_numer_denom_ms((dt * 1000.0) as u32, 1);
    let mut last_sampled_pts = -1.0f64;

    // Decode and sample frames
    for (stream, packet) in input_ctx.packets() {
        if stream.index() != video_stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;

        let mut decoded = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let pts = decoded.timestamp().unwrap_or(0) as f64 * f64::from(video_time_base);

            if pts < start_time {
                continue;
            }
            if pts > end_time {
                break;
            }

            // Sample at fixed FPS intervals
            let frame_idx = ((pts - start_time) / dt).round() as i64;
            let target_pts = start_time + frame_idx as f64 * dt;

            if pts >= target_pts && last_sampled_pts < target_pts {
                last_sampled_pts = target_pts;

                // Convert to RGB
                let mut rgb_frame = ffmpeg::frame::Video::empty();
                scaler.run(&decoded, &mut rgb_frame)?;

                // Create RgbImage from raw data
                let width = rgb_frame.width();
                let height = rgb_frame.height();
                let data = rgb_frame.data(0);
                let img = RgbImage::from_raw(width, height, data.to_vec())
                    .ok_or_else(|| MultimediaError::InvalidParameter(
                        "Failed to create image buffer".to_string()
                    ))?;

                // Convert to ASCII and render
                let grid = frame_to_ascii_grid(
                    &image::DynamicImage::ImageRgb8(img),
                    output_width,
                    output_height,
                    options.color_mode,
                );
                let rendered = renderer.render_frame(&grid);

                // Encode frame
                let frame = Frame::from_parts(rendered, 0, 0, frame_delay);
                gif_encoder.encode_frame(frame)
                    .map_err(|e| MultimediaError::ImageError(e.to_string()))?;
            }
        }

        if last_sampled_pts >= end_time {
            break;
        }
    }

    Ok(())
}
```

- [ ] **Step 2: 编译检查**

Run: `cargo check`
Expected: 编译通过。如有 API 不匹配（如 `Delay` 或 `Frame` 构造方式），根据编译错误调整。

- [ ] **Step 3: Commit**

```bash
git add src/multimedia/ascii/gif_export.rs
git commit -m "feat: add GifExporter for ASCII GIF generation"
```

---

### Task 5: 导出模块接口

**Files:**
- Modify: `src/multimedia/ascii/mod.rs`
- Modify: `src/multimedia/mod.rs`

- [ ] **Step 1: 导出 gif 模块**

修改 `src/multimedia/ascii/mod.rs`，在 `pub mod native;` 下方添加：

```rust
pub mod gif_renderer;
pub mod gif_export;
```

- [ ] **Step 2: 顶层导出**

修改 `src/multimedia/mod.rs`，在 `pub use ascii::play_ascii;` 下方添加：

```rust
pub use ascii::gif_export::{export_ascii_gif, GifExportOptions};
pub use ascii::gif_renderer::GifStyle;
```

- [ ] **Step 3: 编译检查**

Run: `cargo check`
Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add src/multimedia/ascii/mod.rs src/multimedia/mod.rs
git commit -m "feat: export GifRenderer and GifExporter modules"
```

---

### Task 6: 添加 CLI 参数

**Files:**
- Modify: `src/cli/args.rs`

- [ ] **Step 1: 添加 AsciiGif 子命令**

在 `src/cli/args.rs` 的 `MultimediaSubcommands` enum 中，在 `PlayAscii` 变体之后添加：

```rust
    /// Convert video to ASCII art animated GIF
    AsciiGif {
        /// Input video file path
        input: PathBuf,

        /// Output GIF file path
        output: PathBuf,

        /// Target FPS for the GIF
        #[arg(short, long, default_value = "10.0")]
        fps: f64,

        /// ASCII grid width in characters
        #[arg(short, long, default_value = "80")]
        width: u32,

        /// ASCII grid height in characters (auto-calculated if not set)
        #[arg(short = 'H', long)]
        height: Option<u32>,

        /// Rendering style: terminal or pixel-art
        #[arg(short, long, default_value = "terminal")]
        style: String,

        /// Color mode: none, ansi256, or truecolor
        #[arg(long, default_value = "none")]
        color_mode: String,

        /// Start time in seconds
        #[arg(short, long)]
        start: Option<f64>,

        /// Duration in seconds
        #[arg(short, long)]
        duration: Option<f64>,
    },
```

- [ ] **Step 2: 编译检查**

Run: `cargo check`
Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/cli/args.rs
git commit -m "feat: add AsciiGif CLI subcommand"
```

---

### Task 7: main.rs 处理命令

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: 导入新类型**

在 `src/main.rs` 的 `use multimedia::{...}` 行中添加：

```rust
use multimedia::{get_media_info, transcode, extract_frames, play_ascii, export_ascii_gif,
    TranscoderOptions, AsciiPlayOptions, AsciiColorMode,
    GifExportOptions, GifStyle};
```

- [ ] **Step 2: 添加 AsciiGif 分支**

在 `handle_multimedia` 函数的 `PlayAscii` match 分支之后、`}` 之前添加：

```rust
        MultimediaSubcommands::AsciiGif { input, output, fps, width, height, style, color_mode, start, duration } => {
            let style = match style.as_str() {
                "terminal" => GifStyle::Terminal,
                "pixel-art" | "pixelart" => GifStyle::PixelArt,
                _ => {
                    eprintln!("Warning: Unknown style '{}', using terminal", style);
                    GifStyle::Terminal
                }
            };

            let color_mode = match color_mode.as_str() {
                "none" => AsciiColorMode::None,
                "ansi256" => AsciiColorMode::Ansi256,
                "truecolor" | "true-color" | "rgb" => AsciiColorMode::TrueColor,
                _ => {
                    eprintln!("Warning: Unknown color mode '{}', using none", color_mode);
                    AsciiColorMode::None
                }
            };

            let options = GifExportOptions {
                fps,
                width,
                height,
                style,
                color_mode,
                start,
                duration,
            };

            println!("Exporting ASCII GIF: {} -> {}...", input.display(), output.display());
            export_ascii_gif(&input, &output, options)?;
            println!("Done! Saved to {}", output.display());
            Ok(())
        }
```

- [ ] **Step 3: 编译检查**

Run: `cargo check`
Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up AsciiGif command in main"
```

---

### Task 8: 集成测试

**Files:**
- None (manual verification)

- [ ] **Step 1: Release 编译**

Run: `cargo build --release`
Expected: 编译成功。

- [ ] **Step 2: 找一个测试视频**

准备一个短小的 MP4 视频（5-10秒，分辨率 480p 左右），放在项目目录下，例如 `test.mp4`。

- [ ] **Step 3: 运行完整流程**

```bash
cargo run --release -- multimedia ascii-gif test.mp4 output.gif --fps 5 --style terminal --color-mode none
```

Expected: 命令成功执行，生成 `output.gif` 文件。

- [ ] **Step 4: 验证输出**

检查 `output.gif`：
- 文件大小 > 0
- 可以用图片浏览器打开并播放动画
- 帧数是预期的（时长 × FPS）
- ASCII 字符可见，黑底白字

- [ ] **Step 5: 测试 pixel-art 风格**

```bash
cargo run --release -- multimedia ascii-gif test.mp4 output-pixel.gif --fps 10 --style pixel-art --color-mode truecolor --width 120
```

Expected: 生成更小的字符、彩色 GIF。

- [ ] **Step 6: 测试片段导出**

```bash
cargo run --release -- multimedia ascii-gif test.mp4 output-trim.gif --fps 5 --start 2.0 --duration 3.0
```

Expected: 只导出第 2-5 秒的内容。

- [ ] **Step 7: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete ASCII GIF export functionality"
```

---

## Self-Review

### 1. Spec Coverage

| 设计文档要求 | 对应任务 |
|-------------|---------|
| `AsciiCell` 结构体 | Task 1 |
| `frame_to_ascii_grid` 提取 | Task 1 |
| `GifStyle` enum (Terminal/PixelArt) | Task 3 |
| `GifRenderer` (fontdue) | Task 3 |
| `GifExportOptions` | Task 4 |
| `export_ascii_gif` (解码/采样/编码) | Task 4 |
| CLI 参数 (fps/width/height/style/color-mode/start/duration) | Task 6 |
| 模块导出 | Task 5 |
| main.rs 处理 | Task 7 |
| 颜色模式 (none/ansi256/truecolor) | Task 4 (通过 frame_to_ascii_grid) |
| 帧采样逻辑 | Task 4 |
| 字体加载策略 | Task 3 |

✅ 全覆盖，无遗漏。

### 2. Placeholder Scan

- 无 "TBD" / "TODO"
- 无 "add appropriate error handling"
- 无 "similar to Task N"
- 每个步骤都有具体代码或命令
- 类型名一致性：所有任务中 `AsciiCell`、`GifStyle`、`GifRenderer`、`GifExportOptions`、`export_ascii_gif` 的命名和签名一致

### 3. Type Consistency

- `frame_to_ascii_grid` 返回 `Vec<Vec<AsciiCell>>` — Task 1 和 Task 4 一致
- `GifRenderer::new` 接受 `GifStyle` — Task 3 和 Task 4 一致
- `export_ascii_gif` 签名 `(&Path, &Path, GifExportOptions) -> Result<(), MultimediaError>` — Task 4 和 Task 7 一致
- `color_mode` CLI 字符串映射到 `AsciiColorMode` — Task 6 和 Task 7 与现有 `play-ascii` 处理逻辑一致

✅ 通过。
