#![cfg_attr(not(windows), allow(dead_code))]

mod frame;
mod radar;
#[cfg(windows)]
mod serial;

#[cfg(windows)]
const DEFAULT_PORT: &str = "COM3";
#[cfg(windows)]
const DEFAULT_CONFIG: &str = "configs/symphony-hand.cfg";
#[cfg(windows)]
const READ_CHUNK: usize = 4096;
#[cfg(windows)]
const CALIBRATION_MS: u32 = 2000;
#[cfg(windows)]
const BACKGROUND_RADIUS: f32 = 0.08;
#[cfg(windows)]
const ZONE_CENTER: symphony_core::Vec3 = symphony_core::Vec3::new(0.0, 0.6, 0.0);
#[cfg(windows)]
const ZONE_HALF: symphony_core::Vec3 = symphony_core::Vec3::new(0.25, 0.2, 0.25);
#[cfg(windows)]
const START_LEVEL: f32 = 0.5;
#[cfg(windows)]
const RATE_REPORT_FRAMES: u64 = 200;
#[cfg(windows)]
const BAR_WIDTH: usize = 20;
#[cfg(windows)]
const TRACE_FLAG: &str = "--trace";

#[cfg(not(windows))]
fn main() {
    println!("Valid Symphony hub {}: the radar link runs on Windows for now.", symphony_core::VERSION);
}

#[cfg(windows)]
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use radar::Port;
    use std::time::Instant;
    use symphony_core::{Background, Clutch, Event, Tuning, Vec3, Zone};

    let all: Vec<String> = std::env::args().skip(1).collect();
    let trace = all.iter().any(|a| a == TRACE_FLAG);
    let mut args = all.into_iter().filter(|a| a != TRACE_FLAG);
    let port_name = args.next().unwrap_or_else(|| DEFAULT_PORT.to_string());
    let config_path = args.next().unwrap_or_else(|| DEFAULT_CONFIG.to_string());

    println!("Valid Symphony hub {}", symphony_core::VERSION);
    println!("Port: {port_name}");
    println!("Config: {config_path}");
    if trace {
        println!("Trace: printing every point the radar reports");
    }

    let text = std::fs::read_to_string(&config_path).map_err(|e| format!("Could not read the config file {config_path}: {e}"))?;
    let lines = radar::load_config(&text);

    let mut port = serial::SerialPort::open(&port_name, radar::BOOT_BAUD).map_err(|e| format!("Could not open {port_name}: {e}. Close TI's visualizer or anything else using the port."))?;
    radar::configure(&mut port, &lines, &mut |m| println!("{m}"))?;
    println!();
    println!("Learning the room for {} seconds. Keep your hands away from the board.", CALIBRATION_MS / 1000);

    let started = Instant::now();
    let mut reader = frame::FrameReader::new();
    let mut buf = vec![0u8; READ_CHUNK];
    let mut background = Background::new(BACKGROUND_RADIUS);
    let mut clutch = Clutch::new(Zone::new(ZONE_CENTER, ZONE_HALF), Tuning::default(), START_LEVEL);
    let mut calibrating = true;
    let mut frames: u64 = 0;
    let mut window_start = Instant::now();
    let mut shown_percent: i32 = -1;
    loop {
        let n = port.read_some(&mut buf)?;
        if n == 0 {
            continue;
        }
        reader.push(&buf[..n]);
        while let Some(f) = reader.next_frame() {
            frames += 1;
            let now = started.elapsed().as_millis() as u32;
            let points: Vec<Vec3> = f.points.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect();
            if frames == 1 {
                let list: Vec<String> = f.tlvs.iter().map(|(k, l)| format!("{k}:{l}B")).collect();
                println!("First frame ({}) carries TLVs [{}]", f.number, list.join(", "));
            }
            if calibrating {
                background.learn(&points);
                if now >= CALIBRATION_MS {
                    calibrating = false;
                    println!("Room learned: {} fixed reflections will be ignored{}.", background.len(), if background.is_full() { " (limit reached)" } else { "" });
                    println!("Hold your hand still about 60 cm in front of the board to grab. Raise to brighten, lower to dim, pull away to release.");
                    println!("Level starts at {}%.", (START_LEVEL * 100.0).round() as i32);
                }
                continue;
            }
            if trace && !f.points.is_empty() {
                let zone = *clutch.zone();
                let list: Vec<String> = f
                    .points
                    .iter()
                    .zip(&points)
                    .map(|(p, v)| {
                        let tag = if background.contains(v) { "fixed" } else if zone.contains(v) { "IN BOX" } else { "outside" };
                        format!("x {:+.2} y {:+.2} z {:+.2} v {:+.2} snr {:.0} {}", p.x, p.y, p.z, p.doppler, p.snr, tag)
                    })
                    .collect();
                println!("frame {:>6} | {}", f.number, list.join(" | "));
            }
            match clutch.update(&points, &background, now) {
                Event::Arming => println!("hand in the box, hold still..."),
                Event::Cancelled => println!("hand left before the grab"),
                Event::Grabbed { level } => {
                    shown_percent = percent(level);
                    println!("GRAB at {}%", shown_percent);
                }
                Event::Level(level) => {
                    let p = percent(level);
                    if p != shown_percent {
                        shown_percent = p;
                        println!("  {:>3}% {}", p, bar(level));
                    }
                }
                Event::Released { level } => println!("RELEASE at {}%", percent(level)),
                Event::None => {}
            }
            if frames.is_multiple_of(RATE_REPORT_FRAMES) {
                let secs = window_start.elapsed().as_secs_f32();
                println!("-- {:.1} frames/s, {} points in the last frame, {} stray bytes skipped since start", RATE_REPORT_FRAMES as f32 / secs, f.detected, reader.skipped());
                window_start = Instant::now();
            }
        }
    }
}

#[cfg(windows)]
fn percent(level: f32) -> i32 {
    (level * 100.0).round() as i32
}

#[cfg(windows)]
fn bar(level: f32) -> String {
    let filled = ((level * BAR_WIDTH as f32).round() as usize).min(BAR_WIDTH);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
}
