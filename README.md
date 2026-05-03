# lumi


A small experimental language and runtime implemented in Rust.

The core is based on the untyped lambda calculus, with memory management inspired by the Perceus reference counting algorithm.

## Usage

- Build: `cargo build`
- Run examples (i.e. compile to c): `cargo run --example <example_name>`
- also compile the C code and run: `./demo.sh <example_name>`

## Structure

- `src/` — Main source code
- `examples/` — Example programs
- `TODO/` — Design notes and ideas

## Requirements

- Rust toolchain (https://rustup.rs)
