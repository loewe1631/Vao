use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub record: String,
    pub translate: String,
    pub cancel: String,
}
impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            record: "Control+Alt+Space".into(),
            translate: "Control+Alt+KeyT".into(),
            cancel: "Escape".into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Global(HotKey),
    Fn(String),
}
impl Binding {
    pub fn global(&self) -> Option<HotKey> {
        if let Self::Global(k) = self {
            Some(*k)
        } else {
            None
        }
    }
    pub fn native(&self) -> Option<String> {
        if let Self::Fn(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }
}
impl std::fmt::Display for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global(k) => write!(f, "{k}"),
            Self::Fn(s) => write!(f, "{s}"),
        }
    }
}
impl Shortcuts {
    pub fn bindings(&self) -> Result<[Binding; 3], String> {
        let parse = |input: &str, cancel: bool| -> Result<Binding, String> {
            if input.len() > 100 {
                return Err("快捷键过长 / Shortcut too long".into());
            }
            let input = input.trim();
            let parts = input.split('+').map(str::trim).collect::<Vec<_>>();
            let has_fn = parts
                .iter()
                .any(|s| s.eq_ignore_ascii_case("fn") || *s == "🌐");
            if has_fn {
                if !cfg!(target_os = "macos") {
                    return Err("Fn快捷键目前仅支持macOS / Fn requires macOS".into());
                }
                if parts.len() == 1 {
                    return Ok(Binding::Fn("Fn".into()));
                }
                if parts
                    .iter()
                    .filter(|s| s.eq_ignore_ascii_case("fn") || **s == "🌐")
                    .count()
                    != 1
                {
                    return Err("Fn重复 / Duplicate Fn".into());
                }
                let rest = parts
                    .iter()
                    .filter(|s| !s.eq_ignore_ascii_case("fn") && **s != "🌐")
                    .copied()
                    .collect::<Vec<_>>()
                    .join("+");
                let k = rest
                    .parse::<HotKey>()
                    .map_err(|_| "Fn组合格式无效 / Invalid Fn chord")?;
                let name = k.key.to_string();
                if !(name.starts_with("Key")
                    || name.starts_with("Digit")
                    || matches!(
                        k.key,
                        Code::Space
                            | Code::Tab
                            | Code::Enter
                            | Code::Backspace
                            | Code::Escape
                            | Code::ArrowUp
                            | Code::ArrowDown
                            | Code::ArrowLeft
                            | Code::ArrowRight
                    ))
                {
                    return Err(
                        "Fn支持字母、数字、空格、回车、方向键等 / Unsupported Fn key".into(),
                    );
                }
                let mut canonical = String::new();
                for (m, n) in [
                    (Modifiers::CONTROL, "control+"),
                    (Modifiers::ALT, "alt+"),
                    (Modifiers::SHIFT, "shift+"),
                    (Modifiers::SUPER, "super+"),
                ] {
                    if k.mods.contains(m) {
                        canonical.push_str(n);
                    }
                }
                return Ok(Binding::Fn(format!("{canonical}Fn+{name}")));
            }
            let k = input
                .parse::<HotKey>()
                .map_err(|_| "快捷键格式无效 / Invalid shortcut")?;
            let function = k
                .key
                .to_string()
                .strip_prefix('F')
                .and_then(|s| s.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            if !(cancel && k.key == Code::Escape && k.mods.is_empty())
                && !function
                && !k
                    .mods
                    .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
            {
                return Err(
                    "普通键需搭配修饰键；Fn可单独使用 / Add a modifier, or use Fn alone".into(),
                );
            }
            Ok(Binding::Global(k))
        };
        let keys = [
            parse(&self.record, false)?,
            parse(&self.translate, false)?,
            parse(&self.cancel, true)?,
        ];
        if keys[0] == keys[1] || keys[0] == keys[2] || keys[1] == keys[2] {
            return Err("三个快捷键不能相同 / Shortcuts must be different".into());
        }
        Ok(keys)
    }
    #[cfg(test)]
    pub fn parse(&self) -> Result<[HotKey; 3], String> {
        let k = self.bindings()?;
        Ok([
            k[0].global().ok_or("Fn")?,
            k[1].global().ok_or("Fn")?,
            k[2].global().ok_or("Fn")?,
        ])
    }
}

pub fn ready(keys: &[Binding; 3], registered: &[HotKey], native_ready: bool) -> bool {
    keys[..2]
        .iter()
        .all(|k| k.global().is_none_or(|g| registered.contains(&g)))
        && (!keys.iter().any(|k| k.native().is_some()) || native_ready)
}
#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn pending_fn_cannot_be_reported_active_after_another_failure() {
        let keys = Shortcuts {
            record: "Fn".into(),
            translate: "Fn+Space".into(),
            cancel: "Escape".into(),
        }
        .bindings()
        .unwrap();
        assert!(!ready(&keys, &[], false));
        assert!(ready(&keys, &[], true));
        let ordinary = Shortcuts::default().bindings().unwrap();
        assert!(!ready(&ordinary, &[], true));
    }
}
pub trait Registration {
    fn register(&self, k: HotKey) -> Result<(), String>;
    fn unregister(&self, k: HotKey) -> Result<(), String>;
}
impl Registration for GlobalHotKeyManager {
    fn register(&self, k: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::register(self, k).map_err(|e| e.to_string())
    }
    fn unregister(&self, k: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::unregister(self, k).map_err(|e| e.to_string())
    }
}
/// Acquire additions before releasing old keys. Track every successful OS operation.
/// On registration or persistence failure restore the old set; report incomplete rollback.
pub fn replace<R: Registration>(
    os: &R,
    registered: &mut Vec<HotKey>,
    desired: &[HotKey],
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let before = registered.clone();
    let result = (|| {
        for k in desired {
            if !registered.contains(k) {
                os.register(*k).map_err(|e| {
                    format!("快捷键被占用或系统拒绝 / Shortcut unavailable: {k}: {e}")
                })?;
                registered.push(*k);
            }
        }
        for k in &before {
            if !desired.contains(k) {
                os.unregister(*k)?;
                registered.retain(|v| v != k);
            }
        }
        persist()
    })();
    if let Err(e) = result {
        let mut errors = vec![];
        for k in registered.clone() {
            if !before.contains(&k) {
                match os.unregister(k) {
                    Ok(()) => registered.retain(|v| *v != k),
                    Err(e) => errors.push(e),
                }
            }
        }
        for k in before {
            if !registered.contains(&k) {
                match os.register(k) {
                    Ok(()) => registered.push(k),
                    Err(e) => errors.push(e),
                }
            }
        }
        return Err(if errors.is_empty() {
            e
        } else {
            format!(
                "{e}; 原快捷键恢复不完整，请重新设置或重启 / Rollback incomplete: {}",
                errors.join("; ")
            )
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct Fake {
        held: RefCell<Vec<HotKey>>,
        blocked: RefCell<Vec<HotKey>>,
    }
    impl Registration for Fake {
        fn register(&self, k: HotKey) -> Result<(), String> {
            if self.held.borrow().contains(&k) || self.blocked.borrow().contains(&k) {
                Err("occupied".into())
            } else {
                self.held.borrow_mut().push(k);
                Ok(())
            }
        }
        fn unregister(&self, k: HotKey) -> Result<(), String> {
            self.held.borrow_mut().retain(|v| *v != k);
            Ok(())
        }
    }
    #[test]
    fn validation_and_old_settings() {
        assert!(Shortcuts::default().parse().is_ok());
        let mut s = Shortcuts {
            record: "KeyA".into(),
            ..Default::default()
        };
        assert!(s.parse().is_err());
        s.record = "Shift+KeyA".into();
        assert!(s.parse().is_err());
        s.record = s.translate.clone();
        assert!(s.parse().is_err());
        s.record = "F8".into();
        assert!(s.parse().is_ok());
        s.translate = "Command+Shift+T".into();
        assert_eq!(s.parse().unwrap()[1], "Super+Shift+KeyT".parse().unwrap());
        assert_eq!(
            serde_json::from_str::<Shortcuts>("{}").unwrap(),
            Shortcuts::default()
        );
    }
    #[test]
    fn conflict_keeps_previous_and_rolls_back_partial_additions() {
        let os = Fake::default();
        let old = Shortcuts::default().parse().unwrap();
        let mut actual = vec![];
        replace(&os, &mut actual, &old[..2], || Ok(())).unwrap();
        let next = [
            "Control+Shift+KeyR".parse().unwrap(),
            "Control+Shift+KeyT".parse().unwrap(),
        ];
        os.blocked.borrow_mut().push(next[1]);
        let mut persisted = false;
        assert!(replace(&os, &mut actual, &next, || {
            persisted = true;
            Ok(())
        })
        .is_err());
        assert!(!persisted);
        assert_eq!(actual, old[..2]);
        assert_eq!(*os.held.borrow(), old[..2]);
    }
    #[test]
    fn swap_and_persistence_rollback() {
        let os = Fake::default();
        let old = Shortcuts::default().parse().unwrap();
        let mut actual = vec![];
        replace(&os, &mut actual, &old[..2], || Ok(())).unwrap();
        replace(&os, &mut actual, &[old[1], old[0]], || Ok(())).unwrap();
        let next = ["Control+Shift+KeyR".parse().unwrap(), old[1]];
        assert!(replace(&os, &mut actual, &next, || Err("disk unavailable".into())).is_err());
        assert_eq!(actual.len(), 2);
        assert!(actual.contains(&old[0]) && actual.contains(&old[1]));
    }
    #[test]
    fn startup_conflict_can_recover() {
        let os = Fake::default();
        let old = Shortcuts::default().parse().unwrap();
        os.blocked.borrow_mut().push(old[1]);
        let mut actual = vec![];
        assert!(replace(&os, &mut actual, &old[..2], || Ok(())).is_err());
        assert!(actual.is_empty());
        let next = [old[0], "Control+Shift+KeyT".parse().unwrap()];
        replace(&os, &mut actual, &next, || Ok(())).unwrap();
        assert_eq!(actual, next);
    }
    #[test]
    fn cancel_registered_only_while_active() {
        let os = Fake::default();
        let keys = Shortcuts::default().parse().unwrap();
        let mut actual = vec![];
        replace(&os, &mut actual, &keys[..2], || Ok(())).unwrap();
        assert!(!actual.contains(&keys[2]));
        replace(&os, &mut actual, &keys, || Ok(())).unwrap();
        replace(&os, &mut actual, &keys[..2], || Ok(())).unwrap();
        assert!(!os.held.borrow().contains(&keys[2]));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod fn_tests {
    use super::*;
    #[test]
    fn fn_and_space_are_distinct() {
        let s = Shortcuts {
            record: "Fn".into(),
            translate: "Fn+Space".into(),
            cancel: "Escape".into(),
        };
        let b = s.bindings().unwrap();
        assert_eq!(b[0].native().as_deref(), Some("Fn"));
        assert_eq!(b[1].native().as_deref(), Some("Fn+Space"));
    }
    #[test]
    fn aliases_canonicalized_and_duplicate_rejected() {
        let s = Shortcuts {
            record: "Control+Fn+Space".into(),
            translate: "fn+ctrl+Space".into(),
            cancel: "Escape".into(),
        };
        assert!(s.bindings().is_err());
    }
}

// Native Fn chords are consumed before combined-session keyDown counters.
// Ordinary global shortcuts contribute a keyDown when recording is finished.
pub fn delivery_key_events(shortcuts: &Shortcuts, translations: usize) -> Result<usize, String> {
    let keys = shortcuts.bindings()?;
    Ok(usize::from(keys[0].global().is_some())
        + translations * usize::from(keys[1].global().is_some()))
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    #[test]
    fn expected_key_events_exclude_consumed_fn_chords() {
        assert_eq!(delivery_key_events(&Shortcuts::default(), 2).unwrap(), 3);
        #[cfg(target_os = "macos")]
        {
        let fn_keys = Shortcuts {
            record: "Fn".into(),
            translate: "Fn+Space".into(),
            cancel: "Escape".into(),
        };
        assert_eq!(delivery_key_events(&fn_keys, 3).unwrap(), 0);
        let mixed = Shortcuts {
            record: "Control+Alt+Space".into(),
            ..fn_keys
        };
        assert_eq!(delivery_key_events(&mixed, 2).unwrap(), 1);
        }
        #[cfg(not(target_os = "macos"))]
        assert!(Shortcuts { record: "Fn".into(), ..Shortcuts::default() }.bindings().is_err());
    }
}
