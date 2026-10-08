use crate::geometry::Vec3;

pub const BACKGROUND_CAPACITY: usize = 256;

pub struct Background {
    points: [Vec3; BACKGROUND_CAPACITY],
    len: usize,
    radius_sq: f32,
}

impl Background {
    pub const fn new(radius: f32) -> Self {
        Self { points: [Vec3::new(0.0, 0.0, 0.0); BACKGROUND_CAPACITY], len: 0, radius_sq: radius * radius }
    }

    pub fn learn(&mut self, frame: &[Vec3]) {
        for p in frame {
            if self.len == BACKGROUND_CAPACITY {
                return;
            }
            if !self.contains(p) {
                self.points[self.len] = *p;
                self.len += 1;
            }
        }
    }

    pub fn contains(&self, p: &Vec3) -> bool {
        self.points[..self.len].iter().any(|b| b.distance_sq(p) <= self.radius_sq)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == BACKGROUND_CAPACITY
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}
