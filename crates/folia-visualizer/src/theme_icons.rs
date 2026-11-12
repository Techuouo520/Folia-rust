// Lucide outlines shared by theme decorations and GPU line art, flattened once per icon.

use iced::widget::canvas::{Frame, LineCap, LineJoin, Path, Stroke};
use iced::{Color, Point, Radians, Vector};
use lucide_static_svg::Icon;
use lyon_path::iterator::PathIterator;
use lyon_path::math::point;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub type Outline = Vec<Vec<[f32; 2]>>;

pub fn outline(name: &str) -> Option<Arc<Outline>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Outline>>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Mutex::default).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(outline) = cache.get(name) {
        return outline.clone();
    }
    let wanted: Vec<u8> =
        name.bytes().filter(u8::is_ascii_alphanumeric).map(|byte| byte.to_ascii_lowercase()).collect();
    let icon = Icon::all().iter().copied().find(|icon| {
        icon.name()
            .bytes()
            .filter(u8::is_ascii_alphanumeric)
            .map(|byte| byte.to_ascii_lowercase())
            .eq(wanted.iter().copied())
    });
    let Some(icon) = icon else {
        if cache.len() < 256 {
            cache.insert(name.to_owned(), None);
        }
        return None;
    };
    let mut builder = lyon_path::Path::builder();
    let mut open = false;
    let data = icon_path(icon);
    for segment in svgtypes::SimplifyingPathParser::from(data.as_str()) {
        use svgtypes::SimplePathSegment as S;
        let segment = match segment {
            Ok(segment) => segment,
            Err(err) => {
                tracing::warn!(error = %err, icon = icon.name(), "theme icon path invalid");
                return None;
            }
        };
        match segment {
            S::MoveTo { x, y } => {
                if open {
                    builder.end(false);
                }
                builder.begin(point(x as f32, y as f32));
                open = true;
            }
            S::LineTo { x, y } => {
                builder.line_to(point(x as f32, y as f32));
            }
            S::CurveTo { x1, y1, x2, y2, x, y } => {
                builder.cubic_bezier_to(
                    point(x1 as f32, y1 as f32),
                    point(x2 as f32, y2 as f32),
                    point(x as f32, y as f32),
                );
            }
            S::Quadratic { x1, y1, x, y } => {
                builder.quadratic_bezier_to(point(x1 as f32, y1 as f32), point(x as f32, y as f32));
            }
            S::ClosePath => {
                builder.end(true);
                open = false;
            }
        }
    }
    if open {
        builder.end(false);
    }
    let mut outlines = Vec::new();
    let mut current = Vec::new();
    for event in builder.build().iter().flattened(0.12) {
        match event {
            lyon_path::Event::Begin { at } => {
                current.clear();
                current.push([at.x, at.y]);
            }
            lyon_path::Event::Line { to, .. } => current.push([to.x, to.y]),
            lyon_path::Event::End { first, close, .. } => {
                if close {
                    current.push([first.x, first.y]);
                }
                if current.len() > 1 {
                    outlines.push(std::mem::take(&mut current));
                }
            }
            _ => {}
        }
    }
    let outlines = Arc::new(outlines);
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(name.to_owned(), Some(outlines.clone()));
    Some(outlines)
}

fn icon_path(icon: Icon) -> String {
    let mut data = String::new();
    for tag in icon.svg_str().split('<').skip(1) {
        let attr = |name: &str| {
            let from = tag.find(&format!(" {name}=\""))? + name.len() + 3;
            tag[from..].split('"').next()
        };
        let number = |name: &str, default: f32| attr(name).and_then(|text| text.parse::<f32>().ok()).unwrap_or(default);
        match tag.split_whitespace().next().unwrap_or_default() {
            "path" => {
                if let Some(path) = attr("d") {
                    data.push_str(path);
                    data.push(' ');
                }
            }
            "circle" | "ellipse" => {
                let (x, y) = (number("cx", 0.0), number("cy", 0.0));
                let rx = number("rx", number("r", 0.0));
                let ry = number("ry", number("r", 0.0));
                data.push_str(&format!(
                    "M {} {y} a {rx} {ry} 0 1 0 {} 0 a {rx} {ry} 0 1 0 {} 0 z ",
                    x - rx,
                    2.0 * rx,
                    -2.0 * rx
                ));
            }
            "line" => data.push_str(&format!(
                "M {} {} L {} {} ",
                number("x1", 0.0),
                number("y1", 0.0),
                number("x2", 0.0),
                number("y2", 0.0)
            )),
            "polyline" | "polygon" => {
                if let Some(points) = attr("points") {
                    data.push('M');
                    data.push_str(points);
                    if tag.starts_with("polygon") {
                        data.push('z');
                    }
                    data.push(' ');
                }
            }
            "rect" => {
                let (x, y, w, h) = (number("x", 0.0), number("y", 0.0), number("width", 0.0), number("height", 0.0));
                let r = number("rx", 0.0).min(w / 2.0).min(h / 2.0);
                if r == 0.0 {
                    data.push_str(&format!("M {x} {y} h {w} v {h} h {} z ", -w));
                } else {
                    data.push_str(&format!("M {} {y} h {} a {r} {r} 0 0 1 {r} {r} v {} a {r} {r} 0 0 1 {} {r} h {} a {r} {r} 0 0 1 {} {} v {} a {r} {r} 0 0 1 {r} {} z ", x + r, w - 2.0*r, h - 2.0*r, -r, -w + 2.0*r, -r, -r, -h + 2.0*r, -r));
                }
            }
            _ => {}
        }
    }
    data
}

pub fn draw(frame: &mut Frame, outline: &Outline, center: Point, size: f32, rotation: f32, color: Color, width: f32) {
    frame.with_save(|frame| {
        frame.translate(Vector::new(center.x, center.y));
        frame.rotate(Radians(rotation));
        for points in outline {
            let path = Path::new(|path| {
                for (index, p) in points.iter().enumerate() {
                    let p = Point::new((p[0] - 12.0) * size / 24.0, (p[1] - 12.0) * size / 24.0);
                    if index == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
            });
            frame.stroke(
                &path,
                Stroke {
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default().with_color(color).with_width(width)
                },
            );
        }
    });
}
