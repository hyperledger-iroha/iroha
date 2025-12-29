#!/usr/bin/env bash

set -euo pipefail

print_usage() {
    cat <<'USAGE'
Usage: log_sorafs_drill.sh --scenario <name> --status <pass|fail|follow-up|scheduled> \
  [--date YYYY-MM-DD] [--start HH:MMZ] [--end HH:MMZ] \
  [--ic NAME] [--scribe NAME] [--link URL] [--notes "free-form notes"] \
  [--log /custom/path/drill-log.md]

Appends a Markdown row to ops/drill-log.md so chaos drills and incident
exercises remain traceable. The script creates the log file with a header when
it does not yet exist.
USAGE
}

SCENARIO=""
STATUS=""
DATE_OVERRIDE=""
START_TIME=""
END_TIME=""
IC_NAME=""
SCRIBE_NAME=""
NOTES=""
LINK=""
LOG_OVERRIDE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --scenario)
            SCENARIO=$2
            shift 2
            ;;
        --status)
            STATUS=$2
            shift 2
            ;;
        --date)
            DATE_OVERRIDE=$2
            shift 2
            ;;
        --start)
            START_TIME=$2
            shift 2
            ;;
        --end)
            END_TIME=$2
            shift 2
            ;;
        --ic)
            IC_NAME=$2
            shift 2
            ;;
        --scribe)
            SCRIBE_NAME=$2
            shift 2
            ;;
        --notes)
            NOTES=$2
            shift 2
            ;;
        --link)
            LINK=$2
            shift 2
            ;;
        --log)
            LOG_OVERRIDE=$2
            shift 2
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

if [[ -z "${SCENARIO}" || -z "${STATUS}" ]]; then
    echo "Error: --scenario and --status are required." >&2
    print_usage
    exit 1
fi

if [[ "${STATUS}" != "pass" && "${STATUS}" != "fail" && "${STATUS}" != "follow-up" && "${STATUS}" != "scheduled" ]]; then
    echo "Error: --status must be one of pass, fail, follow-up, scheduled." >&2
    exit 1
fi

LOG_PATH="${LOG_OVERRIDE:-ops/drill-log.md}"
DATE_VALUE="${DATE_OVERRIDE:-$(date -u +%F)}"
START_VALUE="${START_TIME:-$(date -u +%H:%MZ)}"
END_VALUE="${END_TIME:-"-"}"
IC_VALUE="${IC_NAME:-"-"}"
SCRIBE_VALUE="${SCRIBE_NAME:-"-"}"
LINK_VALUE="${LINK:-"-"}"

# escape pipe characters in notes to keep Markdown table intact
NOTES_SANITISED="${NOTES//|/\&#124;}"
NOTES_VALUE="${NOTES_SANITISED:-"-"}"

if [[ ! -f "${LOG_PATH}" ]]; then
    mkdir -p "$(dirname "${LOG_PATH}")"
    cat <<'HEADER' > "${LOG_PATH}"
---
title: SoraFS Chaos Drill Log
summary: Registry of executed chaos drills and incident rehearsals.
---

| Date | Scenario | Status | Incident Commander | Scribe | Start (UTC) | End (UTC) | Notes | Follow-up / Incident Link |
|------|----------|--------|--------------------|--------|-------------|-----------|-------|---------------------------|
HEADER
fi

cat <<ENTRY >> "${LOG_PATH}"
| ${DATE_VALUE} | ${SCENARIO} | ${STATUS} | ${IC_VALUE} | ${SCRIBE_VALUE} | ${START_VALUE} | ${END_VALUE} | ${NOTES_VALUE} | ${LINK_VALUE} |
ENTRY

echo "Logged drill: ${DATE_VALUE} ${SCENARIO} (${STATUS})"
