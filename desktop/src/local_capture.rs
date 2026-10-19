//! Shortcut entry inside Vao's focused window. No global input permission required.
// Windows uses the focused WebView keydown handler.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Emitter;
#[derive(Default)]
struct Capture {
    until: Option<Instant>,
    gesture: Recorder,
}
#[derive(Default)]
struct Recorder {
    gesture: crate::native_keys::Gesture,
    fn_down: bool,
}
impl Recorder {
    fn feed(&mut self, kind: u8, key: u16, flags: u64, repeat: bool) -> (Option<String>, String) {
        let mut mods = String::new();
        for (flag, name) in [
            (1 << 18, "Control+"),
            (1 << 19, "Alt+"),
            (1 << 17, "Shift+"),
            (1 << 20, "Super+"),
        ] {
            if flags & flag != 0 {
                mods.push_str(name);
            }
        }
        if kind == 0 && key == 63 {
            self.fn_down = flags & (1 << 23) != 0;
        }
        // Arrow/function keys also carry the secondary-Fn flag on macOS.
        // Only an actual Fn key transition makes a chord an Fn shortcut.
        let fn_value = self.gesture.feed(kind, key, self.fn_down, &mods, repeat);
        if self.fn_down {
            mods.push_str("Fn+");
        }
        let value = fn_value.or_else(|| {
            (kind == 1 && !repeat && !self.fn_down)
                .then(|| crate::native_keys::key_name(key).map(|k| format!("{mods}{k}")))
                .flatten()
        });
        (value, mods.trim_end_matches('+').to_owned())
    }
}
static CAPTURE: OnceLock<Mutex<Capture>> = OnceLock::new();
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn vao_install_local_capture(cb: extern "C" fn(u8, u16, u64, i32) -> i32);
    fn vao_set_local_capture(enabled: i32);
    fn vao_probe_local_event(kind: u8, key: u16, flags: u64);
}
extern "C" fn receive(kind: u8, key: u16, flags: u64, repeat: i32) -> i32 {
    let mut state = CAPTURE
        .get_or_init(|| Mutex::new(Capture::default()))
        .lock()
        .unwrap();
    if !state.until.is_some_and(|t| Instant::now() < t) {
        return 0;
    }
    let (value, preview) = state.gesture.feed(kind, key, flags, repeat != 0);
    if let Some(value) = value {
        state.until = None;
        drop(state);
        if let Some(app) = APP.get() {
            let _ = app.emit("shortcut-captured", value);
        }
        return 1;
    }
    drop(state);
    if let Some(app) = APP.get() {
        let _ = app.emit("shortcut-capture-progress", preview);
    }
    // Own all keys while armed, before AppKit menus/WebView key equivalents.
    1
}
pub fn install(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(target_os = "macos")]
    unsafe {
        vao_install_local_capture(receive)
    };
}
pub fn arm(enabled: bool) {
    let mut state = CAPTURE
        .get_or_init(|| Mutex::new(Capture::default()))
        .lock()
        .unwrap();
    state.until = enabled.then(|| Instant::now() + Duration::from_secs(12));
    state.gesture = Recorder::default();
    drop(state);
    #[cfg(target_os = "macos")]
    unsafe {
        vao_set_local_capture(i32::from(enabled))
    };
}

pub fn probe(kind: u8, key: u16, flags: u64) {
    #[cfg(target_os = "macos")]
    unsafe {
        vao_probe_local_event(kind, key, flags)
    };
    #[cfg(not(target_os = "macos"))]
    let _ = (kind, key, flags);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_combinations_show_progress_then_capture_without_fn() {
        let mut r = Recorder::default();
        assert_eq!(r.feed(0, 55, 1 << 20, false), (None, "Super".into()));
        assert_eq!(r.feed(1, 8, 1 << 20, false).0, Some("Super+KeyC".into()));
        assert_eq!(r.feed(1, 8, 1 << 20, true).0, None);
        assert_eq!(
            r.feed(1, 123, (1 << 20) | (1 << 23), false).0,
            Some("Super+ArrowLeft".into())
        );
        assert_eq!(
            r.feed(1, 44, (1 << 20) | (1 << 17), false).0,
            Some("Shift+Super+Slash".into())
        );
        assert_eq!(r.feed(0, 55, 0, false).0, None);
    }
    #[test]
    fn fn_alone_and_modified_fn_chords_still_work() {
        let mut r = Recorder::default();
        assert_eq!(r.feed(0, 63, 1 << 23, false).1, "Fn");
        assert_eq!(r.feed(0, 63, 0, false).0, Some("Fn".into()));
        r.feed(0, 63, 1 << 23, false);
        assert_eq!(
            r.feed(1, 49, (1 << 23) | (1 << 20), false).0,
            Some("Super+Fn+Space".into())
        );
        assert_eq!(r.feed(0, 63, 0, false).0, None);
    }
}
