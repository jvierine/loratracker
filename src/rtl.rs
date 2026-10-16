//! Load the librtlsdr C API only for live capture. Review needs no SDR driver.
use libloading::Library;
use std::{ffi::c_void, sync::OnceLock};
static API: OnceLock<Api> = OnceLock::new();
macro_rules! rtl_api {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)+) => {
        struct Api { _library: Library, $($name: unsafe extern "C" fn($($ty),*) -> $ret,)+ }
        impl Api {
            unsafe fn open(path: &std::ffi::OsStr) -> Result<Self, libloading::Error> {
                let library = Library::new(path)?;
                $(let $name = *library.get::<unsafe extern "C" fn($($ty),*) -> $ret>(concat!(stringify!($name), "\0").as_bytes())?;)+
                Ok(Self { _library: library, $($name,)+ })
            }
        }
        $(pub unsafe fn $name($($arg: $ty),*) -> $ret { (API.get().expect("RTL API loaded before capture").$name)($($arg),*) })+
    }
}
rtl_api! {
    rtlsdr_get_device_count() -> u32;
    rtlsdr_open(dev: *mut *mut c_void, index: u32) -> i32;
    rtlsdr_close(dev: *mut c_void) -> i32;
    rtlsdr_set_sample_rate(dev: *mut c_void, rate: u32) -> i32;
    rtlsdr_set_center_freq(dev: *mut c_void, freq: u32) -> i32;
    rtlsdr_set_freq_correction(dev: *mut c_void, ppm: i32) -> i32;
    rtlsdr_set_tuner_gain_mode(dev: *mut c_void, mode: i32) -> i32;
    rtlsdr_set_tuner_gain(dev: *mut c_void, gain: i32) -> i32;
    rtlsdr_set_agc_mode(dev: *mut c_void, on: i32) -> i32;
    rtlsdr_reset_buffer(dev: *mut c_void) -> i32;
    rtlsdr_read_async(dev: *mut c_void, cb: unsafe extern "C" fn(*mut u8, u32, *mut c_void), ctx: *mut c_void, n: u32, len: u32) -> i32;
    rtlsdr_cancel_async(dev: *mut c_void) -> i32;
}
pub fn load() -> Result<(), String> {
    if API.get().is_some() { return Ok(()); }
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("RTLSDR_LIBRARY") { candidates.push(path); }
    for name in if cfg!(target_os="windows") { vec!["rtlsdr.dll", "librtlsdr.dll"] }
        else if cfg!(target_os="macos") { vec!["librtlsdr.dylib", "/opt/homebrew/lib/librtlsdr.dylib", "/usr/local/lib/librtlsdr.dylib"] }
        else { vec!["librtlsdr.so.0", "librtlsdr.so"] } {
        candidates.push(name.into());
    }
    let mut errors = Vec::new();
    for path in candidates {
        match unsafe { Api::open(&path) } {
            Ok(api) => { let _ = API.set(api); return Ok(()); }
            Err(e) => errors.push(e.to_string()),
        }
    }
    Err(format!("Cannot load librtlsdr. Install the driver or set RTLSDR_LIBRARY to its full path. {}", errors.join("; ")))
}
