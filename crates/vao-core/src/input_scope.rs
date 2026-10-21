//! Shared input-purpose policy. The gateway export is checked against the core.
use crate::{Layout, WritingPreset};
use serde_json::{json, Value};
fn policy() -> Value { serde_json::from_str(include_str!("input_scope.json")).expect("input policy JSON") }
pub fn scope_prompt() -> String { policy()["scope_system"].as_str().unwrap().to_owned() }
pub(crate) fn selection_prompt() -> String { policy()["selection_system"].as_str().unwrap().to_owned() }
pub fn gateway_prompts() -> Value {
    let mut result = policy();
    let mut dictation = json!({});
    for preset in [WritingPreset::Message, WritingPreset::Actions, WritingPreset::Plan, WritingPreset::TechnicalRecord, WritingPreset::Document] {
        let name = serde_json::to_value(preset).unwrap();
        for layout in [Layout::Paragraph, Layout::Numbered] {
            let key = serde_json::to_value(layout).unwrap();
            dictation[name.as_str().unwrap()][key.as_str().unwrap()] = json!(crate::prompt_for_preset(preset,layout,&[],None,None));
        }
    }
    result["dictation"] = dictation;
    result["translation"] = json!(crate::prompt_for_preset(WritingPreset::Message,Layout::Paragraph,&[],Some("{{TARGET_LANGUAGE}}"),None));
    result
}
#[cfg(test)]
mod tests {
    #[test]
    fn gateway_policy_export_matches_core() {
        let actual: serde_json::Value = serde_json::from_str(include_str!("../../../scripts/gateway/writing_prompts.json")).unwrap();
        assert_eq!(actual,super::gateway_prompts(),"Run the export-input-policy example after changing prompts");
    }
}
