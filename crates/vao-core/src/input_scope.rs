//! Shared input-purpose policy. The gateway export is checked against the core.
use crate::{Layout, LayoutSource, WritingOptions, WritingPreset};
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
            let plan = crate::WritingPlan {preset,layout,scene:crate::Scene::Chat,
                layout_source:crate::LayoutSource::Default,single_paragraph:false};
            dictation[name.as_str().unwrap()][key.as_str().unwrap()] = json!(plan.prompt(&[],None,None));
        }
    }
    result["dictation"] = dictation;
    result["translation"] = json!(crate::prompt_for_preset(WritingPreset::Message,Layout::Paragraph,&[],Some("{{TARGET_LANGUAGE}}"),None));
    result["repair_system"] = json!(crate::WRITING_REPAIR);
    result["policy_version"] = json!(crate::POLICY_VERSION);
    // Hosted connections rebuild this same contract from versioned data. They
    // never execute a client's free-form system prompt.
    let mut intent = json!({"dictation":{},"structure":{},"review":crate::completion::REVIEW_PROMPT});
    for preset in [WritingPreset::Message, WritingPreset::Actions, WritingPreset::Plan, WritingPreset::TechnicalRecord, WritingPreset::Document] {
        let name = serde_json::to_value(preset).unwrap();
        for layout in [Layout::Paragraph, Layout::Numbered] {
            let key = serde_json::to_value(layout).unwrap();
            let mut plan = crate::WritingPlan {preset,layout,scene:crate::Scene::Chat,
                layout_source:LayoutSource::Default,single_paragraph:false};
            for enabled in [false,true] {
                intent["dictation"][name.as_str().unwrap()][key.as_str().unwrap()][if enabled {"on"} else {"off"}] = json!(
                    plan.prompt_with_options(&[],None,None,WritingOptions{semantic_completion:enabled})
                        .replace(&plan.structure_instruction(), "{{STRUCTURE_INSTRUCTION}}"));
            }
            for source in [LayoutSource::Default,LayoutSource::ConfirmedPreference,LayoutSource::CurrentInstruction] {
                plan.layout_source = source;
                let source = serde_json::to_value(source).unwrap();
                intent["structure"][format!("{}_{}",key.as_str().unwrap(),source.as_str().unwrap())] = json!(plan.structure_instruction());
            }
            plan.single_paragraph = true;
            intent["structure"]["single_paragraph"] = json!(plan.structure_instruction());
        }
    }
    result["intent"] = intent;
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
