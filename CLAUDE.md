# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Controller III is a multi-purpose CLI tool written in Rust (edition 2024). It combines two main feature areas:
1. **Fast file search**: Dual-engine architecture with NTFS MFT direct reading (1-3 seconds full disk search on Windows) + cross-platform generic parallel traversal
2. **Multimedia processing**: Audio/video processing with trait-based backend abstraction - format conversion, frame extraction, trimming, audio extraction, and ASCII video playback in terminal.

Key features:
- **Dual-engine search**: NTFS MFT direct reading (Windows-only, 10x speedup) + cross-platform generic parallel traversal
- **Smart sorting**: "User files first" heuristic scoring algorithm prioritizes user documents over system files
- **Flexible matching**: Supports glob patterns (`*`, `?`)
- **Two modes**: Interactive menu and headless command-line search
- **Auto-elevation**: Automatically requests admin privileges for NTFS MFT access on Windows
- **Multimedia processing**: Get media info, transcode/convert, extract frames, trim, extract audio, play video as ASCII art in terminal
- **Pluggable backends**: Trait-based abstraction allows multiple backends (currently FFmpeg-based)

## Commands

### Build
```bash
cargo build              # Debug build
cargo build --release    # Release build (optimized)
# Build without FFmpeg support (smaller binary)
cargo build --release --no-default-features
```

### Run
```bash
# Interactive mode (file search)
cargo run

# Headless file search with arguments
cargo run -- [arguments]
cargo run -- --search "*.txt"        # Example: search for .txt files
cargo run -- --search "*.rs" --root . --force-generic  # Force generic search

# Multimedia commands
cargo run -- multimedia info input.mp4                    # Show media information
cargo run -- multimedia transcode input.mp4 output.webm --bitrate 2000 --resolution 1280x720  # Transcode
cargo run -- multimedia extract-frames input.mp4 frames/ --every 10  # Extract every 10th frame
cargo run -- multimedia trim input.mp4 output.mp4 --start 10.5 --duration 30  # Trim
cargo run -- multimedia extract-audio input.mp4 output.mp3 --bitrate 192  # Extract audio
cargo run -- multimedia play-ascii input.mp4 --speed 1.0 --color-mode truecolor  # Play as ASCII
```

### Test
```bash
cargo test                  # Run all tests
cargo test -- --nocapture   # Run tests with output visible
cargo test test_search      # Run single test
```

### Clean
```bash
cargo clean
```

## Architecture

### Module Structure
```
src/
├── main.rs                 # Entry point, auto-elevation logic, Windows ANSI init
├── cli/
│   └── args.rs             # CLI argument definitions (clap) - includes multimedia subcommands
├── modes/
│   ├── interactive.rs      # Interactive menu mode (dialoguer)
│   └── headless.rs         # Headless command-line search mode
├── search/                 # File search functionality
│   ├── engine.rs           # SearchEngine trait + factory (auto-select engine)
│   ├── entry.rs            # FileEntry struct (file metadata)
│   ├── sort.rs             # Parallel sorting with multi-factor scoring
│   ├── filter.rs           # Query to regex conversion
│   ├── generic/
│   │   └── walk_dir.rs     # jwalk-based parallel directory traversal engine
│   └── ntfs/
│       └── mft_reader.rs   # NTFS MFT direct reading engine (Windows-only)
└── multimedia/             # Multimedia processing (new)
    ├── mod.rs              # Module exports and public API
    ├── error.rs            # MultimediaError type
    ├── traits.rs           # Trait definitions: MediaInfoProvider, Transcoder, FrameExtractor, AsciiPlayer
    ├── media_info.rs       # MediaInfo struct (duration, resolution, streams, codecs)
    ├── info.rs             # Get media information facade (auto-select backend)
    ├── transcode.rs        # Transcoding facade
    ├── extract.rs          # Frame extraction facade
    ├── trim.rs             # Trimming facade
    ├── extract_audio.rs    # Audio extraction facade
    ├── ascii/              # ASCII video conversion and terminal playback
    │   ├── mod.rs          # Playback entry point
    │   ├── convert.rs      # Frame to ASCII conversion
    │   ├── terminal.rs     # Terminal rendering and input handling
    │   └── ffmpeg.rs       # FFmpeg-based frame extraction for playback
    ├── ffmpeg/             # FFmpeg-based backend (uses ffmpeg-the-third)
    │   ├── mod.rs
    │   ├── info.rs         # FFmpeg media info implementation
    │   ├── transcode.rs    # FFmpeg transcoding implementation
    │   ├── extract.rs      # FFmpeg frame extraction implementation
    │   ├── trim.rs         # FFmpeg trimming implementation
    │   └── extract_audio.rs # FFmpeg audio extraction implementation
    └── native/             # Native (pure Rust) backend (incomplete)
        ├── mod.rs
        ├── info.rs
        └── extract.rs
```

### Search Engines
1. **NTFS MFT Engine** (Windows only): Reads NTFS Master File Table directly for ~10x speedup over generic traversal
2. **Generic Engine**: Cross-platform parallel directory traversal using jwalk

### Multimedia Architecture
- **Trait-based abstraction**: All multimedia operations are defined by traits in `multimedia::traits`
- **Backend selection**: Facade functions in `multimedia/*` automatically select the available backend (prefers FFmpeg)
- **FFmpeg backend**: Uses `ffmpeg-the-third` (Rust bindings to FFmpeg 8.1) for robust processing
- **Feature flag**: FFmpeg is enabled by default but can be disabled via `--no-default-features`

### "User Files First" Scoring Algorithm
Lower score = higher ranking:
- Users/Desktop/Documents/Downloads: -50 points
- Windows/Program Files: +50 points
- Current user owned (Windows): -30 points
- System account owned (Windows): +30 points
- Document extensions: -10 points
- System file extensions: +10 points
- Newer files get slight additional priority

## Key Dependencies

- **CLI**: clap (derive + color)
- **Interactive UI**: dialoguer
- **File search**: jwalk (parallel traversal), regex, rayon (parallel sort)
- **NTFS (Windows)**: mft, jiff, windows (Win32 API)
- **Multimedia**: ffmpeg-the-third, ffmpeg-sys-the-third, image (JPEG/PNG), crossterm (terminal), cpal (audio)
- **Error handling**: anyhow, thiserror

## Platform Notes

- NTFS MFT engine is **Windows-only** and requires administrator privileges
- Automatically falls back to generic engine on non-Windows platforms or when privileges are not available
- Auto-elevation (UAC prompt) is implemented for Windows when NTFS access is requested without admin rights
- FFmpeg multimedia processing is cross-platform but requires linking to FFmpeg libraries

## Performance

| Scenario | NTFS MFT | Generic Traversal |
|----------|----------|-------------------|
| Full disk (1M+ files) | 1-3s | 10-30s |
| User directory (10k files) | <1s | 1-3s |

## Features

Features in `Cargo.toml`:
- `default = ["ffmpeg"]`: Enable FFmpeg multimedia backend by default
- `ffmpeg`: Enables FFmpeg dependencies and backend
