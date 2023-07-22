use std::{
    mem::MaybeUninit,
    os::fd::{AsRawFd, OwnedFd},
    ptr,
};

use gles31::{
    glBindBuffer, glBindTexture, glGetError, glTexSubImage2D, GL_NO_ERROR, GL_PIXEL_UNPACK_BUFFER,
    GL_RGB8, GL_TEXTURE_2D, GL_UNSIGNED_BYTE,
};
use libc::{mmap, munmap};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm::Format, wl_shm_pool::WlShmPool};

pub struct MemFdFrame {
    pub buffer: WlBuffer,
    pub pool: WlShmPool,
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub stride: u32,
    pub size: usize,
    pub shm_path: String,
    pub fd: OwnedFd,
    pub status: i32,
}

pub fn texture_load_memfd(texture: u32, f: &MemFdFrame) {
    unsafe {
        let ptr = mmap(ptr::null_mut(), f.size, 0x01, 0x01, f.fd.as_raw_fd(), 0);

        glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glBindTexture(GL_TEXTURE_2D, texture);
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        glTexSubImage2D(
            GL_TEXTURE_2D,
            0,
            0,
            0,
            f.width,
            f.height,
            GL_RGB8,
            GL_UNSIGNED_BYTE,
            ptr,
        );
        debug_assert_eq!(glGetError(), GL_NO_ERROR);

        munmap(ptr, f.size);
    }
}
