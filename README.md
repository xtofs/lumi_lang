# lumi


A small experimental language and runtime implemented in Rust.

The core is based on the untyped lambda calculus, with memory management inspired by the Perceus reference counting algorithm.

## Usage

- Build: `cargo build`
- Run examples (compiles to C, then runs): `cargo run --example <example_name>`

The generated C binaries are produced with debug symbols (`-g`, `-fno-omit-frame-pointer`, `-O0`) for easier debugging with `lldb` or `gdb`.

## Structure

- `src/` — Main source code
- `examples/` — Example programs
- `TODO/` — Design notes and ideas

## Requirements

- Rust toolchain (https://rustup.rs)
