# Stewart

A minimalist, high-performance, and non-exclusive actor system.

- Minimalist: Starts from a small self-contained and thread-local actor system with minimal
assumptions. Everything else is built on top, including threading!
- High-Performance: Built around real-time rendering use cases. Fearlessly use stewart for
anything!
- Non-Exclusive: Plays nicely with other actor systems, async runtimes, web-workers, GPU pipelines,
distributed frameworks, etc... Stewart doesn't limit what you can interact with.

## Why Another Actor Library?

While many actor libraries already exist in Rust, they are usually designed for web servers.
In most frameworks, CPU performance and latency are negligible compared to the cost of IO, and the
framework is expected to run as a native binary.
Stewart doesn't make these assumption, and can be run in 'weird' contexts.

## Usage Guide

[Read the stewart book for a detailed usage guide.](docs/stewart.md)

## Crates

- [![crates.io](https://img.shields.io/crates/v/stewart.svg?label=stewart)](https://crates.io/crates/stewart) [![docs.rs](https://docs.rs/stewart/badge.svg)](https://docs.rs/stewart/) - A minimalist, high-performance, modular, and non-exclusive actor system.
- `stewart-mio` -  Mio event loop runner for stewart.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License (Expat) ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
