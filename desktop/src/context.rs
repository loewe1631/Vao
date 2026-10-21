use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct InputContext {
    pub version: u8,
    pub before_cursor: String,
    pub after_cursor: String,
    #[serde(default)]
    pub nearby_text: String,
    #[serde(default)]
    pub window_title: String,
    #[serde(default)]
    pub source_domain: String,
}
impl InputContext {
    pub fn from_value(value: serde_json::Value) -> Option<Self> {
        let c: Self = serde_json::from_value(value).ok()?;
        (c.version == 1
            && c.before_cursor.chars().count() <= 500
            && c.after_cursor.chars().count() <= 500
            && c.before_cursor.len() + c.after_cursor.len() <= 8192
            && c.nearby_text.chars().count() <= 1200
            && c.window_title.chars().count() <= 120
            && c.source_domain.len() <= 253
            && c.before_cursor.len()+c.after_cursor.len()+c.nearby_text.len()+c.window_title.len()+c.source_domain.len() <= 16384
            && !(c.before_cursor.is_empty() && c.after_cursor.is_empty() && c.nearby_text.is_empty() && c.window_title.is_empty() && c.source_domain.is_empty()))
        .then_some(c)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn context_budgets_are_checked_after_deserialization() {
        assert!(InputContext::from_value(
            json!({"version":1,"before_cursor":"Vao 项目","after_cursor":"🙂"})
        )
        .is_some());
        for value in [
            json!({"version":2,"before_cursor":"a","after_cursor":""}),
            json!({"version":1,"before_cursor":"a".repeat(501),"after_cursor":""}),
            json!({"version":1,"before_cursor":"","after_cursor":""}),
        ] {
            assert!(InputContext::from_value(value).is_none());
        }
    }
}

/// Reject copied reference passages absent from the spoken source. Context is
/// reference data, never an instruction to rewrite the whole input field.
pub fn reference_leaked(context: Option<&InputContext>, source: &str, output: &str) -> bool {
    let Some(c) = context else { return false; };
    fn normalized(s: &str) -> String { s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() }
    let source = normalized(source); let output = normalized(output);
    [&c.before_cursor, &c.after_cursor, &c.nearby_text, &c.window_title].iter().any(|reference| {
        let chars: Vec<char> = normalized(reference).chars().collect();
        chars.windows(12).any(|w| { let text: String = w.iter().collect(); output.contains(&text) && !source.contains(&text) })
    })
}
#[cfg(test)]
mod leak_tests {
    use super::*;
    #[test]
    fn insertion_does_not_copy_existing_suffix_but_spoken_repetition_is_kept() {
        let c=InputContext{version:1,before_cursor:"继续做下一版的改进，加入尚未加入的内容。".into(),after_cursor:"录音期间显示临时转写，这个用设置里面的一个开关来决定开启与否。".into(),nearby_text:String::new(),window_title:String::new(),source_domain:String::new()};
        let spoken="另外，我觉得需要做一些功能上的改进。";
        assert!(reference_leaked(Some(&c),spoken,&format!("{spoken}{}{}",c.before_cursor,c.after_cursor)));
        assert!(!reference_leaked(Some(&c),spoken,spoken));
        assert!(!reference_leaked(Some(&c),&c.after_cursor,&c.after_cursor));
        assert!(!reference_leaked(None,spoken,&c.after_cursor));
    }
}
