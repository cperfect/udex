---
name: review-docs
description: "Launch the doc-reviewer agent to review documentation for accuracy, staleness, broken links, placement, and convention compliance. Optional argument controls scope: a PR number (#123), branch name, commit SHA, file path/glob, or * for all docs. Defaults to docs changed on the current branch vs main, or HEAD commit if on main."
argument-hint: "[pr-number | branch | commit | path | *]"
context: fork
agent: doc-reviewer
---

Scope: $ARGUMENTS

Launch the doc-reviewer agent to review the in-scope documentation. The agent is the single
source of truth for scope resolution, the exclusion rules, and the review guidance (accuracy,
staleness, links, placement, and conventions sourced from CONTRIBUTING.md `#### Docs`) — do
not restate any of those here; just pass the scope through.

Summarize the agent's findings, then offer to work through them one by one. For each finding,
recommend a fix and give three options: a) Apply the fix, b) Think again (I will provide more
input), or c) leave but add an appropriate note (e.g. a `TODO`) and move on. Fixes should
include appropriate notes/links where helpful.

After each finding has been worked through, offer to create a commit.
