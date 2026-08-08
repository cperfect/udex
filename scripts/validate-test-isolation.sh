#!/bin/bash
# Mirrors the "Test isolation" step in 01-Validation.yml (Build & Test job).
#
# Runs every test in a test binary ON ITS OWN, in a fresh process, and reports
# any that cannot pass without other tests having run first.
#
# Why this exists (ST0029): `test_sdk_delete_index_not_empty` asserted that
# deleting a non-empty index is refused while assuming *other* tests had put
# entries in the shared index. Nothing enforced that ordering. When it ran first
# the index was empty, the delete succeeded, and it destroyed the shared fixture
# index — failing nine unrelated tests with a misleading "index not found". It
# looked like flakiness; it was a test that could never pass alone. A second
# instance existed in the OAuth2 tests and was only ever found by this sweep.
#
# A suite that always runs whole cannot tell you which of its tests are
# load-bearing for others. This can.
#
# Usage:
#   bash scripts/validate-test-isolation.sh                 # udex-sdk integration_tests
#   bash scripts/validate-test-isolation.sh udex-sdk obs    # a different binary
#
# Runs serially on purpose: the fixtures bind fixed ports, so two test processes
# at once would fight over them and report port conflicts as test failures.
#
# Note on k8s: the `test_obs_k8s_*` tests return early when K8S_SERVER_URL is
# unset, which is the intended way to run this sweep — they are gated on a
# cluster rather than on other tests. With K8S_SERVER_URL set they run for real
# and the sweep takes minutes instead of seconds.

set -euo pipefail

PACKAGE="${1:-udex-sdk}"
TARGET="${2:-integration_tests}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$(cd "${SCRIPT_DIR}/../projects/rust" && pwd)"
cd "${RUST_DIR}"

command -v jq &>/dev/null || { echo "ERROR: jq is required (see scripts/dev-doctor.sh)" >&2; exit 1; }

echo "Building ${PACKAGE} --test ${TARGET}..."
# --no-run compiles without executing. The JSON stream reports the built
# executable's path, which we invoke directly for each test: going through
# `cargo test` once per test would spend most of the run in cargo's own startup
# (~20s for the whole sweep this way, versus minutes).
#
# Each test is run from the PACKAGE root, because that is the working directory
# cargo gives a test process — not the workspace root this script starts in.
# The distinction matters for anything resolving a relative path, most immediately
# `dotenvy::dotenv_override()`, which searches upward from the working directory
# for `.env`. Taking the package root from cargo's own `manifest_path` rather than
# assuming a layout keeps the two in step if the workspace is ever rearranged.
#
# RESIDUAL DIVERGENCE — read before trusting a surprising result.
#
# Matching the working directory does not make this identical to `cargo test`.
# Cargo also sets runtime variables of its own, notably the dynamic library
# search path; this runs with the ambient environment. So a test could in
# principle pass here and fail under cargo, or the reverse, and the isolation
# verdict would be misleading.
#
# What was checked, rather than assumed:
#   - Fixture paths are cwd-independent anyway: CERTS_DIR/JWT_DIR in
#     `sdk/tests/common/mod.rs` are built from `env!("CARGO_MANIFEST_DIR")`,
#     which is baked in at compile time.
#   - A database-backed test produced the same result run from the workspace
#     root, from the package root, and from /tmp — so no divergence is reachable
#     with the suite as it stands.
#
# The exposure is future tests that depend on a variable only cargo sets. If this
# sweep ever disagrees with `cargo test`, that is the first thing to suspect, and
# the remedy is to run each filter through cargo
# (`cargo test -p "${PACKAGE}" --test "${TARGET}" <name> -- --exact`), trading
# speed for exact fidelity.
ARTIFACT="$(cargo test --package "${PACKAGE}" --test "${TARGET}" --no-run --message-format=json 2>/dev/null \
  | jq -c --arg t "${TARGET}" 'select(.executable != null and .target.name == $t)' \
  | tail -1)"

if [[ -z "${ARTIFACT}" ]]; then
  echo "ERROR: cargo reported no test artifact for ${PACKAGE}/${TARGET}" >&2
  exit 1
fi

BIN="$(jq -r '.executable' <<<"${ARTIFACT}")"
PKG_DIR="$(dirname "$(jq -r '.manifest_path' <<<"${ARTIFACT}")")"

if [[ -z "${BIN}" || ! -x "${BIN}" ]]; then
  echo "ERROR: could not locate the compiled test binary for ${PACKAGE}/${TARGET}" >&2
  exit 1
fi

if [[ ! -d "${PKG_DIR}" ]]; then
  echo "ERROR: package root '${PKG_DIR}' is not a directory" >&2
  exit 1
fi

mapfile -t TESTS < <(cd "${PKG_DIR}" && "${BIN}" --list --format terse 2>/dev/null | sed -n 's/: test$//p')

if [[ ${#TESTS[@]} -eq 0 ]]; then
  echo "ERROR: no tests found in ${BIN} — has the target name changed?" >&2
  exit 1
fi

echo "Running ${#TESTS[@]} tests individually from ${PKG_DIR} (cargo's working directory)..."
echo ""

FAILED=()
for t in "${TESTS[@]}"; do
  # Every test is attempted even after a failure: one run should give the whole
  # picture, not just the first offender. The subshell keeps the cd scoped; BIN is
  # an absolute path, so it resolves regardless of where we run it from.
  if (cd "${PKG_DIR}" && "${BIN}" "${t}" --exact --quiet >/dev/null 2>&1); then
    printf '  ok   %s\n' "${t}"
  else
    printf '  FAIL %s\n' "${t}"
    FAILED+=("${t}")
  fi
done

echo ""
echo "============================================"
printf "  %d/%d tests pass in isolation\n" "$(( ${#TESTS[@]} - ${#FAILED[@]} ))" "${#TESTS[@]}"
echo "============================================"
echo ""

if [[ ${#FAILED[@]} -gt 0 ]]; then
  echo "These tests cannot pass on their own:" >&2
  printf '  - %s\n' "${FAILED[@]}" >&2
  echo "" >&2
  echo "Each depends on state another test produces, or destroys state others need." >&2
  echo "Reproduce one with:" >&2
  echo "  cargo test --package ${PACKAGE} --test ${TARGET} ${FAILED[0]} -- --exact" >&2
  echo "" >&2
  echo "Fix by making the test establish its own preconditions — see ST0029." >&2
  exit 1
fi

echo "No cross-test ordering dependencies."
