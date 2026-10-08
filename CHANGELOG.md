# Changelog

All notable changes to Valid Symphony will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.4] - 2026-10-07

The radar now tracks a moving hand at 20 frames per second under a configuration of our own, and the hand control logic exists in `symphony-core`. The grab and dimming have passed simulated tests but are not yet proven on the board, so M3 stays open.

### Added
- `configs/symphony-hand.cfg`, based on TI's motion detection profile with four changes: 20 frames per second instead of 5, range limited to 1.2 m, low-power mode off, and clutter removal kept on.
- Hand control in `symphony-core`, with no allocation and no operating system: a movable control box, a background filter that learns the room's fixed reflections at startup, and the grab clutch. Holding a hand still in the box grabs at the current level, raising and lowering changes it relative to where the grab started, drifting outside the box keeps tracking, and pulling away releases. A torso behind the hand cannot hold a grab.
- 13 simulated scenarios for the clutch: grab, relative dimming, clamping at the ends, drifting outside the box, release, walking through, people beyond the box, background objects, brief dropouts, and regrab rules.
- Hub learns the room for 2 seconds at startup, then prints grab, level, and release events with a level bar.
- `--trace` option on the hub that prints every point the radar reports, tagged as in the box, outside, or fixed.
- Restart without a replug: if the radar is still streaming at 1,250,000 baud from a previous session, the hub detects it and continues at that rate. Possible now that low-power mode is off.

### Findings
- Clutter removal off made the radar lose moving hands entirely. The only detections were a fixed object at zero velocity, so clutter removal stays on.
- With clutter removal on, a moving hand is detected clearly, with speeds from 0.25 to 0.76 m/s and several points per frame.
- Up and down position is coarse in TI's profile: height only ever read as level, about 30 degrees up, or about 30 degrees down. The elevation angle step count in `sigProcChainCfg` is 4. Raising it is the next test, since up and down is the M3 control axis.
- The radar accepted 20 frames per second and held it steadily.
- Whether a hand held still stays visible with clutter removal on is not yet tested. If it disappears, the grab will change from holding still to hovering in one spot.

### Removed
- Unused nearest point and distance helpers in the hub's frame reader.

## [0.0.3] - 2026-10-07

### Added
- Radar protocol notes in `docs/radar-protocol.md`: serial settings, the configuration handshake, the baudRate switch, low-power mode and restarting, and the frame and point formats, with measured behavior marked as observed.

### Changed
- CI security audit now installs the pinned Rust 1.88.0 toolchain and pins both GitHub Actions to exact commits.
- README privacy wording: no camera and no images, in place of the claim that radar cannot see rooms.

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