use glam::Vec3;
use serde::{Deserialize, Serialize};

/// One tectonic plate.
///
/// `center` and `motion_axis` are serialized as arrays because the save
/// format should not depend on a particular glam version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plate {
    pub id: u16,
    pub name: String,
    pub center: [f32; 3],
    pub motion_axis: [f32; 3],
    /// Angular speed in radians per million years.
    pub angular_speed_rad_per_myr: f32,
    pub oceanic: bool,
}

impl Plate {
    pub fn center_vec(&self) -> Vec3 {
        Vec3::from_array(self.center).normalize_or_zero()
    }

    pub fn axis_vec(&self) -> Vec3 {
        Vec3::from_array(self.motion_axis).normalize_or_zero()
    }

    /// Velocity at a surface point in metres per million years.
    pub fn velocity_at_m_per_myr(&self, direction: Vec3, radius_m: f32) -> Vec3 {
        self.axis_vec().cross(direction.normalize_or_zero())
            * self.angular_speed_rad_per_myr
            * radius_m
    }

    pub fn advance(&mut self, dt_myr: f32) {
        let axis = self.axis_vec();
        let angle = self.angular_speed_rad_per_myr * dt_myr;
        let center = glam::Quat::from_axis_angle(axis, angle) * self.center_vec();
        self.center = center.normalize_or_zero().to_array();
    }
}
