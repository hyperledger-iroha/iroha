#!/usr/bin/env sh
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: schedule_fraud_scoring.sh --completed-on YYYY-MM-DD [--next-due YYYY-MM-DD] [--notes "text"] [--dry-run]

Create a follow-up task stub for the quarterly fraud-scoring red-team exercise.

Options:
  --completed-on   ISO-8601 date when the exercise concluded (required).
  --next-due       Override the next scheduled exercise date. Defaults to completed-on + 98 days.
  --notes          Additional free-form notes appended to the task file.
  --dry-run        Print the generated task to stdout instead of writing a file.
  --allow-duplicate  Write even if a reminder for the same completion date already exists.
  -h, --help       Show this help message and exit.

The script writes Markdown tasks under scripts/ci/fraud_scoring_tasks/.
Each run emits a follow-up checklist so CI can surface upcoming work.
USAGE
}

completed=""
next_due=""
notes=""
dry_run=false
allow_duplicate=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --completed-on)
      completed="$2"
      shift 2
      ;;
    --next-due)
      next_due="$2"
      shift 2
      ;;
    --notes)
      notes="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    --allow-duplicate)
      allow_duplicate=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [ -z "$completed" ]; then
  printf 'Error: --completed-on is required.\n\n' >&2
  usage >&2
  exit 1
fi

if [ -z "$next_due" ]; then
  if command -v python3 >/dev/null 2>&1; then
    next_due="$(python3 - <<PY
import datetime
completed = datetime.date.fromisoformat("$completed")
# 14-week cadence (~98 days) keeps exercises aligned with quarter boundaries.
next_due = completed + datetime.timedelta(days=98)
print(next_due.isoformat())
PY
)"
  else
    printf 'python3 not available; please provide --next-due explicitly.\n' >&2
    exit 1
  fi
fi

reminder_dir="$(dirname "$0")/fraud_scoring_tasks"
output_file="$reminder_dir/${completed}.md"

content="$(cat <<TASK
# Fraud Scoring Exercise Follow-up – $completed

- Completed on: $completed
- Next exercise due: $next_due

## Follow-up checklist
- [ ] Capture red-team findings in the fraud threat model and roadmap.
- [ ] File remediation tickets for each confirmed gap; link ticket IDs here.
- [ ] Review `fraud_psp_*` telemetry panels for each tenant and document regressions.
- [ ] Confirm PSP partners received the summary package and acknowledged action items.
- [ ] Schedule tabletop drill ahead of $next_due using this script.
TASK
)"

if [ -n "$notes" ]; then
  content="${content}
## Notes
${notes}
"
fi

if [ "$dry_run" = true ]; then
  printf '%s\n' "$content"
  exit 0
fi

mkdir -p "$reminder_dir"
if [ -e "$output_file" ]; then
  if [ "$allow_duplicate" = false ] && grep -q "Completed on: $completed" "$output_file"; then
    printf 'Reminder for %s already exists at %s; pass --allow-duplicate to append.\n' "$completed" "$output_file"
    exit 0
  fi
  printf 'Updating existing reminder: %s\n' "$output_file"
  printf '\n%s\n' "$content" >> "$output_file"
else
  printf 'Writing reminder: %s\n' "$output_file"
  printf '%s\n' "$content" > "$output_file"
fi
