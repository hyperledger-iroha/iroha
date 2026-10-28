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
  // Only the envelope is under test here; full parameter-body validation remains with Torii.
  value.payload.proposal.actions = [{ action: "set_parameters", payload: { next: {} } }];
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
      return { next: {} };
    default:
      // The complex action bodies remain outside this normalizer's validation scope.
      return key;
  }
}

function actionFixture(action, payload = actionPayload(action)) {
  const tag = actionSubjects.find(([name]) => name === action)[1];
  const value = fixture();
  value.payload.proposal.actions = [{ action, payload }];
  value.payload.proposal.base_revisions = [{ subject: subject(tag), revision: 0 }];
  return value;
}

for (const [action, tag] of actionSubjects) {
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

const controlActions = actionSubjects.map(([action]) => action).filter((action) => ![
  "register_route", "release_stranded", "initialize_light_client",
  "set_parameters",
].includes(action));
for (const action of controlActions) {
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
