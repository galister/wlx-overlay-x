use std::{
    mem::MaybeUninit,
    os::fd::{AsRawFd, RawFd},
    ptr,
};

use gles31::{
    glBindBuffer, glBindTexture, glGetError, glPixelStorei, glTexImage2D, GL_NO_ERROR,
    GL_PIXEL_UNPACK_BUFFER, GL_RGBA, GL_RGBA8, GL_TEXTURE_2D, GL_UNPACK_ALIGNMENT,
    GL_UNSIGNED_BYTE,
};
use libc::{close, mmap, munmap, MAP_SHARED, PROT_READ};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm::Format, wl_shm_pool::WlShmPool};

use crate::gl::egl::{
    eglCreateImage, eglDestroyImage, eglGetError, glEGLImageTargetTexture2DOES,
    EGL_LINUX_DMABUF_EXT, EGL_SUCCESS,
};

#[rustfmt::skip]
const EGL_DMABUF_PLANE_ATTRS: [isize; 20] = [
//  FD     Offset Stride ModLo  ModHi
    0x3272,0x3273,0x3274,0x3443,0x3444,
    0x3275,0x3276,0x3277,0x3445,0x3446,
    0x3278,0x3279,0x327A,0x3447,0x3448,
    0x3440,0x3441,0x3442,0x3449,0x344A,
];

pub const FRAME_PENDING: i32 = 0;
pub const FRAME_READY: i32 = 1;
pub const FRAME_FAILED: i32 = -1;

#[derive(Debug, Clone, Copy)]
pub struct FrameFormat {
    pub w: u32,
    pub h: u32,
    pub size: usize,
    pub modifier: u64,
    pub format: u32,
}

impl FrameFormat {
    pub fn new() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }

    pub fn get_mod_hi(&self) -> u32 {
        (self.modifier >> 32) as _
    }
    pub fn get_mod_lo(&self) -> u32 {
        (self.modifier & 0xFFFFFFFF) as _
    }
    pub fn set_mod(&mut self, mod_hi: u32, mod_low: u32) {
        self.modifier = ((mod_hi as u64) << 32) + mod_low as u64;
    }
}

pub struct FramePlane {
    pub fd: RawFd,
    pub offset: u32,
    pub stride: i32,
}

impl FramePlane {
    pub fn new() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrmFormat {
    pub code: u32,
    pub modifier: u64,
}

pub struct DmabufFrame {
    pub fmt: FrameFormat,
    pub num_planes: usize,
    pub planes: [FramePlane; 4],
    pub status: i32,
}

impl DmabufFrame {
    pub fn new() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
    pub fn get_attribs(&self) -> Vec<isize> {
        let mut vec: Vec<isize> = vec![
            0x3057, // WIDTH
            self.fmt.w as _,
            0x3056, // HEIGHT
            self.fmt.h as _,
            0x3271, // LINUX_DRM_FOURCC_EXT,
            self.fmt.format as _,
        ];

        for i in 0..self.num_planes {
            let mut a = (i * 5) as usize;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.planes[i].fd.as_raw_fd() as _);
            a += 1;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.planes[i].offset as _);
            a += 1;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.planes[i].stride as _);
            a += 1;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.fmt.get_mod_lo() as _);
            a += 1;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.fmt.get_mod_hi() as _);
        }
        vec.push(0x3038); // NONE

        vec
    }
}

impl Drop for DmabufFrame {
    fn drop(&mut self) {
        for i in 0..self.num_planes {
            if self.planes[i].fd >= 0 {
                unsafe { close(self.planes[i].fd) };
            }
        }
    }
}

pub struct MemFdFrame {
    pub path: String,
    pub fmt: FrameFormat,
    pub plane: FramePlane,
    pub buffer: Option<WlBuffer>,
    pub pool: Option<WlShmPool>,
    pub status: i32,
    pub format: Format,
}

impl MemFdFrame {
    pub fn new(path: String) -> Self {
        MemFdFrame {
            path,
            fmt: FrameFormat::new(),
            plane: FramePlane::new(),
            buffer: None,
            pool: None,
            status: 0,
            format: Format::R8,
        }
    }
}

impl Drop for MemFdFrame {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.as_ref() {
            buffer.destroy();
        }
        if let Some(pool) = self.buffer.as_ref() {
            pool.destroy();
        }
        if self.plane.fd >= 0 {
            unsafe { close(self.plane.fd) };
        }
    }
}

const GL_RGB: u32 = 0x1907;
const GL_BGR: u32 = 0x80E0;
const GL_BGRA: u32 = 0x80E1;
const GL_BGRA8_EXT: u32 = 0x93A1;

pub fn texture_load_memfd(texture: u32, f: &MemFdFrame) {
    unsafe {
        let fd = f.plane.fd.as_raw_fd();

        if fd <= 0 {
            return;
        }

        let ptr = mmap(ptr::null_mut(), f.fmt.size, PROT_READ, MAP_SHARED, fd, 0);

        if ptr.is_null() {
            return;
        }

        glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glBindTexture(GL_TEXTURE_2D, texture);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glPixelStorei(GL_UNPACK_ALIGNMENT, 4);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        let (fmt, pf) = match f.format {
            Format::Bgra8888 | Format::Bgrx8888 => (GL_BGRA8_EXT, GL_BGRA),
            Format::Bgr888 => (GL_BGRA8_EXT, GL_BGR),
            Format::Rgba8888 | Format::Rgbx8888 => (GL_RGBA8, GL_RGBA),
            Format::Rgb888 => (GL_RGBA8, GL_RGB),
            _ => panic!("Unknown format 0x{:x}", f.format as u32),
        };

        //glTexSubImage2D(GL_TEXTURE_2D, 0, 0, 0, f.fmt.w, f.fmt.h, GL_BGRA, GL_UNSIGNED_BYTE, ptr);

        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            fmt as _,
            f.fmt.w,
            f.fmt.h,
            0,
            pf,
            GL_UNSIGNED_BYTE,
            ptr,
        );
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glBindTexture(GL_TEXTURE_2D, 0);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        munmap(ptr, f.fmt.size);
    }
}

pub fn texture_load_dmabuf(texture: u32, frame: &DmabufFrame) {
    let attribs = frame.get_attribs();

    let egl_image = eglCreateImage(EGL_LINUX_DMABUF_EXT, attribs.as_ptr());
    debug_assert_eq!(eglGetError(), EGL_SUCCESS);

    unsafe {
        glBindTexture(GL_TEXTURE_2D, texture);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);
    }

    glEGLImageTargetTexture2DOES(GL_TEXTURE_2D as _, egl_image);
    debug_assert_eq!(unsafe { glGetError() }, GL_NO_ERROR);

    unsafe {
        glBindTexture(GL_TEXTURE_2D, 0);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);
    }

    eglDestroyImage(egl_image);
    debug_assert_eq!(eglGetError(), EGL_SUCCESS);
}
