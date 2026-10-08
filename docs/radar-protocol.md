# Radar Protocol Notes

How the Valid Symphony hub talks to TI's IWRL6432AOPEVM running the motion and presence detection demo from the mmWave L-SDK (`MMWAVE_L_SDK_05_05_04_02`), with the `MotionDetect.cfg` profile. These are working notes from building a host in Rust with no external crates. Items marked **observed** were measured on our board, not taken from TI documentation.

Written for anyone building their own host for this board, in any language.

---

## Hardware Setup

- Use the top micro-USB port. The onboard XDS110 shows up as two serial ports on Windows. On our machine they were COM3 and COM4, and COM3 is the application UART that carries both configuration and data.
- SW1 is the black slide switch. Up, toward the blue button, is functional mode. Down is flashing mode. It must be up to run the demo.
- The XDS110's CMSIS-DAP entries may show as missing drivers in Device Manager. They are only needed for flashing and can be ignored for normal use.
- Only one program can hold the port. "Access is denied" when opening it means TI's visualizer or another tool still has it open.

## Serial Settings

- 8 data bits, no parity, 1 stop bit, binary mode, DTR and RTS on.
- The device boots at **115,200 baud**.
- Configuration and the binary frame stream share the same single UART.

## Sending a Configuration

1. Read the `.cfg` file, drop blank lines and lines starting with `%`.
2. Send each line followed by `\n`, with about **30 ms** between lines.
3. After each line, read until the reply contains `Done` (accepted) or `Error` (rejected). Two seconds was a comfortable timeout.
4. `sensorStop` returns an error when the sensor is not running. That is normal at startup and safe to ignore.
5. `sensorStart` is the last line. Do not wait for `Done` after it: the binary stream starts immediately and the reply is mixed into it.

### The baudRate line

`MotionDetect.cfg` contains `baudRate 1250000`. This switches the device's UART for everything that follows.

- **Observed:** the device switches rate before its `Done` reaches a host still listening at 115,200, so the reply is usually missing or garbled. TI's own Python host does not require `Done` for this line either. It reads briefly, then switches.
- What works: after sending `baudRate`, wait briefly (500 ms) for a reply but do not treat silence or garbage as an error. Then wait about 100 ms, switch the host port to the new rate, purge the input buffer, and continue.
- At 1,250,000 baud, send the remaining lines **one character at a time with about 1 ms between characters**. Sending a whole line at once at this rate was unreliable.

### Low-power mode and restarting

`MotionDetect.cfg` sets `lowPowerCfg 1`.

- In this mode the device does not support `sensorStop`, so a running sensor cannot be stopped from the host.
- **Observed:** after the host exits, the board keeps streaming at 1,250,000 baud. The next run, which opens the port at 115,200, gets no reply to its first line.
- Fix: unplug and replug the USB cable, wait about 5 seconds for the board to boot and Windows to bring the port back, then run again.
- A configuration with `lowPowerCfg 0` should allow a clean `sensorStop` and restart. Not yet tested.

### Flash safety

Configuration lines only change how the sensor runs until power is removed. A rejected line returns `Error` and changes nothing persistent. The one line that touches flash is `factoryCalibCfg`, which in TI's profile restores calibration data from a reserved flash sector (`0x1ff000`). Leave it exactly as TI wrote it.

## Frame Format

All values are little endian.

### Frame header (40 bytes)

| Offset | Type | Field |
| --- | --- | --- |
| 0 | 8 bytes | Magic word `02 01 04 03 06 05 08 07` |
| 8 | u32 | Version |
| 12 | u32 | Total packet length in bytes, including this header |
| 16 | u32 | Platform |
| 20 | u32 | Frame number |
| 24 | u32 | Time in CPU cycles |
| 28 | u32 | Number of detected points |
| 32 | u32 | Number of TLVs |
| 36 | u32 | Subframe number |

To read the stream reliably, search for the magic word, read the total length, wait until that many bytes have arrived, then parse. If the length is impossible (we reject anything over 64 KiB), skip one byte and search again. **Observed:** after configuration, a few dozen stray bytes of command echo precede the first magic word, and none after that.

### TLVs

Each TLV is an 8 byte header, then its payload:

| Offset | Type | Field |
| --- | --- | --- |
| 0 | u32 | Type |
| 4 | u32 | Payload length in bytes (header not included) |

### TLV 301: compressed point cloud

Only present in frames that detected something. A 20 byte units block, then one 10 byte record per point.

Units block:

| Offset | Type | Field |
| --- | --- | --- |
| 0 | f32 | xyz unit (meters per count) |
| 4 | f32 | Doppler unit (m/s per count) |
| 8 | f32 | SNR unit |
| 12 | f32 | Noise unit |
| 16 | u16 x 2 | Point counts (major and minor motion) |

Point record:

| Offset | Type | Field |
| --- | --- | --- |
| 0 | i16 | x |
| 2 | i16 | y |
| 4 | i16 | z |
| 6 | i16 | Doppler |
| 8 | u8 | SNR |
| 9 | u8 | Noise |

Multiply each field by its unit. A useful sanity check: `(payload length - 20)` is always a multiple of 10.

### TLV 1 and TLV 7: float point cloud and side info

Some profiles send uncompressed points instead: TLV 1 holds 16 bytes per point (x, y, z, Doppler as f32), and TLV 7 holds 4 bytes per point (SNR and noise as u16, in units of 0.1 dB).

### Other TLVs seen with MotionDetect.cfg

- **Observed:** TLV 302, 512 bytes every frame. Range profile. Not decoded yet.
- **Observed:** TLV 306, 24 bytes every frame. Processing statistics. Not decoded yet.

## What MotionDetect.cfg Gives You

- **Frame rate:** `frameCfg ... 200 ...` sets a 200 ms frame period, 5 frames per second. **Observed:** 4.7 per second at the host.
- **Moving targets only:** `clutterRemoval 1` drops anything stationary, so a hand held perfectly still can disappear from the point cloud.
- **Coarse velocity:** **Observed** Doppler steps of about 0.25 m/s.
- **Coordinates:** relative to the sensor. y points straight out from the board's face, x is to the side, z is up and down. `sensorPosition` sets the mounting height and tilt used by the demo's own boundary boxes. **Observed:** the point coordinates we read stayed relative to the board.

## Tuning for Hand Control

Findings from adapting `MotionDetect.cfg` for close-range hand tracking (Valid Symphony's `configs/symphony-hand.cfg`). All **observed** on our board.

- **Frame rate:** changing `frameCfg 2 8 600 16 200 0` to `frameCfg 2 8 600 16 50 0` gave a steady 20 frames per second with no other changes.
- **Range:** `rangeSelCfg 0.25 1.2` was accepted and limits detections to about 1.2 m.
- **Low power off:** `lowPowerCfg 0` was accepted. In this mode `sensorStop` should work, so a host can restart the sensor without a replug.
- **Clutter removal must stay on.** With `clutterRemoval 0`, a moving hand stopped producing points altogether. The only detections left were a fixed object near the threshold, always at exactly zero velocity. With `clutterRemoval 1`, the moving hand returned clearly. Turning it off does not make a still hand visible in this demo.
- **Elevation is coarse by default.** In `sigProcChainCfg 64 4 ...`, the second value appears to be the elevation angle step count. With 4, every point's height came out at one of three angles: level, about 30 degrees up, or about 30 degrees down (height divided by distance was always 0 or about 0.5). Azimuth, set to 64, varied smoothly. The antenna array is also narrower vertically than horizontally, so elevation will always be the less precise axis.
- **Velocity steps** stayed at about 0.25 m/s, the same as the original profile.

## Source

The Rust implementation lives in `crates/symphony-hub/src`: `serial.rs` (Windows serial port), `radar.rs` (configuration and the baudRate handshake), and `frame.rs` (frame reader and point decoding).
