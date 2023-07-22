use std::{
    ffi::c_void,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use gles31::{glGenTextures, load_gl_functions};
use libloading::{Library, Symbol};
use stereokit::StereoKitMultiThread;

pub static EGL_CONTEXT: AtomicUsize = AtomicUsize::new(0);
pub static EGL_DISPLAY: AtomicUsize = AtomicUsize::new(0);

pub type EGLenum = i32;
pub type EGLImage = *const u8;
pub type EGLContext = *const u8;
pub type EGLDisplay = *const u8;

pub const EGL_SUCCESS: EGLenum = 0x3000;
pub const EGL_LINUX_DMABUF_EXT: EGLenum = 0x3270;

#[allow(non_upper_case_globals)]
static glEGLImageTargetTexture2DOES_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn glEGLImageTargetTexture2DOES(target: i32, egl_image: EGLImage) -> () {
    let u = glEGLImageTargetTexture2DOES_p.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(i32, EGLImage) -> () = core::mem::transmute(u);
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
pub fn eglCreateImage(target: EGLenum, attrib_list: *const isize) -> EGLImage {
    let u = eglCreateImage_p.load(Ordering::Relaxed);
    let d = EGL_DISPLAY.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    debug_assert_ne!(d, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn(
            EGLDisplay,
            EGLContext,
            EGLenum,
            *const u8,
            *const isize,
        ) -> EGLImage = core::mem::transmute(u);
        _func_p(
            d as _,
            std::ptr::null(),
            target,
            std::ptr::null(),
            attrib_list,
        )
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
        let _func_p: unsafe extern "C" fn(EGLDisplay, EGLImage) -> i32 = core::mem::transmute(u);
        _func_p(d as _, egl_image)
    }
}

#[allow(non_upper_case_globals)]
static eglGetError_p: AtomicUsize = AtomicUsize::new(0);

#[inline]
#[allow(non_snake_case)]
pub fn eglGetError() -> i32 {
    let u = eglGetError_p.load(Ordering::Relaxed);
    debug_assert_ne!(u, 0);
    unsafe {
        let _func_p: unsafe extern "C" fn() -> i32 = core::mem::transmute(u);
        _func_p()
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
        glEGLImageTargetTexture2DOES_p.store(p0 as usize, Ordering::Relaxed);
        debug_assert_ne!(p0, 0 as _);

        let p1 = proc_fn(b"glCopyImageSubData".as_ptr());
        glCopyImageSubData_p.store(p1 as usize, Ordering::Relaxed);
        debug_assert_ne!(p1, 0 as _);

        let egl_context = sk.backend_opengl_egl_get_context();
        EGL_CONTEXT.store(egl_context as _, Ordering::Relaxed);

        let egl_display = sk.backend_opengl_egl_get_display();
        EGL_DISPLAY.store(egl_display as _, Ordering::Relaxed);

        let create_fn: Symbol<
            unsafe extern "C" fn(
                EGLDisplay,
                EGLContext,
                EGLenum,
                *const u8,
                *const isize,
            ) -> EGLImage,
        > = lib
            .get(b"eglCreateImage")
            .expect("Unable to load eglCreateImage");
        eglCreateImage_p.store(create_fn.into_raw().into_raw() as _, Ordering::Relaxed);

        let destroy_fn: Symbol<unsafe extern "C" fn(EGLDisplay, EGLImage) -> i32> = lib
            .get(b"eglDestroyImage")
            .expect("Unable to load eglDestroyImage");
        eglDestroyImage_p.store(destroy_fn.into_raw().into_raw() as _, Ordering::Relaxed);

        let error_fn: Symbol<unsafe extern "C" fn() -> i32> =
            lib.get(b"eglGetError").expect("Unable to load eglGetError");
        eglGetError_p.store(error_fn.into_raw().into_raw() as _, Ordering::Relaxed);

        // TODO: fix this HACK
        let mut vec = MaybeUninit::<[u32; 100]>::zeroed().assume_init();
        glGenTextures(100, vec.as_mut_ptr() as _);
    }
}
