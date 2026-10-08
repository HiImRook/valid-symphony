# Valid Symphony

**Conduct your life.**

Touchless home control. Hold your hand in the air, and your lights follow it. A 60 GHz radar reads where your hand is, and your home responds smoothly and instantly, with no camera, no microphone, no account, and no cloud.

**Status: Prototype.** The radar board is running, and the hub tracks a moving hand live. Hand control is built and being tuned on the board. See [ROADMAP.md](ROADMAP.md) for what's planned and what's done.

**Community:** updates and discussion on the [Valid Discord](https://discord.gg/2SP383cJs9).

---

## How It Works

1. A radar sensor watches a small, invisible control zone in front of it.
2. Hold your hand still in the zone for half a second to grab control. The light gives a small dip to confirm.
3. Raise or lower your hand, and the brightness follows it continuously, like a dimmer you never touch.
4. Pull your hand away, and the light stays where you left it.

Walking through the zone does nothing. Only a deliberate grab changes anything.

## Principles

- **Local only.** The sensor talks to a computer in your home, and that computer talks to your lights. Nothing reaches the internet.
- **Nothing kept.** Hand positions exist in memory for a fraction of a second. No logs, no history, nothing written to disk.
- **No camera, no images.** Radar measures distance and motion.
- **Your lights, your choice.** Works with the lights you already own, starting with LIFX and Zigbee.
- **Open source.** All project code and hardware designs are public.
- **Locked supply chain.** Every dependency is vendored and audited on each commit, and release builds run offline on a pinned Rust toolchain.
- **Engineered, assembled, and chips designed in the USA.** Texas Instruments radar, designed in Dallas.

## Prototype Hardware

| Part | Role |
| --- | --- |
| TI IWRL6432AOPEVM | 60 GHz radar evaluation board, streams 3D points over USB |
| A Windows PC | Runs the control software (Linux later) |
| LIFX bulbs | First light backend, controlled over the local network |

## Architecture

- **`symphony-core`**: hand tracking, the grab-and-release clutch, smoothing, and output pacing. Written once in Rust, built to run on the hub today and on the product's radar chip later.
- **`symphony-hub`**: reads the radar over USB through a serial link written directly against the operating system, with no external crates, runs the core, and drives the lights through interchangeable backends (LIFX first, Zigbee and Home Assistant next).

Notes on talking to the radar, including TI's configuration handshake and frame format, are in [docs/radar-protocol.md](docs/radar-protocol.md).

## Contributing

Contributions are welcome. The codebase follows strict conventions: no comments in code, constants in SCREAMING_SNAKE_CASE, in-memory state, and no new dependencies without discussion first.

## License

To be decided before the first code release.

---

&copy; 2026 by Rook. Part of the Valid ecosystem.