pub const MAGIC: [u8; 8] = [0x02, 0x01, 0x04, 0x03, 0x06, 0x05, 0x08, 0x07];
pub const HEADER_LEN: usize = 40;
pub const TLV_HEADER_LEN: usize = 8;
pub const MAX_PACKET_LEN: usize = 65_536;
pub const TLV_POINTS_FLOAT: u32 = 1;
pub const TLV_POINTS_SIDE_INFO: u32 = 7;
pub const TLV_POINTS_COMPRESSED: u32 = 301;

const OFFSET_TOTAL_LEN: usize = 12;
const OFFSET_FRAME_NUMBER: usize = 20;
const OFFSET_DETECTED: usize = 28;
const OFFSET_NUM_TLVS: usize = 32;
const COMPRESSED_UNITS_LEN: usize = 20;
const COMPRESSED_POINT_LEN: usize = 10;
const FLOAT_POINT_LEN: usize = 16;
const SIDE_INFO_LEN: usize = 4;
const SIDE_INFO_SNR_SCALE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub doppler: f32,
    pub snr: f32,
}

#[derive(Debug)]
pub struct Frame {
    pub number: u32,
    pub detected: u32,
    pub tlvs: Vec<(u32, u32)>,
    pub points: Vec<Point>,
}

pub struct FrameReader {
    buf: Vec<u8>,
    skipped: u64,
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameReader {
    pub fn new() -> Self {
        Self { buf: Vec::new(), skipped: 0 }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    pub fn next_frame(&mut self) -> Option<Frame> {
        loop {
            match find_magic(&self.buf) {
                Some(start) => {
                    if start > 0 {
                        self.skipped += start as u64;
                        self.buf.drain(..start);
                    }
                }
                None => {
                    let keep = MAGIC.len() - 1;
                    if self.buf.len() > keep {
                        let cut = self.buf.len() - keep;
                        self.skipped += cut as u64;
                        self.buf.drain(..cut);
                    }
                    return None;
                }
            }
            if self.buf.len() < HEADER_LEN {
                return None;
            }
            let total = le_u32(&self.buf, OFFSET_TOTAL_LEN) as usize;
            if !(HEADER_LEN..=MAX_PACKET_LEN).contains(&total) {
                self.skipped += 1;
                self.buf.drain(..1);
                continue;
            }
            if self.buf.len() < total {
                return None;
            }
            let packet: Vec<u8> = self.buf.drain(..total).collect();
            return Some(parse_packet(&packet));
        }
    }
}

fn find_magic(buf: &[u8]) -> Option<usize> {
    buf.windows(MAGIC.len()).position(|w| w == MAGIC)
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_i16(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn le_f32(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn parse_packet(packet: &[u8]) -> Frame {
    let number = le_u32(packet, OFFSET_FRAME_NUMBER);
    let detected = le_u32(packet, OFFSET_DETECTED);
    let num_tlvs = le_u32(packet, OFFSET_NUM_TLVS);
    let mut tlvs = Vec::new();
    let mut points = Vec::new();
    let mut side_snr: Vec<f32> = Vec::new();
    let mut at = HEADER_LEN;
    for _ in 0..num_tlvs {
        if at + TLV_HEADER_LEN > packet.len() {
            break;
        }
        let kind = le_u32(packet, at);
        let len = le_u32(packet, at + 4) as usize;
        let start = at + TLV_HEADER_LEN;
        let end = start + len;
        if end > packet.len() {
            break;
        }
        tlvs.push((kind, len as u32));
        let payload = &packet[start..end];
        match kind {
            TLV_POINTS_COMPRESSED => points = parse_compressed(payload),
            TLV_POINTS_FLOAT => points = parse_float(payload),
            TLV_POINTS_SIDE_INFO => side_snr = parse_side_snr(payload),
            _ => {}
        }
        at = end;
    }
    if !side_snr.is_empty() && side_snr.len() == points.len() {
        for (p, snr) in points.iter_mut().zip(side_snr) {
            p.snr = snr;
        }
    }
    Frame { number, detected, tlvs, points }
}

fn parse_compressed(payload: &[u8]) -> Vec<Point> {
    if payload.len() < COMPRESSED_UNITS_LEN {
        return Vec::new();
    }
    let xyz_unit = le_f32(payload, 0);
    let doppler_unit = le_f32(payload, 4);
    let snr_unit = le_f32(payload, 8);
    let declared = le_u16(payload, 16) as usize + le_u16(payload, 18) as usize;
    let available = (payload.len() - COMPRESSED_UNITS_LEN) / COMPRESSED_POINT_LEN;
    let count = declared.min(available);
    (0..count)
        .map(|i| {
            let p = COMPRESSED_UNITS_LEN + i * COMPRESSED_POINT_LEN;
            Point {
                x: le_i16(payload, p) as f32 * xyz_unit,
                y: le_i16(payload, p + 2) as f32 * xyz_unit,
                z: le_i16(payload, p + 4) as f32 * xyz_unit,
                doppler: le_i16(payload, p + 6) as f32 * doppler_unit,
                snr: payload[p + 8] as f32 * snr_unit,
            }
        })
        .collect()
}

fn parse_float(payload: &[u8]) -> Vec<Point> {
    (0..payload.len() / FLOAT_POINT_LEN)
        .map(|i| {
            let p = i * FLOAT_POINT_LEN;
            Point {
                x: le_f32(payload, p),
                y: le_f32(payload, p + 4),
                z: le_f32(payload, p + 8),
                doppler: le_f32(payload, p + 12),
                snr: 0.0,
            }
        })
        .collect()
}

fn parse_side_snr(payload: &[u8]) -> Vec<f32> {
    (0..payload.len() / SIDE_INFO_LEN)
        .map(|i| le_i16(payload, i * SIDE_INFO_LEN) as f32 * SIDE_INFO_SNR_SCALE)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tlv(kind: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&kind.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        v
    }

    fn packet(frame: u32, tlvs: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = tlvs.concat();
        let total = (HEADER_LEN + body.len()) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(&MAGIC);
        v.extend_from_slice(&0x0605_0000u32.to_le_bytes());
        v.extend_from_slice(&total.to_le_bytes());
        v.extend_from_slice(&0x6432u32.to_le_bytes());
        v.extend_from_slice(&frame.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&(tlvs.len() as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    fn compressed(points: &[(i16, i16, i16, i16, u8)]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0.01f32.to_le_bytes());
        v.extend_from_slice(&0.1f32.to_le_bytes());
        v.extend_from_slice(&0.5f32.to_le_bytes());
        v.extend_from_slice(&0.5f32.to_le_bytes());
        v.extend_from_slice(&(points.len() as u16).to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        for &(x, y, z, d, snr) in points {
            v.extend_from_slice(&x.to_le_bytes());
            v.extend_from_slice(&y.to_le_bytes());
            v.extend_from_slice(&z.to_le_bytes());
            v.extend_from_slice(&d.to_le_bytes());
            v.push(snr);
            v.push(4);
        }
        v
    }

    #[test]
    fn parses_compressed_points() {
        let pkt = packet(7, &[tlv(TLV_POINTS_COMPRESSED, &compressed(&[(10, 50, -5, -3, 30), (0, 200, 0, 0, 10)])), tlv(306, &[0u8; 20])]);
        let mut r = FrameReader::new();
        r.push(&pkt);
        let f = r.next_frame().expect("frame");
        assert_eq!(f.number, 7);
        assert_eq!(f.points.len(), 2);
        assert!((f.points[0].x - 0.10).abs() < 1e-6);
        assert!((f.points[0].y - 0.50).abs() < 1e-6);
        assert!((f.points[0].z + 0.05).abs() < 1e-6);
        assert!((f.points[0].doppler + 0.3).abs() < 1e-6);
        assert!((f.points[0].snr - 15.0).abs() < 1e-6);
        assert_eq!(f.tlvs, vec![(TLV_POINTS_COMPRESSED, 40), (306, 20)]);
        assert!(f.points.iter().any(|p| (p.y - 0.50).abs() < 1e-6));
        assert_eq!(r.skipped(), 0);
    }

    #[test]
    fn resyncs_after_garbage_and_split_reads() {
        let a = packet(1, &[tlv(TLV_POINTS_COMPRESSED, &compressed(&[(1, 2, 3, 4, 5)]))]);
        let b = packet(2, &[tlv(TLV_POINTS_COMPRESSED, &compressed(&[]))]);
        let mut stream = b"Done\r\nmmwDemo:/>".to_vec();
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&[0xAA; 13]);
        stream.extend_from_slice(&b);
        let mut r = FrameReader::new();
        let mut frames = Vec::new();
        for chunk in stream.chunks(7) {
            r.push(chunk);
            while let Some(f) = r.next_frame() {
                frames.push(f.number);
            }
        }
        assert_eq!(frames, vec![1, 2]);
        assert_eq!(r.skipped(), 16 + 13);
    }

    #[test]
    fn rejects_impossible_length_and_recovers() {
        let mut bad = MAGIC.to_vec();
        bad.extend_from_slice(&[0u8; 4]);
        bad.extend_from_slice(&u32::MAX.to_le_bytes());
        bad.extend_from_slice(&[0u8; 24]);
        let good = packet(9, &[]);
        let mut r = FrameReader::new();
        r.push(&bad);
        r.push(&good);
        let f = r.next_frame().expect("frame");
        assert_eq!(f.number, 9);
        assert!(f.points.is_empty());
    }

    #[test]
    fn truncated_tlv_is_ignored() {
        let mut pkt = packet(3, &[tlv(TLV_POINTS_COMPRESSED, &compressed(&[(1, 1, 1, 1, 1)]))]);
        let len = pkt.len();
        pkt[HEADER_LEN + 4] = 200;
        let mut r = FrameReader::new();
        r.push(&pkt);
        let f = r.next_frame().expect("frame");
        assert_eq!(f.number, 3);
        assert!(f.points.is_empty());
        assert_eq!(len, pkt.len());
    }

    #[test]
    fn float_points_take_side_info_snr() {
        let mut fp = Vec::new();
        for v in [0.1f32, 0.8, 0.0, -0.2] {
            fp.extend_from_slice(&v.to_le_bytes());
        }
        let mut side = Vec::new();
        side.extend_from_slice(&123i16.to_le_bytes());
        side.extend_from_slice(&40i16.to_le_bytes());
        let pkt = packet(4, &[tlv(TLV_POINTS_FLOAT, &fp), tlv(TLV_POINTS_SIDE_INFO, &side)]);
        let mut r = FrameReader::new();
        r.push(&pkt);
        let f = r.next_frame().expect("frame");
        assert_eq!(f.points.len(), 1);
        assert!((f.points[0].snr - 12.3).abs() < 1e-4);
    }
}
