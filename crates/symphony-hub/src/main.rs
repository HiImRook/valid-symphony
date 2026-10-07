#![cfg_attr(not(windows), allow(dead_code))]

mod frame;
mod radar;
#[cfg(windows)]
mod serial;

#[cfg(windows)]
const DEFAULT_PORT: &str = "COM3";
#[cfg(windows)]
const DEFAULT_CONFIG: &str = r"C:\ti\MMWAVE_L_SDK_05_05_04_02\examples\mmw_demo\motion_and_presence_detection\profiles\xwrL64xx-aop\MotionDetect.cfg";
#[cfg(windows)]
const READ_CHUNK: usize = 4096;
#[cfg(windows)]
const TLV_REPORT_FRAMES: usize = 3;
#[cfg(windows)]
const RATE_REPORT_FRAMES: u64 = 50;

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

    let mut args = std::env::args().skip(1);
    let port_name = args.next().unwrap_or_else(|| DEFAULT_PORT.to_string());
    let config_path = args.next().unwrap_or_else(|| DEFAULT_CONFIG.to_string());

    println!("Valid Symphony hub {}", symphony_core::VERSION);
    println!("Port: {port_name}");
    println!("Config: {config_path}");

    let text = std::fs::read_to_string(&config_path).map_err(|e| format!("Could not read the config file {config_path}: {e}"))?;
    let lines = radar::load_config(&text);

    let mut port = serial::SerialPort::open(&port_name, radar::BOOT_BAUD).map_err(|e| format!("Could not open {port_name}: {e}. Close TI's visualizer or anything else using the port."))?;
    radar::configure(&mut port, &lines, &mut |m| println!("{m}"))?;
    println!("Radar running. Press Ctrl+C to stop.");

    let mut reader = frame::FrameReader::new();
    let mut buf = vec![0u8; READ_CHUNK];
    let mut frames: u64 = 0;
    let mut window_start = Instant::now();
    loop {
        let n = port.read_some(&mut buf)?;
        if n == 0 {
            continue;
        }
        reader.push(&buf[..n]);
        while let Some(f) = reader.next_frame() {
            frames += 1;
            if frames as usize <= TLV_REPORT_FRAMES {
                let list: Vec<String> = f.tlvs.iter().map(|(k, l)| format!("{k}:{l}B")).collect();
                println!("frame {} carries TLVs [{}]", f.number, list.join(", "));
            }
            match f.nearest() {
                Some(p) => println!(
                    "frame {:>6}  points {:>3}  nearest  x {:+.2}  y {:+.2}  z {:+.2} m  v {:+.2} m/s  snr {:.0} dB",
                    f.number,
                    f.detected,
                    p.x,
                    p.y,
                    p.z,
                    p.doppler,
                    p.snr
                ),
                None => println!("frame {:>6}  points {:>3}", f.number, f.detected),
            }
            if frames % RATE_REPORT_FRAMES == 0 {
                let secs = window_start.elapsed().as_secs_f32();
                println!("-- {:.1} frames/s, {} stray bytes skipped since start", RATE_REPORT_FRAMES as f32 / secs, reader.skipped());
                window_start = Instant::now();
            }
        }
    }
}
