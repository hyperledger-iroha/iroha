// SPDX-License-Identifier: Apache-2.0
// SCCP proposal envelope and subject-revision checks shared by wire and Torii reads.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { NetworkId } from "../src/networkId.js";
import { normalizeGovernanceProposalWireV1 } from "../src/governanceProposalV1.js";

const networkId = NetworkId.fromBytes(Uint8Array.from([...Array(31).fill(0), 1])).toString();
const external = (network = "bsc_mainnet") => ({ network, profile: null });
const subject = (tag = "route", network = "bsc_mainnet") => ({
  subject: tag,
  key: tag === "parameters" ? null : external(network),
});
const fixture = () => ({
  kind: "SccpRouteGovernance",
  payload: {
    proposal: {
      network_id: networkId,
      base_revisions: [{ subject: subject(), revision: 0 }],
      actions: [{ action: "remove_staged", payload: { network: external(), revision: 1 } }],
    },
  },
});

test("SCCP proposals preserve canonical subjects and exact revision boundaries", () => {
  for (const revision of [0, Number.MAX_SAFE_INTEGER]) {
    const value = fixture();
    value.payload.proposal.base_revisions[0].revision = revision;
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
});

for (const [name, mutate] of [
  ["unknown action", (p) => { p.actions[0].action = "unrecognized_action"; }],
  ["empty base revisions", (p) => { p.base_revisions = []; }],
  ["unsafe revision", (p) => { p.base_revisions[0].revision = Number.MAX_SAFE_INTEGER + 1; }],
  ["unknown subject", (p) => { p.base_revisions[0].subject.subject = "unrecognized_subject"; }],
  ["mismatched subject", (p) => { p.base_revisions[0].subject = subject("light_client"); }],
  ["duplicate subject", (p) => { p.base_revisions.push(structuredClone(p.base_revisions[0])); }],
  ["unsupported network", (p) => { p.actions[0].payload.network = external("sora_taira"); }],
  ["retired network", (p) => { p.actions[0].payload.network = external("tron_mainnet"); }],
  ["non-null network profile", (p) => { p.base_revisions[0].subject.key.profile = {}; }],
  ["missing network profile", (p) => { delete p.actions[0].payload.network.profile; }],
  ["extra network field", (p) => { p.base_revisions[0].subject.key.extra = 1; }],
  ["extra subject field", (p) => { p.base_revisions[0].subject.extra = 1; }],
  ["extra revision field", (p) => { p.base_revisions[0].extra = 1; }],
  ["missing revision", (p) => { delete p.base_revisions[0].revision; }],
  ["string revision", (p) => { p.base_revisions[0].revision = "1"; }],
  ["negative revision", (p) => { p.base_revisions[0].revision = -1; }],
  ["fractional revision", (p) => { p.base_revisions[0].revision = 0.5; }],
  ["null action payload", (p) => { p.actions[0].payload = null; }],
  ["array action payload", (p) => { p.actions[0].payload = []; }],
  ["extra action field", (p) => { p.actions[0].extra = 1; }],
  ["inherited action tag", (p) => { p.actions[0].action = "constructor"; }],
  ["empty actions", (p) => { p.actions = []; }],
  ["too many actions", (p) => { p.actions = Array(17).fill(p.actions[0]); }],
  ["too many base revisions", (p) => { p.base_revisions = Array(17).fill(p.base_revisions[0]); }],
  ["sparse actions", (p) => { p.actions = Array(1); }],
  ["sparse base revisions", (p) => { p.base_revisions = Array(1); }],
]) {
  test(`SCCP proposals reject ${name}`, () => {
    const value = fixture();
    assert.doesNotThrow(() => normalizeGovernanceProposalWireV1(value));
    mutate(value.payload.proposal);
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  });
}

test("SCCP bases follow Rust enum order while action order and repeated subjects are preserved", () => {
  const value = fixture();
  const proposal = value.payload.proposal;
  proposal.actions = [
    { action: "freeze_light_client", payload: { network: external("ton_mainnet") } },
    { action: "set_taira_paused", payload: { network: external(), paused: true } },
    { action: "set_taira_paused", payload: { network: external("ethereum_mainnet"), paused: false } },
    { action: "set_taira_paused", payload: { network: external(), paused: false } },
  ];
  proposal.base_revisions = [
    { subject: subject("route", "ethereum_mainnet"), revision: 0 },
    { subject: subject(), revision: 1 },
    { subject: subject("light_client", "ton_mainnet"), revision: 2 },
  ];
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  proposal.base_revisions.reverse();
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /canonical order/u);
});

test("SCCP permits sixteen actions on one subject and requires the parameters unit key", () => {
  const value = fixture();
  value.payload.proposal.actions = Array(16).fill(value.payload.proposal.actions[0]);
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  // Only the envelope is under test here; full parameter-body validation remains with Torii.
  value.payload.proposal.actions = [{ action: "set_parameters", payload: { next: {} } }];
  value.payload.proposal.base_revisions = [{ subject: subject("parameters"), revision: 0 }];
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  value.payload.proposal.base_revisions[0].subject.key = external();
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /key must be null/u);
});

// Expected subjects come from SccpGovernanceActionV1::subject(), independently of
// the JavaScript normalizer's mapping. Payload bodies are outside this test's scope.
for (const [action, tag] of [
  ["register_route", "route"],
  ["activate_revision", "route"],
  ["switch_revision", "route"],
  ["deactivate_outbound", "route"],
  ["retire_revision", "route"],
  ["remove_staged", "route"],
  ["release_stranded", "route"],
  ["set_taira_paused", "route"],
  ["set_destination_paused", "route_control"],
  ["initialize_light_client", "light_client"],
  ["install_trusted_checkpoint", "light_client"],
  ["freeze_light_client", "light_client"],
  ["activate_light_client_profile", "light_client"],
  ["set_parameters", "parameters"],
  ["attest_deployment_progress", "forgery_hold"],
  ["quarantine_revision", "forgery_hold"],
  ["open_reconciliation", "forgery_hold"],
  ["set_fast_pause_suspended", "fast_pause_control"],
]) {
  test(`SCCP ${action} requires the ${tag} subject`, () => {
    const value = fixture();
    const proposal = value.payload.proposal;
    proposal.actions = [{ action, payload: tag === "parameters" ? { next: {} } : {
      network: external(),
    } }];
    proposal.base_revisions = [{ subject: subject(tag), revision: 0 }];
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    proposal.base_revisions[0].subject = subject(tag === "route" ? "route_control" : "route");
    assert.throws(() => normalizeGovernanceProposalWireV1(value), /action subjects/u);
  });
}

test("SCCP permits sixteen distinct subjects only in canonical order", () => {
  const value = fixture();
  const proposal = value.payload.proposal;
  proposal.actions = [];
  proposal.base_revisions = [];
  for (const [tag, action] of [
    ["route", "set_taira_paused"],
    ["route_control", "set_destination_paused"],
    ["light_client", "freeze_light_client"],
    ["parameters", "set_parameters"],
    ["forgery_hold", "quarantine_revision"],
    ["fast_pause_control", "set_fast_pause_suspended"],
  ]) {
    for (const network of tag === "parameters" ? [null] : [
      "ethereum_mainnet", "bsc_mainnet", "ton_mainnet",
    ]) {
      proposal.actions.push({ action, payload: network === null ? { next: {} } : {
        network: external(network),
      } });
      proposal.base_revisions.push({ subject: subject(tag, network), revision: 0 });
    }
  }
  proposal.actions.reverse();
  assert.equal(proposal.base_revisions.length, 16);
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  [proposal.base_revisions[9], proposal.base_revisions[10]] = [
    proposal.base_revisions[10], proposal.base_revisions[9],
  ];
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /canonical order/u);
});

const shared = JSON.parse(readFileSync(
  new URL("../../../fixtures/sccp/governance_v1.json", import.meta.url), "utf8",
));
for (const row of shared.rows) {
  test(`shared SCCP governance subject fixture: ${row.label}`, () => {
    const proposal = {
      network_id: NetworkId.fromBytes(Buffer.from(row.input.network_id.slice(2), "hex")).toString(),
      base_revisions: row.input.base_revisions.map((entry) => ({
        subject: subject(entry.subject.kind, entry.subject.network?.replaceAll("-", "_")),
        revision: entry.revision,
      })),
      actions: row.input.actions.map(({ kind, network, ...payload }) => ({
        action: kind,
        payload: { ...payload, network: external(network.replaceAll("-", "_")) },
      })),
    };
    const value = { kind: "SccpRouteGovernance", payload: { proposal } };
    if (row.expected.static_outcome === "Accepted") {
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    } else {
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}
