// Font resolution and Chromium reference regressions for all interface typography.

use iced::advanced::graphics::text::{cosmic_text as cosmic, font_system, to_attributes};
use iced::{Font, font};

#[test]
fn css_weights_stay_in_the_family() {
    use folia_render::typeface::css_weight;
    for (requested, actual) in
        [(100, 290), (300, 290), (400, 400), (450, 400), (500, 400), (550, 700), (600, 700), (800, 700), (900, 700)]
    {
        assert_eq!(css_weight(&[290, 400, 700], requested), Some(actual));
    }
    assert_eq!(css_weight(&[400, 500, 700], 450), Some(500));
    assert_eq!(css_weight(&[400, 600, 900], 800), Some(900));
    assert_eq!(css_weight(&[], 600), None);
}

#[test]
#[cfg(target_os = "windows")]
fn vietnamese_uses_the_original_segoe_override() {
    folia_ui::install_fonts();
    let mut shared = font_system().write().unwrap();
    let system = shared.raw();
    let font = folia_ui::interface_font_for(folia_core::Language::Vi);
    let mut buffer = cosmic::Buffer::new(system, cosmic::Metrics::new(14.0, 20.0));
    buffer.set_text(system, "Tiếng Việt ệ ộ ở", &to_attributes(font), cosmic::Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    for line in buffer.layout_runs() {
        for glyph in line.glyphs {
            assert_ne!(glyph.glyph_id, 0);
            assert!(system.db().face(glyph.font_id).unwrap().families.iter().any(|(family, _)| family == "Segoe UI"));
        }
    }
}

#[test]
fn shared_roles_resolve_the_declared_weight() {
    folia_ui::install_fonts();
    let mut shared = font_system().write().unwrap();
    let system = shared.raw();
    for role in folia_ui::UI_TEXT_STYLES {
        let mut buffer = cosmic::Buffer::new(system, cosmic::Metrics::new(role.size, role.line_height));
        let attrs = to_attributes(folia_render::typeface::font(role.weight));
        buffer.set_text(system, "Tech 登录到 网易云", &attrs, cosmic::Shaping::Advanced, None);
        buffer.shape_until_scroll(system, false);
        for line in buffer.layout_runs() {
            for glyph in line.glyphs {
                assert_ne!(glyph.glyph_id, 0, "{}", role.name);
                assert_eq!(glyph.font_weight, attrs.weight, "{}", role.name);
                assert_eq!(glyph.font_size, role.size, "{}", role.name);
            }
        }
    }
}

#[test]
fn tracked_runs_preserve_selection_clusters_and_bidi() {
    use folia_render::text_run::{Run, Style};
    folia_ui::install_fonts();
    for text in ["office书库", "A\u{301}音乐", "مرحبا世界", "书库（测试）"] {
        for vertical in [false, true] {
            let run = Run::new(
                text,
                Style {
                    font: folia_ui::INTERFACE_FONT,
                    size: 14.0,
                    line_height: 20.0,
                    tracking: 1.4,
                    vertical,
                    upright: false,
                    width: None,
                },
            );
            assert_eq!(run.clusters.len(), text.chars().count());
            assert!(run.size.width.is_finite() && run.size.height.is_finite());
        }
    }
    let style = Style {
        font: folia_ui::INTERFACE_FONT,
        size: 14.0,
        line_height: 20.0,
        tracking: 0.0,
        vertical: true,
        upright: true,
        width: None,
    };
    assert!((Run::new("A", style).size.height - 14.0).abs() < 0.1);
}

#[test]
fn horizontal_tracking_uses_css_pixels() {
    folia_ui::install_fonts();
    let run = folia_render::text_run::Run::new(
        "书库",
        folia_render::text_run::Style {
            font: folia_render::typeface::font(font::Weight::ExtraBold),
            size: 15.0,
            line_height: 19.5,
            tracking: 4.5,
            vertical: false,
            upright: false,
            width: None,
        },
    );
    assert!((run.size.width - 39.0).abs() < 0.1, "{}", run.size.width);
    assert!((run.clusters[1].x - 19.5).abs() < 0.1);
}

#[test]
fn physical_origins_align_after_dpi_scaling() {
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        for origin in [-0.3, 0.0, 0.5, 8.3, 45.125] {
            let physical = folia_render::text_run::pixel_origin(origin, scale) * scale;
            assert!((physical - physical.round()).abs() < 0.0001);
        }
    }
}

#[test]
#[cfg(target_os = "windows")]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU and Windows system fonts")]
fn chromium_vertical_metrics_and_ink() {
    use folia_render::text_run::{self, Run, Style};
    use iced::advanced::renderer::Headless;
    use iced::{Color, Pixels, Size};
    use iced_runtime::user_interface::{Cache, UserInterface};
    folia_ui::install_fonts();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fonts");
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("vertical.json")).unwrap()).unwrap();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    for (index, s) in data.as_array().unwrap().iter().enumerate() {
        let size = s["size"].as_f64().unwrap() as f32;
        let dark = s["dark"].as_bool().unwrap();
        let text = s["text"].as_str().unwrap();
        let style = Style {
            font: Font { weight: font::Weight::Bold, ..folia_ui::INTERFACE_FONT },
            size,
            line_height: size * 1.15,
            tracking: s["tracking"].as_f64().unwrap() as f32,
            vertical: true,
            upright: false,
            width: None,
        };
        let run = Run::new(text, style);
        println!("vertical {text} {size}: {:?} vs {}x{}", run.size, s["width"], s["height"]);
        assert!((run.size.width - s["width"].as_f64().unwrap() as f32).abs() < 0.1);
        assert!((run.size.height - s["height"].as_f64().unwrap() as f32).abs() < 1.0);
        let bg = if dark { Color::from_rgb8(26, 26, 29) } else { Color::from_rgb8(243, 243, 244) };
        let fg = if dark { Color::from_rgb8(244, 244, 245) } else { Color::from_rgb8(24, 24, 27) };
        let root: iced::Element<'_, ()> = iced::widget::container(text_run::text(text, style, fg)).padding(8).into();
        let mut ui = UserInterface::build(root, Size::new(80.0, 240.0), Cache::default(), &mut renderer);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style { text_color: fg },
            iced::mouse::Cursor::Unavailable,
        );
        let pixels =
            renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(80, 240), 1.0), bg);
        let actual = image::RgbaImage::from_raw(80, 240, pixels).unwrap();
        if let Some(dir) = std::env::var_os("FOLIA_FONT_CAPTURE") {
            std::fs::create_dir_all(&dir).unwrap();
            actual.save(std::path::PathBuf::from(dir).join(format!("vertical-{index}.png"))).unwrap();
        }
        let reference = image::open(fixtures.join(format!("vertical-{index}.png"))).unwrap().to_rgba8();
        let ink =
            |im: &image::RgbaImage| im.pixels().map(|p| (f64::from(p[0]) - f64::from(bg.r) * 255.0).abs()).sum::<f64>();
        let ratio = ink(&actual) / ink(&reference);
        println!("vertical ink {index}: {ratio:.4}");
        assert!((ratio - 1.0).abs() < 0.15);
    }
}

#[test]
#[cfg_attr(feature = "ci", ignore = "requires a hardware GPU and Windows system fonts")]
#[cfg(target_os = "windows")]
fn chromium_ink_matrix() {
    use iced::advanced::renderer::Headless;
    use iced::{Color, Pixels, Size};
    use iced_runtime::user_interface::{Cache, UserInterface};
    folia_ui::install_fonts();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fonts");
    let data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("samples.json")).unwrap()).unwrap();
    let mut renderer =
        pollster::block_on(<iced::Renderer as Headless>::new(folia_ui::INTERFACE_FONT, Pixels(16.0), None)).unwrap();
    for (index, sample) in data["samples"].as_array().unwrap().iter().enumerate() {
        let dark = sample["dark"].as_bool().unwrap();
        let size = sample["size"].as_f64().unwrap() as f32;
        let weight = match sample["weight"].as_u64().unwrap() {
            400 => font::Weight::Normal,
            500 => font::Weight::Medium,
            600 => font::Weight::Semibold,
            700 => font::Weight::Bold,
            800 => font::Weight::ExtraBold,
            _ => font::Weight::Black,
        };
        let bg = if dark { Color::from_rgb8(26, 26, 29) } else { Color::from_rgb8(243, 243, 244) };
        let fg = if dark { Color::from_rgb8(244, 244, 245) } else { Color::from_rgb8(24, 24, 27) };
        let text = folia_ui::interface_text(sample["text"].as_str().unwrap())
            .size(size)
            .font(Font { weight, ..folia_ui::INTERFACE_FONT })
            .line_height(iced::widget::text::LineHeight::Absolute(40.0.into()))
            .color(fg);
        let tracking = sample["tracking"].as_f64().unwrap_or(0.0) as f32;
        let text: iced::Element<'_, ()> = if tracking == 0.0 {
            text.into()
        } else {
            folia_render::text_run::text(
                sample["text"].as_str().unwrap(),
                folia_render::text_run::Style {
                    font: Font { weight, ..folia_ui::INTERFACE_FONT },
                    size,
                    line_height: 40.0,
                    tracking,
                    vertical: false,
                    upright: false,
                    width: None,
                },
                fg,
            )
        };
        let root: iced::Element<'_, ()> =
            iced::widget::container(text).padding(iced::Padding { left: 8.0, ..iced::Padding::ZERO }).into();
        let mut ui = UserInterface::build(root, Size::new(600.0, 40.0), Cache::default(), &mut renderer);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style { text_color: fg },
            iced::mouse::Cursor::Unavailable,
        );
        let scale = sample["scale"].as_f64().unwrap_or(1.0) as f32;
        let (w, h) = ((600.0 * scale) as u32, (40.0 * scale) as u32);
        let pixels =
            renderer.screenshot(&iced::advanced::graphics::Viewport::with_physical_size(Size::new(w, h), scale), bg);
        let actual = image::RgbaImage::from_raw(w, h, pixels).unwrap();
        let reference = image::open(fixtures.join(sample["reference"].as_str().unwrap())).unwrap().to_rgba8();
        let ink =
            |im: &image::RgbaImage| im.pixels().map(|p| (f64::from(p[0]) - f64::from(bg.r) * 255.0).abs()).sum::<f64>();
        let ratio = ink(&actual) / ink(&reference);
        println!("ink {index} {size}px {weight:?} dark={dark}: {ratio:.4}");
        if let Some(path) = std::env::var_os("FOLIA_FONT_CAPTURE") {
            std::fs::create_dir_all(&path).unwrap();
            actual.save(std::path::PathBuf::from(path).join(format!("raster-{index}.png"))).unwrap();
        }
        assert!((ratio - 1.0).abs() < 0.15, "{index}: {ratio}");
    }
}

#[test]
fn interface_font_resolution() {
    folia_ui::install_fonts();
    let mut shared = font_system().write().unwrap();
    let system = shared.raw();
    for weight in [
        font::Weight::Normal,
        font::Weight::Medium,
        font::Weight::Semibold,
        font::Weight::Bold,
        font::Weight::ExtraBold,
        font::Weight::Black,
    ] {
        let mut buffer = cosmic::Buffer::new(system, cosmic::Metrics::new(20.0, 28.0));
        buffer.set_text(
            system,
            "Tech悠悠 登录到 网易云",
            &to_attributes(Font { weight, ..folia_ui::INTERFACE_FONT }),
            cosmic::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(system, false);
        let mut seen = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                assert_ne!(glyph.glyph_id, 0);
                if seen.contains(&glyph.font_id) {
                    continue;
                }
                seen.push(glyph.font_id);
                let face = system.db().face(glyph.font_id).unwrap();
                #[cfg(target_os = "windows")]
                assert!(
                    glyph.start >= 4 && !system.locale().starts_with("zh")
                        || face.families.iter().any(|(name, _)| name == "Folia CSS system"),
                    "system UI resolved to {:?}",
                    face.families
                );
                let path = match &face.source {
                    cosmic::fontdb::Source::File(path) | cosmic::fontdb::Source::SharedFile(path, _) => {
                        path.display().to_string()
                    }
                    cosmic::fontdb::Source::Binary(_) => "memory".into(),
                };
                println!("{weight:?}: {:?} weight={} index={} {path}", face.families, face.weight.0, face.index);
            }
        }
    }
}
