use std::{
    mem::MaybeUninit,
    os::fd::{AsRawFd, OwnedFd},
};

use gles31::{glBindTexture, glGetError, GL_NO_ERROR, GL_TEXTURE_2D};

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

pub const FRAME_PENDING: i32 = 1;
pub const FRAME_READY: i32 = 2;
pub const FRAME_FAILED: i32 = 3;

pub struct DmabufFrame {
    pub format: DmabufFrameFormat,
    pub num_planes: usize,
    pub planes: [DmabufPlane; 4],
    pub status: i32,
}

impl DmabufFrame {
    pub fn get_attribs(&self) -> Vec<isize> {
        let mut vec: Vec<isize> = vec![
            0x3057, // WIDTH
            self.format.width as _,
            0x3056, // HEIGHT
            self.format.height as _,
            0x3271, // LINUX_DRM_FOURCC_EXT,
            self.format.format as _,
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
            vec.push(self.format.get_mod_lo() as _);
            a += 1;
            vec.push(EGL_DMABUF_PLANE_ATTRS[a]);
            vec.push(self.format.get_mod_hi() as _);
        }
        vec.push(0x3038); // NONE

        vec
    }
}

impl Default for DmabufFrame {
    fn default() -> Self {
        unsafe { MaybeUninit::<Self>::zeroed().assume_init() }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DmabufFrameFormat {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
}

impl DmabufFrameFormat {
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

pub struct DmabufPlane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct DrmFormat {
    pub code: u32,
    pub modifier: u64,
}

pub fn texture_load_dmabuf(texture: u32, frame: &DmabufFrame) {
    let attribs = frame.get_attribs();

    let egl_image = eglCreateImage(EGL_LINUX_DMABUF_EXT, attribs.as_ptr());
    debug_assert_eq!(eglGetError(), EGL_SUCCESS);

    println!("{:x}", egl_image as usize);

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
