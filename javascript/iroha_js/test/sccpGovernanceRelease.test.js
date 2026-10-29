// SPDX-License-Identifier: Apache-2.0
// SCCP release validation uses the genuine native account codec and its provenance gate.

import assert from "node:assert/strict";
import test from "node:test";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { normalizeGovernanceProposalWireV1 } from "../src/governanceProposalV1.js";

test("SCCP stranded releases preserve canonical recipients, u128 amounts and UTF-8 memo boundaries", () => {
  const recipient = AccountAddress.fromAccount({
    publicKey: Buffer.from("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "hex"),
  }).toI105();
  const network = { network: "bsc_mainnet", profile: null };
  const value = { kind: "SccpRouteGovernance", payload: { proposal: {
    network_id: NetworkId.fromBytes(Uint8Array.from([...Array(31).fill(0), 1])).toString(),
    base_revisions: [{ subject: { subject: "route", key: network }, revision: 0 }],
    actions: [{ action: "release_stranded", payload: { network, amount: "1", recipient, memo: "" } }],
  } } };
  const payload = value.payload.proposal.actions[0].payload;
  for (const amount of ["1", ((1n << 128n) - 1n).toString()]) {
    payload.amount = amount;
    for (const memo of ["", "a".repeat(256), "é".repeat(128), "🦊".repeat(64)]) {
      payload.memo = memo;
      assert.deepEqual(normalizeGovernanceProposalWireV1(value), value);
    }
  }
  value.payload.proposal.base_revisions[0].subject.subject = "route_control";
  assert.throws(() => normalizeGovernanceProposalWireV1(value), /action subjects/u);
  value.payload.proposal.base_revisions[0].subject.subject = "route";
  for (const invalid of ["", "invalid-account", `${recipient} `, null]) {
    payload.recipient = invalid;
    assert.throws(() => normalizeGovernanceProposalWireV1(value));
  }
});
