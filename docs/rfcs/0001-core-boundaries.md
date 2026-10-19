# RFC 0001: Session and adaptation boundaries

Status: accepted on 2026-10-18. The core, host and macOS adapters are implemented; see ARCHITECTURE.md for current behavior and limits.

## Problem

A voice app can accumulate UI-specific state, provider-specific policy, and opaque learned preferences. Changing one part can then break cancellation, fidelity, or user control. The core needs explicit contracts before platform integration.

## Proposal

Adopt the module boundaries in the architecture document. Freeze preferences and connection settings per session. Associate feedback with a result revision, not arbitrary later edits. Restrict the first inferred preference to paragraph versus numbered-list formatting. A revoked or deleted preference must not return from stale feedback.

## Alternatives

- Continue modifying the previous application: preserves implementation speed but does not meet the new independent-core direction.
- Build an event-sourced service platform first: adds retention and operations costs before a local core is validated.
- Learn from every external edit: ambiguous attribution and excessive capture; excluded from this proposal.

## Review questions

- Which transitions can receive late responses, and how are they rejected?
- How does a one-time preference differ from permission to learn?
- What evidence pauses an old inferred rule, and how do explicit rules remain authoritative?
- What exactly does undo or clear delete, and can replay bring it back?
- Which results are confirmed delivered versus merely submitted to the OS?

## Acceptance

Supply a transition table and synthetic counterexamples, finalize the feedback/deletion contract, and select an implementation owner. The technology ADR is a separate decision after a runnable investigation. Accepting this RFC does not certify model quality or application functionality.
