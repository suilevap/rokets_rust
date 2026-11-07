//! World units and the conversion from map coordinates.
//!
//! One world unit is the C# game's "sim unit": 32 of its pixels
//! (`ConvertUnits`), so the car's forces, speeds and masses keep their C#
//! values. A map cell is `MapData.Scale / 32` units (2 for a 64-pixel map).
//!
//! Map coordinates have y pointing down and angles clockwise (0° right,
//! 90° down); the world has y up and angles counter-clockwise, as in Bevy.
//! Convert only at this boundary.

use bevy::math::Vec2;

/// The C# game's pixels per world unit.
pub const PX_PER_UNIT: f32 = 32.0;

/// Size of one map cell in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellSize(pub f32);

impl CellSize {
    pub fn from_map_scale(scale_px: f32) -> Self {
        CellSize(scale_px / PX_PER_UNIT)
    }

    /// A point in map cells (y down) to the world (y up).
    pub fn to_world(self, x: f32, y: f32) -> Vec2 {
        Vec2::new(x * self.0, -y * self.0)
    }

    /// A world point back to map cells (y down).
    pub fn to_cells(self, p: Vec2) -> (f32, f32) {
        (p.x / self.0, -p.y / self.0)
    }
}

/// A map direction in degrees (clockwise, y down) to a world angle in radians.
pub fn map_degrees_to_world(deg: f32) -> f32 {
    -deg.to_radians()
}

/// A length in the C# game's pixels to world units.
pub fn px(v: f32) -> f32 {
    v / PX_PER_UNIT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts() {
        let cell = CellSize::from_map_scale(64.0);
        assert_eq!(cell.to_world(9.5, 3.5), Vec2::new(19.0, -7.0));
        assert_eq!(cell.to_cells(Vec2::new(19.0, -7.0)), (9.5, 3.5));
        assert!((map_degrees_to_world(90.0) + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }
}
