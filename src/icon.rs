//! Draws the tray icon: a coffee cup.
//!
//! | face          | cup     | steam |
//! |---------------|---------|-------|
//! | `Off`         | outline | no    |
//! | `Awake`       | filled  | no    |
//! | `AwakeScreen` | filled  | yes   |
//!
//! Drawn from signed distance functions with 4x4 supersampling rather than
//! shipped as bitmaps: it is crisp at any edge, needs no image decoder, and a
//! new face is a few lines rather than a trip through an editor.
//!
//! On macOS the result is a template image (black + alpha), which AppKit tints
//! to suit the menu bar; colour would be thrown away there anyway.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Off,
    Awake,
    AwakeScreen,
}

/// What the tray asks for. Windows downsamples to the notification-area size
/// itself, and starting from 32 keeps 150% and 200% DPI sharp.
pub const EDGE: u32 = 32;

/// The design grid every coordinate below is expressed in.
const GRID: f32 = 32.0;
const SS: u32 = 4;

type Rgb = [f32; 3];

/// RGBA8, `edge` x `edge`, straight alpha.
pub fn render(face: Face, dark_taskbar: bool, edge: u32) -> Vec<u8> {
    let (fg, fill): (Rgb, Rgb) = if cfg!(target_os = "macos") {
        ([0.0; 3], [0.0; 3])
    } else if dark_taskbar {
        ([0.95, 0.95, 0.95], [0.89, 0.58, 0.24])
    } else {
        ([0.12, 0.12, 0.12], [0.80, 0.47, 0.13])
    };

    let scale = GRID / edge as f32;
    let mut out = Vec::with_capacity((edge * edge * 4) as usize);
    for py in 0..edge {
        for px in 0..edge {
            let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) * scale;
                    if let Some(c) = sample(face, x, y, fg, fill) {
                        r += c[0];
                        g += c[1];
                        b += c[2];
                        a += 1.0;
                    }
                }
            }
            if a == 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let n = (SS * SS) as f32;
                out.extend_from_slice(&[
                    (r / a * 255.0).round() as u8,
                    (g / a * 255.0).round() as u8,
                    (b / a * 255.0).round() as u8,
                    (a / n * 255.0).round() as u8,
                ]);
            }
        }
    }
    out
}

/// The colour at one point of the design grid, or `None` for transparent.
fn sample(face: Face, x: f32, y: f32, fg: Rgb, fill: Rgb) -> Option<Rgb> {
    // The handle pushes the drawing's bounds right of the cup; this centres
    // the whole thing rather than the cup alone.
    let x = x - 1.85;
    let body = rounded_box(x, y, 13.0, 19.0, 8.5, 7.0, 3.0);
    let handle = (len(x - 21.5, y - 18.5) - 3.8).abs() - 1.2;
    let handle = if x > 21.0 { handle } else { f32::MAX };
    let saucer = capsule(x, y, (3.0, 28.8), (23.0, 28.8), 1.2);

    match face {
        Face::Off => {
            let outline = body.abs() - 1.0;
            (outline <= 0.0 || handle <= 0.0 || saucer <= 0.0).then_some(fg)
        }
        Face::Awake | Face::AwakeScreen => {
            if body <= 0.0 {
                return Some(fill);
            }
            if handle <= 0.0 || saucer <= 0.0 {
                return Some(fg);
            }
            (face == Face::AwakeScreen && steam(x, y) <= 0.0).then_some(fg)
        }
    }
}

/// Three wavy wisps above the cup.
fn steam(x: f32, y: f32) -> f32 {
    let (top, bottom) = (1.5, 9.5);
    let mut d = f32::MAX;
    for i in 0..3 {
        let x0 = 9.0 + i as f32 * 4.0;
        let yc = y.clamp(top, bottom);
        let cx = x0 + 0.9 * ((yc - top) * 0.75).sin();
        d = d.min(len(x - cx, y - yc) - 0.95);
    }
    d
}

fn len(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// Box centred at (cx, cy) with half-extents (hx, hy), corners rounded by r.
fn rounded_box(x: f32, y: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - (hx - r);
    let qy = (y - cy).abs() - (hy - r);
    len(qx.max(0.0), qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn capsule(x: f32, y: f32, a: (f32, f32), b: (f32, f32), r: f32) -> f32 {
    let (pax, pay) = (x - a.0, y - a.1);
    let (bax, bay) = (b.0 - a.0, b.1 - a.1);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    len(pax - bax * h, pay - bay * h) - r
}

/// The application icon (installer, Start menu, Finder): a cream cup with
/// steam on a rounded coffee-coloured tile. Same glyph as the tray, so the two
/// read as one product.
pub fn render_app(edge: u32) -> Vec<u8> {
    const CREAM: Rgb = [0.99, 0.95, 0.88];
    const TOP: Rgb = [0.83, 0.52, 0.22];
    const BOTTOM: Rgb = [0.45, 0.24, 0.10];
    // The glyph fills this share of the tile.
    const GLYPH: f32 = 0.70;

    let scale = GRID / edge as f32;
    let n = (SS * SS) as f32;
    let mut out = Vec::with_capacity((edge * edge * 4) as usize);
    for py in 0..edge {
        for px in 0..edge {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) * scale;
                    if rounded_box(x, y, 16.0, 16.0, 15.0, 15.0, 7.0) > 0.0 {
                        continue;
                    }
                    let gx = 16.0 + (x - 16.0) / GLYPH;
                    let gy = 16.0 + (y - 16.5) / GLYPH;
                    let c = sample(Face::AwakeScreen, gx, gy, CREAM, CREAM).unwrap_or_else(|| {
                        let t = (y / GRID).clamp(0.0, 1.0);
                        [0, 1, 2].map(|i| TOP[i] + (BOTTOM[i] - TOP[i]) * t)
                    });
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                    acc[3] += 1.0;
                }
            }
            if acc[3] == 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                out.extend_from_slice(&[
                    (acc[0] / acc[3] * 255.0).round() as u8,
                    (acc[1] / acc[3] * 255.0).round() as u8,
                    (acc[2] / acc[3] * 255.0).round() as u8,
                    (acc[3] / n * 255.0).round() as u8,
                ]);
            }
        }
    }
    out
}

/// Sizes the app icon is written at: everything an .ico, an .icns iconset
/// and the docs want, each drawn rather than resampled.
pub const APP_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 128, 256, 1024];

/// `cafe --dump-icons <dir>`: every tray face and every app icon size as raw
/// RGBA, named `<name>-<edge>.rgba`. `assets/make_icons.py` turns them into
/// icon.ico and the PNGs; nothing in the shipped app needs a PNG encoder.
pub fn dump(dir: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, face) in [
        ("off", Face::Off),
        ("awake", Face::Awake),
        ("screen", Face::AwakeScreen),
    ] {
        for (theme, dark) in [("dark", true), ("light", false)] {
            for edge in [EDGE, 128] {
                std::fs::write(
                    format!("{dir}/tray-{name}-{theme}-{edge}.rgba"),
                    render(face, dark, edge),
                )?;
            }
        }
    }
    for edge in APP_SIZES {
        std::fs::write(format!("{dir}/app-{edge}.rgba"), render_app(edge))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_sum(face: Face) -> u32 {
        render(face, true, EDGE)
            .chunks(4)
            .map(|p| p[3] as u32)
            .sum()
    }

    #[test]
    fn faces_are_distinct_and_ordered_by_ink() {
        let (off, awake, screen) = (
            alpha_sum(Face::Off),
            alpha_sum(Face::Awake),
            alpha_sum(Face::AwakeScreen),
        );
        assert!(off > 0);
        assert!(
            awake > off,
            "filled cup should carry more ink than the outline"
        );
        assert!(screen > awake, "steam should add ink");
    }

    #[test]
    fn corners_are_transparent() {
        let px = render(Face::AwakeScreen, true, EDGE);
        assert_eq!(px[3], 0);
        assert_eq!(px[px.len() - 1], 0);
    }

    #[test]
    fn app_icon_has_rounded_transparent_corners_and_opaque_centre() {
        let edge = 64;
        let px = render_app(edge);
        assert_eq!(px[3], 0);
        let centre = ((edge / 2 * edge + edge / 2) * 4 + 3) as usize;
        assert_eq!(px[centre], 255);
    }
}
