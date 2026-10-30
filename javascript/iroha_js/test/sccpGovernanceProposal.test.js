// SPDX-License-Identifier: Apache-2.0
// SCCP proposal envelopes, action controls and subject revisions shared by wire and Torii reads.

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
  value.payload.proposal.actions = [{ action: "set_parameters", payload: actionPayload("set_parameters") }];
  value.payload.proposal.base_revisions = [{ subject: subject("parameters"), revision: 0 }];
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  value.payload.proposal.base_revisions[0].subject.key = external();
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /key must be null/u);
});

// Expected subjects come from SccpGovernanceActionV1::subject(), independently of
// the JavaScript normalizer's mapping.
const actionSubjects = [
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
];

function actionPayload(action, network = "bsc_mainnet") {
  const key = { network: external(network) };
  switch (action) {
    case "register_route":
      return { ...key, revision: 1, deployment: deployment(network), initial_committee: {
        generation: 1, committee_root: Array(32).fill(1), start_height: 1, until_ms: 1,
      }, max_wrapped_supply: "1" };
    case "initialize_light_client":
      return { ...key, expected: { expected: "absent", detail: null },
        params: lightClientParameters(network), bootstrap: { ...key, bytes: "AA==" } };
    case "activate_revision":
    case "deactivate_outbound":
    case "retire_revision":
    case "remove_staged":
    case "quarantine_revision":
      return { ...key, revision: 1 };
    case "switch_revision":
      return { ...key, from: 1, to: 2 };
    case "set_taira_paused":
      return { ...key, paused: false };
    case "set_destination_paused":
      return { ...key, revision: 1, paused: true };
    case "activate_light_client_profile":
      return { ...key, version: 2, profile_hash: Array(32).fill(1) };
    case "install_trusted_checkpoint":
      return { ...key, checkpoint: {
        source_height: 0, block_hash: Array(32).fill(0), state_root: null,
        receipts_or_tx_root: Array(32).fill(0), source_time_ms: 0,
      } };
    case "attest_deployment_progress":
      return { ...key, revision: 1, at: { at: "frozen", detail: null } };
    case "open_reconciliation":
      return { ...key, revision: 1, window_ms: 1 };
    case "set_fast_pause_suspended":
      return { ...key, duration_ms: 0 };
    case "set_parameters":
      return { next: parameters() };
    case "freeze_light_client":
      return key;
    default:
      throw new Error(`missing SCCP action fixture: ${action}`);
  }
}

function deployment(network) {
  if (network !== "ton_mainnet") {
    return { family: "evm", deployment: { address: Array(20).fill(1), runtime_code_hash: Array(32).fill(0) } };
  }
  return { family: "ton", deployment: {
    master_account: Array(32).fill(1),
    minter_code: { hash: Array(32).fill(0), depth: 0 },
    wallet_code: { hash: Array(32).fill(0), depth: 0 },
    bucket_code: { hash: Array(32).fill(0), depth: 0 },
  } };
}

function lightClientParameters(network) {
  return {
    network: external(network),
    ws_bound_ms: network === "ton_mainnet" ? 0 : network === "bsc_mainnet" ? 432_000_000 : 1_209_600_000,
    set_retention_ms: 15_552_000_000, checkpoint_stride: network === "ton_mainnet" ? 0 : 8192,
    checkpoint_prune_after_ms: 2_592_000_000, max_updates_per_advance: 16,
    max_segment_headers: network === "ton_mainnet" ? 32 : 256,
    max_ancestry_headers: network === "ton_mainnet" ? 32 : 256,
    max_backfill_headers: 256, max_advance_bytes: 1_048_576, max_proof_bytes: 1_048_576,
  };
}

// Canonical taira_default() from sccp/params.rs; live staking/attempt bounds are not read here.
function parameters() {
  const day = 86_400_000;
  return {
    enabled: true,
    committee_validity_ms: 14 * day, committee_heartbeat_ms: day,
    forgery_evidence_grace_ms: 7 * day, destination_progress_cap_ms: 30 * day,
    outbound_ttl_ms: 7 * day, min_outbound_amount: "1000000000", inbound_self_claim_fee: "10000000",
    max_exempt_transactions_per_block: 32, fast_pause_panel_seats: 9,
    fast_pause_panel_term_ms: 7 * day, fast_pause_seat_accept_window_ms: 43_200_000,
    fast_pause_endorse_window_ms: 1_800_000, fast_pause_renew_lead_ms: day,
    fast_pause_hold_ms: 10 * day,
  };
}

function actionFixture(action, payload = actionPayload(action)) {
  const tag = actionSubjects.find(([name]) => name === action)[1];
  const value = fixture();
  value.payload.proposal.actions = [{ action, payload }];
  value.payload.proposal.base_revisions = [{ subject: subject(tag), revision: 0 }];
  return value;
}

// The release_stranded counterpart uses the mandatory native account codec in
// sccpGovernanceRelease.test.js; it is part of the ordinary unit profile.
for (const [action, tag] of actionSubjects.filter(([name]) => name !== "release_stranded")) {
  test(`SCCP ${action} requires the ${tag} subject`, () => {
    const value = fixture();
    const proposal = value.payload.proposal;
    proposal.actions = [{ action, payload: actionPayload(action) }];
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
      proposal.actions.push({ action, payload: actionPayload(action, network) });
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

const validatedActions = actionSubjects.map(([action]) => action).filter((action) => action !== "release_stranded");
for (const action of validatedActions) {
  test(`SCCP ${action} rejects missing and unknown action fields`, () => {
    const value = actionFixture(action);
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    for (const field of Object.keys(value.payload.proposal.actions[0].payload)) {
      const missing = structuredClone(value);
      delete missing.payload.proposal.actions[0].payload[field];
      assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError, field);
    }
    value.payload.proposal.actions[0].payload.extra = 1;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  });
}

for (const [action, field] of [
  ["register_route", "revision"],
  ["activate_revision", "revision"], ["deactivate_outbound", "revision"],
  ["retire_revision", "revision"], ["remove_staged", "revision"],
  ["quarantine_revision", "revision"], ["set_destination_paused", "revision"],
  ["attest_deployment_progress", "revision"], ["open_reconciliation", "revision"],
  ["switch_revision", "from"], ["switch_revision", "to"],
  ["activate_light_client_profile", "version"],
]) {
  test(`SCCP ${action}.${field} requires a nonzero u32`, () => {
    for (const bad of [0, -1, 0.5, "1", true, null, 2 ** 32, Number.MAX_SAFE_INTEGER]) {
      const value = actionFixture(action);
      value.payload.proposal.actions[0].payload[field] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError, String(bad));
    }
    const maximum = actionFixture(action);
    maximum.payload.proposal.actions[0].payload[field] = 0xffff_ffff;
    assert.deepEqual(normalizeGovernanceProposalWireV1(maximum), maximum);
  });
}

test("SCCP revision switches require distinct revisions", () => {
  const value = actionFixture("switch_revision");
  value.payload.proposal.actions[0].payload.to = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /distinct/u);
});

for (const action of ["set_taira_paused", "set_destination_paused"]) {
  test(`SCCP ${action} requires an actual boolean`, () => {
    for (const paused of [true, false, 0, 1, "false", null]) {
      const value = actionFixture(action);
      value.payload.proposal.actions[0].payload.paused = paused;
      if (typeof paused === "boolean") {
        assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
      } else {
        assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
      }
    }
  });
}

for (const [action, field, minimum, maximum] of [
  ["open_reconciliation", "window_ms", 1, Number.MAX_SAFE_INTEGER],
  ["set_fast_pause_suspended", "duration_ms", 0, 3 * 2_592_000_000],
]) {
  test(`SCCP ${action}.${field} obeys its exact bounds`, () => {
    for (const duration of [minimum, maximum, minimum - 1, maximum + 1, 0.5, "1", null]) {
      const value = actionFixture(action);
      value.payload.proposal.actions[0].payload[field] = duration;
      if (duration === minimum || duration === maximum) {
        assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
      } else {
        assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
      }
    }
  });
}

test("SCCP profile activation cannot reactivate the genesis version", () => {
  const value = actionFixture("activate_light_client_profile");
  value.payload.proposal.actions[0].payload.version = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /at least 2/u);
});

test("SCCP progress attestations use exact frozen or generation records", () => {
  const generation = { at: "generation", detail: { generation: 1, committee_root: Array(32).fill(255) } };
  for (const at of [generation, { at: "frozen", detail: null }]) {
    const value = actionFixture("attest_deployment_progress");
    value.payload.proposal.actions[0].payload.at = at;
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
  for (const at of [
    { at: "frozen" }, { at: "frozen", detail: {} }, { at: "unknown", detail: null },
    { ...generation, extra: 1 }, { at: "generation", detail: null },
    { at: "generation", detail: { generation: 1 } },
    { at: "generation", detail: { ...generation.detail, extra: 1 } },
  ]) {
    const value = actionFixture("attest_deployment_progress");
    value.payload.proposal.actions[0].payload.at = at;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
  for (const number of [1, Number.MAX_SAFE_INTEGER, 0, -1, 0.5, "1", Number.MAX_SAFE_INTEGER + 1]) {
    const value = actionFixture("attest_deployment_progress");
    value.payload.proposal.actions[0].payload.at = {
      at: "generation", detail: { ...generation.detail, generation: number },
    };
    if (number === 1 || number === Number.MAX_SAFE_INTEGER) {
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    } else {
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  }
});

for (const action of ["activate_light_client_profile", "attest_deployment_progress"]) {
  test(`SCCP ${action} requires a nonzero dense 32-byte hash`, () => {
    const sparse = Array(32);
    sparse[0] = 1;
    for (const hash of [
      Array(32).fill(0), Array(31).fill(1), Array(33).fill(1),
      Array(32).fill(-1), Array(32).fill(256), Array(32).fill(0.5),
      Array(32).fill("1"), sparse, new Uint8Array(32).fill(1), "01".repeat(32),
    ]) {
      const value = actionFixture(action);
      const payload = value.payload.proposal.actions[0].payload;
      if (action === "attest_deployment_progress") {
        payload.at = { at: "generation", detail: { generation: 1, committee_root: hash } };
      } else {
        payload.profile_hash = hash;
      }
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

test("SCCP trusted checkpoints support EVM networks and exact u64 bounds", () => {
  for (const network of ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]) {
    const value = actionFixture("install_trusted_checkpoint", actionPayload("install_trusted_checkpoint", network));
    value.payload.proposal.base_revisions[0].subject = subject("light_client", network);
    if (network === "ton_mainnet") {
      assert.throws(() => normalizeGovernanceProposalWireV1(value), /TON/u);
    } else {
      for (const number of [0, Number.MAX_SAFE_INTEGER]) {
        const checkpoint = value.payload.proposal.actions[0].payload.checkpoint;
        checkpoint.source_height = number;
        checkpoint.source_time_ms = number;
        assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
      }
    }
  }
});

test("SCCP trusted checkpoints reject missing and unknown nested fields", () => {
  const value = actionFixture("install_trusted_checkpoint");
  for (const field of Object.keys(value.payload.proposal.actions[0].payload.checkpoint)) {
    const missing = structuredClone(value);
    delete missing.payload.proposal.actions[0].payload.checkpoint[field];
    assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError, field);
  }
  value.payload.proposal.actions[0].payload.checkpoint.extra = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
});

for (const field of ["source_height", "source_time_ms"]) {
  test(`SCCP checkpoint ${field} rejects lossy and non-integer inputs`, () => {
    for (const bad of [-1, 0.5, "1", null, Number.MAX_SAFE_INTEGER + 1]) {
      const value = actionFixture("install_trusted_checkpoint");
      value.payload.proposal.actions[0].payload.checkpoint[field] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

for (const field of ["block_hash", "state_root", "receipts_or_tx_root"]) {
  test(`SCCP checkpoint ${field} requires dense bytes without inventing nonzero constraints`, () => {
    const value = actionFixture("install_trusted_checkpoint");
    const checkpoint = value.payload.proposal.actions[0].payload.checkpoint;
    for (const hash of [Array(32).fill(0), Array(32).fill(255)]) {
      checkpoint[field] = hash;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const hash of [Array(31).fill(0), Array(32), Array(32).fill(256), "00".repeat(32)]) {
      checkpoint[field] = hash;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
    checkpoint[field] = null;
    if (field === "state_root") {
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    } else {
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

test("SCCP parameters require the complete exact value and a boolean enable flag", () => {
  const value = actionFixture("set_parameters");
  assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  for (const field of Object.keys(parameters())) {
    const missing = structuredClone(value);
    delete missing.payload.proposal.actions[0].payload.next[field];
    assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError, field);
  }
  const extra = structuredClone(value);
  extra.payload.proposal.actions[0].payload.next.extra = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(extra), TypeError);
  for (const enabled of [false, true, 0, 1, "true", null]) {
    value.payload.proposal.actions[0].payload.next.enabled = enabled;
    if (typeof enabled === "boolean") {
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    } else {
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  }
});

for (const field of Object.keys(parameters()).filter((name) => typeof parameters()[name] === "number")) {
  test(`SCCP parameter ${field} requires an exact unsigned JSON integer`, () => {
    for (const bad of [-1, 0.5, "1", true, null, Number.MAX_SAFE_INTEGER + 1]) {
      const value = actionFixture("set_parameters");
      value.payload.proposal.actions[0].payload.next[field] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

for (const [field, rejected] of [
  ["committee_validity_ms", [2_592_000_001]],
  ["committee_heartbeat_ms", [3_599_999]],
  ["forgery_evidence_grace_ms", [86_399_999, 2_592_000_001]],
  ["destination_progress_cap_ms", [2_592_000_001]],
  ["outbound_ttl_ms", [86_399_999, 7_776_000_001]],
  ["max_exempt_transactions_per_block", [7, 1025]],
  ["fast_pause_panel_seats", [4, 6, 8, 10]],
  ["fast_pause_panel_term_ms", [3_599_999]],
  ["fast_pause_seat_accept_window_ms", [599_999]],
  ["fast_pause_endorse_window_ms", [299_999, 21_600_001]],
  ["fast_pause_hold_ms", [2_505_600_001]],
]) {
  test(`SCCP parameter ${field} enforces its protocol bounds`, () => {
    for (const bad of rejected) {
      const value = actionFixture("set_parameters");
      value.payload.proposal.actions[0].payload.next[field] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

for (const [field, minimum, maximum] of [
  ["min_outbound_amount", 1n, 1_000_000_000_000_000n],
  ["inbound_self_claim_fee", 0n, 10_000_000_000n],
]) {
  test(`SCCP parameter ${field} uses bounded canonical decimal strings`, () => {
    for (const amount of [minimum.toString(), maximum.toString()]) {
      const value = actionFixture("set_parameters");
      value.payload.proposal.actions[0].payload.next[field] = amount;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const bad of [
      (minimum - 1n).toString(), (maximum + 1n).toString(), "01", "+1", "1.0", "1e3", " 1",
      "9".repeat(1000), 1, 1n, null,
    ]) {
      const value = actionFixture("set_parameters");
      value.payload.proposal.actions[0].payload.next[field] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

test("SCCP parameter joint bounds include exact equality and reject the adjacent value", () => {
  for (const [updates, field, invalid] of [
    [{ committee_validity_ms: 13 * 86_400_000 }, "committee_validity_ms", 13 * 86_400_000 - 1],
    [{ fast_pause_panel_term_ms: 3_600_001, fast_pause_seat_accept_window_ms: 1_800_000 }, "fast_pause_seat_accept_window_ms", 1_800_001],
    [{ fast_pause_renew_lead_ms: 3_600_000 }, "fast_pause_renew_lead_ms", 3_599_999],
    [{ fast_pause_renew_lead_ms: 5 * 86_400_000 }, "fast_pause_renew_lead_ms", 5 * 86_400_000 + 1],
    [{ fast_pause_panel_term_ms: Number.MAX_SAFE_INTEGER, fast_pause_seat_accept_window_ms: 4_503_599_627_370_495 }, "fast_pause_seat_accept_window_ms", 4_503_599_627_370_496],
  ]) {
    const value = actionFixture("set_parameters");
    const next = value.payload.proposal.actions[0].payload.next;
    Object.assign(next, updates);
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    next[field] = invalid;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

test("SCCP valid parameter endpoints remain independent of live enactment bounds", () => {
  for (const updates of [
    { committee_validity_ms: 2_592_000_000, forgery_evidence_grace_ms: 2_592_000_000 },
    { destination_progress_cap_ms: 0, inbound_self_claim_fee: "0", max_exempt_transactions_per_block: 8 },
    { destination_progress_cap_ms: 2_592_000_000, outbound_ttl_ms: 7_776_000_000, max_exempt_transactions_per_block: 1024 },
    { outbound_ttl_ms: 86_400_000, forgery_evidence_grace_ms: 86_400_000 },
    { fast_pause_panel_seats: 5 }, { fast_pause_panel_seats: 7 }, { fast_pause_panel_seats: 9 },
    { committee_heartbeat_ms: 3_600_000, fast_pause_panel_term_ms: 3_600_000, fast_pause_seat_accept_window_ms: 600_000 },
    { fast_pause_endorse_window_ms: 300_000, fast_pause_renew_lead_ms: 600_000, fast_pause_hold_ms: 1_200_000 },
    { fast_pause_endorse_window_ms: 21_600_000 },
    { committee_validity_ms: 2_592_000_000, committee_heartbeat_ms: 3_600_000, fast_pause_hold_ms: 2_498_400_000 },
  ]) {
    const value = actionFixture("set_parameters");
    Object.assign(value.payload.proposal.actions[0].payload.next, updates);
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
});

test("SCCP route registration binds each network to its deployment and exact supply cap", () => {
  for (const network of ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]) {
    const value = actionFixture("register_route", actionPayload("register_route", network));
    value.payload.proposal.base_revisions[0].subject = subject("route", network);
    const maximum = (1n << (network === "ton_mainnet" ? 96n : 128n)) - 1n;
    const payload = value.payload.proposal.actions[0].payload;
    for (const amount of ["1", maximum.toString()]) {
      payload.max_wrapped_supply = amount;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const bad of ["0", (maximum + 1n).toString(), "01", "-1", "1e3", 1, 1n, null, "9".repeat(1000)]) {
      payload.max_wrapped_supply = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
    payload.max_wrapped_supply = "1";
    payload.deployment = deployment(network === "ton_mainnet" ? "bsc_mainnet" : "ton_mainnet");
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

for (const field of ["generation", "start_height", "until_ms"]) {
  test(`SCCP committee pin ${field} requires a nonzero exact u64`, () => {
    const value = actionFixture("register_route");
    const pin = value.payload.proposal.actions[0].payload.initial_committee;
    for (const number of [1, Number.MAX_SAFE_INTEGER]) {
      pin[field] = number;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const number of [0, -1, 0.5, "1", null, Number.MAX_SAFE_INTEGER + 1]) {
      pin[field] = number;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

test("SCCP committee pins require exact fields and a nonzero dense root", () => {
  const value = actionFixture("register_route");
  const pin = value.payload.proposal.actions[0].payload.initial_committee;
  for (const field of Object.keys(pin)) {
    const missing = structuredClone(value);
    delete missing.payload.proposal.actions[0].payload.initial_committee[field];
    assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError, field);
  }
  pin.extra = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  delete pin.extra;
  for (const hash of [Array(32).fill(0), Array(31).fill(1), Array(32), Array(32).fill(256), "01".repeat(32)]) {
    pin.committee_root = hash;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

test("SCCP deployment records reject unknown, missing and malformed fields", () => {
  for (const network of ["bsc_mainnet", "ton_mainnet"]) {
    const value = actionFixture("register_route", actionPayload("register_route", network));
    value.payload.proposal.base_revisions[0].subject = subject("route", network);
    const paths = [[], ["deployment"]];
    if (network === "ton_mainnet") paths.push(...["minter_code", "wallet_code", "bucket_code"].map((name) => ["deployment", name]));
    for (const path of paths) {
      const recordAt = (copy) => path.reduce((record, field) => record[field], copy.payload.proposal.actions[0].payload.deployment);
      for (const field of Object.keys(recordAt(value))) {
        const missing = structuredClone(value);
        delete recordAt(missing)[field];
        assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError);
      }
      const extra = structuredClone(value);
      recordAt(extra).extra = 1;
      assert.throws(() => normalizeGovernanceProposalWireV1(extra), TypeError);
    }
    const record = value.payload.proposal.actions[0].payload.deployment;
    const address = network === "ton_mainnet" ? "master_account" : "address";
    const width = network === "ton_mainnet" ? 32 : 20;
    for (const bad of [Array(width).fill(0), Array(width - 1).fill(1), Array(width), Array(width).fill(256)]) {
      record.deployment[address] = bad;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
    record.family = "tron";
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

test("SCCP deployment code hashes may be zero while TON code depths must fit u16", () => {
  for (const network of ["bsc_mainnet", "ton_mainnet"]) {
    const value = actionFixture("register_route", actionPayload("register_route", network));
    value.payload.proposal.base_revisions[0].subject = subject("route", network);
    const record = value.payload.proposal.actions[0].payload.deployment.deployment;
    const codes = network === "ton_mainnet" ? [record.minter_code, record.wallet_code, record.bucket_code] : [record];
    for (const code of codes) {
      const hashField = network === "ton_mainnet" ? "hash" : "runtime_code_hash";
      for (const hash of [Array(32).fill(0), Array(32).fill(255)]) {
        code[hashField] = hash;
        assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
      }
      for (const hash of [Array(31).fill(1), Array(32), Array(32).fill(-1), "00".repeat(32)]) {
        code[hashField] = hash;
        assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
      }
      code[hashField] = Array(32).fill(0);
      if (network === "ton_mainnet") {
        for (const depth of [0, 65535, -1, 65536, "1", 0.5]) {
          code.depth = depth;
          if (depth === 0 || depth === 65535) assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
          else assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
        }
        code.depth = 0;
      }
    }
  }
});

test("SCCP registrations require distinct destination words even across networks and families", () => {
  const value = actionFixture("register_route", actionPayload("register_route", "ethereum_mainnet"));
  const proposal = value.payload.proposal;
  for (const network of ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]) {
    const second = actionPayload("register_route", network);
    if (network === "ton_mainnet") second.deployment.deployment.master_account = [...Array(12).fill(0), ...Array(20).fill(1)];
    proposal.actions = [proposal.actions[0], { action: "register_route", payload: second }];
    proposal.base_revisions = [{ subject: subject("route", "ethereum_mainnet"), revision: 0 }];
    if (network !== "ethereum_mainnet") proposal.base_revisions.push({ subject: subject("route", network), revision: 0 });
    assert.throws(() => normalizeGovernanceProposalWireV1(value), /destination/u);
    second.deployment.deployment[network === "ton_mainnet" ? "master_account" : "address"][19] = 2;
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
});

test("SCCP initialization preserves both expected states on each external network", () => {
  for (const network of ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]) {
    const value = actionFixture("initialize_light_client", actionPayload("initialize_light_client", network));
    value.payload.proposal.base_revisions[0].subject = subject("light_client", network);
    for (const expected of ["absent", "unusable"]) {
      value.payload.proposal.actions[0].payload.expected.expected = expected;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
  }
});

test("SCCP light-client initialization requires exact nested records and matching networks", () => {
  const value = actionFixture("initialize_light_client");
  for (const name of ["expected", "params", "bootstrap"]) {
    for (const field of Object.keys(value.payload.proposal.actions[0].payload[name])) {
      const missing = structuredClone(value);
      delete missing.payload.proposal.actions[0].payload[name][field];
      assert.throws(() => normalizeGovernanceProposalWireV1(missing), TypeError);
    }
    const extra = structuredClone(value);
    extra.payload.proposal.actions[0].payload[name].extra = 1;
    assert.throws(() => normalizeGovernanceProposalWireV1(extra), TypeError);
  }
  for (const name of ["params", "bootstrap"]) {
    const mismatch = structuredClone(value);
    mismatch.payload.proposal.actions[0].payload[name].network = external("ethereum_mainnet");
    assert.throws(() => normalizeGovernanceProposalWireV1(mismatch), TypeError);
  }
  for (const expected of [{ expected: "absent", detail: {} }, { expected: "unknown", detail: null }, "absent"]) {
    value.payload.proposal.actions[0].payload.expected = expected;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

for (const field of ["ws_bound_ms", "set_retention_ms", "checkpoint_stride", "checkpoint_prune_after_ms"]) {
  test(`SCCP light-client ${field} is a positive JSON-safe u64 on EVM networks`, () => {
    const value = actionFixture("initialize_light_client");
    const params = value.payload.proposal.actions[0].payload.params;
    for (const number of [1, Number.MAX_SAFE_INTEGER]) {
      params[field] = number;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const number of [0, -1, 0.5, "1", null, Number.MAX_SAFE_INTEGER + 1]) {
      params[field] = number;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

for (const field of ["max_updates_per_advance", "max_segment_headers", "max_ancestry_headers", "max_backfill_headers", "max_advance_bytes", "max_proof_bytes"]) {
  test(`SCCP light-client ${field} is a positive u32`, () => {
    const value = actionFixture("initialize_light_client");
    const params = value.payload.proposal.actions[0].payload.params;
    for (const number of [1, 0xffff_ffff]) {
      params[field] = number;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
    for (const number of [0, -1, 0.5, "1", null, 2 ** 32]) {
      params[field] = number;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
  });
}

test("SCCP TON light-client derived fields must be exactly zero", () => {
  const value = actionFixture("initialize_light_client", actionPayload("initialize_light_client", "ton_mainnet"));
  value.payload.proposal.base_revisions[0].subject = subject("light_client", "ton_mainnet");
  for (const field of ["ws_bound_ms", "checkpoint_stride"]) {
    for (const number of [1, Number.MAX_SAFE_INTEGER, -1, 0.5, "0", null]) {
      value.payload.proposal.actions[0].payload.params[field] = number;
      assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
    }
    value.payload.proposal.actions[0].payload.params[field] = 0;
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
});

test("SCCP bootstrap bytes require canonical base64 and the exact 1 MiB limit", () => {
  const value = actionFixture("initialize_light_client");
  const bootstrap = value.payload.proposal.actions[0].payload.bootstrap;
  for (const length of [1, 1_048_575, 1_048_576]) {
    bootstrap.bytes = Buffer.alloc(length).toString("base64");
    assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
  }
  for (const bytes of ["", "AA", "AA=", "AB==", "AA==\n", "_w==", [0], null, Buffer.alloc(1_048_577).toString("base64")]) {
    bootstrap.bytes = bytes;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
});

test("SCCP stranded releases reject malformed fields before invoking the native account codec", () => {
  const release = () => actionFixture("release_stranded", {
    network: external(), amount: "1", recipient: "native-codec-required", memo: "",
  });
  for (const field of ["network", "amount", "recipient", "memo"]) {
    const value = release();
    delete value.payload.proposal.actions[0].payload[field];
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
  const extra = release();
  extra.payload.proposal.actions[0].payload.extra = 1;
  assert.throws(() => normalizeGovernanceProposalWireV1(extra), TypeError);
  for (const amount of ["0", "-1", "01", "1.0", "1e3", (1n << 128n).toString(), "9".repeat(1000), 1, null]) {
    const value = release();
    value.payload.proposal.actions[0].payload.amount = amount;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
  for (const memo of [null, 1, "a".repeat(257), "é".repeat(129), "🦊".repeat(65), "\ud800", "\udc00"]) {
    const value = release();
    value.payload.proposal.actions[0].payload.memo = memo;
    assert.throws(() => normalizeGovernanceProposalWireV1(value), TypeError);
  }
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
