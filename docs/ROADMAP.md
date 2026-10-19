# Roadmap

## Available in the Mac beta

The Rust core, Tauri UI and macOS adapters implement dictation, selected-text questions/edits, translation segments, original-target insertion, custom model connections, optional trial access, local history and reversible explicit layout preferences. Synthetic core/network/UI tests and local build checks are included. Windows and Intel Mac are not shipped.

## Priorities

| Area | Work | Acceptance |
| --- | --- | --- |
| Installation | Signing, notarization, clean-Mac and upgrade checks | Download, first permissions and upgrade independently tested |
| App compatibility | Opaque inputs, selections and WeChat | Synthetic counterexample and verified receipt in the real app |
| Models | Protocol/capability adapters and errors | Fixed fixtures, cancellation and provider contract checks |
| Adaptation | Evaluate explicit preferences, reversals and new patterns | Held-out evaluation of rework and wrong learning |
| Windows | Credentials, shortcuts, capture, target and delivery adapters | Windows build, installer, permissions and receipt |
| Store distribution | Sandbox-compatible interaction and license requirements | Separate sandbox validation and Apple review |

No date or compatibility guarantee is implied. iOS is outside the current desktop scope.

## First contributions

- Add synthetic cases exposing a scene or formatting failure.
- Reproduce a protocol mismatch without real keys or recordings.
- Improve localization, keyboard navigation or screen-reader labels.
- Discuss a Windows adapter or narrowly scoped writing policy before implementation.

## Evaluation

Compare fixed defaults, a static profile and adaptation with matched inputs/model settings. Separate learning examples from evaluation. Measure confirmed rework, false learning, cross-context mistakes, reversal, critical semantic errors and intervention. Set thresholds before evaluation; always previewing is not proof of useful quality.

Use synthetic contribution fixtures. Real audio/user studies need participant agreement. Accuracy and long-term adaptation claims need measured baselines.
