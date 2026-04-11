---
verblock: "05 Apr 2026:v0.1: vscode - Initial version; 11 Apr 2026:v1.0: vscode - All WPs complete, marked Done"
intent_version: 2.4.0
status: Done
slug: add-structured-logging
created: 20260405
completed: 20260411
---

# ST0001: Add structured logging

## Objective

Udex should use structured logging - with the default output in json format using a suitable library. 

## Context

The initial phase of the project just used `Println!` macros. For Udex to be usable in a real environments and observable it needs to use structured logging and be consistent about how and when it logs.

In the future we might progress to tracing and performance logging but for now structured logs will be enough.

## Related Steel Threads

- [List any related steel threads here]

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
