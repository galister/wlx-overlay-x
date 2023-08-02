use std::{mem::size_of, ptr::null};

use glam::Vec3;
use gles31::{
    glActiveTexture, glAttachShader, glBindBuffer, glBindFramebuffer, glBindTexture,
    glBindVertexArray, glBlendEquationSeparate, glBlendFunc, glBlendFuncSeparate, glBufferData,
    glCheckFramebufferStatus, glClear, glColorMask, glCompileShader, glCreateProgram,
    glCreateShader, glDeleteBuffers, glDeleteFramebuffers, glDeleteProgram, glDeleteShader,
    glDeleteTextures, glDeleteVertexArrays, glDetachShader, glDrawBuffers, glDrawElements,
    glEnableVertexAttribArray, glFramebufferTexture2D, glGenBuffers, glGenFramebuffers,
    glGenTextures, glGenVertexArrays, glGetError, glGetShaderInfoLog, glGetShaderiv,
    glGetUniformLocation, glLinkProgram, glShaderSource, glTexImage2D, glTexParameteri,
    glUniform1i, glUniform4f, glUseProgram, glVertexAttribPointer, glViewport, GL_ARRAY_BUFFER,
    GL_CLAMP_TO_EDGE, GL_COLOR_ATTACHMENT0, GL_COLOR_BUFFER_BIT, GL_COMPILE_STATUS,
    GL_DRAW_FRAMEBUFFER, GL_ELEMENT_ARRAY_BUFFER, GL_FALSE, GL_FLOAT, GL_FRAGMENT_SHADER,
    GL_FRAMEBUFFER_COMPLETE, GL_FUNC_ADD, GL_INFO_LOG_LENGTH, GL_LINEAR, GL_NO_ERROR, GL_ONE,
    GL_ONE_MINUS_SRC_ALPHA, GL_PIXEL_PACK_BUFFER, GL_PIXEL_UNPACK_BUFFER, GL_RGBA, GL_SRC_ALPHA,
    GL_SRGB8_ALPHA8, GL_STATIC_DRAW, GL_TEXTURE0, GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER,
    GL_TEXTURE_MIN_FILTER, GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T, GL_TRIANGLES, GL_UNSIGNED_BYTE,
    GL_UNSIGNED_INT, GL_VERTEX_SHADER, GL_ZERO,
};
use stereokit::{SkDraw, StereoKitMultiThread};

pub mod egl;

pub const PANEL_SHADER_BYTES: &[u8] = include_bytes!("shaders/unlit_simula.sks");

// --- GlTexture ---

pub struct GlTexture {
    pub handle: u32,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub target: u32,
}

impl GlTexture {
    pub fn new() -> GlTexture {
        let mut handle: u32 = 0;

        unsafe {
            glGenTextures(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        let tex = GlTexture {
            handle,
            width: 0,
            height: 0,
            format: GL_SRGB8_ALPHA8 as i32,
            target: GL_TEXTURE_2D,
        };

        unsafe {
            glTexParameteri(tex.target, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glTexParameteri(tex.target, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glTexParameteri(tex.target, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glTexParameteri(tex.target, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        tex
    }

    pub fn from_handle(handle: u32, width: u32, height: u32) -> GlTexture {
        GlTexture {
            handle,
            width,
            height,
            format: GL_SRGB8_ALPHA8 as i32,
            target: GL_TEXTURE_2D,
        }
    }

    pub fn allocate_empty(&mut self, width: u32, height: u32, format: i32) {
        self.allocate(width, height, format, std::ptr::null());
    }

    pub fn allocate(&mut self, width: u32, height: u32, format: i32, data: *const u8) {
        self.width = width;
        self.height = height;
        self.format = format;

        unsafe {
            glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBindBuffer(GL_PIXEL_PACK_BUFFER, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glTexImage2D(
                self.target,
                0,
                format,
                width,
                height,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                data as _,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn bind(&self, slot: u32) {
        unsafe {
            glActiveTexture(GL_TEXTURE0 + slot);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBindTexture(self.target, self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.allocate_empty(width, height, self.format);
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        unsafe {
            glDeleteTextures(1, &self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

// --- GlShader ---

const UNIFORM_TEX0: usize = 0;
const UNIFORM_COL0: usize = 1;

const UNIFORM_NAMES: [&str; 2] = ["uTexture0\0", "uColor\0"];

pub struct GlShader {
    pub handle: u32,
    pub locations: Vec<i32>,
}

impl GlShader {
    pub fn new(vert_src: &str, frag_src: &str) -> GlShader {
        let vert = Self::load_shader(GL_VERTEX_SHADER, vert_src);
        let frag = Self::load_shader(GL_FRAGMENT_SHADER, frag_src);

        unsafe {
            let program = glCreateProgram();
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glAttachShader(program, vert);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glAttachShader(program, frag);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glLinkProgram(program);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDetachShader(program, vert);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glDetachShader(program, frag);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDeleteShader(vert);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glDeleteShader(frag);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            GlShader {
                handle: program,
                locations: vec![-1, -1],
            }
        }
    }

    fn load_shader(shader_type: u32, src: &str) -> u32 {
        unsafe {
            let shader = glCreateShader(shader_type);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glShaderSource(
                shader,
                1,
                &src.as_ptr() as *const *const u8 as *const *const _,
                &(src.len() as i32) as *const _,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glCompileShader(shader);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            let mut status = 0i32;
            glGetShaderiv(shader, GL_COMPILE_STATUS, &mut status);
            if status == GL_FALSE as _ {
                let mut max_len = 0;
                glGetShaderiv(shader, GL_INFO_LOG_LENGTH, &mut max_len as *mut _);

                let mut error = Vec::with_capacity(max_len as usize);
                let mut len = 0u32;
                glGetShaderInfoLog(shader, max_len as _, &mut len, error.as_mut_ptr() as *mut _);
                error.set_len(len as usize);

                panic!(
                    "[GL] {}",
                    std::str::from_utf8(&error).unwrap_or("<Error Message no utf8>")
                );
            }
            shader
        }
    }

    pub fn use_shader(&self) {
        unsafe {
            glUseProgram(self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn has_uniform(&mut self, uniform: usize) {
        unsafe {
            let name = UNIFORM_NAMES[uniform];
            let location = glGetUniformLocation(self.handle, name.as_ptr());
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            debug_assert_ne!(location, -1);
            self.locations[uniform] = location;
        }
    }
}

impl Drop for GlShader {
    fn drop(&mut self) {
        unsafe { glDeleteProgram(self.handle) };
    }
}

// --- GlFramebuffer ---

pub struct GlFramebuffer {
    pub handle: u32,
}

impl GlFramebuffer {
    pub fn new() -> GlFramebuffer {
        let mut handle = 0u32;

        unsafe {
            glGenFramebuffers(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        GlFramebuffer { handle }
    }

    pub fn bind(&self, texture: u32) {
        unsafe {
            glBindFramebuffer(GL_DRAW_FRAMEBUFFER, self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glFramebufferTexture2D(
                GL_DRAW_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                texture,
                0,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawBuffers(1, &GL_COLOR_ATTACHMENT0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            let status = glCheckFramebufferStatus(GL_DRAW_FRAMEBUFFER);
            debug_assert_eq!(status, GL_FRAMEBUFFER_COMPLETE);
        }
    }
}

impl Drop for GlFramebuffer {
    fn drop(&mut self) {
        unsafe {
            glDeleteFramebuffers(1, &self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

// --- GlBuffer ---

pub struct GlBuffer {
    pub handle: u32,
    pub buffer_type: u32,
}

impl GlBuffer {
    pub fn new(buffer_type: u32) -> GlBuffer {
        let mut handle = 0u32;
        unsafe {
            glGenBuffers(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        GlBuffer {
            handle,
            buffer_type,
        }
    }

    pub fn data<T>(&self, data: &Vec<T>) {
        self.bind();
        unsafe {
            let size = data.len() * size_of::<T>();
            glBufferData(
                self.buffer_type,
                size as _,
                data.as_ptr() as _,
                GL_STATIC_DRAW,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn bind(&self) {
        unsafe {
            glBindBuffer(self.buffer_type, self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn unbind(&self) {
        unsafe {
            glBindBuffer(self.buffer_type, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

impl Drop for GlBuffer {
    fn drop(&mut self) {
        unsafe {
            glDeleteBuffers(1, &self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

// --- GlVertexArray ---

pub struct GlVertexArray {
    pub handle: u32,
    pub vbo: GlBuffer,
    pub ebo: GlBuffer,
}

impl GlVertexArray {
    pub fn new(vbo: GlBuffer, ebo: GlBuffer) -> GlVertexArray {
        let mut handle = 0u32;
        unsafe {
            glGenVertexArrays(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        GlVertexArray { handle, vbo, ebo }
    }

    pub fn bind(&self) {
        unsafe {
            glBindVertexArray(self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
        self.vbo.bind();
        self.ebo.bind();
    }

    pub fn unbind(&self) {
        unsafe {
            glBindVertexArray(0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
        self.vbo.unbind();
        self.ebo.unbind();
    }

    pub fn vert_attrib_ptr<T>(
        &self,
        index: u32,
        count: i32,
        attrib_type: u32,
        vert_size: u32,
        offset: i32,
    ) {
        let t_size = size_of::<T>();

        self.bind();

        unsafe {
            glVertexAttribPointer(
                index,
                count,
                attrib_type,
                GL_FALSE,
                vert_size * t_size as u32,
                (offset * t_size as i32) as *const _,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glEnableVertexAttribArray(index);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

impl Drop for GlVertexArray {
    fn drop(&mut self) {
        unsafe {
            glDeleteVertexArrays(1, &self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}

// --- GlRenderer ---

const VERT_COMMON: &str = include_str!("shaders/common.vert");
const FRAG_COLOR: &str = include_str!("shaders/color.frag");
const FRAG_SPRITE: &str = include_str!("shaders/sprite.frag");
const FRAG_GLYPH: &str = include_str!("shaders/glyph.frag");
const FRAG_SRGB: &str = include_str!("shaders/srgb.frag");

pub struct GlRenderer {
    vao: GlVertexArray,
    framebuffer: GlFramebuffer,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    shader_sprite: GlShader,
    shader_glyph: GlShader,
    shader_color: GlShader,
    shader_srgb: GlShader,
    width: u32,
    height: u32,
}

impl GlRenderer {
    pub fn new() -> GlRenderer {
        let vbo = GlBuffer::new(GL_ARRAY_BUFFER);
        let ebo = GlBuffer::new(GL_ELEMENT_ARRAY_BUFFER);

        #[rustfmt::skip]
        let vertices: Vec<f32> = vec![
        //  X   Y    U   V
            0., 0.,  0., 0.,
            0., 1.,  0., 1.,
            1., 0.,  1., 0.,
            1., 1.,  1., 1.,
       ];

        let indices: Vec<u32> = vec![2, 1, 0, 1, 2, 3];

        vbo.data(&vertices);
        ebo.data(&indices);

        let vao = GlVertexArray::new(vbo, ebo);

        vao.vert_attrib_ptr::<f32>(0, 2, GL_FLOAT, 4, 0);
        vao.vert_attrib_ptr::<f32>(1, 2, GL_FLOAT, 4, 2);

        let mut shader_sprite = GlShader::new(VERT_COMMON, FRAG_SPRITE);
        shader_sprite.has_uniform(UNIFORM_TEX0);

        let mut shader_glyph = GlShader::new(VERT_COMMON, FRAG_GLYPH);
        shader_glyph.has_uniform(UNIFORM_TEX0);
        shader_glyph.has_uniform(UNIFORM_COL0);

        let mut shader_color = GlShader::new(VERT_COMMON, FRAG_COLOR);
        shader_color.has_uniform(UNIFORM_COL0);

        let mut shader_srgb = GlShader::new(VERT_COMMON, FRAG_SRGB);
        shader_srgb.has_uniform(UNIFORM_TEX0);

        GlRenderer {
            vao,
            framebuffer: GlFramebuffer::new(),
            vertices,
            indices,
            shader_sprite,
            shader_glyph,
            shader_color,
            shader_srgb,
            width: 0,
            height: 0,
        }
    }

    pub fn begin_sk(&mut self, sk: &SkDraw, tex: &stereokit::Tex) {
        self.width = sk.tex_get_width(tex) as _;
        self.height = sk.tex_get_height(tex) as _;

        let texture = unsafe { sk.tex_get_surface(&tex) as usize as u32 };
        self.framebuffer.bind(texture);
        self.begin();
    }

    pub fn begin_gl(&mut self, texture: GlTexture) {
        self.width = texture.width;
        self.height = texture.height;

        self.framebuffer.bind(texture.handle);
        self.begin();
    }

    fn begin(&mut self) {
        unsafe {
            glViewport(0, 0, self.width as _, self.height as _);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBlendFuncSeparate(
                GL_SRC_ALPHA,
                GL_ONE_MINUS_SRC_ALPHA,
                GL_ONE,
                GL_ONE_MINUS_SRC_ALPHA,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBlendEquationSeparate(GL_FUNC_ADD, GL_FUNC_ADD);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glColorMask(1, 1, 1, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    fn use_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rw = self.width as f32;
        let rh = self.height as f32;

        let x0 = x / rw;
        let y0 = y / rh;

        let x1 = w / rw + x0;
        let y1 = h / rh + y0;

        self.vertices[0] = x0;
        self.vertices[4] = x0;

        self.vertices[8] = x1;
        self.vertices[12] = x1;

        self.vertices[1] = y0;
        self.vertices[9] = y0;

        self.vertices[5] = y1;
        self.vertices[13] = y1;

        self.vao.vbo.data(&self.vertices);
    }

    pub fn srgb_correction(&mut self, texture: u32) {
        self.use_rect(0., 0., self.width as _, self.height as _);
        self.vao.bind();

        self.shader_srgb.use_shader();

        let location = self.shader_srgb.locations[UNIFORM_TEX0];
        debug_assert_ne!(location, -1);

        unsafe {
            glBindTexture(GL_TEXTURE_2D, texture);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
            glBlendFunc(GL_ONE, GL_ZERO);
            glUniform1i(location, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawElements(
                GL_TRIANGLES,
                self.indices.len() as _,
                GL_UNSIGNED_INT,
                null(),
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn draw_sprite(&mut self, texture: &GlTexture, x: f32, y: f32, w: f32, h: f32) {
        self.use_rect(x, y, w, h);
        self.vao.bind();

        self.shader_sprite.use_shader();
        texture.bind(0);

        let location = self.shader_sprite.locations[UNIFORM_TEX0];
        debug_assert_ne!(location, -1);
        unsafe {
            glBlendFunc(GL_ONE, GL_ZERO);
            glUniform1i(location, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawElements(
                GL_TRIANGLES,
                self.indices.len() as _,
                GL_UNSIGNED_INT,
                null(),
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn draw_color(&mut self, color: Vec3, x: f32, y: f32, w: f32, h: f32) {
        self.use_rect(x, y, w, h);

        self.vao.bind();
        self.shader_color.use_shader();
        let location = self.shader_color.locations[UNIFORM_COL0];
        unsafe {
            glUniform4f(location, color.x, color.y, color.z, 1.);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawElements(
                GL_TRIANGLES,
                self.indices.len() as _,
                GL_UNSIGNED_INT,
                null(),
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn clear(&self) {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn end(&mut self) {
        self.vao.unbind();
        unsafe {
            glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
            self.vao.unbind();
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}
