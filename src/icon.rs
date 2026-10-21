//! Draws the icons: a cup of coffee on its saucer, from the side.
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
//! On macOS the tray result is a template image (black + alpha), which AppKit
//! tints to suit the menu bar; colour would be thrown away there anyway.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Off,
    Awake,
    AwakeScreen,
}

/// What the tray asks for. Windows downsamples to the notification-area size
/// itself, and starting from 32 keeps 150% and 200% DPI sharp. On macOS it is
/// the @2x of a 16pt menu bar glyph.
pub const EDGE: u32 = 32;

const SS: u32 = 4;

type Rgb = [f32; 3];

/// A premultiplied RGBA sample.
type Rgba = [f32; 4];

/// The tray glyph's design grid: 16 units across, centred on the origin.
const TRAY_GRID: f32 = 16.0;

/// How the tray glyph is coloured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ink {
    /// macOS: black + alpha, tinted by AppKit.
    Template,
    /// Windows, against a dark taskbar.
    Dark,
    /// Windows, against a light taskbar.
    Light,
}

/// RGBA8, `edge` x `edge`, straight alpha.
pub fn render(face: Face, dark_taskbar: bool, edge: u32) -> Vec<u8> {
    let ink = if cfg!(target_os = "macos") {
        Ink::Template
    } else if dark_taskbar {
        Ink::Dark
    } else {
        Ink::Light
    };
    paint(face, ink, edge)
}

fn paint(face: Face, ink: Ink, edge: u32) -> Vec<u8> {
    let (fg, fill): (Rgb, Rgb) = match ink {
        Ink::Template => ([0.0; 3], [0.0; 3]),
        Ink::Dark => ([0.95, 0.95, 0.95], [0.89, 0.58, 0.24]),
        Ink::Light => ([0.12, 0.12, 0.12], [0.80, 0.47, 0.13]),
    };
    let half = TRAY_GRID / 2.0;
    raster(edge, |x, y, px| {
        tray(face, x * half, y * half, px * half, fg, fill)
    })
}

/// One sample of the tray glyph, in design units.
fn tray(face: Face, x: f32, y: f32, px: f32, fg: Rgb, fill: Rgb) -> Rgba {
    // The handle pushes the drawing's bounds right of the cup; this centres
    // the whole thing rather than the cup alone.
    let x = x + 1.0;
    let body = cup(x, y, -1.6, 5.4, 5.0, 3.7, 1.6);
    let handle = (len(x - 5.0, y - 1.2) - 1.9).abs() - 0.8;
    let handle = if x > 4.4 { handle } else { f32::MAX };
    let saucer = capsule(x, y, (-6.4, 7.0), (6.4, 7.0), 0.8);

    let mut out = [0.0; 4];
    match face {
        Face::Off => {
            let d = (body.abs() - 0.8).min(handle).min(saucer);
            over(&mut out, fg, cover(d, px));
        }
        Face::Awake | Face::AwakeScreen => {
            over(&mut out, fill, cover(body, px));
            over(&mut out, fg, cover(handle.min(saucer), px));
            if face == Face::AwakeScreen {
                let wisps = wisp(x, y, -1.5, -7.6, -3.0, 0.6, 0.75)
                    .min(wisp(x, y, 1.7, -7.0, -3.0, 0.6, 0.75));
                over(&mut out, fg, cover(wisps, px));
            }
        }
    }
    out
}

/// A cup's body: flat across the top at `top`, rounded at the bottom by
/// `r`, and tapering from half-width `w_top` to `w_bottom`.
fn cup(x: f32, y: f32, top: f32, bottom: f32, w_top: f32, w_bottom: f32, r: f32) -> f32 {
    let taper = 1.0 - (1.0 - w_bottom / w_top) * ((y - top) / (bottom - top)).clamp(0.0, 1.0);
    // The box runs past `top` so its rounded corners are cut off there and
    // only the bottom two show.
    let hy = (bottom - top + r) / 2.0;
    let rounded = rounded_box(x / taper, y, 0.0, bottom - hy, w_top, hy, r) * taper;
    rounded.max(top - y)
}

/// A wisp of steam: an S-curve rising from `(x0, bottom)` to `top`, with
/// half-width `r`.
fn wisp(x: f32, y: f32, x0: f32, top: f32, bottom: f32, sway: f32, r: f32) -> f32 {
    let yc = y.clamp(top, bottom);
    let phase = (yc - bottom) / (top - bottom) * std::f32::consts::TAU;
    len(x - (x0 + sway * phase.sin()), y - yc) - r
}

/// How far in from the edge the app icon's tile sits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    /// Edge to edge, for the .ico: Windows draws no frame of its own and
    /// every pixel counts at 16.
    Full,
    /// Apple's icon grid: an 824/1024 tile with room for its own shadow, so it
    /// sits the same size as every other app in the Dock and Finder.
    Mac,
}

/// The application icon (installer, Start menu, Finder, Dock).
pub fn render_app(edge: u32, tile: Tile) -> Vec<u8> {
    raster(edge, |x, y, px| app(x, y, px, tile))
}

fn hex(v: u32) -> Rgb {
    [(v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff].map(|c| c as f32 / 255.0)
}

/// One sample of the app icon, in unit space: [-1, 1] across the canvas.
///
/// A porcelain cup on its saucer, seen from a little above so the coffee
/// shows at the rim, with two wisps of steam: the tray glyph, given depth.
fn app(x: f32, y: f32, px: f32, tile: Tile) -> Rgba {
    let ink_top = hex(0x2b3a55);
    let ink_bottom = hex(0x18223a);
    let porcelain = hex(0xfffbf4);
    let porcelain_lo = hex(0xd8ccb9);
    let saucer_edge = hex(0xc4b6a0);
    let espresso = hex(0x2a140a);
    let crema = hex(0xa9693a);
    let steam = hex(0xf4ecdf);

    let half = match tile {
        Tile::Full => 0.96,
        Tile::Mac => 824.0 / 1024.0,
    };
    let mut out = [0.0; 4];

    // The tile: a superellipse, close to the continuous corners macOS uses.
    if tile == Tile::Mac {
        let shadow = soft(squircle(x, y - 0.025, half), 0.06) * 0.35;
        over(&mut out, [0.0; 3], shadow);
    }
    let inside = cover(squircle(x, y, half), px);
    over(&mut out, mix(ink_top, ink_bottom, (y + 1.0) / 2.0), inside);

    // Everything on the tile keeps its share of it whatever the padding.
    let k = half / (824.0 / 1024.0);
    let (x, y, px) = (x / k, y / k, px / k);
    // The drawing runs from the top of the steam to the foot of the saucer;
    // this centres that span on the tile.
    let y = y - 0.05;

    const RIM: f32 = -0.08;
    const BASE: f32 = 0.385;
    const W_TOP: f32 = 0.34;
    const W_BOTTOM: f32 = 0.25;
    const SAUCER: f32 = 0.39;

    // Saucer: its shadow on the tile, its edge, then its top.
    over(
        &mut out,
        [0.0; 3],
        soft(ellipse(x, y - SAUCER - 0.07, 0.58, 0.11), 0.08) * 0.35 * inside,
    );
    over(
        &mut out,
        saucer_edge,
        cover(ellipse(x, y - SAUCER - 0.03, 0.56, 0.10), px),
    );
    let saucer_top = mix(porcelain, porcelain_lo, (y - SAUCER + 0.1) / 0.2);
    over(
        &mut out,
        saucer_top,
        cover(ellipse(x, y - SAUCER, 0.56, 0.10), px),
    );
    // Where the cup stands in the saucer's well: a tight contact shadow, in
    // a deeper tone of the porcelain rather than grey.
    over(
        &mut out,
        hex(0x8a7560),
        soft(ellipse(x + 0.02, y - SAUCER - 0.004, 0.27, 0.035), 0.05) * 0.45,
    );

    // Handle, under the cup so the cup covers its root.
    let ring = (len(x - 0.34, y - 0.07) - 0.115).abs() - 0.042;
    let handle = if x > 0.28 { ring } else { f32::MAX };
    let handle_shade = mix(porcelain, porcelain_lo, 0.35 + (y - 0.07) * 2.0);
    over(&mut out, handle_shade, cover(handle, px));

    // Body: lit from the upper left, so it falls off to the right like a
    // cylinder does, with a soft highlight down the left.
    let body = cup(x, y, RIM, BASE, W_TOP, W_BOTTOM, 0.10);
    let w = W_TOP + (W_BOTTOM - W_TOP) * ((y - RIM) / (BASE - RIM)).clamp(0.0, 1.0);
    let u = (x / w).clamp(-1.0, 1.0);
    let mut shade = mix(
        porcelain,
        porcelain_lo,
        ((u + 0.35) / 1.35).max(0.0).powf(1.6),
    );
    shade = mix(shade, porcelain_lo, ((y - 0.15) / 0.25) * 0.6);
    let highlight = (1.0 - ((u + 0.55) / 0.16).powi(2)).max(0.0)
        * 0.45
        * (1.0 - ((y - 0.10) / 0.22).clamp(0.0, 1.0));
    shade = mix(shade, [1.0; 3], highlight);
    over(&mut out, shade, cover(body, px));

    // The rim, seen from above, and the coffee inside it: darker at the back
    // where the rim shades it, crema toward the front.
    over(
        &mut out,
        porcelain,
        cover(ellipse(x, y - RIM, W_TOP, 0.075), px),
    );
    let coffee = mix(
        espresso,
        crema,
        ((y - RIM + 0.05) / 0.10).clamp(0.0, 1.0).powf(1.4),
    );
    over(
        &mut out,
        coffee,
        cover(ellipse(x, y - RIM - 0.004, W_TOP - 0.035, 0.052), px),
    );
    over(
        &mut out,
        [1.0; 3],
        soft(ellipse(x + 0.12, y - RIM - 0.018, 0.07, 0.012), 0.03) * 0.35,
    );

    // Steam: two wisps drifting the same way, as steam in still air does,
    // thinning and fading in from the coffee and out at the top.
    for (x0, top) in [(-0.08, -0.64), (0.10, -0.54)] {
        let bottom = RIM - 0.08;
        let t = ((bottom - y) / (bottom - top)).clamp(0.0, 1.0);
        let r = 0.034 * (1.0 - 0.5 * t);
        let d = wisp(x, y, x0, top, bottom, 0.045, r);
        let rise = (t / 0.3).min(1.0);
        let fade = (1.0 - t).powf(0.8) * rise * rise * (3.0 - 2.0 * rise);
        over(&mut out, steam, cover(d, px) * 0.9 * fade);
    }
    out
}

/// Ellipse with radii `(rx, ry)` at the origin. An approximate distance,
/// exact enough at the edge for antialiasing.
fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> f32 {
    let k0 = len(x / rx, y / ry);
    let k1 = len(x / (rx * rx), y / (ry * ry));
    if k1 == 0.0 {
        return -rx.min(ry);
    }
    k0 * (k0 - 1.0) / k1
}

/// Supersample `f` over an `edge` x `edge` canvas. `f` gets unit-space
/// coordinates, [-1, 1] across with y down, and the size of one sample in
/// the same units, and returns premultiplied RGBA.
fn raster(edge: u32, f: impl Fn(f32, f32, f32) -> Rgba) -> Vec<u8> {
    let n = (edge * SS) as f32;
    let px = 2.0 / n;
    let mut out = Vec::with_capacity((edge * edge * 4) as usize);
    for py in 0..edge {
        for pxi in 0..edge {
            let mut acc = [0.0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = ((pxi * SS + sx) as f32 + 0.5) * px - 1.0;
                    let y = ((py * SS + sy) as f32 + 0.5) * px - 1.0;
                    let c = f(x, y, px);
                    for i in 0..4 {
                        acc[i] += c[i];
                    }
                }
            }
            let a = acc[3];
            if a <= 0.0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                out.extend_from_slice(&[
                    to8(acc[0] / a),
                    to8(acc[1] / a),
                    to8(acc[2] / a),
                    to8(a / (SS * SS) as f32),
                ]);
            }
        }
    }
    out
}

/// Paint `rgb` at coverage `a` over a premultiplied sample.
fn over(dst: &mut Rgba, rgb: Rgb, a: f32) {
    for i in 0..3 {
        dst[i] = rgb[i] * a + dst[i] * (1.0 - a);
    }
    dst[3] = a + dst[3] * (1.0 - a);
}

/// Coverage of a shape at signed distance `d`, antialiased over one sample.
fn cover(d: f32, px: f32) -> f32 {
    (0.5 - d / px).clamp(0.0, 1.0)
}

/// A soft falloff across `blur`, for shadows.
fn soft(d: f32, blur: f32) -> f32 {
    (0.5 - d / blur).clamp(0.0, 1.0).powi(2)
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn len(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// Superellipse of half-size `h`. Not an exact distance, but close enough
/// near the edge, which is the only place it is used for antialiasing.
fn squircle(x: f32, y: f32, h: f32) -> f32 {
    const N: f32 = 5.0;
    ((x / h).abs().powf(N) + (y / h).abs().powf(N)).powf(1.0 / N) * h - h
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

/// Sizes the app icon is written at: everything an .ico, an .icns iconset
/// and the docs want, each drawn rather than resampled.
pub const APP_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 128, 256, 1024];

/// `cafe --dump-icons <dir>`: every tray face and every app icon size as raw
/// RGBA, named `<name>-<edge>.rgba`. `assets/make_icons.py` turns them into
/// icon.ico and the PNGs; nothing in the shipped app needs a PNG encoder.
///
/// The `cup-*` faces are for the landing page, which shows the Windows
/// dark-taskbar colours whatever OS the dump runs on.
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
    for (name, face) in [
        ("off", Face::Off),
        ("awake", Face::Awake),
        ("screen", Face::AwakeScreen),
    ] {
        std::fs::write(
            format!("{dir}/cup-{name}-128.rgba"),
            paint(face, Ink::Dark, 128),
        )?;
    }
    for edge in APP_SIZES {
        std::fs::write(
            format!("{dir}/app-{edge}.rgba"),
            render_app(edge, Tile::Full),
        )?;
    }
    std::fs::write(format!("{dir}/mac-1024.rgba"), render_app(1024, Tile::Mac))?;
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
        for tile in [Tile::Full, Tile::Mac] {
            let edge = 64;
            let px = render_app(edge, tile);
            assert_eq!(px[3], 0, "{tile:?}");
            let centre = ((edge / 2 * edge + edge / 2) * 4 + 3) as usize;
            assert_eq!(px[centre], 255, "{tile:?}");
        }
    }

    #[test]
    fn mac_tile_leaves_apples_margin() {
        // 100px of 1024 is clear on the left; at 64 that is ~6px.
        let edge = 64;
        let px = render_app(edge, Tile::Mac);
        let mid = (edge / 2 * edge * 4) as usize;
        assert!(px[mid + 3] == 0 && px[mid + 4 * 4 + 3] == 0);
    }
}
