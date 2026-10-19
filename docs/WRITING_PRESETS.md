# Built-in writing presets

Vao ships writing defaults in the core. A new profile needs no feedback before
it can format steps, conversational messages or technical records. Presets are
compiled into the app; there is no downloaded script or manually selected mode.

| Preset | Input | Default behavior |
| --- | --- | --- |
| Message | Everyday conversation and social-app fallback | Natural wording and paragraphs; retain questions and uncertainty |
| Actions | Explicit operational instructions or agent-app fallback | Number independent actions; keep attributes together and shared restrictions separate |
| Plan | Personal plans or sequential arrangements in task context | Retain stated order, dates and participants; keep unconfirmed arrangements unconfirmed |
| Technical record | Test observations, errors, versions and diagnostic hypotheses | Separate confirmed facts, unverified scope and hypotheses; never manufacture a diagnosis |
| Document | Dictated meeting notes, technical notes and requirements records | Group supplied material into paragraphs; do not invent decisions or author a requested document |

`WritingPlan::resolve` uses bounded content rules and application-name hints.
An operational instruction to inspect logs is a task; a description of an error
is a technical record. A generic word such as “检查” alone is insufficient.
Application names are a fallback, not permission to answer dictated questions.
This remains a heuristic classifier, not a general semantic understanding model.
App hints use names or bundle identifiers; the policy does not read a browser
page to identify the site or inspect chat history.

Presets share fidelity rules: preserve numbers and their currencies, negation,
scope, conditions and uncertainty. Keep the final explicit correction except
where the user asks to preserve the history or verbatim text. Protect quoted
examples. Commands, paths, URLs and code remain unchanged.

Layout is resolved separately. Fresh task profiles prefer numbered actions;
chat and technical records prefer paragraphs. A saved per-scene layout wins over
the default, but paragraph preference still allows—and calls for—paragraph
boundaries around shared restrictions and separate technical facts. Installing
an update does not clear or replace a user's profile. No feedback is fabricated
from generated output. Translation and selected-text instructions use separate
prompts without the dictation presets or their example facts.
Only examples for the effective layout enter the prompt, so a fresh numbered
plan does not compete with a paragraph example in the same request. A tentative
plan and a task that has a later confirmation condition remain distinct.

The OpenAI chat adapter sends the effective policy as a system message and the
transcript as JSON data. The restricted Vao refine adapter carries a compact
policy and preset through its existing preferences field, within its eight-item,
800-character contract. That restricted protocol still does not support selected
text or segment translation. Primary and configured backup retain the same plan.

## Validate actual outputs

`cargo test --offline --workspace --locked` checks routing, profile precedence,
translation/selection isolation and captured HTTP request bodies. It does not
prove model output quality.

After ordinary refinement, a single numbered item is unnumbered unless the
source explicitly used numbering, code or verbatim content. Narrow preservation
checks cover quoted spans, substantive text after a verbatim quote, common
Chinese date references (with correction exceptions and weekday aliases),
lost cancellation negation and clearly connected three-step action clauses. A
failure gets at most one repair request to the same connection using the
original transcript. If it still fails, the original transcript is returned and
marked `unrefined`, rather than claiming successful refinement or adding a
manual review step. The result does not offer learning feedback in that state.
Translation and selected-text operations bypass these dictation checks.
These checks do not establish complete semantic fidelity.

`crates/vao-core/tests/writing-cases.json` contains 30 fixed text cases and
explicit acceptance criteria, including two same-input app pairs and protected
content. A signed build has an opt-in replay command:

```sh
Vao.app/Contents/MacOS/vao --verify-writing-fixtures crates/vao-core/tests/writing-cases.json \
  --writing-database "$HOME/Library/Application Support/app.vaovao.desktop/vao.sqlite3" \
  --writing-route custom --writing-profile fresh > replay.json
python3 scripts/check-writing-replay.py crates/vao-core/tests/writing-cases.json replay.json
```

Use `--writing-route trial` only after opening the app and consenting to the
trial service. The flag sends these fixed texts to the chosen model and consumes
its normal service quota. There is no enrollment or silent route fallback.
`--writing-profile current` reads saved layout preferences; use corresponding
expectations rather than assuming a fresh profile's layout. Database access is
read-only. Replay does not record audio, capture other apps, insert text, add
history or create learning feedback. The report contains fixture text and model
output but no credentials or service keys.

Examples are guidance, not canned replacements. Different models can produce
different wording. Real model replay and user speech acceptance are separate
from policy and protocol tests; neither establishes quality on arbitrary input.
