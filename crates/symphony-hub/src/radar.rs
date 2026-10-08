use std::fmt;
use std::io;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const BOOT_BAUD: u32 = 115_200;

const LINE_GAP: Duration = Duration::from_millis(30);
const FAST_CHAR_GAP: Duration = Duration::from_millis(1);
const ACK_TIMEOUT: Duration = Duration::from_millis(2000);
const BAUD_ACK_TIMEOUT: Duration = Duration::from_millis(500);
const BAUD_SETTLE: Duration = Duration::from_millis(100);
const ACK_DONE: &str = "Done";
const ACK_ERROR: &str = "Error";
const ACK_ERROR_CODE: &str = "Error -";
const CMD_BAUD: &str = "baudRate";
const CMD_START: &str = "sensorStart";
const CMD_STOP: &str = "sensorStop";
const COMMENT: char = '%';
const READ_CHUNK: usize = 256;

pub trait Port {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn set_baud(&mut self, baud: u32) -> io::Result<()>;
    fn clear_input(&mut self) -> io::Result<()>;
}

#[derive(Debug)]
pub enum RadarError {
    Io(io::Error),
    Rejected { command: String, reply: String },
    NoReply { command: String },
    BadBaud { command: String },
    NoStart,
}

impl fmt::Display for RadarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadarError::Io(e) => write!(f, "Serial port error: {e}"),
            RadarError::Rejected { command, reply } => write!(
                f,
                "The radar rejected \"{command}\" and configuration stopped there. Reply: {}",
                reply.trim()
            ),
            RadarError::NoReply { command } => write!(
                f,
                "No reply to \"{command}\". Unplug and replug the board's USB cable, wait about 5 seconds, then run again."
            ),
            RadarError::BadBaud { command } => write!(f, "Could not read the baud rate in \"{command}\""),
            RadarError::NoStart => write!(f, "The config has no sensorStart line, so the radar would never start"),
        }
    }
}

impl std::error::Error for RadarError {}

impl From<io::Error> for RadarError {
    fn from(e: io::Error) -> Self {
        RadarError::Io(e)
    }
}

enum Ack {
    Done,
    Error(String),
    Timeout,
}

pub fn load_config(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(COMMENT))
        .map(String::from)
        .collect()
}

pub fn configure<P: Port>(port: &mut P, lines: &[String], log: &mut dyn FnMut(&str)) -> Result<(), RadarError> {
    if !lines.iter().any(|l| first_word(l) == CMD_START) {
        return Err(RadarError::NoStart);
    }
    let fast_baud = lines
        .iter()
        .find(|l| first_word(l) == CMD_BAUD)
        .map(|l| parse_baud(l))
        .transpose()?;
    let mut baud = BOOT_BAUD;
    for (index, line) in lines.iter().enumerate() {
        let word = first_word(line);
        if word == CMD_BAUD && Some(baud) == fast_baud {
            log(&format!("> {line}  skipped (already at {baud} baud)"));
            continue;
        }
        sleep(LINE_GAP);
        send_line(port, line, baud)?;
        if word == CMD_START {
            log(&format!("> {line}"));
            return Ok(());
        }
        if word == CMD_BAUD {
            let value = parse_baud(line)?;
            match wait_ack_for(port, BAUD_ACK_TIMEOUT)? {
                Ack::Done => log(&format!("> {line}  ok")),
                Ack::Error(reply) if reply.contains(ACK_ERROR_CODE) => {
                    return Err(RadarError::Rejected { command: line.clone(), reply });
                }
                _ => log(&format!("> {line}  sent (device switched before replying)")),
            }
            sleep(BAUD_SETTLE);
            port.set_baud(value)?;
            port.clear_input()?;
            baud = value;
            log(&format!("  host switched to {value} baud"));
            continue;
        }
        let mut ack = wait_ack(port)?;
        if index == 0
            && matches!(ack, Ack::Timeout)
            && let Some(fast) = fast_baud
        {
            log(&format!("  no reply at {baud} baud, the radar may still be running at {fast} baud from a previous session"));
            port.set_baud(fast)?;
            port.clear_input()?;
            sleep(BAUD_SETTLE);
            baud = fast;
            send_line(port, line, baud)?;
            ack = wait_ack(port)?;
        }
        match ack {
            Ack::Done => log(&format!("> {line}  ok")),
            Ack::Error(reply) => {
                if word == CMD_STOP {
                    log(&format!("> {line}  skipped (not running)"));
                } else {
                    return Err(RadarError::Rejected { command: line.clone(), reply });
                }
            }
            Ack::Timeout => return Err(RadarError::NoReply { command: line.clone() }),
        }
    }
    Ok(())
}

fn parse_baud(line: &str) -> Result<u32, RadarError> {
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| RadarError::BadBaud { command: line.to_string() })
}

fn first_word(line: &str) -> &str {
    line.split_whitespace().next().unwrap_or("")
}

fn send_line<P: Port>(port: &mut P, line: &str, baud: u32) -> io::Result<()> {
    let mut bytes = line.as_bytes().to_vec();
    bytes.push(b'\n');
    if baud > BOOT_BAUD {
        for b in bytes {
            port.write_all(&[b])?;
            sleep(FAST_CHAR_GAP);
        }
        Ok(())
    } else {
        port.write_all(&bytes)
    }
}

fn wait_ack<P: Port>(port: &mut P) -> io::Result<Ack> {
    wait_ack_for(port, ACK_TIMEOUT)
}

fn wait_ack_for<P: Port>(port: &mut P, timeout: Duration) -> io::Result<Ack> {
    let deadline = Instant::now() + timeout;
    let mut text = String::new();
    let mut buf = [0u8; READ_CHUNK];
    while Instant::now() < deadline {
        let n = port.read_some(&mut buf)?;
        if n == 0 {
            continue;
        }
        text.push_str(&String::from_utf8_lossy(&buf[..n]));
        if text.contains(ACK_ERROR) {
            return Ok(Ack::Error(text));
        }
        if text.contains(ACK_DONE) {
            return Ok(Ack::Done);
        }
    }
    Ok(Ack::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct MockPort {
        written: Vec<u8>,
        pending: VecDeque<u8>,
        replies: VecDeque<(&'static str, &'static str)>,
        bauds: Vec<u32>,
        clears: usize,
        device_baud: u32,
    }

    impl MockPort {
        fn new(replies: &[(&'static str, &'static str)]) -> Self {
            Self { written: Vec::new(), pending: VecDeque::new(), replies: replies.iter().copied().collect(), bauds: Vec::new(), clears: 0, device_baud: BOOT_BAUD }
        }
    }

    impl Port for MockPort {
        fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.written.extend_from_slice(bytes);
            if self.written.ends_with(b"\n") && self.bauds.last().copied().unwrap_or(BOOT_BAUD) == self.device_baud {
                let line = String::from_utf8_lossy(&self.written).lines().last().unwrap_or("").to_string();
                if let Some((cmd, reply)) = self.replies.front().copied()
                    && line.starts_with(cmd)
                {
                    self.replies.pop_front();
                    self.pending.extend(format!("{line}\r\n{reply}\r\nmmwDemo:/>").bytes());
                }
                if let Some(rate) = line.strip_prefix("baudRate ").and_then(|v| v.trim().parse().ok()) {
                    self.device_baud = rate;
                }
            }
            Ok(())
        }
        fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut n = 0;
            while n < buf.len() {
                match self.pending.pop_front() {
                    Some(b) => {
                        buf[n] = b;
                        n += 1;
                    }
                    None => break,
                }
            }
            Ok(n)
        }
        fn set_baud(&mut self, baud: u32) -> io::Result<()> {
            self.bauds.push(baud);
            Ok(())
        }
        fn clear_input(&mut self) -> io::Result<()> {
            self.clears += 1;
            Ok(())
        }
    }

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn strips_comments_and_blanks() {
        let cfg = "% header\n\nsensorStop 0\n  channelCfg 7 3 0  \n% note\nsensorStart 0 0 0 0\n";
        assert_eq!(load_config(cfg), lines(&["sensorStop 0", "channelCfg 7 3 0", "sensorStart 0 0 0 0"]));
    }

    #[test]
    fn switches_baud_after_ack_and_starts() {
        let mut port = MockPort::new(&[("sensorStop", "Error -1"), ("channelCfg", "Done"), ("baudRate", "Done")]);
        let cfg = lines(&["sensorStop 0", "channelCfg 7 3 0", "baudRate 1250000", "sensorStart 0 0 0 0"]);
        let mut log = Vec::new();
        configure(&mut port, &cfg, &mut |m| log.push(m.to_string())).expect("configured");
        assert_eq!(port.bauds, vec![1_250_000]);
        assert_eq!(port.clears, 1);
        assert!(String::from_utf8_lossy(&port.written).ends_with("sensorStart 0 0 0 0\n"));
        assert!(log.iter().any(|l| l.contains("skipped")));
    }

    #[test]
    fn continues_when_baud_rate_gets_no_reply() {
        let mut port = MockPort::new(&[("channelCfg", "Done"), ("frameCfg", "Done")]);
        let cfg = lines(&["channelCfg 7 3 0", "baudRate 1250000", "frameCfg 2 8 600 16 200 0", "sensorStart 0 0 0 0"]);
        let mut log = Vec::new();
        configure(&mut port, &cfg, &mut |m| log.push(m.to_string())).expect("configured");
        assert_eq!(port.bauds, vec![1_250_000]);
        assert!(String::from_utf8_lossy(&port.written).ends_with("sensorStart 0 0 0 0\n"));
        assert!(log.iter().any(|l| l.contains("switched before replying")));
        assert!(log.iter().any(|l| l.starts_with("> frameCfg") && l.ends_with("ok")));
    }

    #[test]
    fn recovers_when_radar_is_still_at_fast_baud() {
        let mut port = MockPort::new(&[("sensorStop", "Done"), ("channelCfg", "Done"), ("frameCfg", "Done")]);
        port.device_baud = 1_250_000;
        let cfg = lines(&["sensorStop 0", "channelCfg 7 3 0", "baudRate 1250000", "frameCfg 2 8 600 16 50 0", "sensorStart 0 0 0 0"]);
        let mut log = Vec::new();
        configure(&mut port, &cfg, &mut |m| log.push(m.to_string())).expect("configured");
        assert_eq!(port.bauds, vec![1_250_000]);
        assert!(log.iter().any(|l| l.contains("previous session")));
        assert!(log.iter().any(|l| l.contains("skipped (already at 1250000 baud)")));
        assert!(log.iter().any(|l| l.starts_with("> frameCfg") && l.ends_with("ok")));
    }

    #[test]
    fn stops_on_rejected_command() {
        let mut port = MockPort::new(&[("channelCfg", "Done"), ("frameCfg", "Error -5")]);
        let cfg = lines(&["channelCfg 7 3 0", "frameCfg 2 8 600 16 200 0", "guiMonitor 2", "sensorStart 0 0 0 0"]);
        let err = configure(&mut port, &cfg, &mut |_| {}).expect_err("rejected");
        assert!(matches!(err, RadarError::Rejected { ref command, .. } if command.starts_with("frameCfg")));
        assert!(!String::from_utf8_lossy(&port.written).contains("guiMonitor"));
    }

    #[test]
    fn requires_sensor_start() {
        let mut port = MockPort::new(&[]);
        let err = configure(&mut port, &lines(&["channelCfg 7 3 0"]), &mut |_| {}).expect_err("no start");
        assert!(matches!(err, RadarError::NoStart));
        assert!(port.written.is_empty());
    }
}
