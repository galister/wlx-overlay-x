use std::{ffi::CStr, mem::MaybeUninit, os::fd::RawFd, ptr};

use gles31::{
    glBindBuffer, glBindTexture, glGetError, glGetString, glPixelStorei, glTexImage2D, GL_NO_ERROR,
    GL_PIXEL_UNPACK_BUFFER, GL_RGBA, GL_RGBA8, GL_TEXTURE_2D, GL_UNPACK_ALIGNMENT,
    GL_UNSIGNED_BYTE, GL_VENDOR,
};
use libc::{close, mmap, munmap, MAP_SHARED, PROT_READ};
use log::debug;
use once_cell::sync::Lazy;

use crate::gl::egl::{
    eglCreateImage, eglDestroyImage, eglGetError, glEGLImageTargetTexture2DOES,
    DRM_FORMAT_ABGR8888, DRM_FORMAT_ARGB8888, DRM_FORMAT_XBGR8888, DRM_FORMAT_XRGB8888,
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
    pub modifier: u64,
    pub format: u32,
}

impl Default for FrameFormat {
    fn default() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

impl FrameFormat {
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

#[derive(Debug, Clone, Copy)]
pub struct FramePlane {
    pub fd: RawFd,
    pub offset: u32,
    pub stride: i32,
}

impl Default for FramePlane {
    fn default() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

#[derive(Debug, Clone)]
pub struct DrmFormat {
    pub code: u32,
    pub modifiers: Vec<u64>,
}

pub struct DmabufFrame {
    pub fmt: FrameFormat,
    pub num_planes: usize,
    pub planes: [FramePlane; 4],
    pub status: i32,
}

impl Default for DmabufFrame {
    fn default() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

impl DmabufFrame {
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
            let mut a = i * 5usize;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.planes[i].fd as _);
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

    pub fn is_valid(&self) -> bool {
        for i in 0..self.num_planes {
            if self.planes[i].fd > 0 {
                return true;
            }
        }
        false
    }

    pub fn close(&mut self) {
        for i in 0..self.num_planes {
            if self.planes[i].fd >= 0 {
                unsafe { close(self.planes[i].fd) };
                self.planes[i].fd = -1;
            }
        }
    }
}

impl Drop for DmabufFrame {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Default)]
pub struct MemFdFrame {
    pub fmt: FrameFormat,
    pub plane: FramePlane,
}

pub struct MemPtrFrame {
    pub fmt: FrameFormat,
    pub ptr: usize,
}

const GL_RGB: u32 = 0x1907;
const GL_BGR: u32 = 0x80E0;
const GL_BGRA: u32 = 0x80E1;
const GL_BGRA8_EXT: u32 = 0x93A1;

// Nvidia can't load BGRA data into an RGBA texture -> needs GL_BGRA8_EXT
// AMD doesn't support BGRA internal formats -> needs GL_RGBA8
static BGRA_INTERNAL: Lazy<u32> = Lazy::new(|| {
    let vendor = unsafe { glGetString(GL_VENDOR) };
    let vendor = unsafe { CStr::from_ptr(vendor as _) };
    let vendor = vendor.to_str().unwrap();
    debug!("GL_VENDOR: {}", vendor);
    if vendor.contains("NVIDIA") {
        GL_BGRA8_EXT
    } else {
        GL_RGBA8
    }
});

fn fmt_to_gl(fmt: &FrameFormat) -> (u32, u32) {
    match fmt.format {
        DRM_FORMAT_ARGB8888 | DRM_FORMAT_XRGB8888 => (*BGRA_INTERNAL, GL_BGRA),
        DRM_FORMAT_ABGR8888 | DRM_FORMAT_XBGR8888 => (GL_RGBA8, GL_RGBA),
        _ => panic!("Unknown format 0x{:x}", { fmt.format }),
    }
}

pub fn texture_load_memptr(texture: u32, f: &MemPtrFrame) {
    unsafe {
        let (fmt, pf) = fmt_to_gl(&f.fmt);

        glBindTexture(GL_TEXTURE_2D, texture);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            fmt as _,
            f.fmt.w,
            f.fmt.h,
            0,
            pf,
            GL_UNSIGNED_BYTE,
            f.ptr as _,
        );
        debug_assert_eq!(glGetError(), GL_NO_ERROR);
    }
}

pub fn texture_load_memfd(texture: u32, f: &MemFdFrame) {
    unsafe {
        let fd = f.plane.fd;

        if fd <= 0 {
            return;
        }

        let size = f.fmt.h as usize * f.plane.stride as usize;

        let ptr = mmap(ptr::null_mut(), size, PROT_READ, MAP_SHARED, fd, 0);

        if ptr.is_null() {
            return;
        }

        glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glBindTexture(GL_TEXTURE_2D, texture);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glPixelStorei(GL_UNPACK_ALIGNMENT, 4);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        let (fmt, pf) = fmt_to_gl(&f.fmt);
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

        munmap(ptr, size);
    }
}

pub fn texture_load_dmabuf(texture: u32, frame: &DmabufFrame) {
    let attribs = frame.get_attribs();

    let egl_image = eglCreateImage(EGL_LINUX_DMABUF_EXT, attribs.as_ptr());
    if eglGetError() != EGL_SUCCESS {
        debug!("eglCreateImage failed");
        return;
    }

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
