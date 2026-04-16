# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Controller III is a fast file search CLI tool written in Rust (edition 2024).

Key features:
- **Dual-engine architecture**: NTFS MFT direct reading (Windows-only, 1-3 seconds full disk search) + cross-platform generic parallel traversal
- **Smart sorting**: "User files first" heuristic scoring algorithm prioritizes user documents over system files
- **Flexible matching**: Supports glob patterns (`*`, `?`)
- **Two modes**: Interactive menu and headless command-line search
- **Auto-elevation**: Automatically requests admin privileges for NTFS MFT access on Windows

## Commands

### Build
```bash
cargo build              # Debug build
cargo build --release    # Release build (optimized)
```

### Run
```bash
cargo run                            # Interactive mode
cargo run -- [arguments]             # Headless mode with arguments
cargo run -- --search "*.txt"        # Example: search for .txt files
cargo run -- --search "*.rs" --root . --force-generic  # Force generic search
```

### Test
```bash
cargo test                  # Run all tests
cargo test -- --nocapture   # Run tests with output visible
```

### Clean
```bash
cargo clean
```

## Architecture

### Module Structure
```
src/
├── main.rs                 # Entry point, auto-elevation logic
├── cli/
│   └── args.rs             # CLI argument definitions (clap)
├── modes/
│   ├── interactive.rs      # Interactive menu mode (dialoguer)
│   └── headless.rs         # Headless command-line search mode
└── search/
    ├── engine.rs           # SearchEngine trait + factory (auto-select engine)
    ├── entry.rs            # FileEntry struct (file metadata)
    ├── sort.rs             # Parallel sorting with multi-factor scoring
    ├── filter.rs           # Query to regex conversion
    ├── generic/
    │   └── walk_dir.rs     # jwalk-based parallel directory traversal
    └── ntfs/
        └── mft_reader.rs   # NTFS MFT direct reading engine (Windows-only)
```

### Search Engines
1. **NTFS MFT Engine** (Windows only): Reads NTFS Master File Table directly for ~10x speedup over generic traversal
2. **Generic Engine**: Cross-platform parallel directory traversal using jwalk

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
- **Parallel traversal**: jwalk
- **Parallel sorting**: rayon
- **Pattern matching**: regex
- **NTFS (Windows)**: mft, jiff, windows (Win32 API)
- **Error handling**: anyhow, thiserror

## Platform Notes

- NTFS MFT engine is **Windows-only** and requires administrator privileges
- Automatically falls back to generic engine on non-Windows platforms or when privileges are not available
- Auto-elevation (UAC prompt) is implemented for Windows when NTFS access is requested without admin rights

## Performance

| Scenario | NTFS MFT | Generic Traversal |
|----------|----------|-------------------|
| Full disk (1M+ files) | 1-3s | 10-30s |
| User directory (10k files) | <1s | 1-3s |
