# NRC Error Design Guidelines — Curated Summary

Source: https://nrc.github.io/error-docs/error-design/index.html

---

## Thinking About Errors

### Internal vs External Errors
- **External errors**: stem from incorrect input or environment (e.g. bad filename, malformed JSON). Programs should expect these.
- **Internal errors**: result from bugs or unexpected program state. Should not occur in correct code.
- The boundary is perspective-dependent: what is external to a library may be internal to its client.

### Recoverable vs Unrecoverable
Recovery is context-dependent. Consider: program state before/after, location in the call stack, available context, and effort/risk. Even if technically recoverable, excessive effort or risk makes an error effectively unrecoverable.

---

## Error Handling Strategy

**Treat error handling as an architectural decision made early.** Your strategy should address:
- Robustness expectations
- Environmental assumptions
- How errors propagate and are reported
- Logging and telemetry needs
- Who handles which errors, and how recovery/error states are tested

### Result vs Panic
- **Most programs should use `Result` for most error handling.**
- Prefer `Result` over `unwrap`/`expect`, even in prototypes.
- Design APIs to return `Result` and let the caller decide whether to panic.
- Reserve panics for surfacing bugs in states that *should be impossible*.

### Recovery Techniques
| Technique | When to use |
|---|---|
| Stop | Cease the action; ensure consistent state via cleanup/undo |
| Retry | Reattempt, possibly with modified parameters |
| Ignore | Action is optional or a default exists |
| Fallback | Use an alternate path or default value |

### Information in Errors
- **Store error data structurally, not as pre-formatted strings.**
- Structural data lets callers decide how to log/report, enables recovery, allows localisation, and permits structured logging.
- Use `tracing` for production-strength structured logging; `log` + `env-logger` for simple needs.

---

## Error Type Design

### Three Main Approaches

#### 1. Enum Style
One enum per class of errors, one variant per specific error kind.
- Design based on *how errors arise*, not how they're handled — better future-proofing.
- **Advantages**: maximum recovery information, easy to extend, works well with `thiserror`.
- **Warning**: nested errors (`MyError::Io(io::Error)`) are **overused — treat as an anti-pattern** unless justified. Add context via `.map_err(|e| ...)` rather than bare `?`. Normalise aggressively in `From` implementations.

#### 2. Single Struct Style (`std::io::Error` pattern)
One error struct with consistent fields; a C-like enum (no embedded data) for the kind.
- **Advantages**: scales to many errors, simpler logging, easier customisation at error sites, less upfront design.
- Optional fields: backtrace, optional source error, error code, program state.

#### 3. Trait Objects (`Box<dyn Error>`)
Convert concrete types to `Box<dyn Error>` (or `anyhow::Error`) at a boundary.
- **Advantages**: statically simpler, uniform logging, flexible for backwards compatibility.
- **Disadvantage**: not suitable where fine-grained recovery is needed.

### Choosing an Approach
- Different modules can use different approaches — mix as appropriate.
- Libraries → prefer concrete types (give users flexibility).
- Applications → trait objects are fine for simplicity, but concrete types enable local recovery.
- Large applications → enum style supports localisation and API modularity.

| Recovery need | Recommended approach |
|---|---|
| Fine-grained recovery at distance | Enum style |
| Many similar errors / coarse recovery | Single struct style |
| Logging-focused, minimal recovery | Trait objects |

### Naming
- Avoid repeating `Error` in variant names: prefer `MyError::Io` over `MyError::IoError`.
- Acceptable to shadow standard names like `Result` within a module.
- Keep errors in the module they serve — avoid a dedicated top-level `errors` module.

### Stability
- Errors are part of your public API.
- Mark error enums `#[non_exhaustive]` to allow future variants without breaking changes.
- Document whether downcasting or additional context fields are part of the stable API.
- At API boundaries, convert internal errors to API error types. Generally do **not** expose the internal error as a `source`.
