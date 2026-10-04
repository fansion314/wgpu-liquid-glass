use egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFit {
    Fit,
    Fill,
}

#[derive(Clone, Debug)]
pub struct ImageView {
    pub fit: ImageFit,
    pub zoom: f32,
    pub pan: Vec2,
    pub edit_image: bool,
}

impl Default for ImageView {
    fn default() -> Self {
        Self {
            fit: ImageFit::Fit,
            zoom: 1.0,
            pan: Vec2::ZERO,
            edit_image: false,
        }
    }
}

pub const CANVAS_ASPECT: f32 = 3.0 / 2.0;

/// One layout shared by painting, hit testing and image controls. Diagnostic
/// slots are disjoint; narrow windows move the side rail below the canvas.
pub struct SceneLayout {
    pub canvas: Rect,
    pub caption: Rect,
    pub samples: [Rect; 9],
    pub compact: bool,
}

impl SceneLayout {
    pub fn new(stage: Vec2) -> Self {
        let compact = stage.x < 620.0 || stage.y < 580.0;
        let padding = if compact { 16.0 } else { 32.0 };
        let top = if compact { 132.0 } else { 196.0 };
        let bottom = if compact { 112.0 } else { 116.0 };
        let side = !compact;
        let rail = if side { 152.0 } else { 0.0 };
        let available = Vec2::new(
            (stage.x - padding * 2.0 - rail).max(1.0),
            (stage.y - top - bottom).max(1.0),
        );
        let width = available.x.min(available.y * CANVAS_ASPECT);
        let size = Vec2::new(width, width / CANVAS_ASPECT);
        let canvas = Rect::from_min_size(
            Pos2::new(
                padding + (available.x - size.x) * 0.5,
                top + (available.y - size.y) * 0.5,
            ),
            size,
        );
        let caption = Rect::from_min_max(
            Pos2::new(canvas.min.x, top - 52.0),
            Pos2::new(canvas.max.x, top - 4.0),
        );
        let mut samples = [Rect::NOTHING; 9];
        let footer_y = stage.y - bottom + 12.0;
        if side {
            let cell_height = available.y / 4.0;
            for (i, slot) in samples[..4].iter_mut().enumerate() {
                *slot = Rect::from_min_size(
                    Pos2::new(
                        stage.x - padding - rail + 16.0,
                        top + i as f32 * cell_height,
                    ),
                    Vec2::new(rail - 16.0, cell_height),
                )
                .shrink(8.0);
            }
        }
        let start = if side { 4 } else { 0 };
        let columns = 5;
        let cell_width = (stage.x - 2.0 * padding) / columns as f32;
        for (i, slot) in samples[start..].iter_mut().enumerate() {
            *slot = Rect::from_min_size(
                Pos2::new(
                    padding + (i % columns) as f32 * cell_width,
                    footer_y + (i / columns) as f32 * 48.0,
                ),
                Vec2::new(cell_width, if side { 76.0 } else { 48.0 }),
            )
            .shrink(4.0);
        }
        Self {
            canvas,
            caption,
            samples,
            compact,
        }
    }
}

/// Fixed 3:2 aspect, maximized within the remaining photo area.
pub fn canvas(stage: Vec2) -> Rect {
    SceneLayout::new(stage).canvas
}

impl ImageView {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    pub fn set_fit(&mut self, fit: ImageFit) {
        self.fit = fit;
        self.reset();
    }

    pub fn image_rect(&self, canvas: Rect, dimensions: [usize; 2]) -> Rect {
        let image = Vec2::new(dimensions[0] as f32, dimensions[1] as f32);
        let ratios = canvas.size() / image;
        let scale = match self.fit {
            ImageFit::Fit => ratios.min_elem(),
            ImageFit::Fill => ratios.max_elem(),
        };
        Rect::from_center_size(canvas.center() + self.pan, image * scale * self.zoom)
    }

    /// Zoom about the pointer, preserving the image coordinate underneath it.
    pub fn zoom_at(&mut self, factor: f32, anchor: Pos2, canvas: Rect) {
        let old_zoom = self.zoom;
        self.zoom = (old_zoom * factor).clamp(0.1, 8.0);
        let relative = anchor - canvas.center();
        self.pan = relative - (relative - self.pan) * (self.zoom / old_zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_fills_available_area_without_overlapping_samples() {
        for viewport in [
            Vec2::new(720.0, 520.0),
            Vec2::new(1280.0, 800.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(900.0, 1200.0),
        ] {
            for scale in [0.75, 1.1, 1.25, 1.5, 2.0] {
                let logical = viewport / scale;
                let stage = Vec2::new(logical.x - crate::glass::panel_width(logical.x), logical.y);
                let layout = SceneLayout::new(stage);
                let bounds = Rect::from_min_size(Pos2::ZERO, stage);
                assert!(bounds.contains_rect(layout.canvas));
                assert!((layout.canvas.aspect_ratio() - CANVAS_ASPECT).abs() < 0.001);
                assert!(!layout.canvas.intersects(layout.caption));
                for (i, slot) in layout.samples.iter().enumerate() {
                    assert!(bounds.contains_rect(*slot), "{stage:?}: {slot:?}");
                    assert!(!layout.canvas.intersects(*slot));
                    for other in &layout.samples[i + 1..] {
                        assert!(!slot.intersects(*other));
                    }
                }
            }
        }
        assert!(canvas(Vec2::new(1600.0, 1200.0)).width() > 1000.0);
    }

    #[test]
    fn fit_and_fill_preserve_aspect_ratio() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
        let mut view = ImageView::default();
        let fitted = view.image_rect(area, [1000, 2000]);
        assert_eq!(fitted.size(), Vec2::new(200.0, 400.0));
        view.set_fit(ImageFit::Fill);
        let filled = view.image_rect(area, [1000, 2000]);
        assert_eq!(filled.size(), Vec2::new(600.0, 1200.0));
        assert!(filled.contains_rect(area));
    }

    #[test]
    fn zoom_preserves_pointer_anchor_and_reset_recovers_view() {
        let area = canvas(Vec2::new(960.0, 800.0));
        let anchor = area.center() + Vec2::new(60.0, -40.0);
        let mut view = ImageView {
            pan: Vec2::new(20.0, 10.0),
            ..Default::default()
        };
        let before = (anchor - area.center() - view.pan) / view.zoom;
        view.zoom_at(2.0, anchor, area);
        let after = (anchor - area.center() - view.pan) / view.zoom;
        assert!((before - after).length() < 0.001);
        view.reset();
        assert_eq!(view.pan, Vec2::ZERO);
        assert_eq!(view.zoom, 1.0);
    }
}
