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

## Intent-based dictation and completion

Policy `2026-10-23.intent.8` supports explicit post-dictation corrections, entity lists and message signatures on both hosted trial and custom Chat Completions connections. Layout provenance distinguishes built-in paragraph defaults from confirmed preferences. Literal spelling evidence is extracted only from narrow, unquoted spelling clauses, and remains separate from the original transcript; it is not a name dictionary or a general semantic proof.

The optional **Smart completion (test)** setting is on by default, respects saved choices and is frozen at recording start. Mac 0.2.3 identifies missing names using all spoken clues, including plot, author, actor and category. For a single work, its title can replace a purely identifying description while retaining authors, actors and other facts. Lists and explicit requests to preserve descriptions use parentheses. Books, films, songs, public entities and technical terms are examples, not a category allowlist or answer mapping. Ambiguous or excluded candidates remain descriptions; ordinary questions remain questions.

The first model response supplies a cleaned body with the source description intact and a separate candidate. Only exact, unique, unquoted, non-overlapping source spans are eligible. A second request receives the entire transcript and those descriptions, without the candidate name, first response or ambient input context. The client applies a candidate only when this independent pass identifies the same entity (including known aliases) and reports no ambiguity. Replacement also requires an identifying-only span; explicit attributes, numeric fields and preservation requests are protected. Disagreement, malformed review or review failure leaves the cleaned description intact. A repaired body consumes the two-request budget, so its candidates remain unreviewed and are withheld. Primary, fallback, repair and review share the existing maximum of two generation attempts.

Both passes use the selected model connection and its knowledge; agreement is not external factual verification and can repeat a shared error. No retrieval is performed, and completion may add waiting time. Private numbers, identities, amounts, dates or commitments must not be inferred. Translation and selection do not receive these dictation rules. The hosted trial supports the same cleanup, completion and blind-review workflow. Legacy `/refine` does not implement the completion protocol. Actual primary/backup route capabilities are checked separately. Local history records route/model, effective options, generation count, completion review state and issue codes without credentials, endpoints or extra model response text.

The stored boolean key `complete_book_titles` is retained for compatibility with the earlier local preview; its current UI and policy meaning is general smart completion. New reports use `completion_requested`/`completion_effective`. New hosted requests carry a policy version and typed options; the server reconstructs the shared policy and never forwards arbitrary client system prompts. Requests from older clients retain the frozen legacy policy.

Replay retains the read-only database workflow. Options: `--writing-completion on|off` (`--writing-books` remains an exclusive alias), `--writing-profile fresh|current|paragraph|numbered`, `--writing-budget 1..80`, and optional `--writing-connection FILE --writing-credential-slot llm|backup`. Connection JSON contains only `base`, `model`, `protocol`; credentials stay in the matching existing Keychain slot. Unknown/duplicate arguments or unknown fixture fields fail. Limits remain 40 items/128 KiB. Each result reports effective options, actual model/protocol, first/repair output, fallback and generation attempts. Errors conservatively reserve the per-case budget; three consecutive request errors stop the batch. Selection scope checks are separate from generation attempts.

Shared synthetic cases in `crates/vao-core/tests/semantic-cases.json` use fictional identities. Their evaluation fields are specifications: convert only `id/text/app` for replay, then use `scripts/check-semantic-replay.py`. These checks cover narrow invariants, not complete semantic quality. Private acceptance inputs and model reports stay outside public source and packages.

Older versions may ignore new settings and drop them when saving through their old typed serializer. This preview preserves unknown saved fields; it cannot change older binaries' behavior.


## Intent preview cleanup and source anchors

Ordinary dictation removes meaningless hesitation tokens and accidental repeats;
referential phrases, emphasis and quoted/verbatim examples remain protected.
The local fallback only removes repeated standalone filler tokens and never
operates across a quotation. Translation and selection keep their separate paths.

Completion descriptions retain the same words and digits in the transcript and
draft. Matching tolerates punctuation and line wrapping only; it does not use
fuzzy spelling, skip words, cross quoted text or accept ambiguous occurrences.
A reviewed replacement preserves author/actor/director relation prefixes,
including variants such as an actor “演的” a film. A complete title wrapper
inserted by ASR is replaced as a unit to avoid nested title marks; partial edits
inside an existing title are withheld. These checks preserve local structure;
they do not prove ASR accuracy or model knowledge. Quiet speech and ordinary
volume require separate microphone acceptance.

### build20261023.2008 回放修复

重复的独立停顿词不作为作品描述保真锚点；回退仍执行保守的重复停顿词清理。若模型提前在正文替换了候选名，先恢复完整转写，再对原文锚定的候选进行盲复核，不接受模糊词匹配；此恢复可能牺牲模型排版。多候选默认附注；明确保留描述的完整控制句在应用前消费，后续事实保留。真实模型可能仍无法可靠补全，回退不算补全成功。

### build20261023.2009 本地收尾候选

Mac移除整个输入增强：临时转写、输入框前后文与附近窗口参考的UI、采集及模型传输路径一并删除；保留最终录音识别、分段合并和本地回填保护。旧开关失效，旧Realtime配置保留服务和模型并迁移文件识别。智能补全文案为“根据输入内容线索分析并补全信息”。

普通“另一部/另一本”枚举不再全局关闭补全，明确否定和歧义继续拒绝猜测。取件码/提货码/运单号的逐位口述在整理前转为数字字符串，保留前导零。模型提前在正文写候选名称时恢复原描述，再使用剩余调用独立复核；仍最多两次生成，未复核名称不得输出。完整的保留描述控制句支持逗号衔接，消费后保留后续事实和引用。
