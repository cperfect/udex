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
# `cargo test` 40 times would spend most of the run in cargo's own startup.
BIN="$(cargo test --package "${PACKAGE}" --test "${TARGET}" --no-run --message-format=json 2>/dev/null \
  | jq -r --arg t "${TARGET}" 'select(.executable != null and .target.name == $t) | .executable' \
  | tail -1)"

if [[ -z "${BIN}" || ! -x "${BIN}" ]]; then
  echo "ERROR: could not locate the compiled test binary for ${PACKAGE}/${TARGET}" >&2
  exit 1
fi

mapfile -t TESTS < <("${BIN}" --list --format terse 2>/dev/null | sed -n 's/: test$//p')

if [[ ${#TESTS[@]} -eq 0 ]]; then
  echo "ERROR: no tests found in ${BIN} — has the target name changed?" >&2
  exit 1
fi

echo "Running ${#TESTS[@]} tests individually..."
echo ""

FAILED=()
for t in "${TESTS[@]}"; do
  # Every test is attempted even after a failure: one run should give the whole
  # picture, not just the first offender.
  if "${BIN}" "${t}" --exact --quiet >/dev/null 2>&1; then
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
