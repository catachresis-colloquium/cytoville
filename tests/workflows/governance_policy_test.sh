#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# PR #244 configuration regressions: the retired runtime policy must not ship,
# and governance must run for pull requests targeting any branch with a locked
# checkout of metadatastician/standards at a full 40-hex SHA.
# Includes the proof-policy mapping and the current draft canon 2.1.5 identity.
# These are configuration checks, not proof compiler runs or release evidence.
# Usage: bash tests/workflows/governance_policy_test.sh
# Requires Bun; reads local policy documents, YAML and deed fields only.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
command -v bun >/dev/null 2>&1 || { echo 'FAIL: Bun is required' >&2; exit 1; }

bun run - "$REPO_DIR" <<'JS'
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const workflows = resolve(process.argv[2], ".github/workflows");
const readYaml = (name) => Bun.YAML.parse(readFileSync(resolve(workflows, name), "utf8"));
const governance = readYaml("governance.yml");
const lock = readYaml("actions.lock");

for (const extension of ["yml", "yaml"]) {
  const retired = `runtime-policy.${extension}`;
  assert.equal(existsSync(resolve(workflows, retired)), false,
    `${retired} must not be copied into newly minted repositories`);
  assert.equal(Object.hasOwn(lock.workflows, `.github/workflows/${retired}`), false,
    `actions.lock must not retain the retired ${retired} record`);
}
console.log("PASS: runtime policy workflow and lock records are retired");

// Check the parsed event, rather than grep for 'branches': push deliberately
// retains its branch filter, while pull_request must have no branch/path/type
// restrictions. Missing pull_request must fail as well as a filtered event.
assert.ok(Object.hasOwn(governance.on, "pull_request"),
  "governance must be triggered by pull requests");
assert.deepEqual(governance.on.pull_request ?? {}, {},
  "governance must accept pull requests targeting any branch without filters");
assert.deepEqual(governance.on.push.branches, ["main", "master"],
  "removing the pull-request branch filter must preserve the push filter");
assert.ok(Object.hasOwn(governance.on, "workflow_dispatch"),
  "manual governance dispatch must remain available");
console.log("PASS: unrestricted pull requests coexist with filtered pushes and manual dispatch");

// Governance must pin metadatastician/standards at a full 40-hex SHA and lock
// every step-level action it invokes in actions.lock.
const steps = governance.jobs.governance.steps ?? [];
const standardsCheckout = steps.find(
  (s) => s.with && /^(metadatastician|hyperpolymath)\/standards$/.test(String(s.with.repository ?? ""))
);
assert.ok(standardsCheckout, "governance must check out standards policy scripts");
assert.match(String(standardsCheckout.with.ref ?? ""), /^[a-f0-9]{40}$/,
  "governance must pin standards at a full 40-hex commit SHA");
const checkoutUse = String(standardsCheckout.uses ?? "");
assert.ok(
  (lock.workflows[".github/workflows/governance.yml"] ?? []).includes(checkoutUse),
  "governance's lock record must match its checkout action"
);
assert.ok(lock.dependencies[checkoutUse]?.commit,
  "governance's checkout action must have a corresponding locked dependency commit");
console.log("PASS: governance executes the pinned standards policy with locked actions");

const root = process.argv[2];
const read = (path) => readFileSync(resolve(root, path), "utf8");
// Ignore wrapping and emphasis when checking prose; keep headings and table
// boundaries intact until the relevant section or row has been selected.
const prose = (text) => text.replace(/[*`]/g, "").replace(/\s+/g, " ").trim();
const section = (text, heading) => {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === heading);
  assert.notEqual(start, -1, `missing section: ${heading}`);
  const depth = heading.match(/^[#=]+/)[0].length;
  const end = lines.findIndex((line, index) => index > start &&
    new RegExp(`^[#=]{1,${depth}} `).test(line));
  return lines.slice(start + 1, end < 0 ? undefined : end).join("\n");
};
const claude = read("CLAUDE.md");
const arrival = section(claude, "## Proofs — which prover for which obligation");
const guide = read("verification/proofs/README.adoc");
const mapping = section(guide, "== Which prover for which obligation");
const dependencies = section(guide, "== Third-party code");
const layout = section(guide, "== Directory structure");
const adding = section(guide, "== Adding new proofs");

for (const [name, text] of [["arrival guidance", arrival], ["proof guide", guide]]) {
  assert.match(text, /`3-practice\/PROOF-POLICY\.adoc`/,
    `${name} must name the normative proof policy`);
  assert.match(text, /`hyperpolymath\/standards`/,
    `${name} must identify the policy's authoritative repository`);
}
assert.match(arrival, /`verification\/proofs\/README\.adoc`/);
assert.notEqual(claude.indexOf("<!-- ARRIVAL-PACK:END"), -1,
  "arrival guidance must retain its generated-region boundary");
assert.ok(claude.indexOf("## Proofs — which prover for which obligation") >
  claude.indexOf("<!-- ARRIVAL-PACK:END"),
  "hand-authored proof guidance must survive arrival-pack regeneration");
console.log("PASS: proof guidance points to the canon and lives outside the generated arrival pack");

// Check each association independently: finding every prover somewhere in the
// document would still pass if Rust and Ada, or theory and engineering, swapped.
const markdownRows = arrival.split(/\r?\n/).filter((line) => line.startsWith("|"))
  .map((line) => line.split("|").slice(1, -1).map(prose));
const adocTable = mapping.split("|===")[1];
assert.ok(adocTable, "proof guide must contain the obligation table");
const adocCells = adocTable.split("|").slice(1).map(prose);
assert.equal(adocCells.length % 3, 0, "proof guide table must have three cells per row");
const adocRows = [];
for (let index = 0; index < adocCells.length; index += 3) {
  adocRows.push(adocCells.slice(index, index + 3));
}
for (const [label, arrivalLabel, verifier] of [
  ["Rust code", "Rust code", "Creusot"],
  ["Ada code", "Ada code", "SPARK"],
  ["APIs, ABIs and engineering properties", "APIs, ABIs, engineering properties", "Idris2"],
  ["Theory", "Theory", "Agda"],
]) {
  for (const [name, rows, key] of [
    ["arrival guidance", markdownRows, arrivalLabel], ["proof guide", adocRows, label],
  ]) {
    const matches = rows.filter((row) => row[0] === key);
    assert.equal(matches.length, 1, `${name} must map ${key} exactly once`);
    assert.match(matches[0][1], new RegExp(`^${verifier}\\b`),
      `${name} must assign ${key} to ${verifier}`);
    if (key === "Rust code") {
      assert.match(matches[0][1], /\bPearlite\b/, `${name} must name Creusot's contract language`);
      assert.match(matches[0][1], /\bVerus\b/, `${name} must retain the complementary Rust verifier`);
    }
    if (key === "Theory") {
      assert.match(matches[0][1], /Agda, rechecked in Rocq and Lean/);
    }
  }
  console.log(`PASS: both policy tables assign ${arrivalLabel} to ${verifier}`);
}
const location = (label) => adocRows.find((row) => row[0] === label)[2];
assert.match(location("Rust code"), /on the Rust code itself/i);
assert.match(location("Ada code"), /on the Ada code itself.*gnatprove/i);
assert.match(location("APIs, ABIs and engineering properties"), /idris2\/.*ABI proofs are mandatory/);
assert.match(location("Theory"), /agda\/ is the source of truth; coq\/ and lean4\/ hold the recheck legs/);
assert.match(location("Theory"), /Dedukti transport first, hand re-proof as the fallback; no Mathlib/);
assert.match(prose(arrival), /no Mathlib/);
console.log("PASS: proof locations preserve mandatory ABI proofs and both theory recheck legs");

const obligations = prose(mapping);
assert.match(obligations, /types, functions or wire formats are engineering \(Idris2\)/);
assert.match(obligations, /any implementation are theory \(Agda\)/);
assert.match(obligations, /first-party Rust or Ada code.*Creusot\/Pearlite and Verus, or SPARK for Ada, on that code, not restated in Idris2/);
assert.match(obligations, /recorded beside the artefact and in docs\/status\/PROOF-STATUS\.adoc/);
assert.match(obligations, /tlaplus\/.*finite model check is reported as TESTED, with its bounds/);
assert.match(obligations, /Kani \(unsafe and FFI seams, reported TESTED with bounds\)/);
assert.match(obligations, /Aeneas \(Rust that implements an Agda-proved theorem\)/);
assert.match(obligations, /alongside Creusot, never instead of it/);
assert.match(prose(arrival), /Kani for unsafe\/FFI seams and Aeneas.*Agda theorem.*alongside, never instead/);
console.log("PASS: engineering boundaries and bounded exceptions preserve the primary verifier");

assert.match(prose(dependencies), /not converted, re-proved or annotated/);
assert.match(prose(dependencies), /trusted boundary specification.*ASSUMED.*dependency and version.*docs\/status\/PROOF-STATUS\.adoc/);
assert.match(prose(arrival), /not converted or re-proved.*ASSUMED boundary specification/);
for (const text of [dependencies, arrival]) {
  assert.match(prose(text), /fit equally well.*prefer one that carries its own verification/i);
  assert.match(prose(text), /preference, not a gate/i);
}
console.log("PASS: third-party dependencies remain ASSUMED boundaries and verification is a preference");

// Negative regression: a shipped example, even one with a gated manifest
// entry, must never become evidence of completed proofs or a theory recheck.
const examples = prose(layout);
for (const file of ["Properties.agda", "TypeSafety.v", "ApiTypes.lean", "Types.idr", "StateMachine.tla"]) {
  assert.ok(examples.includes(file), `layout guidance must identify example ${file}`);
}
assert.match(examples, /layout examples, not evidence/);
assert.match(examples, /quarantine.*MANIFEST/);
assert.match(examples, /TypeSafety\.v and ApiTypes\.lean are not rechecks of Properties\.agda/);
assert.match(examples, /replaces them with its own proofs or deletes them, and never counts them in docs\/status\/PROOF-STATUS\.adoc/);
console.log("PASS: shipped examples cannot be counted as proof evidence or theory rechecks");

assert.match(prose(adding), /classify the obligation.*docs\/status\/PROOF-NEEDS\.adoc/i);
assert.match(prose(adding), /put Rust and Ada proofs on the code/i);
assert.match(prose(adding), /%default total/);
assert.match(prose(adding), /run the verification command/i);
assert.match(prose(adding), /docs\/status\/PROOF-STATUS\.adoc.*recheck legs.*ASSUMED boundary specifications/);
for (const path of ["docs/status/PROOF-NEEDS.adoc", "docs/status/PROOF-STATUS.adoc"]) {
  assert.ok(existsSync(resolve(root, path)), `proof tracking document must exist: ${path}`);
}
console.log("PASS: new proof instructions require needs, verification, rechecks and assumptions");

// Keep the independently specified canon tuple together: a version-only
// bump or retaining the previous gates hash would misidentify the canon.
// The current upstream identity is draft; this is not a publication claim.
const field = (key) => execFileSync("bash", [resolve(root, "scripts/deed-field.sh"),
  resolve(root, "cytoville_chora.deed"), "canon", key], { encoding: "utf8" }).trim();
assert.deepEqual([field("version"), field("criteria-sha256"), field("gates-sha256")], [
  "2.1.5",
  "6a5aa8857bd0d0d58ef48827938ca17c251b6b854dacf59d306388d61694d82a",
  "5c510bba025be718deebb12d0333bfe7db2da43c66578b728b1a334df65290b1",
], "canon 2.1.5 retains the criteria and adopts corrected gate-table identity");
assert.equal(execFileSync("bash", [resolve(root, "scripts/deed-field.sh"),
  resolve(root, "cytoville_chora.deed"), "provenance", "canon-version"],
  { encoding: "utf8" }).trim(), "2.1.4", "mint provenance must not follow adoption");
console.log("PASS: current draft canon identity and original mint provenance are distinct");

// A skipped or failed reusable acceptance job must never turn the established
// required Rust check green. Exercise its actual shell verdict for every result.
const rust = readYaml("rust-ci.yml");
// CI/gate must run on PRs and synthetic merge-queue commits, with producers
// that cannot disappear behind path filters. Advisory reporters stay advisory.
const required = read("ci/required-workflows.txt").split(/\r?\n/)
  .filter((line) => line && !line.startsWith("#"));
assert.equal(new Set(required).size, required.length);
const validateProducer = (doc, jobName) => {
  assert.ok(Object.hasOwn(doc.on, "pull_request"));
  const pr = doc.on.pull_request ?? {};
  for (const filter of ["paths", "paths-ignore", "branches-ignore", "types"]) {
    assert.equal(pr[filter], undefined, `required producer cannot use ${filter}`);
  }
  if (pr.branches) {
    assert.ok(pr.branches.includes("main") || pr.branches.includes("**"));
    assert.ok(pr.branches.every((branch) => !branch.startsWith("!")));
  }
  assert.deepEqual(doc.on.merge_group, { types: ["checks_requested"] });
  if (jobName) {
    const jobs = Object.values(doc.jobs).filter((job) => job.name === jobName);
    assert.equal(jobs.length, 1, `missing or ambiguous required job: ${jobName}`);
    assert.equal(jobs[0].if, undefined, "required conformance job cannot be skipped");
    assert.equal(jobs[0].strategy, undefined, "job selector must identify one concrete job");
  }
};
for (const entry of required) {
  const [path, name] = entry.split("|");
  validateProducer(Bun.YAML.parse(read(path)), name);
}
const aggregate = readYaml("ci.yml");
validateProducer(aggregate);
assert.equal(aggregate.jobs.gate.name, "CI/gate");
assert.deepEqual(aggregate.permissions, { contents: "read", actions: "read" });
assert.ok(aggregate.jobs.gate.steps.some((s) => s.run === "bash tests/workflows/ci_gate_test.sh"));
const collect = aggregate.jobs.gate.steps.find((s) => s.run === "bash scripts/check-ci-gate.sh");
assert.ok(collect);
assert.equal(collect.env.TARGET_SHA, "${{ github.event.pull_request.head.sha || github.sha }}");
assert.equal(collect.env.RUN_EVENT, "${{ github.event_name }}");
for (const mutate of [
  (d) => { delete d.on.merge_group; },
  (d) => { d.on.pull_request.paths = ["src/**"]; },
  (d) => { d.on.merge_group.types = ["destroyed"]; },
]) {
  const broken = structuredClone(aggregate);
  mutate(broken);
  assert.throws(() => validateProducer(broken));
}
const fork = readYaml("fork-governance.yml");
assert.equal(fork.jobs.deed.steps.find((s) => s.env?.BASE_REF).env.BASE_REF,
  "${{ github.event.merge_group.base_sha || github.event.pull_request.base.sha || github.base_ref }}");
console.log("PASS: required producers and CI/gate run for PR and merge queue; missing and filtered wiring refused");
const e2e = readYaml("e2e.yml");
assert.equal(rust.jobs.acceptance.uses, "./.github/workflows/e2e.yml");
assert.equal(rust.jobs.acceptance.needs, "windows-native");
const gate = rust.jobs["rust-ci"];
assert.deepEqual(gate.needs, ["windows-native", "acceptance"]);
assert.equal(gate.if, "${{ always() }}");
assert.equal(gate.steps.length, 1);
assert.deepEqual(gate.steps[0].env, {
  WINDOWS_RESULT: "${{ needs.windows-native.result }}",
  ACCEPTANCE_RESULT: "${{ needs.acceptance.result }}",
});
for (const windows of ["success", "failure", "cancelled", "skipped"]) {
  for (const acceptance of ["success", "failure", "cancelled", "skipped"]) {
    const result = spawnSync("bash", ["-c", gate.steps[0].run], {
      env: { ...process.env, WINDOWS_RESULT: windows, ACCEPTANCE_RESULT: acceptance },
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status === 0, windows === "success" && acceptance === "success");
  }
}
assert.ok(Object.hasOwn(e2e.on, "workflow_call"));
const acceptanceSteps = e2e.jobs.acceptance.steps;
for (const script of ["scripts/cytoville-check.sh", "scripts/cytoville-graphical-acceptance.sh"]) {
  const step = acceptanceSteps.find((s) => (s.run ?? "").includes(script));
  assert.ok(step, `${script} must execute in the reusable acceptance job`);
  assert.equal(step["continue-on-error"], undefined);
}
assert.equal(e2e.jobs.acceptance["continue-on-error"], undefined);
console.log("PASS: both native platforms and complete acceptance are required; skipped work refuses green");
JS
