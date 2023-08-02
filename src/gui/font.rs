use std::rc::Rc;

use freetype::{Library, Face, face::LoadFlag, bitmap::PixelMode};
use gles31::{glBindTexture, GL_TEXTURE_2D, glTexImage2D, GL_R8, GL_UNSIGNED_BYTE, GL_NO_ERROR, glGetError, GL_UNSIGNED_SHORT, GL_UNSIGNED_INT, GL_PIXEL_UNPACK_BUFFER, glBindBuffer};
use idmap::IdMap;
use rust_fontconfig::{FcFontCache, FcPattern, PatternMatch};
use stereokit::{Tex, SkDraw, StereoKitMultiThread, TextureType};

const PRIMARY_FONT: &str = "LiberationSans";
const GL_RED: u32 = 0x1903;

pub struct FontCache {
    fc: FcFontCache,
    ft: Library,
    collections: IdMap<isize, FontCollection>
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

struct Glyph {
    tex: Option<Tex>,
    top: i32,
    left: i32,
    advance: i32,
}

impl FontCache {
    pub fn new() -> Self {
        let ft = Library::init().expect("Failed to initialize freetype");
        let fc = FcFontCache::build();

        FontCache {
            fc,
            ft,
            collections: IdMap::new(),
        }
    }

    pub fn get_text_size(&mut self, text: &str, size: isize, sk: &SkDraw) -> i32 {
        let mut max = (size as i32) / 3;
        for line in text.lines() {
            let w : i32 = line.chars().map(|c| { 
                self.get_glyph_for_cp(c as usize, size, sk).advance 
            }).sum();

            if w > max {
                max = w;
            }
        }
        max
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
            self.collections.insert(size, FontCollection {
                fonts: Vec::new(),
                cp_map: IdMap::new(),
            });
        }
        let coll = self.collections.get_mut(size).unwrap();

        if let Some(font) = coll.cp_map.get(&cp) {
            return *font;
        }
        
        let maybe_path = self.fc.query(&FcPattern { 
            family: Some(PRIMARY_FONT.to_string()),
            italic: PatternMatch::False,
            oblique: PatternMatch::False,
            monospace: PatternMatch::False,
            condensed: PatternMatch::False,
            bold: PatternMatch::True,
            unicode_range: [cp, cp],
            ..Default::default()
        });
        
        if let Some(path) = maybe_path {
            // Load font
            let face = self.ft.new_face(&path.path, path.font_index as _).expect("Failed to load font face");
            face.set_char_size(size << 6, size << 6, 96, 96).expect("Failed to set font size");

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
                top: 0,
                left: 0,
                advance: size as i32 / 3,
            });
            let mut glyphs = IdMap::new();
            glyphs.insert(0, zero_glyph);

            let font = Font {
                face,
                path: path.path.to_string(),
                size,
                index: path.font_index as _,
                glyphs,
            };
            coll.fonts.push(font);

            return idx;
        } else {
            coll.cp_map.insert(cp, 0);
            return 0;
        }
    }

    fn get_glyph_for_cp(&mut self, cp: usize, size: isize, sk: &SkDraw) -> Rc<Glyph> {
        let key = self.get_font_for_cp(cp, size);
        let font = &mut self.collections[size].fonts[key];

        if let Some(glyph) = font.glyphs.get(&cp) {
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

        let mode = bmp.pixel_mode();
        if mode.is_err() {
            return font.glyphs[0].clone();
        }

        let (pf, pt) = match mode.unwrap() {
            PixelMode::Gray  => { (GL_RED, GL_UNSIGNED_BYTE) },
            PixelMode::Gray2 => { (GL_RED, GL_UNSIGNED_SHORT) },
            PixelMode::Gray4 => { (GL_RED, GL_UNSIGNED_INT) },
            _ => return font.glyphs[0].clone(),
        };

        let buf = bmp.buffer();

        let tex = sk.tex_create(TextureType::IMAGE_NO_MIPS, stereokit::TextureFormat::R8);
        unsafe {
            let handle = sk.tex_get_surface(&tex.as_ref()) as usize as u32;
            glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBindTexture(GL_TEXTURE_2D, handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glTexImage2D(GL_TEXTURE_2D, 0, GL_R8 as _, bmp.width() as _, bmp.rows() as _, 0, pf, pt, buf.as_ptr() as _);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        } 

        let metrics = glyph.metrics();
     
        let g = Glyph { 
            tex: Some(tex),
            top: (bmp.rows() as i32) - (metrics.horiBearingY as i32 >> 6),
            left: metrics.horiBearingX as i32 >> 6,
            advance: metrics.horiAdvance as i32 >> 6,
        };

        font.glyphs.insert(cp, Rc::new(g));
        font.glyphs[cp].clone()
    }
}

