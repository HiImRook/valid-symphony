#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_sq(&self, other: &Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn lerp(&self, target: &Vec3, amount: f32) -> Vec3 {
        Vec3::new(
            self.x + (target.x - self.x) * amount,
            self.y + (target.y - self.y) * amount,
            self.z + (target.z - self.z) * amount,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub center: Vec3,
    pub half: Vec3,
}

impl Zone {
    pub const fn new(center: Vec3, half: Vec3) -> Self {
        Self { center, half }
    }

    pub fn contains(&self, p: &Vec3) -> bool {
        within(p.x, self.center.x, self.half.x) && within(p.y, self.center.y, self.half.y) && within(p.z, self.center.z, self.half.z)
    }

    pub fn expanded(&self, margin: f32) -> Zone {
        Zone::new(self.center, Vec3::new(self.half.x + margin, self.half.y + margin, self.half.z + margin))
    }

    pub fn moved_to(&self, center: Vec3) -> Zone {
        Zone::new(center, self.half)
    }
}

fn within(value: f32, center: f32, half: f32) -> bool {
    value >= center - half && value <= center + half
}
