# Rust Style Guide — Curated Summary

Source: https://doc.rust-lang.org/style-guide/index.html

> Most formatting rules are enforced automatically by `rustfmt`. Flag issues only where `rustfmt` cannot catch them (logic, naming, structure).

## Indentation & Line Width
- Spaces only — no tabs
- 4 spaces per indentation level
- Maximum line width: **100 characters**
- Prefer **block indent** over visual indent:

```rust
// Preferred
a_function_call(
    foo,
    bar,
);

// Avoid
a_function_call(foo,
                bar);
```

## Trailing Commas
Use trailing commas in multi-line comma-separated lists:
```rust
function_call(
    argument,
    another_argument,  // trailing comma
);
```

## Blank Lines
Separate items and statements by zero or one blank line. No double-blank-line gaps.

## Trailing Whitespace
No trailing whitespace anywhere — including blank lines, comments, and string literals.

## Comments
- Prefer line comments (`//`) over block comments (`/* */`)
- Single space after `//`
- Prefer comments on their own line; if inline, one space before
- Comment lines (excluding indentation) limited to **80 characters**
- Doc comments: prefer `///` over `/** */`; use `//!` / `/*!` only for module/crate-level docs
- Put doc comments **before** attributes

## Attributes
- Each attribute on its own line, indented to the item level
- Single `derive` attribute per item; preserve ordering when merging
- `#[foo = 42]` — single space either side of `=`

## Sorting
When sorting (e.g. `use` statements, derive lists): numeric chunks compare by value, `_` sorts after space, UpperCamelCase sorts before lowercase.
