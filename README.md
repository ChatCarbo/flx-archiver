# FLX Archiver

FLX Archiver is a modern, high-performance Windows desktop archiving tool built in Rust and egui. It features a sleek Mauve & Slate design, smart file analysis, multi-format support (ZIP, 7z, Zstd, Gzip), an archive explorer, and a performance dashboard. Open-source under the MIT license.

---

## Features

- **Smart Analysis**: Automatically analyzes dropped files or folders to recommend the best compression format and level.
- **Multi-Format Support**: Handle **ZIP**, **7z**, **Zstd**, and **Gzip** seamlessly.
- **Archive Explorer**: Inspect, add, and extract contents using a clean, intuitive interface.
- **Performance Dashboard**: Track historical compression ratios, sizes, and speeds (MB/s).
- **Asynchronous & Fast**: Powered by Rust, `egui`, and `tokio` for a smooth, non-blocking experience.

---

## Tech Stack

- **Language**: Rust
- **GUI Framework**: `eframe` / `egui` (v0.28.1)
- **Runtime**: `tokio`
- **Compression Crates**: `zip`, `sevenz-rust`, `zstd`, `flate2`, `tar`

---

## Building from Source

Make sure you have [Rust](https://www.rust-lang.org/) installed on your system.

```bash
# Clone the repository
git clone [https://github.com/ChatCarbo/flx-archiver.git](https://github.com/ChatCarbo/flx-archiver.git)
cd flx-archiver

# Build in release mode
cargo build --release

# Run the application
cargo run --release