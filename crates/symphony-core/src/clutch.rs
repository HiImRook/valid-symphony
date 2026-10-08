use crate::background::Background;
use crate::geometry::{Vec3, Zone};

const LEVEL_EPSILON: f32 = 0.001;
const LEVEL_MIN: f32 = 0.0;
const LEVEL_MAX: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub hand_radius: f32,
    pub still_radius: f32,
    pub grab_hold_ms: u32,
    pub dropout_ms: u32,
    pub release_ms: u32,
    pub cooldown_ms: u32,
    pub track_margin: f32,
    pub jump_limit: f32,
    pub travel: f32,
    pub smoothing: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            hand_radius: 0.12,
            still_radius: 0.05,
            grab_hold_ms: 600,
            dropout_ms: 250,
            release_ms: 400,
            cooldown_ms: 500,
            track_margin: 0.15,
            jump_limit: 0.15,
            travel: 0.5,
            smoothing: 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Arming,
    Grabbed,
    Cooldown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    None,
    Arming,
    Cancelled,
    Grabbed { level: f32 },
    Level(f32),
    Released { level: f32 },
}

pub struct Clutch {
    zone: Zone,
    tuning: Tuning,
    phase: Phase,
    level: f32,
    anchor: Vec3,
    arm_start: u32,
    last_seen: u32,
    last_hand: Vec3,
    smooth: Vec3,
    grab_z: f32,
    grab_level: f32,
}

impl Clutch {
    pub fn new(zone: Zone, tuning: Tuning, level: f32) -> Self {
        Self {
            zone,
            tuning,
            phase: Phase::Idle,
            level: level.clamp(LEVEL_MIN, LEVEL_MAX),
            anchor: Vec3::default(),
            arm_start: 0,
            last_seen: 0,
            last_hand: Vec3::default(),
            smooth: Vec3::default(),
            grab_z: 0.0,
            grab_level: 0.0,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn zone(&self) -> &Zone {
        &self.zone
    }

    pub fn hand(&self) -> Option<Vec3> {
        match self.phase {
            Phase::Grabbed => Some(self.smooth),
            Phase::Arming => Some(self.last_hand),
            _ => None,
        }
    }

    pub fn set_level(&mut self, level: f32) {
        if self.phase != Phase::Grabbed {
            self.level = level.clamp(LEVEL_MIN, LEVEL_MAX);
        }
    }

    pub fn set_zone(&mut self, zone: Zone) {
        if self.phase != Phase::Grabbed {
            self.zone = zone;
        }
    }

    pub fn update(&mut self, points: &[Vec3], background: &Background, now: u32) -> Event {
        match self.phase {
            Phase::Idle => self.idle(points, background, now),
            Phase::Arming => self.arming(points, background, now),
            Phase::Grabbed => self.grabbed(points, background, now),
            Phase::Cooldown => self.cooldown(points, background, now),
        }
    }

    fn idle(&mut self, points: &[Vec3], background: &Background, now: u32) -> Event {
        match find_hand(points, background, &self.zone, None, &self.tuning) {
            Some(hand) => {
                self.phase = Phase::Arming;
                self.anchor = hand;
                self.last_hand = hand;
                self.arm_start = now;
                self.last_seen = now;
                Event::Arming
            }
            None => Event::None,
        }
    }

    fn arming(&mut self, points: &[Vec3], background: &Background, now: u32) -> Event {
        match find_hand(points, background, &self.zone, None, &self.tuning) {
            Some(hand) => {
                self.last_seen = now;
                self.last_hand = hand;
                if hand.distance_sq(&self.anchor) > self.tuning.still_radius * self.tuning.still_radius {
                    self.anchor = hand;
                    self.arm_start = now;
                    return Event::None;
                }
                if now.wrapping_sub(self.arm_start) >= self.tuning.grab_hold_ms {
                    self.phase = Phase::Grabbed;
                    self.smooth = hand;
                    self.grab_z = hand.z;
                    self.grab_level = self.level;
                    return Event::Grabbed { level: self.level };
                }
                Event::None
            }
            None => {
                if now.wrapping_sub(self.last_seen) > self.tuning.dropout_ms {
                    self.phase = Phase::Idle;
                    return Event::Cancelled;
                }
                Event::None
            }
        }
    }

    fn grabbed(&mut self, points: &[Vec3], background: &Background, now: u32) -> Event {
        let track = self.zone.expanded(self.tuning.track_margin);
        match find_hand(points, background, &track, Some(self.last_hand), &self.tuning) {
            Some(hand) => {
                self.last_seen = now;
                self.last_hand = hand;
                self.smooth = self.smooth.lerp(&hand, self.tuning.smoothing);
                let target = (self.grab_level + (self.smooth.z - self.grab_z) / self.tuning.travel).clamp(LEVEL_MIN, LEVEL_MAX);
                let change = target - self.level;
                if !(-LEVEL_EPSILON..=LEVEL_EPSILON).contains(&change) {
                    self.level = target;
                    return Event::Level(target);
                }
                Event::None
            }
            None => {
                if now.wrapping_sub(self.last_seen) >= self.tuning.release_ms {
                    self.phase = Phase::Cooldown;
                    self.last_seen = now;
                    return Event::Released { level: self.level };
                }
                Event::None
            }
        }
    }

    fn cooldown(&mut self, points: &[Vec3], background: &Background, now: u32) -> Event {
        if find_hand(points, background, &self.zone, None, &self.tuning).is_some() {
            self.last_seen = now;
        } else if now.wrapping_sub(self.last_seen) >= self.tuning.cooldown_ms {
            self.phase = Phase::Idle;
        }
        Event::None
    }
}

fn find_hand(points: &[Vec3], background: &Background, zone: &Zone, near: Option<Vec3>, tuning: &Tuning) -> Option<Vec3> {
    let limit_sq = tuning.jump_limit * tuning.jump_limit;
    let usable = |p: &&Vec3| zone.contains(p) && !background.contains(p) && near.is_none_or(|n| n.distance_sq(p) <= limit_sq);
    let seed = match near {
        Some(n) => points.iter().filter(usable).min_by(|a, b| a.distance_sq(&n).total_cmp(&b.distance_sq(&n)))?,
        None => points.iter().filter(usable).min_by(|a, b| a.y.total_cmp(&b.y))?,
    };
    let radius_sq = tuning.hand_radius * tuning.hand_radius;
    let mut sum = Vec3::default();
    let mut count = 0.0;
    for p in points.iter().filter(usable).filter(|p| p.distance_sq(seed) <= radius_sq) {
        sum.x += p.x;
        sum.y += p.y;
        sum.z += p.z;
        count += 1.0;
    }
    Some(Vec3::new(sum.x / count, sum.y / count, sum.z / count))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    const FRAME_MS: u32 = 50;

    fn zone() -> Zone {
        Zone::new(Vec3::new(0.0, 0.6, 0.0), Vec3::new(0.25, 0.2, 0.25))
    }

    fn jitter(i: u32) -> f32 {
        ((i * 7919) % 13) as f32 / 13.0 * 0.03 - 0.015
    }

    struct Sim {
        clutch: Clutch,
        background: Background,
        now: u32,
        events: Vec<Event>,
    }

    impl Sim {
        fn new(level: f32) -> Self {
            Self { clutch: Clutch::new(zone(), Tuning::default(), level), background: Background::new(0.1), now: 0, events: Vec::new() }
        }

        fn step(&mut self, points: &[Vec3]) {
            self.now += FRAME_MS;
            let e = self.clutch.update(points, &self.background, self.now);
            if e != Event::None {
                self.events.push(e);
            }
        }

        fn hold(&mut self, at: Vec3, ms: u32) {
            for i in 0..ms / FRAME_MS {
                let j = jitter(self.now / FRAME_MS + i);
                self.step(&[Vec3::new(at.x + j, at.y - j, at.z + j)]);
            }
        }

        fn sweep(&mut self, from: Vec3, to: Vec3, ms: u32) {
            let n = ms / FRAME_MS;
            for i in 1..=n {
                self.step(&[from.lerp(&to, i as f32 / n as f32)]);
            }
        }

        fn empty(&mut self, ms: u32) {
            for _ in 0..ms / FRAME_MS {
                self.step(&[]);
            }
        }

        fn grabs(&self) -> usize {
            self.events.iter().filter(|e| matches!(e, Event::Grabbed { .. })).count()
        }
    }

    #[test]
    fn still_hand_grabs_after_hold() {
        let mut s = Sim::new(0.5);
        s.hold(Vec3::new(0.0, 0.6, 0.0), 400);
        assert_eq!(s.clutch.phase(), Phase::Arming);
        s.hold(Vec3::new(0.0, 0.6, 0.0), 400);
        assert_eq!(s.clutch.phase(), Phase::Grabbed);
        assert!(s.events.contains(&Event::Grabbed { level: 0.5 }));
    }

    #[test]
    fn grab_keeps_current_level_and_raising_brightens() {
        let mut s = Sim::new(0.4);
        let start = Vec3::new(0.05, 0.6, -0.1);
        s.hold(start, 800);
        s.sweep(start, Vec3::new(0.05, 0.6, 0.1), 1000);
        s.hold(Vec3::new(0.05, 0.6, 0.1), 1000);
        let level = s.clutch.level();
        assert!((level - 0.8).abs() < 0.03, "level {level}");
    }

    #[test]
    fn lowering_dims_and_clamps_at_zero() {
        let mut s = Sim::new(0.2);
        let start = Vec3::new(0.0, 0.6, 0.1);
        s.hold(start, 800);
        s.sweep(start, Vec3::new(0.0, 0.6, -0.3), 1000);
        s.hold(Vec3::new(0.0, 0.6, -0.3), 600);
        assert_eq!(s.clutch.level(), 0.0);
        assert_eq!(s.clutch.phase(), Phase::Grabbed);
    }

    #[test]
    fn drifting_outside_box_keeps_tracking() {
        let mut s = Sim::new(0.5);
        let start = Vec3::new(0.0, 0.6, 0.2);
        s.hold(start, 800);
        s.sweep(start, Vec3::new(0.0, 0.6, 0.35), 600);
        s.hold(Vec3::new(0.0, 0.6, 0.35), 600);
        assert_eq!(s.clutch.phase(), Phase::Grabbed);
        assert!(s.clutch.level() > 0.75);
    }

    #[test]
    fn pulling_away_releases_and_keeps_level() {
        let mut s = Sim::new(0.5);
        let start = Vec3::new(0.0, 0.6, 0.0);
        s.hold(start, 800);
        s.sweep(start, Vec3::new(0.0, 0.6, 0.1), 500);
        s.hold(Vec3::new(0.0, 0.6, 0.1), 500);
        let before = s.clutch.level();
        s.sweep(Vec3::new(0.0, 0.6, 0.1), Vec3::new(0.0, 1.3, 0.1), 300);
        s.empty(600);
        assert_eq!(s.clutch.phase(), Phase::Cooldown);
        assert!(s.events.iter().any(|e| matches!(e, Event::Released { .. })));
        assert!((s.clutch.level() - before).abs() < 0.05);
        s.empty(600);
        assert_eq!(s.clutch.phase(), Phase::Idle);
    }

    #[test]
    fn torso_behind_hand_does_not_hold_the_grab() {
        let mut s = Sim::new(0.5);
        let hand = Vec3::new(0.0, 0.6, 0.0);
        let torso = Vec3::new(0.0, 0.92, -0.05);
        for _ in 0..16 {
            s.step(&[hand, torso]);
        }
        assert_eq!(s.clutch.phase(), Phase::Grabbed);
        for _ in 0..12 {
            s.step(&[torso]);
        }
        assert_eq!(s.clutch.phase(), Phase::Cooldown);
    }

    #[test]
    fn walking_through_never_grabs() {
        let mut s = Sim::new(0.5);
        s.sweep(Vec3::new(-0.6, 0.65, 0.0), Vec3::new(0.6, 0.65, 0.0), 1200);
        s.sweep(Vec3::new(0.6, 0.5, 0.1), Vec3::new(-0.6, 0.5, 0.1), 900);
        assert_eq!(s.grabs(), 0);
        assert_eq!(s.clutch.level(), 0.5);
    }

    #[test]
    fn people_beyond_the_box_are_ignored() {
        let mut s = Sim::new(0.5);
        s.hold(Vec3::new(0.0, 1.1, 0.0), 2000);
        assert_eq!(s.clutch.phase(), Phase::Idle);
    }

    #[test]
    fn background_points_never_grab() {
        let mut s = Sim::new(0.5);
        let desk = Vec3::new(0.1, 0.55, -0.2);
        s.background.learn(&[desk]);
        s.hold(desk, 2000);
        assert_eq!(s.grabs(), 0);
    }

    #[test]
    fn brief_dropout_does_not_cancel_arming() {
        let mut s = Sim::new(0.5);
        let at = Vec3::new(0.0, 0.6, 0.0);
        s.hold(at, 300);
        s.empty(150);
        s.hold(at, 400);
        assert_eq!(s.clutch.phase(), Phase::Grabbed);
    }

    #[test]
    fn released_hand_must_leave_before_regrab() {
        let mut s = Sim::new(0.5);
        let at = Vec3::new(0.0, 0.6, 0.0);
        s.hold(at, 800);
        s.empty(500);
        assert_eq!(s.clutch.phase(), Phase::Cooldown);
        s.hold(at, 1500);
        assert_eq!(s.clutch.phase(), Phase::Cooldown);
        assert_eq!(s.grabs(), 1);
    }

    #[test]
    fn zone_moves_only_when_not_grabbed() {
        let mut s = Sim::new(0.5);
        let moved = zone().moved_to(Vec3::new(0.3, 0.6, 0.0));
        s.hold(Vec3::new(0.0, 0.6, 0.0), 800);
        s.clutch.set_zone(moved);
        assert_eq!(*s.clutch.zone(), zone());
        s.empty(1200);
        s.clutch.set_zone(moved);
        assert_eq!(*s.clutch.zone(), moved);
    }

    #[test]
    fn background_dedupes_and_caps() {
        let mut b = Background::new(0.1);
        b.learn(&[Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.02, 0.5, 0.0)]);
        assert_eq!(b.len(), 1);
        let many: Vec<Vec3> = (0..400).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        b.learn(&many);
        assert!(b.is_full());
    }
}
