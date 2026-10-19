//! libmpv: a small hand-written binding (client API 2.x and the OpenGL render API), and
//! `VideoArea`, a GtkGLArea that mpv draws into. Being a normal GTK widget, it can sit
//! under the player controls, scroll with the page and go full screen like anything else.

#![allow(non_camel_case_types)]

use gtk::glib;
use gtk::prelude::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_char, c_double, c_int, c_void};
use std::rc::Rc;
use std::sync::OnceLock;

#[repr(C)]
pub struct mpv_handle {
    _p: [u8; 0],
}
#[repr(C)]
pub struct mpv_render_context {
    _p: [u8; 0],
}

#[repr(C)]
struct mpv_event {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct mpv_event_property {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct mpv_event_end_file {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
struct mpv_event_log_message {
    prefix: *const c_char,
    level: *const c_char,
    text: *const c_char,
}

#[repr(C)]
struct mpv_render_param {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct mpv_opengl_init_params {
    get_proc_address: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
struct mpv_opengl_fbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;

const EVENT_SHUTDOWN: c_int = 1;
const EVENT_LOG_MESSAGE: c_int = 2;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const EVENT_PLAYBACK_RESTART: c_int = 21;
const EVENT_PROPERTY_CHANGE: c_int = 22;

const END_EOF: c_int = 0;
const END_ERROR: c_int = 4;

const RENDER_API_TYPE: c_int = 1;
const RENDER_OPENGL_INIT_PARAMS: c_int = 2;
const RENDER_OPENGL_FBO: c_int = 3;
const RENDER_FLIP_Y: c_int = 4;

#[link(name = "mpv")]
unsafe extern "C" {
    fn mpv_create() -> *mut mpv_handle;
    fn mpv_initialize(ctx: *mut mpv_handle) -> c_int;
    fn mpv_terminate_destroy(ctx: *mut mpv_handle);
    fn mpv_set_option_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_command(ctx: *mut mpv_handle, args: *mut *const c_char) -> c_int;
    fn mpv_set_property_string(
        ctx: *mut mpv_handle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_set_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_get_property(
        ctx: *mut mpv_handle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_get_property_string(ctx: *mut mpv_handle, name: *const c_char) -> *mut c_char;
    fn mpv_observe_property(
        ctx: *mut mpv_handle,
        userdata: u64,
        name: *const c_char,
        format: c_int,
    ) -> c_int;
    fn mpv_wait_event(ctx: *mut mpv_handle, timeout: c_double) -> *mut mpv_event;
    fn mpv_set_wakeup_callback(
        ctx: *mut mpv_handle,
        cb: Option<unsafe extern "C" fn(*mut c_void)>,
        d: *mut c_void,
    );
    fn mpv_free(data: *mut c_void);
    fn mpv_request_log_messages(ctx: *mut mpv_handle, min_level: *const c_char) -> c_int;
    fn mpv_render_context_create(
        res: *mut *mut mpv_render_context,
        mpv: *mut mpv_handle,
        params: *mut mpv_render_param,
    ) -> c_int;
    fn mpv_render_context_set_update_callback(
        ctx: *mut mpv_render_context,
        cb: Option<unsafe extern "C" fn(*mut c_void)>,
        d: *mut c_void,
    );
    fn mpv_render_context_render(
        ctx: *mut mpv_render_context,
        params: *mut mpv_render_param,
    ) -> c_int;
    fn mpv_render_context_free(ctx: *mut mpv_render_context);
}

fn c(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap()
}

// ------------------------------------------------------------------ GL loader

struct GlLoader {
    egl: Option<libloading::Library>,
    glx: Option<libloading::Library>,
}

static GL: OnceLock<GlLoader> = OnceLock::new();

fn gl() -> &'static GlLoader {
    GL.get_or_init(|| unsafe {
        GlLoader {
            egl: libloading::Library::new("libEGL.so.1").ok(),
            glx: libloading::Library::new("libGL.so.1").ok(),
        }
    })
}

/// GTK 4 draws with EGL (Wayland and X11); GLX is the fallback for old X11 setups.
unsafe extern "C" fn get_proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let loader = gl();
    unsafe {
        if let Some(egl) = &loader.egl {
            if let Ok(f) = egl
                .get::<unsafe extern "C" fn(*const c_char) -> *mut c_void>(b"eglGetProcAddress\0")
            {
                let p = f(name);
                if !p.is_null() {
                    return p;
                }
            }
        }
        if let Some(glx) = &loader.glx {
            if let Ok(f) = glx.get::<unsafe extern "C" fn(*const c_char) -> *mut c_void>(
                b"glXGetProcAddressARB\0",
            ) {
                return f(name);
            }
        }
    }
    std::ptr::null_mut()
}

/// glGetString for the log ("llvmpipe", "Mesa Intel…"), from the current context.
fn gl_string(name: u32) -> String {
    type GetString = unsafe extern "C" fn(u32) -> *const c_char;
    let f = unsafe { get_proc_address(std::ptr::null_mut(), c"glGetString".as_ptr()) };
    if f.is_null() {
        return "?".into();
    }
    // SAFETY: glGetString from the current GL context; returns a static string or NULL.
    let p = unsafe { std::mem::transmute::<*mut c_void, GetString>(f)(name) };
    if p.is_null() {
        "?".into()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn gl_flush() {
    type Flush = unsafe extern "C" fn();
    static F: OnceLock<usize> = OnceLock::new();
    let f = *F.get_or_init(|| unsafe {
        get_proc_address(std::ptr::null_mut(), c"glFlush".as_ptr()) as usize
    });
    if f != 0 {
        // SAFETY: glFlush of the current GL context.
        unsafe { std::mem::transmute::<usize, Flush>(f)() };
    }
}

fn current_fbo() -> c_int {
    type GetIntegerv = unsafe extern "C" fn(u32, *mut c_int);
    static F: OnceLock<usize> = OnceLock::new();
    let f = *F.get_or_init(|| unsafe {
        get_proc_address(std::ptr::null_mut(), c"glGetIntegerv".as_ptr()) as usize
    });
    if f == 0 {
        return 0;
    }
    let mut fbo: c_int = 0;
    // SAFETY: glGetIntegerv from the current GL context; GL_FRAMEBUFFER_BINDING = 0x8CA6.
    unsafe { std::mem::transmute::<usize, GetIntegerv>(f)(0x8CA6, &mut fbo) };
    fbo
}

// ------------------------------------------------------------------ events

#[derive(Debug, Clone)]
pub enum Event {
    /// A file opened and started (or restarted after a seek).
    Playing,
    Paused(bool),
    /// End of the file (films: mark watched, episodes: play the next one).
    EndReached,
    Error,
}

thread_local! {
    static HANDLER: RefCell<Option<Rc<dyn Fn(Event)>>> = const { RefCell::new(None) };
    static PLAYER: RefCell<Option<std::rc::Weak<Mpv>>> = const { RefCell::new(None) };
    static AREAS: RefCell<Vec<glib::WeakRef<gtk::GLArea>>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn on_wakeup(_d: *mut c_void) {
    // mpv's own thread: hop to the GTK main loop, which drains the queue.
    glib::MainContext::default().invoke(|| {
        let mpv = PLAYER.with(|p| p.borrow().as_ref().and_then(|w| w.upgrade()));
        if let Some(mpv) = mpv {
            mpv.drain_events();
        }
    });
}

pub static STATS: [std::sync::atomic::AtomicU64; 4] =
    [const { std::sync::atomic::AtomicU64::new(0) }; 4];

fn count(i: usize) {
    STATS[i].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// "updates, queued, unmapped, drawn" (debug log in screenshot mode).
pub fn stats() -> String {
    let v: Vec<u64> = STATS
        .iter()
        .map(|s| s.load(std::sync::atomic::Ordering::Relaxed))
        .collect();
    format!(
        "updates {} queued {} unmapped {} drawn {}",
        v[0], v[1], v[2], v[3]
    )
}

unsafe extern "C" fn on_render_update(_d: *mut c_void) {
    count(0);
    glib::MainContext::default().invoke(|| {
        AREAS.with(|a| {
            for area in a.borrow().iter().filter_map(|w| w.upgrade()) {
                if area.is_mapped() {
                    count(1);
                    area.queue_render();
                } else {
                    count(2);
                }
            }
        });
    });
}

// ------------------------------------------------------------------ player

pub struct Mpv {
    handle: *mut mpv_handle,
    render: Cell<*mut mpv_render_context>,
    render_area: RefCell<Option<glib::WeakRef<gtk::GLArea>>>,
}

impl Mpv {
    /// One player for the whole app (one video surface moved between pages, as on Windows).
    pub fn new(on_event: impl Fn(Event) + 'static) -> Option<Rc<Mpv>> {
        // mpv parses numbers with the C locale.
        unsafe { libc_setlocale_numeric_c() };
        let handle = unsafe { mpv_create() };
        if handle.is_null() {
            return None;
        }
        let mpv = Rc::new(Mpv {
            handle,
            render: Cell::new(std::ptr::null_mut()),
            render_area: RefCell::new(None),
        });
        for (k, v) in [
            ("vo", "libmpv"),
            ("idle", "yes"),
            ("keep-open", "no"),
            ("terminal", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("osc", "no"),
            // Loads the YouTube hook (trailers); play_at turns it off per stream for IPTV URLs.
            ("ytdl", "yes"),
            ("audio-client-name", crate::brand::NAME),
            ("title", crate::brand::NAME),
            ("sub-auto", "fuzzy"),
            ("network-timeout", "20"),
        ] {
            mpv.option(k, v);
        }
        if unsafe { mpv_initialize(handle) } < 0 {
            return None;
        }
        unsafe {
            mpv_observe_property(handle, 1, c"pause".as_ptr(), FORMAT_FLAG);
            // mpv's own warnings and errors go to app.log (a stream that fails, a GPU problem).
            let level = c(&std::env::var("MADHYAM_MPV_LOG").unwrap_or_else(|_| "warn".into()));
            mpv_request_log_messages(handle, level.as_ptr());
        }
        HANDLER.with(|h| *h.borrow_mut() = Some(Rc::new(on_event)));
        PLAYER.with(|p| *p.borrow_mut() = Some(Rc::downgrade(&mpv)));
        unsafe { mpv_set_wakeup_callback(handle, Some(on_wakeup), std::ptr::null_mut()) };
        Some(mpv)
    }

    fn option(&self, name: &str, value: &str) {
        let (n, v) = (c(name), c(value));
        unsafe { mpv_set_option_string(self.handle, n.as_ptr(), v.as_ptr()) };
    }

    pub fn set(&self, name: &str, value: &str) {
        let (n, v) = (c(name), c(value));
        unsafe { mpv_set_property_string(self.handle, n.as_ptr(), v.as_ptr()) };
    }

    pub fn set_f64(&self, name: &str, mut value: f64) {
        let n = c(name);
        unsafe {
            mpv_set_property(
                self.handle,
                n.as_ptr(),
                FORMAT_DOUBLE,
                (&mut value as *mut f64).cast(),
            )
        };
    }

    pub fn set_flag(&self, name: &str, value: bool) {
        let n = c(name);
        let mut v: c_int = value as c_int;
        unsafe {
            mpv_set_property(
                self.handle,
                n.as_ptr(),
                FORMAT_FLAG,
                (&mut v as *mut c_int).cast(),
            )
        };
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        let n = c(name);
        let mut v: f64 = 0.0;
        let r = unsafe {
            mpv_get_property(
                self.handle,
                n.as_ptr(),
                FORMAT_DOUBLE,
                (&mut v as *mut f64).cast(),
            )
        };
        (r >= 0).then_some(v)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        let n = c(name);
        let mut v: i64 = 0;
        let r = unsafe {
            mpv_get_property(
                self.handle,
                n.as_ptr(),
                FORMAT_INT64,
                (&mut v as *mut i64).cast(),
            )
        };
        (r >= 0).then_some(v)
    }

    pub fn get_flag(&self, name: &str) -> bool {
        let n = c(name);
        let mut v: c_int = 0;
        let r = unsafe {
            mpv_get_property(
                self.handle,
                n.as_ptr(),
                FORMAT_FLAG,
                (&mut v as *mut c_int).cast(),
            )
        };
        r >= 0 && v != 0
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        let n = c(name);
        let p = unsafe { mpv_get_property_string(self.handle, n.as_ptr()) };
        if p.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { mpv_free(p.cast()) };
        Some(s)
    }

    pub fn command(&self, args: &[&str]) -> bool {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        unsafe { mpv_command(self.handle, ptrs.as_mut_ptr()) >= 0 }
    }

    pub fn load(&self, url: &str) {
        self.command(&["loadfile", url, "replace"]);
    }

    pub fn stop(&self) {
        self.command(&["stop"]);
    }

    pub fn paused(&self) -> bool {
        self.get_flag("pause")
    }

    pub fn set_pause(&self, on: bool) {
        self.set_flag("pause", on);
    }

    /// Position and length in milliseconds (0 when unknown).
    pub fn time_ms(&self) -> i64 {
        self.get_f64("time-pos")
            .map(|t| (t * 1000.0) as i64)
            .unwrap_or(0)
    }

    pub fn length_ms(&self) -> i64 {
        self.get_f64("duration")
            .map(|t| (t * 1000.0) as i64)
            .unwrap_or(0)
    }

    pub fn seek_ms(&self, ms: i64) {
        self.command(&["seek", &format!("{:.3}", ms as f64 / 1000.0), "absolute"]);
    }

    /// Audio, video and subtitle tracks: (type, id, label, selected).
    pub fn tracks(&self, kind: &str) -> Vec<(i64, String, bool)> {
        let Some(json) = self.get_string("track-list") else {
            return Vec::new();
        };
        let Ok(Value::Array(list)) = serde_json::from_str::<Value>(&json) else {
            return Vec::new();
        };
        list.iter()
            .filter(|t| t.get("type").and_then(Value::as_str) == Some(kind))
            .map(|t| {
                let id = t.get("id").and_then(Value::as_i64).unwrap_or(0);
                let title = t.get("title").and_then(Value::as_str).unwrap_or("");
                let lang = t.get("lang").and_then(Value::as_str).unwrap_or("");
                let label = match (title.is_empty(), lang.is_empty()) {
                    (false, false) => format!("{title} [{lang}]"),
                    (false, true) => title.to_string(),
                    (true, false) => lang.to_string(),
                    _ => format!("Track {id}"),
                };
                (
                    id,
                    label,
                    t.get("selected").and_then(Value::as_bool).unwrap_or(false),
                )
            })
            .collect()
    }

    fn drain_events(&self) {
        let handler = HANDLER.with(|h| h.borrow().clone());
        loop {
            let ev = unsafe { &*mpv_wait_event(self.handle, 0.0) };
            let event = match ev.event_id {
                0 => break,
                EVENT_SHUTDOWN => break,
                EVENT_LOG_MESSAGE if !ev.data.is_null() => {
                    let m = unsafe { &*(ev.data as *const mpv_event_log_message) };
                    let text = |p: *const c_char| {
                        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
                    };
                    crate::log(&format!(
                        "mpv {} [{}]: {}",
                        text(m.level),
                        text(m.prefix),
                        text(m.text).trim_end()
                    ));
                    None
                }
                EVENT_FILE_LOADED | EVENT_PLAYBACK_RESTART => Some(Event::Playing),
                EVENT_END_FILE if !ev.data.is_null() => {
                    let end = unsafe { &*(ev.data as *const mpv_event_end_file) };
                    match end.reason {
                        END_EOF => Some(Event::EndReached),
                        END_ERROR => Some(Event::Error),
                        _ => None,
                    }
                }
                EVENT_PROPERTY_CHANGE if !ev.data.is_null() => {
                    let p = unsafe { &*(ev.data as *const mpv_event_property) };
                    let name = unsafe { CStr::from_ptr(p.name) }.to_string_lossy();
                    if name == "pause" && p.format == FORMAT_FLAG && !p.data.is_null() {
                        Some(Event::Paused(unsafe { *(p.data as *const c_int) } != 0))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let (Some(e), Some(h)) = (event, &handler) {
                h(e);
            }
        }
    }

    // ---------------------------------------------------------- rendering

    /// Called when a VideoArea's GL context is ready. One render context at a time: a newly
    /// realised area takes over (the surface moves between the live and the movie page).
    fn attach(&self, area: &gtk::GLArea) {
        self.detach();
        area.make_current();
        if let Some(e) = area.error() {
            crate::log(&format!("video surface has no OpenGL context: {e}"));
            return;
        }
        let mut init = mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        let api = c"opengl";
        let mut params = [
            mpv_render_param {
                kind: RENDER_API_TYPE,
                data: api.as_ptr() as *mut c_void,
            },
            mpv_render_param {
                kind: RENDER_OPENGL_INIT_PARAMS,
                data: (&mut init as *mut mpv_opengl_init_params).cast(),
            },
            mpv_render_param {
                kind: 0,
                data: std::ptr::null_mut(),
            },
        ];
        let mut ctx: *mut mpv_render_context = std::ptr::null_mut();
        let r = unsafe { mpv_render_context_create(&mut ctx, self.handle, params.as_mut_ptr()) };
        if r < 0 || ctx.is_null() {
            crate::log(&format!("mpv render context failed ({r})"));
            return;
        }
        unsafe {
            mpv_render_context_set_update_callback(
                ctx,
                Some(on_render_update),
                std::ptr::null_mut(),
            )
        };
        crate::log(&format!(
            "video surface ready (GLES: {}, {} / {})",
            area.context().is_some_and(|c| c.uses_es()),
            gl_string(0x1F01), // GL_RENDERER
            gl_string(0x1F02), // GL_VERSION
        ));
        self.render.set(ctx);
        *self.render_area.borrow_mut() = Some(area.downgrade());
    }

    fn detach(&self) {
        let ctx = self.render.replace(std::ptr::null_mut());
        if !ctx.is_null() {
            if let Some(area) = self.render_area.borrow().as_ref().and_then(|w| w.upgrade()) {
                area.make_current();
            }
            unsafe { mpv_render_context_free(ctx) };
        }
        *self.render_area.borrow_mut() = None;
    }

    fn draw(&self, area: &gtk::GLArea) -> bool {
        let ctx = self.render.get();
        let owner = self.render_area.borrow().as_ref().and_then(|w| w.upgrade());
        if ctx.is_null() || owner.as_ref() != Some(area) {
            return false;
        }
        let scale = area.scale_factor();
        let mut fbo = mpv_opengl_fbo {
            fbo: current_fbo(),
            w: area.width() * scale,
            h: area.height() * scale,
            internal_format: 0,
        };
        let mut flip: c_int = 1;
        let mut params = [
            mpv_render_param {
                kind: RENDER_OPENGL_FBO,
                data: (&mut fbo as *mut mpv_opengl_fbo).cast(),
            },
            mpv_render_param {
                kind: RENDER_FLIP_Y,
                data: (&mut flip as *mut c_int).cast(),
            },
            mpv_render_param {
                kind: 0,
                data: std::ptr::null_mut(),
            },
        ];
        let r = unsafe { mpv_render_context_render(ctx, params.as_mut_ptr()) };
        count(3);
        // GTK composites the area's texture from its own context: make sure mpv's commands
        // have reached the GPU first, or the frame can show up black (seen with Mesa).
        gl_flush();
        static FIRST: std::sync::Once = std::sync::Once::new();
        FIRST.call_once(|| {
            crate::log(&format!(
                "first video frame: fbo {} {}x{} result {r}",
                fbo.fbo, fbo.w, fbo.h
            ))
        });
        true
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.detach();
        unsafe { mpv_terminate_destroy(self.handle) };
    }
}

unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}

/// LC_NUMERIC = 1 on glibc and musl.
unsafe fn libc_setlocale_numeric_c() {
    unsafe { setlocale(1, c"C".as_ptr()) };
}

/// A GL area mpv draws into. Black until a picture arrives.
pub fn video_area(mpv: &Rc<Mpv>) -> gtk::GLArea {
    let area = gtk::GLArea::new();
    area.set_auto_render(false);
    area.set_has_depth_buffer(false);
    area.set_hexpand(true);
    area.set_vexpand(true);
    AREAS.with(|a| a.borrow_mut().push(area.downgrade()));
    let m = Rc::downgrade(mpv);
    area.connect_realize(move |a| {
        if let Some(mpv) = m.upgrade() {
            mpv.attach(a);
        }
    });
    let m = Rc::downgrade(mpv);
    area.connect_unrealize(move |a| {
        if let Some(mpv) = m.upgrade() {
            if mpv
                .render_area
                .borrow()
                .as_ref()
                .and_then(|w| w.upgrade())
                .as_ref()
                == Some(a)
            {
                mpv.detach();
            }
        }
    });
    let m = Rc::downgrade(mpv);
    area.connect_render(move |a, _| {
        let drawn = m.upgrade().is_some_and(|mpv| mpv.draw(a));
        if !drawn {
            // Not ours (yet): clear to black.
            type ClearColor = unsafe extern "C" fn(f32, f32, f32, f32);
            type Clear = unsafe extern "C" fn(u32);
            unsafe {
                let cc = get_proc_address(std::ptr::null_mut(), c"glClearColor".as_ptr());
                let cl = get_proc_address(std::ptr::null_mut(), c"glClear".as_ptr());
                if !cc.is_null() && !cl.is_null() {
                    std::mem::transmute::<*mut c_void, ClearColor>(cc)(0.0, 0.0, 0.0, 1.0);
                    std::mem::transmute::<*mut c_void, Clear>(cl)(0x4000); // GL_COLOR_BUFFER_BIT
                }
            }
        }
        glib::Propagation::Stop
    });
    area
}
