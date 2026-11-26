//! Small deterministic 3D gradient noise used for terrain generation.
//!
//! The implementation is intentionally local (no external noise dependency)
//! so that generation is stable across crate updates. Reproducibility is
//! guaranteed for a given seed on a given platform.

use glam::Vec3;

/// Classic Perlin-style 3D gradient noise with a splitmix64-seeded permutation.
#[derive(Clone, Debug)]
pub struct Noise3 {
    perm: [u8; 512],
}

impl Noise3 {
    pub fn new(seed: u64) -> Self {
        let mut p = [0u8; 256];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = i as u8;
        }
        let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
        for i in (1..256usize).rev() {
            let r = splitmix64(&mut state) as usize;
            let j = r % (i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        perm[..256].copy_from_slice(&p);
        perm[256..].copy_from_slice(&p);
        Self { perm }
    }

    fn grad_dot(&self, hash: u8, x: f32, y: f32, z: f32) -> f32 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        let a = if h & 1 == 0 { u } else { -u };
        let b = if h & 2 == 0 { v } else { -v };
        a + b
    }

    /// Raw noise in roughly [-1, 1].
    pub fn get(&self, p: Vec3) -> f32 {
        let xi = p.x.floor();
        let yi = p.y.floor();
        let zi = p.z.floor();
        let xf = p.x - xi;
        let yf = p.y - yi;
        let zf = p.z - zi;
        let x0 = (xi as i32 & 255) as usize;
        let y0 = (yi as i32 & 255) as usize;
        let z0 = (zi as i32 & 255) as usize;
        let x1 = (x0 + 1) & 255;
        let y1 = (y0 + 1) & 255;
        let z1 = (z0 + 1) & 255;

        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let p = &self.perm;
        let aa = p[p[x0] as usize + y0];
        let ab = p[p[x0] as usize + y1];
        let ba = p[p[x1] as usize + y0];
        let bb = p[p[x1] as usize + y1];
        let aaa = p[aa as usize + z0];
        let aab = p[aa as usize + z1];
        let aba = p[ab as usize + z0];
        let abb = p[ab as usize + z1];
        let baa = p[ba as usize + z0];
        let bab = p[ba as usize + z1];
        let bba = p[bb as usize + z0];
        let bbb = p[bb as usize + z1];

        let x00 = lerp(
            self.grad_dot(aaa, xf, yf, zf),
            self.grad_dot(baa, xf - 1.0, yf, zf),
            u,
        );
        let x10 = lerp(
            self.grad_dot(aba, xf, yf - 1.0, zf),
            self.grad_dot(bba, xf - 1.0, yf - 1.0, zf),
            u,
        );
        let x01 = lerp(
            self.grad_dot(aab, xf, yf, zf - 1.0),
            self.grad_dot(bab, xf - 1.0, yf, zf - 1.0),
            u,
        );
        let x11 = lerp(
            self.grad_dot(abb, xf, yf - 1.0, zf - 1.0),
            self.grad_dot(bbb, xf - 1.0, yf - 1.0, zf - 1.0),
            u,
        );
        let y0v = lerp(x00, x10, v);
        let y1v = lerp(x01, x11, v);
        lerp(y0v, y1v, w)
    }

    pub fn fbm(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += self.get(p * freq) * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
        }
        sum / norm.max(1.0e-6)
    }

    /// Ridged multifractal, useful for mountain chains.
    pub fn ridged(&self, p: Vec3, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            let n = 1.0 - self.get(p * freq).abs();
            sum += n * n * amp;
            norm += amp;
            freq *= lacunarity;
            amp *= gain;
        }
        sum / norm.max(1.0e-6)
    }
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Deterministic hash-based RNG helper for one-off placement decisions.
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        splitmix64(&mut self.state)
    }

    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as u32
    }

    pub fn unit_vec(&mut self) -> Vec3 {
        // Uniform point on the unit sphere.
        let z = self.range(-1.0, 1.0);
        let theta = self.range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * theta.cos(), z, r * theta.sin())
    }
}
