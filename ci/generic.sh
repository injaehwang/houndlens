#!/bin/bash
# houndlens generic CI script
# Works with: Jenkins, Bitbucket Pipelines, Azure DevOps, CircleCI, Travis, etc.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/injaehwang/houndlens/main/ci/generic.sh | bash
#
# Environment variables:
#   HOUNDLENS_DIFF_BASE  - Git ref to compare against (default: auto-detect)
#   HOUNDLENS_FAIL_ON    - Fail on: error, warning, never (default: error)
#   HOUNDLENS_FORMAT     - Output format: text, json, sarif (default: text)

set -euo pipefail

DIFF_BASE="${HOUNDLENS_DIFF_BASE:-}"
FAIL_ON="${HOUNDLENS_FAIL_ON:-error}"
FORMAT="${HOUNDLENS_FORMAT:-text}"

echo "=== houndlens CI verification ==="

# Install if not present.
if ! command -v houndlens &> /dev/null; then
    echo "Installing houndlens..."
    cargo install houndlens 2>/dev/null || {
        echo "cargo install failed, trying from source..."
        git clone --depth 1 https://github.com/injaehwang/houndlens /tmp/houndlens
        cargo install --path /tmp/houndlens
    }
fi

# Auto-detect diff base if not set.
if [ -z "$DIFF_BASE" ]; then
    # Try common CI environment variables.
    if [ -n "${GITHUB_BASE_REF:-}" ]; then
        DIFF_BASE="origin/${GITHUB_BASE_REF}"
    elif [ -n "${CI_MERGE_REQUEST_DIFF_BASE_SHA:-}" ]; then
        DIFF_BASE="$CI_MERGE_REQUEST_DIFF_BASE_SHA"
    elif [ -n "${CHANGE_TARGET:-}" ]; then
        # Jenkins
        DIFF_BASE="origin/${CHANGE_TARGET}"
    elif [ -n "${BITBUCKET_PR_DESTINATION_BRANCH:-}" ]; then
        DIFF_BASE="origin/${BITBUCKET_PR_DESTINATION_BRANCH}"
    elif [ -n "${SYSTEM_PULLREQUEST_TARGETBRANCH:-}" ]; then
        # Azure DevOps
        DIFF_BASE="origin/${SYSTEM_PULLREQUEST_TARGETBRANCH}"
    elif [ -n "${CIRCLE_BRANCH:-}" ]; then
        DIFF_BASE="origin/main"
    else
        DIFF_BASE="HEAD~1"
    fi
fi

echo "Diff base: $DIFF_BASE"
echo "Fail on:   $FAIL_ON"
echo "Format:    $FORMAT"
echo ""

# Run verification.
set +e
houndlens --format "$FORMAT" verify --diff "$DIFF_BASE"
EXIT_CODE=$?
set -e

# Also generate JSON report.
houndlens --format json verify --diff "$DIFF_BASE" > houndlens-report.json 2>/dev/null || true

# Determine exit.
case "$FAIL_ON" in
    never)
        exit 0
        ;;
    warning)
        WARNINGS=$(jq -r '.summary.warnings // 0' houndlens-report.json 2>/dev/null || echo "0")
        if [ "$WARNINGS" -gt 0 ] || [ "$EXIT_CODE" -ne 0 ]; then
            echo ""
            echo "=== FAILED: $WARNINGS warnings found ==="
            exit 1
        fi
        ;;
    *)
        exit $EXIT_CODE
        ;;
esac

echo ""
echo "=== houndlens verification complete ==="
