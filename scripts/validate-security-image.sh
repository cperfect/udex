#!/bin/bash
# Mirrors the "Scan udex image" step in 02-Security.yml.
#
# Scans container images for vulnerabilities and secrets. `trivy fs` (see
# validate-security-fs.sh / the 02-Security job) reads the source tree and the
# dependency manifests; it says nothing about what is actually inside a built
# image — the base layer, its OS packages, or anything the build adds. Those
# drift on their own schedule: an image can acquire a new CVE with no change to
# this repository at all, which is why the security workflow runs on a cron.
#
# Usage:
#   bash scripts/validate-security-image.sh              # udex:latest (the CI gate)
#   bash scripts/validate-security-image.sh --fixtures   # also the dev fixture images
#
# The gate ignores findings with no available fix (see --ignore-unfixed below).
# To see everything, including the unfixable base-image CVEs:
#   trivy image --config .trivy.yaml --scanners vuln udex:latest
#
# The gate covers ONLY udex:latest, the single image this project ships and the
# only one whose findings are ours to act on. The compose fixtures (OpenObserve,
# the collector, Vector, PostgreSQL, Hydra) are third-party, dev-only and never
# deployed; gating on them would mostly produce findings that can only be
# suppressed, and a gate that mostly needs suppressing is a gate people learn to
# ignore. `--fixtures` keeps the capability for when curiosity or an incident
# warrants it.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_DIR}"

SCAN_FIXTURES=false
for arg in "$@"; do
  case "$arg" in
    --fixtures) SCAN_FIXTURES=true ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

command -v trivy &>/dev/null || { echo "ERROR: trivy is required (see scripts/dev-doctor.sh)" >&2; exit 1; }

UDEX_IMAGE="udex:latest"

# Fail with the remedy rather than building silently: the release build takes
# minutes, and a scan script that quietly starts one is a surprise.
if ! docker image inspect "${UDEX_IMAGE}" &>/dev/null; then
  echo "ERROR: ${UDEX_IMAGE} is not present locally." >&2
  echo "Build it first: bash projects/k8s/scripts/image-build.sh" >&2
  exit 1
fi

IMAGES=("${UDEX_IMAGE}")

if [[ "${SCAN_FIXTURES}" == true ]]; then
  # Read the image list from compose rather than duplicating it here, so this
  # cannot drift when the fixture changes.
  if [[ ! -f .env ]]; then
    echo "ERROR: --fixtures needs .env to resolve the compose file (run scripts/gen-env.sh)" >&2
    exit 1
  fi
  mapfile -t FIXTURE_IMAGES < <(
    docker compose -f projects/compose/docker-compose.yml --env-file .env config --images 2>/dev/null | sort -u
  )
  if [[ ${#FIXTURE_IMAGES[@]} -eq 0 ]]; then
    echo "ERROR: could not read fixture images from the compose file" >&2
    exit 1
  fi
  IMAGES+=("${FIXTURE_IMAGES[@]}")
fi

echo "Scanning ${#IMAGES[@]} image(s)..."
echo ""

FAILED=()
for image in "${IMAGES[@]}"; do
  echo "── ${image} ──────────────────────────────────────────"
  # --config keeps severity thresholds and .trivyignore identical to the
  # filesystem scan, so a suppression means the same thing in both.
  #
  # --scanners narrows to vuln,secret, overriding the config's list. The other
  # two are meaningful for a source tree but not for a built image: misconfig has
  # no IaC to read, and license would emit a table of every OS package's licence
  # on every run — noise that trains people to skim past the output.
  #
  # --ignore-unfixed is the difference between a gate people act on and one they
  # switch off. udex:latest currently carries 167 findings from its
  # debian:bookworm-slim base across all severities (5 CRITICAL, 17 HIGH), and
  # NOT ONE has a fix available — every status is will_not_fix, affected or
  # fix_deferred. Gating on
  # those would mean a permanently red build that no change to this repository
  # could turn green. Restricted to fixed findings, the gate means something
  # precise: Debian has shipped a fix and this image has not picked it up, which
  # is resolved by rebuilding.
  #
  # The excluded findings are not invisible — see the unfixed count printed
  # below, and the command in this script's header to list them.
  #
  # Fixture images are reported but never gate: they are third-party and
  # dev-only, so a finding there is information, not a defect in this repo.
  if trivy image --config .trivy.yaml --scanners vuln,secret --ignore-unfixed \
       --skip-version-check "${image}"; then
    echo "  -> clean"
  else
    if [[ "${image}" == "${UDEX_IMAGE}" ]]; then
      FAILED+=("${image}")
    else
      echo "  -> findings in a third-party fixture image (informational, not gated)"
    fi
  fi

  # Report what the gate deliberately excluded, so "clean" is never mistaken for
  # "no vulnerabilities". These are base-image CVEs with no upstream fix; they
  # move when the base image does, not when this repo does.
  # --exit-code 0 overrides the config: this run is informational, and a non-zero
  # exit here would be read as the scan failing rather than as findings existing.
  UNFIXED_JSON=$(trivy image --config .trivy.yaml --scanners vuln --skip-version-check \
                   --exit-code 0 --format json "${image}" 2>/dev/null || true)
  UNFIXED=$(jq '[.Results[]?.Vulnerabilities[]? | select(.FixedVersion == null)] | length' \
              <<<"${UNFIXED_JSON}" 2>/dev/null)
  [[ -n "${UNFIXED}" ]] || UNFIXED="unknown"
  echo "  (${UNFIXED} additional finding(s) at or above the severity threshold with no fix"
  echo "   available — excluded from the gate; see the header for how to list them)"
  echo ""
done

if [[ ${#FAILED[@]} -gt 0 ]]; then
  echo "============================================" >&2
  echo "Findings in the shipped image:" >&2
  printf '  - %s\n' "${FAILED[@]}" >&2
  echo "" >&2
  echo "Triage with the trivy-triage skill, or re-run a single image with:" >&2
  echo "  trivy image --config .trivy.yaml ${FAILED[0]}" >&2
  exit 1
fi

echo "============================================"
echo "  No gating findings."
echo "============================================"
