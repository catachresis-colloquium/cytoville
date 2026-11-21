#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
gate="$root/scripts/check-ci-gate.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
sha=1111111111111111111111111111111111111111
sed '/^[[:space:]]*#/d; /^[[:space:]]*$/d' "$root/ci/required-workflows.txt" |
  jq -Rsc --arg sha "$sha" 'split("\n") | map(select(length > 0) | split("|"))
    | group_by(.[0]) | to_entries
    | map((.key+1) as $id | {id:$id,run_attempt:1,path:.value[0][0],head_sha:$sha,
        event:"pull_request",status:"completed",conclusion:"success",
        jobs:[.value[] | select(length == 2) | {name:.[1],run_id:$id,run_attempt:1,
          head_sha:$sha,status:"completed",conclusion:"success"}]})' > "$scratch/good.json"
expect() {
  local want="$1" file="$2" event="${3:-pull_request}" actual=0
  bash "$gate" --evaluate "$file" "$sha" "$event" > "$scratch/result" 2>&1 || actual=$?
  if [[ $actual != "$want" ]]; then
    cat "$scratch/result"; echo "FAIL: expected $want, got $actual" >&2; exit 1
  fi
}
expect 0 "$scratch/good.json"
for conclusion in failure cancelled timed_out skipped neutral action_required stale; do
  jq --arg result "$conclusion" '.[0].conclusion=$result' "$scratch/good.json" > "$scratch/bad.json"
  expect 1 "$scratch/bad.json"
done
for change in '.[0].status="in_progress"' '.[0].head_sha="2222222222222222222222222222222222222222"' '.[0].event="push"' '.[0].path="unrelated.yml"' '.[1:]' '[]'; do
  jq "$change" "$scratch/good.json" > "$scratch/pending.json"
  expect 2 "$scratch/pending.json"
done
# An older success cannot hide a newer failed or unfinished run of that workflow.
jq '. + [.[0] + {id:9999,conclusion:"failure"}]' "$scratch/good.json" > "$scratch/newer.json"
expect 1 "$scratch/newer.json"
jq '. + [.[0] + {id:9999,status:"in_progress",conclusion:null}]' "$scratch/good.json" > "$scratch/newer.json"
expect 2 "$scratch/newer.json"
jq 'map(.event="merge_group")' "$scratch/good.json" > "$scratch/queue.json"
expect 0 "$scratch/queue.json" merge_group
expect 2 "$scratch/good.json" merge_group
# Advisory failures do not change the existing conformance policy.
jq 'map(if (.jobs | length) > 0 then .conclusion="failure" else . end)' "$scratch/good.json" > "$scratch/advisory.json"
expect 0 "$scratch/advisory.json"
for conclusion in failure skipped cancelled neutral; do
  jq --arg result "$conclusion" 'map(if (.jobs | length) > 0 then .jobs[0].conclusion=$result else . end)' "$scratch/good.json" > "$scratch/job.json"
  expect 1 "$scratch/job.json"
done
for change in '.run_attempt=2' '.id=999' '.jobs=[]' '.jobs[0].name="not the required job"' '.jobs[0].head_sha="2222222222222222222222222222222222222222"' '.jobs += [.jobs[0]]'; do
  jq "map(if (.jobs | length) > 0 then $change else . end)" "$scratch/good.json" > "$scratch/job.json"
  expect 2 "$scratch/job.json"
done
# A job row is judged by its job, not by the rest of its run: required jobs
# that finished decide the row while an advisory job keeps the run open.
running='map(if (.jobs | length) > 0 then .status="in_progress" | .conclusion=null else . end)'
jq "$running" "$scratch/good.json" > "$scratch/running.json"
expect 0 "$scratch/running.json"
jq "$running | map(if (.jobs | length) > 0 then .jobs[0].conclusion=\"failure\" else . end)" "$scratch/good.json" > "$scratch/running.json"
expect 1 "$scratch/running.json"
jq "$running | map(if (.jobs | length) > 0 then .jobs[0].status=\"in_progress\" | .jobs[0].conclusion=null else . end)" "$scratch/good.json" > "$scratch/running.json"
expect 2 "$scratch/running.json"
printf 'not json\n' > "$scratch/malformed.json"
expect 3 "$scratch/malformed.json"
printf '{}\n' > "$scratch/malformed.json"
expect 3 "$scratch/malformed.json"
for change in '.[0].id="not an id"' 'del(.[0].run_attempt)'; do
  jq "$change" "$scratch/good.json" > "$scratch/malformed.json"
  expect 3 "$scratch/malformed.json"
done
echo 'PASS: exact-commit/event successes required; missing, skipped, failed, malformed and stale evidence refused'
