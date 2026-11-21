#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Read-only collector for the repository's required CI/gate check. Never writes
# statuses or substitutes old-commit, wrong-event or skipped runs for execution.
# --evaluate FILE SHA EVENT is the offline refusal-control interface.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
requirements="$root/ci/required-workflows.txt"
mapfile -t entries < <(sed '/^[[:space:]]*#/d; /^[[:space:]]*$/d' "$requirements")
[[ ${#entries[@]} -gt 0 ]] || { echo 'ERROR: empty workflow requirements' >&2; exit 3; }
declare -A seen=()
for entry in "${entries[@]}"; do
  path="${entry%%|*}"
  [[ $path =~ ^\.github/workflows/[a-z0-9-]+\.yml$ && $path != .github/workflows/ci.yml ]] || {
    echo "ERROR: invalid or self-referential workflow: $path" >&2; exit 3; }
  if [[ $entry == *'|'* ]]; then
    job="${entry#*|}"
    [[ -n $job && $job != *'|'* ]] || { echo 'ERROR: invalid job selector' >&2; exit 3; }
  fi
  [[ -z ${seen[$entry]+present} ]] || { echo "ERROR: duplicate requirement: $entry" >&2; exit 3; }
  seen[$entry]=1
done
required="$(printf '%s\n' "${entries[@]}" | jq -Rsc 'split("\n") | map(select(length > 0) | split("|") | {path:.[0],job:(.[1] // null)})')"

evaluate() {
  local snapshot="$1" sha="$2" event="$3" report
  [[ $sha =~ ^[0-9a-f]{40}$ ]] || { echo 'ERROR: invalid target SHA' >&2; return 3; }
  [[ $event == pull_request || $event == merge_group ]] || { echo 'ERROR: unsupported event' >&2; return 3; }
  report="$(jq -er --arg sha "$sha" --arg event "$event" --argjson required "$required" '
    if type != "array" then error("workflow snapshot must be an array") else . end
    | if all(.[]; type == "object" and (.id | type == "number" and . > 0 and floor == .)
             and (.run_attempt | type == "number" and . > 0 and floor == .))
      then . else error("invalid workflow run identity") end
    | . as $runs
    | [$required[] as $need
       | ([$runs[] | select(.head_sha == $sha and .event == $event and .path == $need.path)]
          | max_by(.id)) as $run
       | (if $need.job == null then $run else
           ([$run.jobs[]? | select(.name == $need.job and .run_id == $run.id
             and .run_attempt == $run.run_attempt and .head_sha == $sha)]
            | if length == 1 then .[0] else null end) end) as $evidence
       | {path: ($need.path + (if $need.job == null then "" else " / " + $need.job end)),
          conclusion: ($evidence.conclusion // ""),
          # A job row is judged by its own job alone, so a slow advisory job
          # in the same run cannot hold a finished required lane pending.
          state: (if $run == null then "missing"
                  elif $need.job == null and $run.status != "completed" then "pending"
                  elif $need.job == null and ($run.conclusion != "success" and $run.conclusion != "failure") then "failed"
                  elif $evidence == null then "missing"
                  elif $evidence.status != "completed" then "pending"
                  elif $evidence.conclusion == "success" then "success"
                  else "failed" end)}] as $rows
    | ($rows[] | "\(.path): \(.state) \(.conclusion)"),
      (if all($rows[]; .state == "success") then "VERDICT=success"
       elif any($rows[]; .state == "failed") then "VERDICT=failure"
       else "VERDICT=pending" end)
  ' "$snapshot")" || { echo 'ERROR: invalid workflow evidence' >&2; return 3; }
  printf '%s\n' "$report"
  case "${report##*$'\n'}" in
    VERDICT=success) return 0 ;;
    VERDICT=failure) return 1 ;;
    VERDICT=pending) return 2 ;;
    *) echo 'ERROR: absent verdict' >&2; return 3 ;;
  esac
}

if [[ ${1:-} == --evaluate && $# == 4 ]]; then
  evaluate "$2" "$3" "$4"
  exit $?
fi
[[ $# == 0 ]] || { echo 'Usage: check-ci-gate.sh [--evaluate FILE SHA EVENT]' >&2; exit 3; }
: "${GITHUB_REPOSITORY:?}" "${TARGET_SHA:?}" "${RUN_EVENT:?}" "${GH_TOKEN:?}"
[[ $TARGET_SHA =~ ^[0-9a-f]{40}$ ]] || { echo 'ERROR: invalid target SHA' >&2; exit 3; }
[[ $RUN_EVENT == pull_request || $RUN_EVENT == merge_group ]] || { echo 'ERROR: unsupported event' >&2; exit 3; }
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
deadline=$((SECONDS + 3300))
previous=""
while (( SECONDS < deadline )); do
  # Pagination is required: an incomplete first page must not become green.
  gh api --paginate --slurp \
    "repos/$GITHUB_REPOSITORY/actions/runs?head_sha=$TARGET_SHA&event=$RUN_EVENT&per_page=100" \
    | jq '[.[] | .workflow_runs[]]' > "$scratch/runs.json"
  # Only two producer workflows mix required lanes with advisory reporting.
  # Read selected jobs from the latest attempt, never from an earlier rerun.
  jq -r --arg sha "$TARGET_SHA" --arg event "$RUN_EVENT" --argjson required "$required" '
    . as $runs | [$required[] | select(.job != null) | .path] | unique[] as $path
    | [$runs[] | select(.head_sha == $sha and .event == $event and .path == $path)]
    | max_by(.id) | select(. != null)
    | [.id, .run_attempt] | @tsv' "$scratch/runs.json" > "$scratch/selected.tsv"
  while IFS=$'\t' read -r run_id attempt; do
    [[ $run_id =~ ^[0-9]+$ && $attempt =~ ^[0-9]+$ ]] || { echo 'ERROR: invalid run identity' >&2; exit 1; }
    gh api --paginate --slurp \
      "repos/$GITHUB_REPOSITORY/actions/runs/$run_id/attempts/$attempt/jobs?per_page=100" \
      | jq '[.[] | .jobs[]]' > "$scratch/jobs.json"
    jq --argjson id "$run_id" --slurpfile jobs "$scratch/jobs.json" \
      'map(if .id == $id then . + {jobs:$jobs[0]} else . end)' \
      "$scratch/runs.json" > "$scratch/enriched.json"
    mv "$scratch/enriched.json" "$scratch/runs.json"
  done < "$scratch/selected.tsv"
  status=0
  report="$(evaluate "$scratch/runs.json" "$TARGET_SHA" "$RUN_EVENT")" || status=$?
  if [[ $report != "$previous" ]]; then printf '%s\n' "$report"; previous="$report"; fi
  case "$status" in
    0) exit 0 ;;
    2) sleep 20 ;;
    *) exit 1 ;;
  esac
done
echo 'ERROR: required workflows did not all succeed before the evidence deadline' >&2
exit 1
