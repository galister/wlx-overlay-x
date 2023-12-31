use std::{rc::Rc, str::FromStr};

use fontconfig::{FontConfig, OwnedPattern};
use freetype::{bitmap::PixelMode, face::LoadFlag, Face, Library};
use gles31::{
    glBindBuffer, glBindTexture, glGetError, glPixelStorei, glTexImage2D, GL_NO_ERROR,
    GL_PACK_ALIGNMENT, GL_PIXEL_UNPACK_BUFFER, GL_R8, GL_TEXTURE_2D, GL_UNPACK_ALIGNMENT,
    GL_UNSIGNED_BYTE, GL_UNSIGNED_INT, GL_UNSIGNED_SHORT,
};
use idmap::IdMap;
use log::debug;
use stereokit::{SkDraw, StereoKitMultiThread, Tex, TextureType};

use crate::overlay::COLOR_FALLBACK;

const PRIMARY_FONT: &str = "LiberationSans";
const GL_RED: u32 = 0x1903;

pub struct FontCache {
    fc: FontConfig,
    ft: Library,
    collections: IdMap<isize, FontCollection>,
}

struct FontCollection {
    fonts: Vec<Font>,
    cp_map: IdMap<usize, usize>,
}

struct Font {
    face: Face,
    path: String,
    index: isize,
    size: isize,
    glyphs: IdMap<usize, Rc<Glyph>>,
}

pub struct Glyph {
    pub tex: Option<Tex>,
    pub top: f32,
    pub left: f32,
    pub width: f32,
    pub height: f32,
    pub advance: f32,
}

impl FontCache {
    pub fn new() -> Self {
        let ft = Library::init().expect("Failed to initialize freetype");
        let fc = FontConfig::default();

        FontCache {
            fc,
            ft,
            collections: IdMap::new(),
        }
    }

    pub fn get_text_size(&mut self, text: &str, size: isize, sk: &SkDraw) -> (f32, f32) {
        let sizef = size as f32;

        let height = sizef + ((text.lines().count() as f32) - 1f32) * (sizef * 1.5);

        let mut max_w = sizef * 0.33;
        for line in text.lines() {
            let w: f32 = line
                .chars()
                .map(|c| self.get_glyph_for_cp(c as usize, size, sk).advance)
                .sum();

            if w > max_w {
                max_w = w;
            }
        }
        (max_w, height)
    }

    pub fn get_glyphs(&mut self, text: &str, size: isize, sk: &SkDraw) -> Vec<Rc<Glyph>> {
        let mut glyphs = Vec::new();
        for line in text.lines() {
            for c in line.chars() {
                glyphs.push(self.get_glyph_for_cp(c as usize, size, sk));
            }
        }
        glyphs
    }

    fn get_font_for_cp(&mut self, cp: usize, size: isize) -> usize {
        if !self.collections.contains_key(size) {
            self.collections.insert(
                size,
                FontCollection {
                    fonts: Vec::new(),
                    cp_map: IdMap::new(),
                },
            );
        }
        let coll = self.collections.get_mut(size).unwrap();

        if let Some(font) = coll.cp_map.get(cp) {
            return *font;
        }

        let pattern_str = format!("{PRIMARY_FONT}-{size}:style=bold:charset={cp:04x}");

        let mut pattern =
            OwnedPattern::from_str(&pattern_str).expect("Failed to create fontconfig pattern");
        self.fc
            .substitute(&mut pattern, fontconfig::MatchKind::Pattern);
        pattern.default_substitute();

        let pattern = pattern.font_match(&mut self.fc);

        if let Some(path) = pattern.filename() {
            debug!(
                "Loading font: {} {}pt",
                pattern.name().unwrap_or(path),
                size
            );

            let font_idx = pattern.face_index().unwrap_or(0);

            let face = self
                .ft
                .new_face(path, font_idx as _)
                .expect("Failed to load font face");
            face.set_char_size(size << 6, size << 6, 96, 96)
                .expect("Failed to set font size");

            let idx = coll.fonts.len();
            for cp in 0..0xFFFF {
                if coll.cp_map.contains_key(cp) {
                    continue;
                }
                let g = face.get_char_index(cp);
                if g > 0 {
                    coll.cp_map.insert(cp, idx);
                }
            }

            let zero_glyph = Rc::new(Glyph {
                tex: None,
                top: 0.,
                left: 0.,
                width: 0.,
                height: 0.,
                advance: size as f32 / 3.,
            });
            let mut glyphs = IdMap::new();
            glyphs.insert(0, zero_glyph);

            let font = Font {
                face,
                path: path.to_string(),
                size,
                index: font_idx as _,
                glyphs,
            };
            coll.fonts.push(font);

            idx
        } else {
            coll.cp_map.insert(cp, 0);
            0
        }
    }

    fn get_glyph_for_cp(&mut self, cp: usize, size: isize, sk: &SkDraw) -> Rc<Glyph> {
        let key = self.get_font_for_cp(cp, size);

        let font = &mut self.collections[size].fonts[key];

        if let Some(glyph) = font.glyphs.get(cp) {
            return glyph.clone();
        }

        if font.face.load_char(cp, LoadFlag::DEFAULT).is_err() {
            return font.glyphs[0].clone();
        }

        let glyph = font.face.glyph();
        if glyph.render_glyph(freetype::RenderMode::Normal).is_err() {
            return font.glyphs[0].clone();
        }

        let bmp = glyph.bitmap();
        let buf = bmp.buffer();
        let metrics = glyph.metrics();

        let (pf, pt) = match bmp.pixel_mode() {
            Ok(PixelMode::Gray) => (GL_RED, GL_UNSIGNED_BYTE),
            Ok(PixelMode::Gray2) => (GL_RED, GL_UNSIGNED_SHORT),
            Ok(PixelMode::Gray4) => (GL_RED, GL_UNSIGNED_INT),
            _ => return font.glyphs[0].clone(),
        };

        let tex = sk.tex_gen_color(
            COLOR_FALLBACK,
            bmp.width() as _,
            bmp.rows() as _,
            TextureType::IMAGE_NO_MIPS,
            stereokit::TextureFormat::R8,
        );
        unsafe {
            let handle = sk.tex_get_surface(tex.as_ref()) as usize as u32;
            glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBindTexture(GL_TEXTURE_2D, handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glPixelStorei(GL_PACK_ALIGNMENT, 1);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_R8 as _,
                bmp.width() as _,
                bmp.rows() as _,
                0,
                pf,
                pt,
                buf.as_ptr() as _,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        let g = Glyph {
            tex: Some(tex),
            top: (metrics.horiBearingY >> 6i64) as _,
            left: (metrics.horiBearingX >> 6i64) as _,
            advance: (metrics.horiAdvance >> 6i64) as _,
            width: bmp.width() as _,
            height: bmp.rows() as _,
        };

        font.glyphs.insert(cp, Rc::new(g));
        font.glyphs[cp].clone()
    }
}
