//! Native macOS Fn recognition. No text is stored; only configured shortcuts or
//! an explicitly armed, time-limited shortcut capture can emit events.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Default)]
pub struct Gesture {
    down: bool,
    used: bool,
    consumed: Vec<u16>,
}
impl Gesture {
    // Return (candidate, suppress). Fn alone fires on release; any other key
    // makes this a chord, including keys not bound by Vao.
    pub fn feed(
        &mut self,
        kind: u8,
        key: u16,
        fn_down: bool,
        mods: &str,
        repeat: bool,
    ) -> Option<String> {
        if kind == 0 && key == 63 {
            if fn_down && !self.down {
                self.down = true;
                self.used = !mods.is_empty();
            } else if !fn_down && self.down {
                self.down = false;
                let fire = !self.used;
                self.used = false;
                if fire {
                    return Some("Fn".into());
                }
            }
        } else if self.down {
            if kind == 0 {
                self.used = true;
            }
            if kind == 1 {
                self.used = true;
                if !repeat {
                    return key_name(key).map(|name| format!("{mods}Fn+{name}"));
                }
            }
        }
        None
    }
}
pub(crate) fn key_name(key: u16) -> Option<&'static str> {
    Some(match key {
        0 => "KeyA",
        1 => "KeyS",
        2 => "KeyD",
        3 => "KeyF",
        4 => "KeyH",
        5 => "KeyG",
        6 => "KeyZ",
        7 => "KeyX",
        8 => "KeyC",
        9 => "KeyV",
        11 => "KeyB",
        12 => "KeyQ",
        13 => "KeyW",
        14 => "KeyE",
        15 => "KeyR",
        16 => "KeyY",
        17 => "KeyT",
        18 => "Digit1",
        19 => "Digit2",
        20 => "Digit3",
        21 => "Digit4",
        22 => "Digit6",
        23 => "Digit5",
        25 => "Digit9",
        26 => "Digit7",
        28 => "Digit8",
        29 => "Digit0",
        31 => "KeyO",
        32 => "KeyU",
        34 => "KeyI",
        35 => "KeyP",
        37 => "KeyL",
        38 => "KeyJ",
        40 => "KeyK",
        45 => "KeyN",
        46 => "KeyM",
        24 => "Equal",
        27 => "Minus",
        30 => "BracketRight",
        33 => "BracketLeft",
        39 => "Quote",
        41 => "Semicolon",
        42 => "Backslash",
        43 => "Comma",
        44 => "Slash",
        47 => "Period",
        50 => "Backquote",
        36 => "Enter",
        48 => "Tab",
        49 => "Space",
        51 => "Backspace",
        53 => "Escape",
        65 => "NumpadDecimal",
        67 => "NumpadMultiply",
        69 => "NumpadAdd",
        71 => "NumLock",
        75 => "NumpadDivide",
        76 => "NumpadEnter",
        78 => "NumpadSubtract",
        81 => "NumpadEqual",
        82 => "Numpad0",
        83 => "Numpad1",
        84 => "Numpad2",
        85 => "Numpad3",
        86 => "Numpad4",
        87 => "Numpad5",
        88 => "Numpad6",
        89 => "Numpad7",
        91 => "Numpad8",
        92 => "Numpad9",
        64 => "F17",
        79 => "F18",
        80 => "F19",
        90 => "F20",
        96 => "F5",
        97 => "F6",
        98 => "F7",
        99 => "F3",
        100 => "F8",
        101 => "F9",
        103 => "F11",
        105 => "F13",
        106 => "F16",
        107 => "F14",
        109 => "F10",
        111 => "F12",
        113 => "F15",
        114 => "Insert",
        115 => "Home",
        116 => "PageUp",
        117 => "Delete",
        118 => "F4",
        119 => "End",
        120 => "F2",
        121 => "PageDown",
        122 => "F1",
        123 => "ArrowLeft",
        124 => "ArrowRight",
        125 => "ArrowDown",
        126 => "ArrowUp",
        _ => return None,
    })
}
#[derive(Default)]
struct Configuration {
    bindings: [Option<String>; 3],
    active: bool,
    capture_until: Option<Instant>,
    suspend_until: Option<Instant>,
    gesture: Gesture,
}
#[derive(Default)]
pub struct NativeKeys {
    config: Arc<Mutex<Configuration>>,
    running: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}
impl NativeKeys {
    pub fn configure(&self, bindings: [Option<String>; 3]) {
        let mut c = self.config.lock().unwrap();
        c.bindings = bindings;
        c.gesture = Gesture::default();
    }
    pub fn active(&self, enabled: bool) {
        self.config.lock().unwrap().active = enabled;
    }
    pub fn suspend(&self, enabled: bool) {
        self.config.lock().unwrap().suspend_until = if enabled {
            Some(Instant::now() + Duration::from_secs(60))
        } else {
            None
        };
    }
    pub fn suspended(&self) -> bool {
        self.config
            .lock()
            .unwrap()
            .suspend_until
            .is_some_and(|t| Instant::now() < t)
    }
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
    pub fn capture(&self, enabled: bool) {
        let mut c = self.config.lock().unwrap();
        c.capture_until = if enabled {
            Some(Instant::now() + Duration::from_secs(12))
        } else {
            None
        };
        c.gesture = Gesture::default();
    }
    pub fn capture_global(&self, app: &tauri::AppHandle, value: String) -> bool {
        let mut c = self.config.lock().unwrap();
        if c.capture_until.is_some_and(|t| Instant::now() < t) {
            c.capture_until = None;
            c.suspend_until = Some(Instant::now() + Duration::from_secs(60));
            drop(c);
            let _ = app.emit("shortcut-captured", value);
            true
        } else {
            false
        }
    }
    pub fn ensure(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if self.ready() {
            return Ok(());
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("Fn监听正在准备，请稍后重试 / Fn listener starting".into());
        }
        #[cfg(target_os = "macos")]
        {
            let c = self.config.clone();
            let running = self.running.clone();
            let ready = self.ready.clone();
            let h = app.clone();
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                use core_foundation::runloop::{
                    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop,
                };
                use core_graphics::event::{
                    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
                    CGEventTapPlacement, CGEventType, CallbackResult, EventField,
                };
                let disabled = Arc::new(AtomicBool::new(false));
                let disabled_cb = disabled.clone();
                let tap = CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![
                        CGEventType::FlagsChanged,
                        CGEventType::KeyDown,
                        CGEventType::KeyUp,
                    ],
                    move |_, kind, event| {
                        if matches!(
                            kind,
                            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
                        ) {
                            disabled_cb.store(true, Ordering::SeqCst);
                            return CallbackResult::Keep;
                        }
                        let mut config = c.lock().unwrap();
                        let now = Instant::now();
                        let suspended = config.suspend_until.is_some_and(|t| now < t);
                        // While the settings recorder is armed, Fn belongs to the local
                        // AppKit monitor. Do not consume its chord before AppKit sees it.
                        // Registered ordinary shortcuts still use capture_global below.
                        let capture = config.capture_until.is_some_and(|t| now < t) && !suspended;
                        if config.bindings.iter().all(Option::is_none) && !capture {
                            return CallbackResult::Keep;
                        }
                        let key = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                            as u16;
                        let kind = match kind {
                            CGEventType::FlagsChanged => 0,
                            CGEventType::KeyDown => 1,
                            CGEventType::KeyUp => 2,
                            _ => return CallbackResult::Keep,
                        };
                        if kind == 2 && config.gesture.consumed.contains(&key) {
                            config.gesture.consumed.retain(|v| *v != key);
                            return CallbackResult::Drop;
                        }
                        let flags = event.get_flags();
                        let fn_down = flags.contains(CGEventFlags::CGEventFlagSecondaryFn);
                        let mut mods = String::new();
                        for (flag, name) in [
                            (CGEventFlags::CGEventFlagControl, "control+"),
                            (CGEventFlags::CGEventFlagAlternate, "alt+"),
                            (CGEventFlags::CGEventFlagShift, "shift+"),
                            (CGEventFlags::CGEventFlagCommand, "super+"),
                        ] {
                            if flags.contains(flag) {
                                mods.push_str(name);
                            }
                        }
                        let repeat = event
                            .get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT)
                            != 0;
                        if let Some(chord) = config.gesture.feed(kind, key, fn_down, &mods, repeat)
                        {
                            if capture {
                                config.capture_until = None;
                                config.suspend_until = Some(now + Duration::from_secs(60));
                                let _ = h.emit("shortcut-captured", chord);
                                if kind == 1 {
                                    config.gesture.consumed.push(key);
                                    return CallbackResult::Drop;
                                }
                            } else if !suspended {
                                let index = config
                                    .bindings
                                    .iter()
                                    .position(|v| v.as_ref() == Some(&chord));
                                if let Some(action) = index.filter(|i| *i != 2 || config.active) {
                                    drop(config);
                                    crate::dispatch_shortcut(h.clone(), action);
                                    if kind == 1 {
                                        let mut config = c.lock().unwrap();
                                        config.gesture.consumed.push(key);
                                        return CallbackResult::Drop;
                                    }
                                    return CallbackResult::Keep;
                                }
                            }
                        } else if kind == 1 && repeat && config.gesture.consumed.contains(&key) {
                            return CallbackResult::Drop;
                        }
                        CallbackResult::Keep
                    },
                );
                match tap {
                    Ok(tap) => match tap.mach_port().create_runloop_source(0) {
                        Ok(source) => {
                            CFRunLoop::get_current()
                                .add_source(&source, unsafe { kCFRunLoopCommonModes });
                            tap.enable();
                            ready.store(true, Ordering::SeqCst);
                            let _ = tx.send(true);
                            while running.load(Ordering::SeqCst) {
                                CFRunLoop::run_in_mode(
                                    unsafe { kCFRunLoopDefaultMode },
                                    Duration::from_millis(250),
                                    false,
                                );
                                if disabled.swap(false, Ordering::SeqCst) {
                                    tap.enable();
                                }
                            }
                        }
                        Err(_) => {
                            let _ = tx.send(false);
                        }
                    },
                    Err(_) => {
                        let _ = tx.send(false);
                    }
                }
                ready.store(false, Ordering::SeqCst);
                running.store(false, Ordering::SeqCst);
            });
            if rx.recv_timeout(Duration::from_secs(3)) == Ok(true) {
                Ok(())
            } else {
                Err("Fn需要macOS键盘事件权限。请在系统设置中允许Vao的辅助功能，再重试；若Fn被系统用于切换输入法，请将键盘设置中的Fn操作设为不执行任何操作。 / Allow Accessibility for Vao, then retry.".into())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            self.running.store(false, Ordering::SeqCst);
            Err("Fn shortcuts are currently available on macOS only".into())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alone_only_on_release() {
        let mut g = Gesture::default();
        assert_eq!(g.feed(0, 63, true, "", false), None);
        assert_eq!(g.feed(0, 63, false, "", false), Some("Fn".into()));
        assert_eq!(g.feed(0, 63, false, "", false), None);
    }
    #[test]
    fn chord_does_not_trigger_fn_again() {
        let mut g = Gesture::default();
        g.feed(0, 63, true, "", false);
        assert_eq!(g.feed(1, 49, true, "", false), Some("Fn+Space".into()));
        assert_eq!(g.feed(1, 49, true, "", true), None);
        g.feed(2, 49, true, "", false);
        assert_eq!(g.feed(0, 63, false, "", false), None);
    }
    #[test]
    fn unbound_chords_and_modifier_changes_cancel_bare_fn() {
        let mut g = Gesture::default();
        g.feed(0, 63, true, "", false);
        g.feed(1, 99, true, "", false);
        assert_eq!(g.feed(0, 63, false, "", false), None);
        g.feed(0, 63, true, "", false);
        g.feed(0, 56, true, "shift+", false);
        assert_eq!(g.feed(0, 63, false, "", false), None);
    }
    #[test]
    fn arrow_function_flags_without_fn_key_do_not_fire_bare_fn() {
        let mut g = Gesture::default();
        g.feed(1, 123, true, "", false);
        assert_eq!(g.feed(0, 63, false, "", false), None);
    }
}
