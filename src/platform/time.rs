use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_long};
use std::time::{SystemTime, UNIX_EPOCH};

#[repr(C)]
#[derive(Clone, Copy)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
    tm_gmtoff: c_long,
    tm_zone: *const c_char,
}

unsafe extern "C" {
    fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    fn strftime(s: *mut c_char, max: usize, format: *const c_char, tm: *const Tm) -> usize;
}

pub fn epoch_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|err| format!("invalid system clock: {err}"))
}

pub fn local_time_strings(epoch: u64) -> (String, String) {
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let time = epoch as i64;

    unsafe {
        if localtime_r(&time, &mut tm).is_null() {
            return (epoch.to_string(), "unknown".to_string());
        }
    }

    (
        format_tm(&tm, "%Y-%m-%d %H:%M:%S"),
        format_tm(&tm, "%Y-%m-%d %H:00"),
    )
}

fn format_tm(tm: &Tm, fmt: &str) -> String {
    let mut fmt_bytes = fmt.as_bytes().to_vec();
    fmt_bytes.push(0);
    let mut out = vec![0_i8; 64];

    unsafe {
        let written = strftime(
            out.as_mut_ptr(),
            out.len(),
            fmt_bytes.as_ptr() as *const c_char,
            tm,
        );
        if written == 0 {
            return "unknown".to_string();
        }
        CStr::from_ptr(out.as_ptr()).to_string_lossy().into_owned()
    }
}
