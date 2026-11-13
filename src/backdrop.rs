//! A backdrop: a picture shown behind the drawing, to trace by hand or to place the drawing on a photo of the work.
//! It is kept inside the project file (as a JPEG or PNG in base64), so the project can be moved or sent without it.
use crate::i18n::{tr, tr_f};
use serde::{Deserialize, Serialize};
use image::ImageEncoder;
use std::sync::Arc;

/// Pictures are reduced to this many pixels on the long side before they are kept.
pub const MAX_SIDE: u32 = 2048;

fn d_true() -> bool { true }
fn d_opacity() -> f64 { 0.4 }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Backdrop {
    pub name: String,
    /// Lower-left corner on the stock, mm.
    pub x: f64,
    pub y: f64,
    /// Width in mm; the height follows the picture's proportions.
    pub w: f64,
    /// Height divided by width of the picture.
    pub aspect: f64,
    #[serde(default = "d_opacity")] pub opacity: f64,
    #[serde(default = "d_true")] pub visible: bool,
    /// A locked backdrop is not moved by a click in the drawing.
    #[serde(default)] pub locked: bool,
    /// The picture, JPEG or PNG, in base64 (shared between the undo steps).
    pub img: Arc<str>,
}

impl Backdrop {
    /// Keep a picture (PNG, JPEG or BMP) as a backdrop `w_mm` wide with its lower-left corner at (x, y).
    pub fn from_bytes(name: &str, bytes: &[u8], w_mm: f64, x: f64, y: f64) -> Result<Backdrop, String> {
        let mut img = image::load_from_memory(bytes).map_err(|e| tr_f("Could not read the picture: {e}", &[("e", &e)]))?;
        let (w, h) = (img.width(), img.height());
        if w < 2 || h < 2 { return Err(tr("The picture is too small.").into()); }
        if w.max(h) > MAX_SIDE {
            let f = MAX_SIDE as f64 / w.max(h) as f64;
            img = img.resize_exact(((w as f64 * f).round() as u32).max(2), ((h as f64 * f).round() as u32).max(2), image::imageops::FilterType::Triangle);
        }
        let (w, h) = (img.width(), img.height());
        let mut out: Vec<u8> = Vec::new();
        if img.color().has_alpha() {
            let rgba = img.to_rgba8();
            image::codecs::png::PngEncoder::new(&mut out).write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
        } else {
            let rgb = img.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())?;
        }
        Ok(Backdrop { name: name.into(), x, y, w: w_mm.max(0.1), aspect: h as f64 / w as f64, opacity: d_opacity(), visible: true, locked: true, img: crate::relief::b64_encode(&out).into() })
    }

    /// A small copy of a picture for a preview: width, height and RGBA pixels, at most `max_side` on the long side.
    pub fn preview(bytes: &[u8], max_side: u32) -> Result<(usize, usize, Vec<u8>), String> {
        let img = image::load_from_memory(bytes).map_err(|e| tr_f("Could not read the picture: {e}", &[("e", &e)]))?;
        let img = if img.width().max(img.height()) > max_side { img.thumbnail(max_side, max_side) } else { img };
        let i = img.to_rgba8();
        Ok((i.width() as usize, i.height() as usize, i.into_raw()))
    }

    pub fn h(&self) -> f64 { self.w * self.aspect }

    /// The pixels, row by row from the top, as RGBA.
    pub fn rgba(&self) -> Option<(usize, usize, Vec<u8>)> {
        let bytes = crate::relief::b64_decode(&self.img)?;
        let i = image::load_from_memory(&bytes).ok()?.to_rgba8();
        Some((i.width() as usize, i.height() as usize, i.into_raw()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut px = Vec::new();
        for y in 0..h { for x in 0..w { px.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 90]); } }
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out).write_image(&px, w, h, image::ExtendedColorType::Rgb8).unwrap();
        out
    }

    #[test]
    fn keeps_proportions_and_round_trips() {
        let b = Backdrop::from_bytes("photo", &png(300, 150), 200.0, 10.0, 20.0).unwrap();
        assert!((b.aspect - 0.5).abs() < 1e-9 && (b.h() - 100.0).abs() < 1e-9);
        let (w, h, px) = b.rgba().unwrap();
        assert_eq!((w, h, px.len()), (300, 150, 300 * 150 * 4));
        let json = serde_json::to_string(&b).unwrap();
        let back: Backdrop = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn big_pictures_are_reduced() {
        let b = Backdrop::from_bytes("big", &png(4000, 1000), 100.0, 0.0, 0.0).unwrap();
        let (w, h, _) = b.rgba().unwrap();
        assert_eq!((w as u32, h as u32), (MAX_SIDE, 512));
    }

    #[test]
    fn a_project_without_a_backdrop_has_no_key() {
        let p = crate::shapes::blank_project();
        assert!(!serde_json::to_string(&p).unwrap().contains("backdrop"));
    }

    #[test]
    fn garbage_is_refused() { assert!(Backdrop::from_bytes("x", b"not a picture", 10.0, 0.0, 0.0).is_err()); }
}
