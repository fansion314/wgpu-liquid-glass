use crate::glass::{GlassMode, GlassParams, panel_width};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};

pub fn context() -> egui::Context {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "wenkai".into(),
        egui::FontData::from_static(include_bytes!("../assets/LXGWWenKai-Regular.ttf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "wenkai".into());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("wenkai".into());
    ctx.set_fonts(fonts);
    ctx.set_visuals(egui::Visuals::light());
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(17.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(17.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, FontId::monospace(15.0));
        style.spacing.interact_size.y = 26.0;
    });
    ctx
}

pub struct SceneImage {
    pub texture: egui::TextureHandle,
    pub label: &'static str,
    pub title: String,
    pub credit: String,
    pub source: Option<&'static str>,
    pub dimensions: [usize; 2],
}

pub const VIRTUAL_IMAGE: &[u8] = include_bytes!("../assets/virtual.jpg");
pub const PHOTO_IMAGE: &[u8] = include_bytes!("../assets/photo.jpg");
pub const THIRD_IMAGE: &[u8] = include_bytes!("../assets/third-photo.jpg");
pub const DEFAULT_PICTURE: usize = 1;
pub const BUILTIN_PICTURES: usize = 3;

pub fn images(
    ctx: &egui::Context,
    max_texture_side: usize,
) -> anyhow::Result<[SceneImage; BUILTIN_PICTURES]> {
    // Assets are loaded before the first frame. Egui otherwise still uses its
    // conservative initial limit of 2048, regardless of the actual GPU limits.
    ctx.input_mut(|input| input.max_texture_side = max_texture_side);
    let definitions = [
        (
            VIRTUAL_IMAGE,
            "虚拟场景",
            "SPRITE FRIGHT",
            "Blender Foundation · CC BY 1.0",
            Some("https://studio.blender.org/projects/sprite-fright/pages/about/"),
        ),
        (
            PHOTO_IMAGE,
            "真实摄影",
            "MORAINE LAKE",
            "Saupra1984 · CC BY-SA 3.0",
            Some("https://commons.wikimedia.org/wiki/File:Morainelake.JPG"),
        ),
        (THIRD_IMAGE, "第三张照片", "LOCAL PHOTO", "用户提供", None),
    ];
    let mut pictures = Vec::new();
    for (bytes, label, title, credit, source) in definitions {
        let image = image::load_from_memory(bytes)?.to_rgba8();
        let dimensions = [image.width() as usize, image.height() as usize];
        anyhow::ensure!(
            dimensions.iter().all(|&side| side <= max_texture_side),
            "图片 {label} 超出 GPU 纹理尺寸限制 {max_texture_side}"
        );
        let texture = ctx.load_texture(
            label,
            egui::ColorImage::from_rgba_unmultiplied(dimensions, image.as_raw()),
            egui::TextureOptions::LINEAR,
        );
        pictures.push(SceneImage {
            texture,
            label,
            title: title.into(),
            credit: credit.into(),
            source,
            dimensions,
        });
    }
    Ok(pictures.try_into().ok().expect("three built-in pictures"))
}

pub fn local_image(
    ctx: &egui::Context,
    max_texture_side: usize,
    path: &std::path::Path,
) -> anyhow::Result<SceneImage> {
    use anyhow::Context;
    let image = image::ImageReader::open(path)
        .context("无法读取文件")?
        .with_guessed_format()?
        .decode()
        .context("图片格式不支持、文件损坏或图片超过解码内存限制")?;
    let original = [image.width(), image.height()];
    let image = if original.iter().any(|&side| side > max_texture_side as u32) {
        image.resize(
            max_texture_side as u32,
            max_texture_side as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let image = image.to_rgba8();
    let dimensions = [image.width() as usize, image.height() as usize];
    ctx.input_mut(|input| input.max_texture_side = max_texture_side);
    let texture = ctx.load_texture(
        "User picture",
        egui::ColorImage::from_rgba_unmultiplied(dimensions, image.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    let credit = if dimensions == [original[0] as usize, original[1] as usize] {
        "本地文件".into()
    } else {
        format!(
            "本地文件 · 原图 {} × {}，已适配 GPU 尺寸限制",
            original[0], original[1]
        )
    };
    Ok(SceneImage {
        texture,
        label: "用户图片",
        title: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        credit,
        source: None,
        dimensions,
    })
}

pub fn background(
    ui: &mut egui::Ui,
    stage: Vec2,
    picture: &SceneImage,
    view: &crate::image_view::ImageView,
) {
    let stage_rect = Rect::from_min_size(Pos2::ZERO, stage);
    let p = ui.painter().with_clip_rect(stage_rect);
    let ink = Color32::from_rgb(25, 55, 66);
    p.rect_filled(stage_rect, 0.0, Color32::from_rgb(242, 245, 240));
    let grid = Color32::from_rgb(219, 229, 220);
    for x in (0..stage.x as usize).step_by(32) {
        p.line_segment(
            [Pos2::new(x as f32, 0.0), Pos2::new(x as f32, stage.y)],
            Stroke::new(0.7, grid),
        );
    }
    for y in (0..stage.y as usize).step_by(32) {
        p.line_segment(
            [Pos2::new(0.0, y as f32), Pos2::new(stage.x, y as f32)],
            Stroke::new(0.7, grid),
        );
    }
    let layout = crate::image_view::SceneLayout::new(stage);
    let left = if layout.compact { 16.0 } else { 32.0 };
    p.text(
        Pos2::new(left, 20.0),
        Align2::LEFT_TOP,
        "透过玻璃，看见变化",
        FontId::proportional(if layout.compact { 25.0 } else { 34.0 }),
        ink,
    );
    p.text(
        Pos2::new(left, if layout.compact { 56.0 } else { 76.0 }),
        Align2::LEFT_TOP,
        "拖动玻璃 / Shift 拖动图片 / 滚轮缩放",
        FontId::proportional(14.0),
        ink,
    );
    let canvas = layout.canvas;
    let caption = p.with_clip_rect(layout.caption);
    caption.text(
        layout.caption.min,
        Align2::LEFT_TOP,
        &picture.title,
        FontId::proportional(17.0),
        ink,
    );
    caption.text(
        layout.caption.min + Vec2::new(0.0, 25.0),
        Align2::LEFT_TOP,
        format!(
            "{}  /  {} × {}",
            picture.label, picture.dimensions[0], picture.dimensions[1]
        ),
        FontId::proportional(14.0),
        Color32::from_rgb(84, 113, 100),
    );
    p.rect_filled(canvas, 0.0, Color32::from_rgb(224, 231, 226));
    // Every image, including portraits, uses the exact same fixed clipping rect.
    p.with_clip_rect(canvas).image(
        picture.texture.id(),
        view.image_rect(canvas, picture.dimensions),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    p.rect_stroke(
        canvas,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(167, 191, 178)),
        egui::StrokeKind::Outside,
    );

    for (index, slot) in layout.samples.iter().enumerate() {
        let p = p.with_clip_rect(*slot);
        let c = slot.center();
        let r = (slot.width().min(slot.height()) * 0.42).max(1.0);
        let coral = Color32::from_rgb(228, 106, 76);
        let teal = Color32::from_rgb(85, 144, 132);
        match index {
            0 => {
                for i in 1..=5 {
                    p.circle_stroke(c, r * i as f32 / 5.0, Stroke::new(1.5, ink));
                }
            }
            1 => {
                p.add(egui::Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, -r),
                        c + Vec2::new(r, r * 0.8),
                        c + Vec2::new(-r, r * 0.8),
                    ],
                    coral,
                    Stroke::NONE,
                ));
            }
            2 => {
                let hex = (0..6)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 6.0;
                        c + Vec2::new(angle.cos(), angle.sin()) * r
                    })
                    .collect();
                p.add(egui::Shape::convex_polygon(
                    hex,
                    teal,
                    Stroke::new(2.0, ink),
                ));
            }
            3 => {
                for i in 0..9 {
                    let x = c.x - r + i as f32 * r / 4.0;
                    p.line_segment(
                        [Pos2::new(x, c.y - r), Pos2::new(x, c.y + r)],
                        Stroke::new(2.0, ink),
                    );
                }
            }
            4 => {
                p.circle_filled(c, r, coral);
            }
            5 => {
                p.rect_filled(
                    Rect::from_center_size(c, Vec2::splat(r * 2.0)),
                    r * 0.3,
                    teal,
                );
            }
            6 => {
                p.add(egui::Shape::convex_polygon(
                    vec![
                        c + Vec2::new(0.0, -r),
                        c + Vec2::new(r, 0.0),
                        c + Vec2::new(0.0, r),
                        c + Vec2::new(-r, 0.0),
                    ],
                    Color32::from_rgb(236, 188, 86),
                    Stroke::new(2.0, ink),
                ));
            }
            7 => {
                for y in 0..5 {
                    for x in 0..8 {
                        let cell = (r * 2.0 / 8.0).min(slot.height() / 5.0);
                        p.rect_filled(
                            Rect::from_min_size(
                                c + Vec2::new((x as f32 - 4.0) * cell, (y as f32 - 2.5) * cell),
                                Vec2::splat(cell),
                            ),
                            0.0,
                            if (x + y) % 2 == 0 {
                                ink
                            } else {
                                Color32::WHITE
                            },
                        );
                    }
                }
            }
            _ => {
                p.text(
                    c - Vec2::new(0.0, 10.0),
                    Align2::CENTER_CENTER,
                    "Aa 玻璃",
                    FontId::proportional((slot.width() / 4.5).min(24.0)),
                    ink,
                );
                p.text(
                    c + Vec2::new(0.0, 15.0),
                    Align2::CENTER_CENTER,
                    "0123456789",
                    FontId::monospace((slot.width() / 7.0).min(14.0)),
                    ink,
                );
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct UiScale {
    pub applied: f32,
    pub draft: f32,
}

impl Default for UiScale {
    fn default() -> Self {
        Self {
            applied: default_ui_scale(),
            draft: default_ui_scale(),
        }
    }
}

pub fn controls(
    ui: &mut egui::Ui,
    glass: &mut GlassParams,
    adapter: &str,
    pictures: &[SceneImage],
    selected: &mut usize,
    image_error: Option<&str>,
    display: (&mut crate::image_view::ImageView, &mut UiScale),
) -> bool {
    use crate::glass::{BlurMode, GlassPattern, GlassShape};
    use crate::image_view::{ImageFit, canvas};
    let (view, ui_scale) = display;
    let mut open_image = false;
    let viewport = ui.ctx().viewport_rect();
    let area = canvas(Vec2::new(
        (viewport.width() - panel_width(viewport.width())).max(1.0),
        viewport.height(),
    ));
    egui::Panel::right("parameters").exact_size(panel_width(viewport.width())).resizable(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(251, 251, 248)).inner_margin(18.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 100.0).clamp(48.0, 170.0);
                ui.add_space(8.0);
                ui.label(egui::RichText::new("玻璃实验室").size(24.0).strong());
                ui.small("轴对称曲面 / 厚度传播 / 表面散射");
                ui.add_space(8.0);
                ui.add(egui::Slider::new(&mut ui_scale.draft, 0.75..=2.0).text("界面缩放").suffix("×").max_decimals(2));
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(ui_scale.draft != ui_scale.applied, egui::Button::new("应用缩放")).clicked() {
                        ui_scale.applied = ui_scale.draft;
                    }
                    if ui.small_button("恢复默认").clicked() { ui_scale.draft = default_ui_scale(); }
                });
                ui.small(format!("当前 {:.2}× · 点击应用后生效", ui_scale.applied));
                ui.small("叠加系统 DPI；图片缩放独立调节。");
                ui.add_space(12.0);
                let previous = *selected;
                ui.horizontal_wrapped(|ui| {
                    for (index, picture) in pictures.iter().enumerate() { ui.selectable_value(selected, index, picture.label); }
                });
                if previous != *selected { view.reset(); }
                let picture = &pictures[*selected];
                ui.small(format!("{} × {} · {}", picture.dimensions[0], picture.dimensions[1], picture.credit));
                if let Some(source) = picture.source { ui.hyperlink_to("图片来源与许可", source); }
                if ui.button("打开图片…").clicked() { open_image = true; }
                if let Some(error) = image_error { ui.colored_label(Color32::from_rgb(180, 45, 40), error); }
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut view.edit_image, false, "玻璃参数");
                    ui.selectable_value(&mut view.edit_image, true, "图片操作");
                });
                ui.separator();
                if view.edit_image {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.selectable_label(view.fit == ImageFit::Fit, "合适大小").clicked() { view.set_fit(ImageFit::Fit); }
                        if ui.selectable_label(view.fit == ImageFit::Fill, "填充").clicked() { view.set_fit(ImageFit::Fill); }
                    });
                    let mut zoom = view.zoom;
                    ui.add(egui::Slider::new(&mut zoom, 0.1..=8.0).logarithmic(true).text("缩放"));
                    if zoom != view.zoom { view.zoom_at(zoom / view.zoom, area.center(), area); }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("− 缩小").clicked() { view.zoom_at(1.0 / 1.25, area.center(), area); }
                        if ui.button("+ 放大").clicked() { view.zoom_at(1.25, area.center(), area); }
                        if ui.button("重置图片").clicked() { view.reset(); }
                    });
                    ui.add_space(14.0);
                    ui.label("在固定图片区域直接拖动图片，滚轮以鼠标位置为中心缩放。图片始终保持比例，超出区域的部分会被裁切。");
                    ui.small("切换图片会重置缩放和位移。切回玻璃参数即可拖动玻璃；Shift 拖动图片在两种模式下都有效。");
                } else {
                    ui.checkbox(&mut glass.enabled, "启用玻璃效果");
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut glass.mode, GlassMode::Liquid, "液态玻璃");
                        ui.selectable_value(&mut glass.mode, GlassMode::Lens, "凸透镜");
                        ui.selectable_value(&mut glass.mode, GlassMode::Concave, "凹透镜");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut glass.shape, GlassShape::RoundedRect, "圆角矩形");
                        if ui.selectable_value(&mut glass.shape, GlassShape::Circle, "圆形").changed() {
                            glass.size = Vec2::splat(glass.size.min_elem());
                        }
                    });
                    slider(ui, "曲率", &mut glass.curvature, 0.0..=1.0);
                    slider(ui, "基础厚度", &mut glass.thickness, 0.0..=120.0);
                    ui.small("最薄处厚度；曲面会增加局部厚度。");
                    slider(ui, "背景距离", &mut glass.distance, 0.0..=300.0);
                    slider(ui, "折射率", &mut glass.ior, 1.0..=2.0);
                    if glass.mode == GlassMode::Liquid {
                        let max_margin = glass.size.min_elem() * 0.5;
                        slider(ui, "打磨宽度", &mut glass.margin, 0.0..=max_margin);
                    }
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut glass.blur_mode, BlurMode::Gaussian, "高斯模糊");
                        ui.selectable_value(&mut glass.blur_mode, BlurMode::Frosted, "毛玻璃");
                    });
                    match glass.blur_mode {
                        BlurMode::Gaussian => slider(ui, "模糊半径", &mut glass.blur, 0.0..=24.0),
                        BlurMode::Frosted => {
                            let previous = glass.pattern;
                            egui::ComboBox::from_label("玻璃图案")
                                .selected_text(glass.pattern.label())
                                .show_ui(ui, |ui| {
                                    for pattern in GlassPattern::ALL {
                                        ui.selectable_value(&mut glass.pattern, pattern, pattern.label());
                                    }
                                });
                            if previous != glass.pattern {
                                glass.pattern_scale = 1.0;
                                glass.pattern_depth = 1.0;
                                glass.pattern_rotation = 0.0;
                                glass.roughness = if glass.pattern == GlassPattern::Frost { 0.15 } else { 0.012 };
                            }
                            if glass.pattern != GlassPattern::Frost {
                                slider(ui, "纹理尺度", &mut glass.pattern_scale, 0.3..=3.0);
                                slider(ui, "纹理起伏", &mut glass.pattern_depth, 0.0..=2.5);
                                slider(ui, "纹理旋转", &mut glass.pattern_rotation, 0.0..=180.0);
                                ui.small("起伏形成折射纹路；尺度越大，图案越疏。");
                            }
                            slider(ui, "表面粗糙度", &mut glass.roughness, 0.0..=0.5);
                            ui.small("粗糙度控制细微散射；调低可看清压花纹路。厚度和背景距离影响折射范围。");
                        }
                    }
                    slider(ui, "边缘高光", &mut glass.highlight, 0.0..=1.0);
                    ui.separator();
                    if glass.shape == GlassShape::Circle {
                        slider(ui, "直径", &mut glass.size.x, 100.0..=600.0);
                        glass.size.y = glass.size.x;
                    } else {
                        slider(ui, "宽度", &mut glass.size.x, 120.0..=600.0);
                        slider(ui, "高度", &mut glass.size.y, 100.0..=420.0);
                        let max_radius = glass.size.min_elem().min(200.0) * 0.5;
                        slider(ui, "圆角", &mut glass.radius, 0.0..=max_radius);
                    }
                    ui.add_space(10.0);
                    if ui.button("重置玻璃参数与位置").clicked() { *glass = GlassParams::default(); glass.center = area.center(); }
                    ui.label("拖动玻璃；Shift 拖动图片。图片上滚轮缩放。长度单位为逻辑像素。");
                    ui.small("平背面厚玻璃模型；全反射使用近似反射，非完整光线追踪。");
                }
                ui.add_space(14.0);
                ui.small(format!("GPU: {adapter}"));
            });
        });
    open_image
}

pub fn default_ui_scale() -> f32 {
    if cfg!(target_os = "windows") {
        1.25
    } else {
        1.1
    }
}

fn slider(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.add_space(4.0);
    ui.add(egui::Slider::new(value, range).max_decimals(2).text(label));
}
