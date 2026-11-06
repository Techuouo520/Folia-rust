// The sign-in code as a QR matrix the interface can paint, so it never sees the address it encodes.

use crate::OmniError;

/// A sign-in code: the key to poll with and its QR modules, row by row, `true` where dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginQr {
    pub key: String,
    pub width: usize,
    pub modules: Vec<bool>,
    /// The sign-in happens on this page in the system browser instead; there are no modules.
    pub browser: Option<String>,
}

/// QR versions are 21 modules wide and four more each, up to version 40's 177.
const MIN_WIDTH: usize = 21;
const MAX_WIDTH: usize = 177;
/// A correct width alternates on every timing module; a wrong one drifts off and scores about half.
const MIN_TIMING_AGREEMENT: f64 = 0.9;
/// How far the estimated width may sit from a candidate, so a stray finder run cannot reach an unrelated version.
const WIDTH_SLACK: f64 = 12.0;

impl LoginQr {
    pub(crate) fn encode(key: String, url: &str) -> Result<Self, OmniError> {
        let code = qrcode::QrCode::new(url.as_bytes())?;
        let modules = code.to_colors().into_iter().map(|color| color == qrcode::Color::Dark).collect();
        Ok(Self { key, width: code.width(), modules, browser: None })
    }

    pub(crate) fn browser(key: String, url: String) -> Self {
        Self { key, width: 0, modules: Vec::new(), browser: Some(url) }
    }

    /// Reads the modules back from a platform's PNG of the code, for the platforms that hand out a picture rather
    /// than an address: the dark area is cropped, only legal QR widths are tried against the timing pattern, and
    /// the chosen width is sampled at each module's centre. A picture with no legal width is refused.
    pub fn decode_png(key: String, png: &[u8]) -> Result<Self, OmniError> {
        let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .map_err(|err| OmniError::QrImage(err.to_string()))?
            .to_luma8();
        let (w, h) = picture.dimensions();
        let dark = |x: u32, y: u32| picture.get_pixel(x, y).0[0] < 128;
        let mut bounds: Option<(u32, u32, u32, u32)> = None;
        for y in 0..h {
            for x in 0..w {
                if dark(x, y) {
                    let (x0, y0, x1, y1) = bounds.unwrap_or((x, y, x, y));
                    bounds = Some((x0.min(x), y0.min(y), x1.max(x), y1.max(y)));
                }
            }
        }
        let (x0, y0, x1, y1) = bounds.ok_or_else(|| OmniError::QrImage("no dark modules".to_owned()))?;
        let run = (x0..=x1).take_while(|x| dark(*x, y0)).count() as f64;
        let finder = run / 7.0;
        if finder < 1.0 {
            return Err(OmniError::QrImage(format!("finder of {finder}px per module")));
        }
        let side = ((x1 - x0 + 1).max(y1 - y0 + 1)) as f64;
        let width = best_width(&picture, x0, y0, side, side / finder)?;
        let module = side / width as f64;
        let mut modules = Vec::with_capacity(width * width);
        for row in 0..width {
            for column in 0..width {
                modules.push(sample(&picture, x0, y0, column, row, module));
            }
        }
        let qr = Self { key, width, modules, browser: None };
        if !qr.well_formed() {
            return Err(OmniError::QrImage(format!("{width} modules are not a QR code")));
        }
        tracing::info!(width, module = module as f32, "read the sign-in code picture");
        Ok(qr)
    }

    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.width && self.modules.get(y * self.width + x).copied().unwrap_or(false)
    }

    /// The three finder patterns with their separators and the module that is always dark, where the spec puts them.
    fn well_formed(&self) -> bool {
        let width = self.width;
        let finder = |left: usize, top: usize| {
            (0..7).all(|row| (0..7).all(|column| self.dark(left + column, top + row) == finder_module(column, row)))
        };
        let top = (0..8).all(|x| !self.dark(x, 7)) && (0..8).all(|y| !self.dark(7, y));
        let right = ((width - 8)..width).all(|x| !self.dark(x, 7)) && (0..8).all(|y| !self.dark(width - 8, y));
        let bottom = (0..8).all(|x| !self.dark(x, width - 8)) && ((width - 8)..width).all(|y| !self.dark(7, y));
        finder(0, 0)
            && finder(width - 7, 0)
            && finder(0, width - 7)
            && top
            && right
            && bottom
            && self.dark(8, width - 8)
    }
}

/// The legal width whose timing patterns agree best, or an error when every candidate drifts.
fn best_width(picture: &image::GrayImage, x0: u32, y0: u32, side: f64, estimate: f64) -> Result<usize, OmniError> {
    let mut best: Option<(usize, f64)> = None;
    for width in (MIN_WIDTH..=MAX_WIDTH).step_by(4) {
        if (width as f64 - estimate).abs() > WIDTH_SLACK {
            continue;
        }
        let agreement = timing_agreement(picture, x0, y0, side / width as f64, width);
        if agreement >= MIN_TIMING_AGREEMENT && best.is_none_or(|(_, score)| agreement > score) {
            best = Some((width, agreement));
        }
    }
    best.map(|(width, _)| width).ok_or_else(|| OmniError::QrImage("no QR width fits the timing pattern".to_owned()))
}

/// How well the modules at row six and column six alternate over `width`, as the timing pattern must.
fn timing_agreement(picture: &image::GrayImage, x0: u32, y0: u32, module: f64, width: usize) -> f64 {
    let mut matched = 0;
    let mut total = 0;
    for index in 8..=width - 9 {
        let expected = (index - 8) % 2 == 0;
        matched += usize::from(sample(picture, x0, y0, index, 6, module) == expected);
        matched += usize::from(sample(picture, x0, y0, 6, index, module) == expected);
        total += 2;
    }
    matched as f64 / total as f64
}

/// The dark flag at the centre of the module in `column`/`row`.
fn sample(picture: &image::GrayImage, x0: u32, y0: u32, column: usize, row: usize, module: f64) -> bool {
    let x = x0 as f64 + (column as f64 + 0.5) * module;
    let y = y0 as f64 + (row as f64 + 0.5) * module;
    let (w, h) = picture.dimensions();
    x < f64::from(w) && y < f64::from(h) && picture.get_pixel(x as u32, y as u32).0[0] < 128
}

/// A 7×7 finder at its own origin: a dark border, a light ring and a 3×3 dark centre.
fn finder_module(column: usize, row: usize) -> bool {
    let border = column == 0 || column == 6 || row == 0 || row == 6;
    border || ((2..=4).contains(&column) && (2..=4).contains(&row))
}
