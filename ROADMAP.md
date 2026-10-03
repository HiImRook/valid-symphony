# Valid Symphony Roadmap

**Current Version:** v0.0.1 - **Status: Prototype**

---

## Prototype Milestones

- ✅ **M0 - Repository.** Project created with README, ROADMAP, CHANGELOG, and a Rust workspace that builds with no external dependencies.
- 📋 **M1 - Radar alive.** TI's demo visualizer shows points from a moving hand.
- 📋 **M2 - Rust reads frames.** The hub reads radar frames at about 30 per second with no parse errors.
- 📋 **M3 - Hand in a box.** A live 0 to 1 value follows the hand smoothly, and walking behind the zone does not move it.
- 📋 **M4 - First light.** Grab, raise, lower, and release dims a LIFX bulb smoothly. Walking through the zone never changes it.
- 📋 **M5 - Tuned.** Ten grabs in a row register, the level holds steady with a still hand, and the light responds within about 100 ms.
- 📋 **M6 - Second axis.** Left and right set color temperature.

## Path to Product

- 📋 Custom board around TI's low-power IWRL6432 radar chip and a pre-certified Zigbee/Thread radio module
- 📋 Ten tester units in real homes
- 📋 Pre-compliance radio testing
- 📋 Crowd Supply campaign
- 📋 FCC certification and first production run

## Light Backends

- 📋 LIFX over the local network
- 📋 Zigbee through Zigbee2MQTT
- 📋 Home Assistant
