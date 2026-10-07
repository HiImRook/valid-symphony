# Changelog

All notable changes to Valid Symphony will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2026-10-07

### Added
- Radar link in `symphony-hub`: a Windows serial port written directly against the operating system, with no external crates.
- Radar configuration sender that follows TI's host handshake for the L-SDK motion and presence demo, including the switch to 1,250,000 baud.
- Frame reader that resyncs on the frame marker and decodes the radar's compressed and float point clouds.
- Live console readout of each frame's point count and nearest point, plus frame rate.

### Changed
- Prototype radar board is the TI IWRL6432AOPEVM.

## [0.0.1] - 2026-10-03

### Added
- Project repository with README, ROADMAP, and CHANGELOG.
- Rust workspace with two crates: `symphony-core` (no_std control logic) and `symphony-hub` (the hub program). No external dependencies.
- Vendored dependency workflow: pinned Rust 1.88.0 toolchain, CI security audit, and offline locked release builds.