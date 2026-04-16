---
name: trivy-triage
description: "Run a Trivy security scan, compile an ordered list of findings, then triage each one interactively: fetch advisory details, analyse exploitability in context, propose a fix or suppression, and commit after user approval. Stops after each finding for review."
argument-hint: "[path]"
---

Triage Trivy security findings one at a time. Default scan path is the repo root (`.`); use `$ARGUMENTS` if a path was supplied.

## Step 1 — Run the scan and compile the finding list

```bash
trivy fs --config .trivy.yaml ${ARGUMENTS:-.} 2>&1
```

Collect every finding. Build an ordered list, highest severity first (CRITICAL → HIGH → MEDIUM → LOW), then alphabetically by package name within each band. For each entry record:
- Severity
- Package name and current version
- Finding ID (CVE / GHSA / RUSTSEC)
- Fixed version (if any)
- Advisory URL from the Trivy output

Present the full ordered list to the user before starting triage.

## Step 2 — Triage each finding, one at a time

Work through the list in order. Do not move to the next finding until the user approves the current one.

### 2a — Research

For each finding, gather information in parallel:

1. **Fetch the advisory** — WebFetch the advisory URL from the scan output. If that returns empty, try in order:
   - `https://rustsec.org/advisories/<RUSTSEC-ID>.html`
   - `https://github.com/advisories/<GHSA-ID>`
   - `https://nvd.nist.gov/vuln/detail/<CVE-ID>`
   - `https://github.com/rustsec/advisory-db/tree/main/crates/<crate-name>`

2. **Map the dependency path** — `cargo tree --invert <crate>` to see every consumer chain from this crate up to the workspace crates.

3. **Check for a patch** — `cargo update -p <crate>` (dry-run mentally first; then run it). Note whether the fix requires a semver-compatible bump or a major version change.

### 2b — Analyse exploitability

Answer these questions before presenting the finding:

1. **What is the vulnerability?** Describe the root cause in one sentence.
2. **What triggers it?** Identify the specific function, API, or code path required.
3. **Is that code path reachable?** Check how the crate is actually used — read relevant source if needed. Consider: are the vulnerable functions called? Are required feature flags enabled? Does user-controlled input reach the vulnerable code?
4. **Is a fix available?** Is it a drop-in patch, a major version bump, or blocked on upstream?
5. **What is the real-world risk?** Consider attacker prerequisites, deployment environment (64-bit Linux), and whether any mitigating controls exist.

### 2c — Present the analysis

Format each finding as:

---

**\<CVE/GHSA/RUSTSEC\> — `\<package\>` \<version\> (\<severity\>)**

**What it is:** \<root cause, one sentence\>

**How it's triggered:** \<specific function or API path\>

**Is it exploitable here?** \<yes/no/unlikely + reasoning, referencing the dependency path and code usage\>

**Fix available?** \<version bump via `cargo update` / major version change needed / blocked on upstream\>

**Options:**
- **a)** Apply the fix — \<describe exactly what will change\>
- **b)** Think again
- **c)** Suppress in `.trivyignore` with rationale comment

---

Then **stop and wait** for the user's choice.

## Step 3 — Apply the chosen action

### Option a — Apply the fix

1. Make the change:
   - For a `cargo update` bump: the lock file is already updated; verify with `grep <crate> Cargo.lock`
   - For a `Cargo.toml` version change: edit the workspace `Cargo.toml` and run `cargo update -p <crate>`
   - For a major version bump: edit `Cargo.toml`, then fix any API breakage (read the changelog/docs first)

2. Run checks — all three must pass before committing:
   ```bash
   cargo fmt --check
   cargo clippy
   cargo test
   ```

3. Commit using `fix(security):` type:
   ```
   fix(security): upgrade <crate> <old> → <new> (<CVE/GHSA>)

   <One-paragraph explanation: what the vulnerability is, why it matters
   here, and why the fix is safe to apply. Include the dependency path
   if non-obvious.>

   Advisory: <URL>
   [RUSTSEC: <URL>]

   Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
   ```

### Option b — Think again

The user will supply more input. Wait for their follow-up before doing anything. Once they respond, re-examine the analysis in light of what they said — verify the specific assumption they are questioning (e.g. run `cargo tree --invert` again, grep the source for the specific function, re-read the advisory). Present the revised analysis before offering options again.

### Option c — Suppress in `.trivyignore`

Add an entry to `.trivyignore` using this structure:

```
# projects/rust/Cargo.lock — <crate> <version>
#
# [FIXME if fix is blocked on upstream:]
# Cannot be patched without upstream action. Fix is in <crate> <fix-version>,
# but <consumer(s)> pin <crate> = "<semver-range>".
# To resolve: wait for <upstream> to bump <crate>, then `cargo update` and
# remove this suppression.
# Track upstream: <issue/PR URL if known>
<FINDING-ID>  # Not exploitable: <one-line summary>
              # <detailed rationale — answer all three:>
              # 1. What the vulnerable function/API is
              # 2. Why it cannot be reached in this codebase
              #    (reference consumers, feature flags, code paths)
              # 3. If fix is blocked: what unblocks it
              # Advisory: <URL>
              # [RUSTSEC: <URL>]
```

Then commit:
```
fix(security): suppress <crate> <version> <FINDING-ID> (<reason in 3 words>)

<Paragraph: what the vulnerability is, the five-condition analysis of
why it is not exploitable here, and — if applicable — why a fix cannot
be applied and what would unblock it.>

Advisory: <URL>
[RUSTSEC: <URL>]

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

## Rules

- **One finding at a time.** Never start a new finding until the user has approved the current one.
- **Verify before suppressing.** For transitive dependencies: run `cargo tree --invert <crate>` and grep the source before claiming a code path is unreachable.
- **Keep suppression comments durable.** Future readers must be able to re-evaluate the decision without re-doing the research. Include advisory links, the exact reasoning, and FIXME notes for anything blocked on upstream.
- **All three checks must pass** (`fmt`, `clippy`, `test`) before every commit.
- **Commit type is always `fix(security):`** with advisory links in the body.
- **Update `.trivyignore` section headers** when adding a new suppression — keep the file grouped by package.
- **If a suppression comment becomes stale** (e.g. a previously suppressed `aws-lc-sys` finding gains a new consumer), update the comment in the same commit that introduces the change.
