use bytemuck::{Pod, Zeroable};
use egui::{Pos2, Rect, Vec2};

pub const PANEL_WIDTH: f32 = 340.0;

pub fn panel_width(viewport_width: f32) -> f32 {
    PANEL_WIDTH.min(viewport_width * 0.46)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassMode {
    Liquid,
    Lens,
    Concave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassShape {
    RoundedRect,
    Circle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurMode {
    Gaussian,
    Frosted,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassPattern {
    Frost = 0,
    OrangePeel,
    Hammered,
    Crystal,
    Quatrefoil,
    Fern,
}

impl GlassPattern {
    pub const ALL: [Self; 6] = [
        Self::Frost,
        Self::OrangePeel,
        Self::Hammered,
        Self::Crystal,
        Self::Quatrefoil,
        Self::Fern,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Frost => "细磨砂",
            Self::OrangePeel => "橘皮纹",
            Self::Hammered => "锤纹",
            Self::Crystal => "水晶碎面",
            Self::Quatrefoil => "复古花格",
            Self::Fern => "冰花叶脉",
        }
    }

    /// Characteristic feature width in logical pixels, independent of aperture.
    pub fn feature_size(self) -> f32 {
        match self {
            Self::Frost => 8.0,
            Self::OrangePeel => 8.0,
            Self::Hammered => 22.0,
            Self::Crystal => 18.0,
            Self::Quatrefoil => 90.0,
            Self::Fern => 140.0,
        }
    }

    pub fn tile_units(self) -> f32 {
        match self {
            Self::Frost => 1.0,
            Self::OrangePeel => 32.0,
            Self::Hammered => 12.0,
            Self::Crystal => 14.0,
            Self::Quatrefoil => 1.0,
            Self::Fern => 3.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GlassParams {
    pub mode: GlassMode,
    pub shape: GlassShape,
    pub center: Pos2,
    pub size: Vec2,
    pub radius: f32,
    pub curvature: f32,
    pub thickness: f32,
    pub margin: f32,
    pub distance: f32,
    pub blur: f32,
    pub blur_mode: BlurMode,
    pub roughness: f32,
    pub pattern: GlassPattern,
    pub pattern_scale: f32,
    pub pattern_depth: f32,
    pub pattern_rotation: f32,
    pub ior: f32,
    pub highlight: f32,
    pub enabled: bool,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            mode: GlassMode::Liquid,
            shape: GlassShape::RoundedRect,
            center: Pos2::new(450.0, 370.0),
            size: Vec2::new(300.0, 220.0),
            radius: 32.0,
            curvature: 0.6,
            thickness: 16.0,
            margin: 32.0,
            distance: 100.0,
            blur: 0.0,
            blur_mode: BlurMode::Gaussian,
            roughness: 0.15,
            pattern: GlassPattern::Frost,
            pattern_scale: 1.0,
            pattern_depth: 1.0,
            pattern_rotation: 0.0,
            ior: 1.45,
            highlight: 0.7,
            enabled: true,
        }
    }
}

impl GlassParams {
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }

    pub fn contains(&self, point: Pos2) -> bool {
        if !self.enabled {
            return false;
        }
        let offset = point - self.center;
        match self.shape {
            GlassShape::RoundedRect => {
                rounded_distance(offset, self.size * 0.5, self.radius) <= 0.0
            }
            GlassShape::Circle => offset.length() <= self.size.min_elem() * 0.5,
        }
    }

    /// Preserve size and position across window boundaries. Only normalize
    /// geometry; the render stage clips coverage and supplies black samples.
    pub fn normalize_shape(&mut self) {
        self.size = self.size.max(Vec2::splat(1.0));
        if self.shape == GlassShape::Circle {
            self.size = Vec2::splat(self.size.min_elem());
        }
        self.radius = self.radius.clamp(0.0, self.size.min_elem() * 0.5);
    }

    pub fn uniform(
        &self,
        size: [u32; 2],
        pixels_per_point: f32,
        stage_width: f32,
        srgb: bool,
    ) -> GlassUniform {
        GlassUniform {
            viewport: [
                size[0] as f32,
                size[1] as f32,
                pixels_per_point,
                stage_width,
            ],
            rect: [self.center.x, self.center.y, self.size.x, self.size.y],
            optics: [self.radius, self.curvature, self.distance, self.blur],
            appearance: [
                self.ior,
                self.highlight,
                u32::from(self.enabled) as f32,
                match self.mode {
                    GlassMode::Liquid => 0.0,
                    GlassMode::Lens => 1.0,
                    GlassMode::Concave => 2.0,
                },
            ],
            geometry: [
                self.thickness,
                match self.shape {
                    GlassShape::RoundedRect => 0.0,
                    GlassShape::Circle => 1.0,
                },
                self.margin,
                match self.blur_mode {
                    BlurMode::Gaussian => 0.0,
                    BlurMode::Frosted => 1.0,
                },
            ],
            material: [
                self.roughness,
                self.pattern as u32 as f32,
                self.pattern.feature_size()
                    * self.pattern.tile_units()
                    * self.pattern_scale.max(0.1),
                self.pattern_depth,
            ],
            pattern: [self.pattern_rotation.to_radians(), 0.0, 0.0, 0.0],
            output: [u32::from(srgb) as f32, 0.0, 0.0, 0.0],
        }
    }
}

/// Same rounded-box distance used for the shader mask and mouse hit testing.
pub fn rounded_distance(point: Vec2, half: Vec2, radius: f32) -> f32 {
    let radius = radius.min(half.min_elem());
    let q = point.abs() - half + Vec2::splat(radius);
    q.max(Vec2::ZERO).length() + q.max_elem().min(0.0) - radius
}

/// Eight vec4s; the Rust and WGSL layout are identical without implicit padding.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GlassUniform {
    pub viewport: [f32; 4],
    pub rect: [f32; 4],
    pub optics: [f32; 4],
    pub appearance: [f32; 4],
    pub geometry: [f32; 4],
    pub material: [f32; 4],
    pub pattern: [f32; 4],
    pub output: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_hit_test_excludes_transparent_corners() {
        let glass = GlassParams::default();
        assert!(glass.contains(glass.center));
        assert!(!glass.contains(glass.rect().min));
        assert!(glass.contains(glass.center + Vec2::new(149.0, 0.0)));
    }

    #[test]
    fn glass_can_cross_stage_edges_without_moving_or_shrinking() {
        let mut glass = GlassParams {
            center: Pos2::new(-30.0, 90.0),
            ..Default::default()
        };
        let original_size = glass.size;
        glass.normalize_shape();
        assert_eq!(glass.center, Pos2::new(-30.0, 90.0));
        assert_eq!(glass.size, original_size);
        assert!(glass.rect().min.x < 0.0);
    }

    #[test]
    fn uniform_matches_wgsl_vec4_layout() {
        assert_eq!(std::mem::size_of::<GlassUniform>(), 128);
        assert_eq!(std::mem::align_of::<GlassUniform>(), 16);
        assert_eq!(std::mem::offset_of!(GlassUniform, output), 112);
    }

    #[test]
    fn circle_hit_testing_and_resize_use_actual_circle() {
        let mut glass = GlassParams {
            shape: GlassShape::Circle,
            size: Vec2::splat(200.0),
            ..Default::default()
        };
        assert!(!glass.contains(glass.center + Vec2::new(80.0, 80.0)));
        assert!(glass.contains(glass.center + Vec2::new(99.0, 0.0)));
        glass.size = Vec2::new(150.0, 90.0);
        glass.normalize_shape();
        assert_eq!(glass.size, Vec2::splat(90.0));
    }
}
