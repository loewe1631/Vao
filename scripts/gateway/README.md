# Speech-input purpose adapter

`input_scope.py` is a framework-independent adapter for the default text gateway. It accepts dictation cleanup, translation and selection actions; it never forwards client system messages. Selection actions require a separate model-based purpose check before generation. Invalid or unavailable checks return a fixed error, and a denied request does not proceed to generation.

`writing_prompts.json` is an export of the shared Rust input policy and built-in writing templates. Regenerate after changing the core prompts:

```sh
cargo run --offline --locked -q -p vao-core --example export-input-policy > scripts/gateway/writing_prompts.json
```

The core test compares every exported prompt with the current Rust source. Host applications provide authenticated transport, token budgets, concurrency, cancellation and storage rules; this adapter does not grant access, deploy a service or include any credentials. Legacy clients receive server-owned templates through recognized fields and fixed markers. Arbitrary chat bodies and unknown operations are rejected. Model judgments can misclassify requests and are not a complete security boundary.
