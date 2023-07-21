use std::{
    ffi::c_void,
    mem::size_of,
    ptr::null,
    sync::atomic::{AtomicUsize, Ordering},
};

use libloading::{Library, Symbol};

use glam::Vec3;
use gles31::{
    glActiveTexture, glAttachShader, glBindBuffer, glBindFramebuffer, glBindTexture,
    glBindVertexArray, glBlendEquationSeparate, glBlendFuncSeparate, glBufferData,
    glCheckFramebufferStatus, glClear, glColorMask, glCompileShader, glCreateProgram,
    glCreateShader, glDeleteBuffers, glDeleteFramebuffers, glDeleteProgram, glDeleteShader,
    glDeleteTextures, glDeleteVertexArrays, glDetachShader, glDrawArrays, glDrawBuffers,
    glDrawElements, glEnableVertexAttribArray, glFramebufferTexture2D, glGenBuffers,
    glGenFramebuffers, glGenTextures, glGenVertexArrays, glGetError, glGetShaderiv,
    glGetUniformLocation, glLinkProgram, glShaderSource, glTexImage2D, glTexParameteri,
    glUniform1ui, glUniform4f, glUseProgram, glVertexAttribPointer, glViewport, load_gl_functions,
    GL_ARRAY_BUFFER, GL_CLAMP_TO_EDGE, GL_COLOR_ATTACHMENT0, GL_COLOR_BUFFER_BIT,
    GL_COMPILE_STATUS, GL_ELEMENT_ARRAY_BUFFER, GL_FALSE, GL_FLOAT, GL_FRAGMENT_SHADER,
    GL_FRAMEBUFFER, GL_FRAMEBUFFER_COMPLETE, GL_FUNC_ADD, GL_LINEAR, GL_NO_ERROR, GL_ONE,
    GL_ONE_MINUS_SRC_ALPHA, GL_RGBA8, GL_SRC_ALPHA, GL_SRGB8_ALPHA8, GL_STATIC_DRAW, GL_TEXTURE_2D,
    GL_TEXTURE_MAG_FILTER, GL_TEXTURE_MIN_FILTER, GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T,
    GL_TRIANGLES, GL_TRIANGLE_STRIP, GL_UNSIGNED_BYTE, GL_UNSIGNED_INT, GL_VERTEX_SHADER,
};
use stereokit::StereoKitMultiThread;

static EGL_CONTEXT: AtomicUsize = AtomicUsize::new(0);
static EGL_DISPLAY: AtomicUsize = AtomicUsize::new(0);

type EGLenum = i32;
type EGLImage = *const c_void;

const EGL_TEXTURE_2D: EGLenum = 0x305F;

#[allow(non_upper_case_globals)]
static glEGLImageTargetTexture2DOES_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn glEGLImageTargetTexture2DOES(target: i32, egl_image: EGLImage) -> () {
    let u = glEGLImageTargetTexture2DOES_p.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(i32, *const c_void) -> () = core::mem::transmute(u);
        _func_p(target, egl_image)
    }
}

#[allow(non_upper_case_globals)]
static glCopyImageSubData_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn glCopyImageSubData(
    src: u32,
    s_target: u32,
    s_level: i32,
    s_x: i32,
    s_y: i32,
    s_z: i32,
    dst: u32,
    d_target: u32,
    d_level: i32,
    d_x: i32,
    d_y: i32,
    d_z: i32,
    s_width: i32,
    s_height: i32,
    s_depth: i32,
) -> () {
    let u = glCopyImageSubData_p.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(
            u32,
            u32,
            i32,
            i32,
            i32,
            i32,
            u32,
            u32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
        ) -> () = core::mem::transmute(u);
        _func_p(
            src, s_target, s_level, s_x, s_y, s_z, dst, d_target, d_level, d_x, d_y, d_z, s_width,
            s_height, s_depth,
        );
    }
}

#[allow(non_upper_case_globals)]
static eglCreateImage_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn eglCreateImage(
    target: EGLenum,
    buffer: *const c_void,
    attrib_list: *const c_void,
) -> *const c_void {
    let u = eglCreateImage_p.load(Ordering::Relaxed);
    let d = EGL_DISPLAY.load(Ordering::Relaxed);
    let c = EGL_CONTEXT.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    debug_assert_ne!(d, 0);
    debug_assert_ne!(c, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(
            *const c_void,
            *const c_void,
            EGLenum,
            *const c_void,
            *const c_void,
        ) -> EGLImage = core::mem::transmute(u);
        _func_p(d as _, c as _, target, buffer, attrib_list)
    }
}

#[allow(non_upper_case_globals)]
static eglDestroyImage_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn eglDestroyImage(egl_image: EGLImage) -> i32 {
    let u = eglDestroyImage_p.load(Ordering::Relaxed);
    let d = EGL_DISPLAY.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    debug_assert_ne!(d, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(*const c_void, *const c_void) -> i32 =
            core::mem::transmute(u);
        _func_p(d as _, egl_image)
    }
}

pub fn gl_init(sk: &stereokit::SkSingle) {
    unsafe {
        let lib = Library::new("libEGL.so.1").expect("Unable to load libEGL.so.1");

        let proc_fn: Symbol<unsafe extern "C" fn(*const u8) -> *const c_void> = lib
            .get(b"eglGetProcAddress")
            .expect("Unable to load eglGetProcAddress");

        let wrap = |name: *const u8| proc_fn(name);

        load_gl_functions(&wrap).expect("Failed to load GL functions.");

        let p0 = proc_fn(b"glEGLImageTargetTexture2DOES".as_ptr());
        glEGLImageTargetTexture2DOES_p.store(p0 as usize, std::sync::atomic::Ordering::Relaxed);
        debug_assert_ne!(p0, 0 as _);

        let p1 = proc_fn(b"glCopyImageSubData".as_ptr());
        glCopyImageSubData_p.store(p1 as usize, std::sync::atomic::Ordering::Relaxed);
        debug_assert_ne!(p1, 0 as _);

        let egl_context = sk.backend_opengl_egl_get_context();
        EGL_CONTEXT.store(egl_context as _, Ordering::Relaxed);

        let egl_display = sk.backend_opengl_egl_get_display();
        EGL_DISPLAY.store(egl_display as _, Ordering::Relaxed);

        let create_fn: Symbol<
            unsafe extern "C" fn(
                *const c_void,
                *const c_void,
                EGLenum,
                *const c_void,
                *const c_void,
            ) -> *const c_void,
        > = lib
            .get(b"eglCreateImage")
            .expect("Unable to load eglCreateImage");
        eglCreateImage_p.store(create_fn.into_raw().into_raw() as _, Ordering::Relaxed);

        let destroy_fn: Symbol<unsafe extern "C" fn(*const c_void, *const c_void) -> i32> = lib
            .get(b"eglDestroyImage")
            .expect("Unable to load eglDestroyImage");
        eglDestroyImage_p.store(destroy_fn.into_raw().into_raw() as _, Ordering::Relaxed);
    }
}

// --- GlTexture ---

pub struct GlTexture {
    pub handle: u32,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub target: u32,
    pub framebuffer: Option<GlFramebuffer>,
}

impl GlTexture {
    pub fn new(width: u32, height: u32) -> GlTexture {
        let mut handle: u32 = 0;

        unsafe {
            glGenTextures(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        let tex = GlTexture {
            handle,
            width,
            height,
            format: GL_SRGB8_ALPHA8 as i32,
            target: GL_TEXTURE_2D,
            framebuffer: None,
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

    pub fn allocate(&mut self, width: u32, height: u32, format: i32) {
        self.width = width;
        self.height = height;
        self.format = format;

        unsafe {
            glTexImage2D(
                self.target,
                0,
                format,
                width,
                height,
                0,
                GL_RGBA8,
                GL_UNSIGNED_BYTE,
                std::ptr::null_mut(),
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn bind(&self) {
        unsafe {
            glActiveTexture(self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glBindTexture(self.target, self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.allocate(width, height, self.format);
    }

    pub fn load_egl_image(&mut self, egl_image: EGLImage, width: u32, height: u32) {
        self.bind();

        unsafe {
            glEGLImageTargetTexture2DOES(GL_TEXTURE_2D as _, egl_image);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
        self.width = width;
        self.height = height;
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

const UNIFORM_NAMES: [&str; 2] = ["uTexture0", "uColor"];

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
                locations: vec![],
            }
        }
    }

    fn load_shader(shader_type: u32, src: &str) -> u32 {
        unsafe {
            let shader = glCreateShader(shader_type);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glShaderSource(shader, 1, src.as_ptr() as _, &(src.len() as i32) as _);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glCompileShader(shader);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            let mut status = 0i32;
            glGetShaderiv(shader, GL_COMPILE_STATUS, &mut status);
            debug_assert_ne!(status, GL_FALSE as _);

            shader
        }
    }

    pub fn use_shader(&self) {
        unsafe {
            glUseProgram(self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    pub fn map_uniform(&mut self, uniform: usize) {
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
    pub texture: u32,
}

impl GlFramebuffer {
    pub fn new(texture_handle: u32) -> GlFramebuffer {
        let mut handle = 0u32;

        unsafe {
            glGenFramebuffers(1, &mut handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }

        GlFramebuffer {
            handle,
            texture: texture_handle,
        }
    }

    pub fn bind(&self) {
        unsafe {
            glBindFramebuffer(GL_FRAMEBUFFER, self.handle);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glFramebufferTexture2D(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                self.texture,
                0,
            );
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawBuffers(1, &GL_COLOR_ATTACHMENT0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            let status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
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

struct GlRenderer {
    vao: GlVertexArray,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    shader_sprite: GlShader,
    shader_glyph: GlShader,
    shader_color: GlShader,
    width: f32,
    height: f32,
}

impl GlRenderer {
    pub fn new() -> GlRenderer {
        let vbo = GlBuffer::new(GL_ARRAY_BUFFER);
        let ebo = GlBuffer::new(GL_ELEMENT_ARRAY_BUFFER);

        let vertices: Vec<f32> = vec![
            0.5, 0.5, 1., 0., 0.5, -0.5, 1., 1., -0.5, -0.5, 0., 1., -0.5, 0.5, 0., 0.,
        ];

        let indices: Vec<u32> = vec![0, 1, 3, 1, 2, 3];

        vbo.data(&vertices);
        ebo.data(&indices);

        let vao = GlVertexArray::new(vbo, ebo);

        vao.vert_attrib_ptr::<f32>(0, 2, GL_FLOAT, 4, 0);
        vao.vert_attrib_ptr::<f32>(1, 2, GL_FLOAT, 4, 2);

        let shader_sprite = GlShader::new(VERT_COMMON, FRAG_SPRITE);
        let shader_glyph = GlShader::new(VERT_COMMON, FRAG_GLYPH);
        let shader_color = GlShader::new(VERT_COMMON, FRAG_COLOR);

        GlRenderer {
            vao,
            vertices,
            indices,
            shader_sprite,
            shader_glyph,
            shader_color,
            width: 0.,
            height: 0.,
        }
    }

    pub fn begin(&mut self, texture: &GlTexture) {
        self.width = texture.width as f32;
        self.height = texture.height as f32;

        if let Some(fb) = &texture.framebuffer {
            fb.bind();
        }

        unsafe {
            glViewport(0, 0, texture.width, texture.height);
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

            glColorMask(1, 1, 1, 1);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }

    fn use_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let x2 = x / self.width;
        let y2 = y / self.height;

        self.vertices[8] = x2;
        self.vertices[12] = x2;
        self.vertices[5] = y2;
        self.vertices[9] = y2;

        let w2 = (x + w) / self.width;
        let h2 = (y + h) / self.height;

        self.vertices[0] = w2;
        self.vertices[4] = w2;
        self.vertices[1] = h2;
        self.vertices[13] = h2;

        self.vao.vbo.data(&self.vertices);
    }

    pub fn draw_sprite(&mut self, texture: &GlTexture, x: f32, y: f32, w: f32, h: f32) {
        self.use_rect(x, y, w, h);

        self.vao.bind();
        self.shader_sprite.use_shader();
        texture.bind();

        let location = self.shader_sprite.locations[UNIFORM_TEX0];
        unsafe {
            glUniform1ui(location, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
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

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
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
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            debug_assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}
