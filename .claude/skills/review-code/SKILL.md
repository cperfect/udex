---
name: review-code
description: "Launch the code-reviewer agent. Optional argument controls scope: a PR number (#123), branch name, commit SHA, file path/glob, or * for the full project. Defaults to current branch vs main, or HEAD commit if on main."
argument-hint: "[pr-number | branch | commit | path | *]"
context: fork
agent: code-reviewer
---

Scope: $ARGUMENTS

Resolve the scope as follows:
- No argument, not on `main` → diff of the current branch vs `main`
- No argument, on `main` → the current HEAD commit
- `*` → the entire project
- A PR number (`123` or `#123`) → the changeset of that PR's branch vs its base
- A branch name → diff of that branch vs `main`
- A commit SHA or ref → that specific commit
- A file path or glob → restrict the default scope to matching files only
