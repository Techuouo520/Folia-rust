// Panel grooves and glass against the original CSS: color-mix, saturate(), then source-over.

use folia_core::Theme;
use folia_render::{glass_over, source_over, to_linear_channel, to_srgb_channel};
use folia_ui::{Faded, Tokens, panel_glass, quick_tile_fill, tools_glass};
use iced::Color;

const BACKDROP: [f32; 3] = [64.0 / 255.0, 140.0 / 255.0, 204.0 / 255.0];

fn close(got: [f32; 3], bytes: [u8; 3]) {
    for (channel, expect) in got.into_iter().zip(bytes) {
        let want = f32::from(expect) / 255.0;
        assert!((channel - want).abs() <= 1.0 / 255.0, "{got:?} vs {bytes:?}");
    }
}

fn rgb(color: Color) -> [f32; 3] {
    [color.r, color.g, color.b]
}

fn hex(value: u32) -> [f32; 3] {
    [((value >> 16) & 0xff) as f32 / 255.0, ((value >> 8) & 0xff) as f32 / 255.0, (value & 0xff) as f32 / 255.0]
}

fn same(got: [f32; 3], expect: [f32; 3]) {
    close(got, expect.map(|channel| (channel * 255.0).round() as u8));
    for (left, right) in got.into_iter().zip(expect) {
        assert!((left - right).abs() <= 1.0 / 255.0, "{got:?} vs {expect:?}");
    }
}

fn paint(base: [f32; 3], color: Color) -> [f32; 3] {
    source_over(base, [color.r, color.g, color.b, color.a])
}

#[test]
fn kit_tokens_match_daylight_and_dark() {
    for daylight in [true, false] {
        let theme = if daylight { Theme::DAYLIGHT } else { Theme::MIDNIGHT };
        let tokens = Tokens::for_panel(&theme, daylight);
        let primary = hex(if daylight { 0x1c1917 } else { 0xf4f4f5 });
        same(rgb(tokens.win), hex(if daylight { 0xf3f3f4 } else { 0x1a1a1d }));
        same(rgb(tokens.side), hex(if daylight { 0xe9e9eb } else { 0x141416 }));
        same(rgb(tokens.card), hex(if daylight { 0xffffff } else { 0x232327 }));
        same(rgb(tokens.menu), hex(if daylight { 0xffffff } else { 0x2b2b30 }));
        same(rgb(tokens.pv), hex(if daylight { 0xececef } else { 0x151517 }));
        same(rgb(tokens.knob), hex(if daylight { 0xffffff } else { 0xececee }));
        same(rgb(tokens.t1), primary);
        assert!((tokens.t1.a - 1.0).abs() <= 1.0 / 255.0);
        for (color, share) in [
            (tokens.t2, 0.62),
            (tokens.t3, 0.42),
            (tokens.line, 0.08),
            (tokens.line2, 0.16),
            (tokens.fill, 0.07),
            (tokens.fill2, 0.12),
            (tokens.track, 0.17),
        ] {
            same(rgb(color), primary);
            assert!((color.a - share).abs() <= 1.0 / 255.0, "alpha {} vs {share}", color.a);
        }
        let raise = if daylight { (hex(0xffffff), 1.0) } else { ([1.0, 1.0, 1.0], 0.18) };
        same(rgb(tokens.raise), raise.0);
        assert!((tokens.raise.a - raise.1).abs() <= 1.0 / 255.0);
        same(rgb(tokens.accent), hex(if daylight { 0xea580c } else { 0xf4f4f5 }));
        same(rgb(tokens.on_accent), hex(0x17171a));
        let glass = panel_glass(daylight);
        let expect = if daylight {
            [244.0 / 255.0, 244.0 / 255.0, 245.0 / 255.0, 0.8]
        } else {
            [28.0 / 255.0, 28.0 / 255.0, 31.0 / 255.0, 0.8]
        };
        for (got, want) in glass.into_iter().zip(expect) {
            assert!((got - want).abs() <= 1.0 / 255.0);
        }
    }
}

#[test]
fn grooves_and_glass_match_the_css_composite() {
    let encoded = BACKDROP[1];
    assert!((to_srgb_channel(to_linear_channel(encoded)) - encoded).abs() <= 1.0 / 255.0);

    let day_tokens = Tokens::for_panel(&Theme::DAYLIGHT, true);
    let day_glass = glass_over(BACKDROP, panel_glass(true), 1.3);
    close(day_glass, [204, 224, 241]);
    let day_tile = quick_tile_fill(day_tokens, false, true, false);
    let day_off = quick_tile_fill(day_tokens, false, false, false);
    assert_eq!(
        (day_tile.r, day_tile.g, day_tile.b, day_tile.a),
        (day_tokens.fill.r, day_tokens.fill.g, day_tokens.fill.b, day_tokens.fill.a)
    );
    close(paint(day_glass, day_tile), [192, 210, 226]);
    close(paint(day_glass, day_tokens.fill), [192, 210, 226]);
    close(paint(day_glass, day_off), [199, 218, 235]);
    for (groove, glass) in paint(day_glass, day_tile).into_iter().zip(day_glass) {
        assert!(groove < glass, "daylight groove stays under the glass");
    }

    let night_tokens = Tokens::for_panel(&Theme::MIDNIGHT, false);
    let night_glass = glass_over(BACKDROP, panel_glass(false), 1.3);
    close(night_glass, [31, 51, 70]);
    let night_tile = quick_tile_fill(night_tokens, false, true, false);
    let night_off = quick_tile_fill(night_tokens, false, false, false);
    close(paint(night_glass, night_tile), [46, 65, 82]);
    close(paint(night_glass, night_tokens.fill), [46, 65, 82]);
    close(paint(night_glass, night_off), [37, 56, 75]);

    let tools = Faded::new(day_tokens, day_tokens.t1, day_tokens.accent, true, 1.0);
    let tools_glass_color = glass_over(BACKDROP, tools_glass(true), 1.0);
    close(tools_glass_color, [182, 211, 236]);
    close(paint(tools_glass_color, tools.wash(0.05, 0.06)), [173, 201, 224]);
    close(paint(tools_glass_color, tools.faded(0.35).wash(0.05, 0.06)), [179, 208, 231]);

    let night_tools = Faded::new(night_tokens, night_tokens.t1, night_tokens.accent, false, 1.0);
    let night_tools_glass = glass_over(BACKDROP, tools_glass(false), 1.0);
    close(night_tools_glass, [38, 84, 122]);
    close(paint(night_tools_glass, night_tools.wash(0.05, 0.06)), [51, 94, 130]);
    close(paint(night_tools_glass, night_tools.faded(0.35).wash(0.05, 0.06)), [43, 88, 125]);
}
