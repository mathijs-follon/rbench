# rbench

A lightweight terminal benchmarking and visualization tool built in Rust.

rbench provides an interactive TUI for running and comparing benchmarks in a structured, readable way. It focuses on speed, clarity, and portability across platforms.

---

## Features

- Interactive terminal UI (TUI)
- Benchmark execution and comparison
- Clean layout with real-time updates
- Cross-platform support (Linux, macOS, Windows)
- Built with Rust for performance and safety

---

## Installation

### From source

Make sure you have Rust installed (via https://rustup.rs):

```bash
git clone https://github.com/mathijs-follon/rbench
cd rbench
cargo build --release
````

The binary will be available at:

```bash
target/release/rbench
```

To install globally:

```bash
cargo install --path .
```

---

## Usage

Run rbench from the terminal:

```bash
rbench <options>
```

For help:

```bash
rbench --help
```

---

## Project Structure

* `src/` - Core application logic
* `ui/` - Terminal UI components
* `bench/` - Benchmark execution logic
* `config/` - Configuration handling

---

## Build Support

rbench supports Linux and Windows:

### 1. Native Linux (Recommended)

```bash
cargo build --release
./target/release/rbench
```

---


### 2. Native Windows

You can compile and run rbench directly on Windows using Rust:

#### Requirements

* Rust (MSVC toolchain recommended)
* Windows Terminal (recommended for best rendering)
* Visual Studio Build Tools or MSVC installed

Install Rust with MSVC target:

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

Then build:

```powershell
cargo build --release
```

Run:

```powershell
.\target\release\rbench.exe
```

### Notes for Windows Users

* The TUI is built using `crossterm`, which supports Windows console APIs.
* For best experience, use **Windows Terminal** instead of the legacy cmd.exe.
* If colors or layout appear broken, ensure ANSI support is enabled (usually automatic in modern terminals).
* You can also use WSL

---

## Dependencies

* `clap` - CLI parsing
* `crossterm` - terminal handling
* `ratatui` - TUI framework

---

## License

MIT License
