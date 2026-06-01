# ASCII GIF 导出功能设计文档

## 背景

Controller III 的 `multimedia play-ascii` 已支持将视频以 ASCII 艺术形式播放到终端，并可将 ASCII 文本帧导出到目录。但用户希望进一步将这些 ASCII 帧编码为**动画 GIF**，便于分享和展示。

## 目标

新增 `multimedia ascii-gif` 子命令，将视频转换为 ASCII 艺术风格的动画 GIF，支持两种渲染风格和三种颜色模式。

## CLI 接口

```bash
controller-iii multimedia ascii-gif <input> <output> \
  --fps 10 \
  --width 80 \
  --height 45 \
  --style terminal \
  --color-mode truecolor \
  --start 10.5 \
  --duration 5.0
```

### 参数说明

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `input` | 必填 | 输入视频文件路径 |
| `output` | 必填 | 输出 GIF 文件路径 |
| `--fps` | `10.0` | 目标 GIF 帧率 |
| `--width` | `80` | ASCII 网格列数（字符数） |
| `--height` | `None` | ASCII 网格行数，None 时按宽高比自动计算 |
| `--style` | `terminal` | `terminal`（大字体，终端截图风格）或 `pixel-art`（小字体，像素画风格） |
| `--color-mode` | `none` | `none`（单色）、`ansi256`、`truecolor` |
| `--start` | `None` | 起始时间（秒），None 时从头开始 |
| `--duration` | `None` | 持续时长（秒），None 时到视频结束 |

## 架构

### 新增/修改文件

```
src/
├── cli/
│   └── args.rs              # + AsciiGif 子命令
├── multimedia/
│   ├── mod.rs               # + 导出 ascii_gif 函数
│   ├── error.rs             # 可能 + FontLoadError
│   ├── ascii/
│   │   ├── mod.rs           # + 导出 gif 模块
│   │   ├── convert.rs       # 重构：提取 frame_to_ascii_grid()
│   │   ├── gif_export.rs    # 新增：GIF 导出主逻辑
│   │   ├── gif_renderer.rs  # 新增：ASCII 网格 → RGB 图像渲染
│   │   └── ffmpeg.rs        # 复用：解码逻辑
│   └── ...
├── main.rs                  # + 处理 AsciiGif 命令
└── Cargo.toml               # + fontdue, image gif feature
```

## 核心组件

### AsciiCell（convert.rs 新增）

将字符与颜色从 ANSI 字符串表示中解耦：

```rust
pub struct AsciiCell {
    pub char: char,
    pub color: image::Rgb<u8>,
}
```

### frame_to_ascii_grid（convert.rs 重构）

新增函数，将视频帧转换为带颜色的 ASCII 网格：

```rust
pub fn frame_to_ascii_grid(
    image: &DynamicImage,
    width: u32,
    height: u32,
    color_mode: AsciiColorMode,
) -> Vec<Vec<AsciiCell>>
```

现有的 `image_to_ascii()` 改为调用 `frame_to_ascii_grid()` 后再格式化为 ANSI 转义字符串，保证行为不变。

### GifStyle（gif_renderer.rs）

```rust
pub enum GifStyle {
    Terminal,   // 16px 字体，标准终端间距
    PixelArt,   // 4px 字体，紧密排列
}
```

### GifRenderer（gif_renderer.rs）

```rust
pub struct GifRenderer {
    font: fontdue::Font,
    style: GifStyle,
    cell_width: u32,
    cell_height: u32,
}

impl GifRenderer {
    pub fn render_frame(&self, grid: &[Vec<AsciiCell>]) -> RgbImage;
}
```

**渲染逻辑**：
1. 根据 ASCII 网格行列数和单元格尺寸，创建 RGB 图像
2. 填充黑色背景
3. 对每个 `AsciiCell`，用 `fontdue` 栅格化字符为灰度位图
4. 将位图像素乘以 `AsciiCell.color`，绘制到对应位置

**字体策略**：
1. 优先尝试加载系统等宽字体（Windows: Consolas, Linux: DejaVu Sans Mono, macOS: Monaco）
2. 回退：内嵌一个极简等宽字体（~30KB，仅含 ASCII 32-126），通过 `include_bytes!` 编译进二进制

### GifExporter（gif_export.rs）

```rust
pub struct GifExportOptions {
    pub fps: f64,
    pub width: u32,
    pub height: Option<u32>,
    pub style: GifStyle,
    pub color_mode: AsciiColorMode,
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

pub fn export_ascii_gif(
    input: &Path,
    output: &Path,
    options: GifExportOptions,
) -> Result<(), MultimediaError>
```

**帧采样逻辑**：
1. 计算帧间隔 `dt = 1.0 / fps`
2. 从 `start` 时间开始，每隔 `dt` 秒提取一帧
3. 使用 FFmpeg 的 `seek` 跳转到目标时间点，然后解码到目标 PTS
4. 将每帧转换为 `DynamicImage::Rgb8`
5. 调用 `frame_to_ascii_grid()` 获取 ASCII 网格
6. 调用 `GifRenderer::render_frame()` 渲染为 RGB 图像
7. 用 `image::codecs::gif::GifEncoder` 写入输出文件
8. 设置每帧延迟为 `dt * 100`（百分之一秒，GIF 标准单位）

## 数据流

```
输入视频
  │
  ▼
FFmpeg/Native 解码器（复用现有逻辑，支持 --start 跳转）
  │ RGB 帧
  ▼
frame_to_ascii_grid()（复用现有亮度/颜色计算）
  │ Vec<Vec<AsciiCell{char, color}>>
  ▼
GifRenderer（fontdue 栅格化，绘制到 ImageBuffer）
  │ RGB 帧序列
  ▼
image::codecs::gif::GifEncoder
  │
  ▼
output.gif
```

## 颜色处理

| color_mode | 背景 | 文字颜色 |
|-----------|------|---------|
| `none` | 黑色 (`Rgb([0, 0, 0])`) | 白色 (`Rgb([255, 255, 255])`) |
| `ansi256` | 黑色 | 将 ANSI 256 色码转回 RGB |
| `truecolor` | 黑色 | 原始像素 RGB |

## 错误处理

复用 `MultimediaError`，无需新增错误类型：
- `FfmpegError`：解码失败
- `IoError`：文件读写失败
- `InvalidParameter`：非法参数（fps <= 0，width == 0 等）
- `FfmpegNotFound`：FFmpeg 不可用时尝试 native 回退

若字体加载失败（系统和内嵌都失败），视为 `IoError`。

## 依赖变更

**Cargo.toml**：

```toml
[dependencies]
# 修改：为 image 增加 gif feature
image = { version = "0.25", features = ["jpeg", "png", "gif"] }

# 新增
fontdue = "1.0"
```

## 边界情况

| 场景 | 处理 |
|------|------|
| 输入无视频流 | 返回 `MultimediaError::NoVideoStream` |
| `--fps` > 原视频帧率 | 正常处理，部分帧会重复或停留更久 |
| `--duration` 超出视频长度 | 导到视频结束为止 |
| 输出文件已存在 | 直接覆盖 |
| 字体加载失败 | 先尝试系统字体，再尝试内嵌字体，都失败则报错 |
| 长视频 + 高 fps | 可能生成巨大文件，当前版本不限制，后续可加 `--max-frames` |

## 未来扩展（非本次实现）

- `--max-frames` 限制总帧数
- `--loop` 控制 GIF 循环次数
- `--bg-color` / `--fg-color` 自定义背景/前景色
- 更多字符集（Unicode block elements）
