//! The madhyam-core engine through its C ABI (`iptv_call` / `iptv_import`), the same six
//! functions the Windows (C#) and Android (Kotlin) bindings use. JSON in, JSON out; calls
//! block, so the UI runs them on worker threads.

use madhyam_core_ffi as ffi;
use serde_json::Value;
use std::ffi::{CStr, CString, c_char};
use std::fmt;

/// An engine error: `code` is "api", "parse", "invalid", "not_found", "io", …
#[derive(Debug, Clone)]
pub struct EngineError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EngineError {}

/// Owns one catalog. Thread-safe: the engine serialises calls with its own mutex.
pub struct Engine {
    handle: *mut ffi::MadhyamCore,
}

// SAFETY: the C ABI is documented thread-safe (one internal mutex around all state).
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            handle: ffi::iptv_core_new(),
        }
    }

    pub fn version() -> String {
        // SAFETY: iptv_version returns a static NUL-terminated string.
        unsafe { CStr::from_ptr(ffi::iptv_version()) }
            .to_string_lossy()
            .into_owned()
    }

    fn unwrap(p: *mut c_char) -> Result<Value, EngineError> {
        if p.is_null() {
            return Err(EngineError {
                code: "internal".into(),
                message: "engine returned nothing".into(),
            });
        }
        // SAFETY: p came from the engine and is freed exactly once below.
        let text = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { ffi::iptv_string_free(p) };
        let mut v: Value = serde_json::from_str(&text).map_err(|e| EngineError {
            code: "parse".into(),
            message: e.to_string(),
        })?;
        if let Some(err) = v.get("error") {
            return Err(EngineError {
                code: err
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("error")
                    .to_string(),
                message: err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
        Ok(v.get_mut("ok").map(Value::take).unwrap_or(Value::Null))
    }

    fn cstring(s: &str) -> CString {
        CString::new(s.replace('\0', "")).expect("no NUL")
    }

    pub fn call(&self, method: &str, args: Value) -> Result<Value, EngineError> {
        let m = Self::cstring(method);
        let a = Self::cstring(&args.to_string());
        // SAFETY: live handle, NUL-terminated strings that outlive the call.
        Self::unwrap(unsafe { ffi::iptv_call(self.handle, m.as_ptr(), a.as_ptr()) })
    }

    pub fn import(&self, method: &str, data: &[u8], args: Value) -> Result<Value, EngineError> {
        let m = Self::cstring(method);
        let a = Self::cstring(&args.to_string());
        // SAFETY: as above; data is a valid slice for the duration of the call.
        Self::unwrap(unsafe {
            ffi::iptv_import(
                self.handle,
                m.as_ptr(),
                data.as_ptr(),
                data.len(),
                a.as_ptr(),
            )
        })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: handle came from iptv_core_new and is not used afterwards.
        unsafe { ffi::iptv_core_free(self.handle) };
    }
}
