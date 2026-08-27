// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { sha256 } from "@noble/hashes/sha2";

import { crc64Xz } from "../src/crc64Xz.js";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  NetworkId,
  PARLIAMENT_API_VERSION_V1,
  PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
  PARLIAMENT_ATTEMPT_DRAFT_PATH_V1,
  PARLIAMENT_ATTEMPT_READ_PATH_V1,
  PARLIAMENT_ATTEMPT_STATE_MAX_BYTES_V1,
  PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1,
  PARLIAMENT_BODY_STATE_FIELDS_V1,
  PARLIAMENT_CERTIFICATE_BODY_BINDING_FIELDS_V1,
  PARLIAMENT_NO_RESULT_KINDS_V1,
  PARLIAMENT_PUBLIC_FINDING_CERTIFICATE_FIELDS_V1,
  PARLIAMENT_PUBLIC_TRANSITIONS_V1,
  PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_READ_PATH_V1,
  PARLIAMENT_TIMED_OVN_CASTING_PROOF_PATH_V1,
  PARLIAMENT_TIMED_OVN_BALLOT_RECORD_BYTES_V1,
  PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1,
  PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1,
  PARLIAMENT_TLE_PARTIAL_RELEASE_PATH_V1,
  PARLIAMENT_TLE_RELEASE_CONTEXT_READ_PATH_V1,
  PARLIAMENT_TRANSITION_DRAFT_PATH_V1,
  PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
  ToriiClient,
  buildParliamentAttemptDraftRequestV1,
  buildParliamentTransitionDraftRequestV1,
  normalizeParliamentAttemptDraftResponseV1,
  normalizeParliamentAttemptReadResponseV1,
  normalizeParliamentTimedOvnCastingContextResponseV1,
  normalizeParliamentTlePartialReleaseShareV1,
  normalizeParliamentTleReleaseContextResponseV1,
  normalizeParliamentTransitionDraftResponseV1,
  parliamentAttemptReadPathV1,
  parliamentTimedOvnCastingContextReadPathV1,
  parliamentTimedOvnCastingProofPathV1,
  parliamentTlePartialReleasePathV1,
  parliamentTleReleaseContextReadPathV1,
} from "../src/index.js";

const ATTEMPT_ID = "ab".repeat(32);
const PROPOSAL_ID = "cd".repeat(32);
const ROOT = Object.freeze(Array(32).fill(0x55));
const ID = (byte) => byte.toString(16).padStart(2, "0").repeat(32);
const CONTRACT_ADDRESS =
  "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
const RUST_INVALID_ZERO_KEY_ACCOUNT =
  ["sora", "uﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6"].join("");
const NETWORK_ID = NetworkId.fromBytes(Uint8Array.from([
  ...Array(31).fill(0),
  1,
])).toString();
const ACCOUNTS = Object.freeze([
  "treasury",
  "vault",
  "recipient-a",
  "recipient-b",
  "recipient-c",
  "recipient-d",
].map(fixtureAccountId));
const fixturePath = fileURLToPath(
  new URL("../../../fixtures/governance/parliament_api_v1.json", import.meta.url),
);

test("shared Parliament fixture pins routes, all transition inventories, and certificate supporters", () => {
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  assert.equal(fixture.schema, "iroha.governance.parliament.api_fixture.v1");
  assert.equal(fixture.api_version, PARLIAMENT_API_VERSION_V1);
  assert.deepEqual(fixture.routes, {
    attempt_draft: PARLIAMENT_ATTEMPT_DRAFT_PATH_V1,
    attempt_read: PARLIAMENT_ATTEMPT_READ_PATH_V1,
    timed_ovn_casting_context_read: PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_READ_PATH_V1,
    timed_ovn_casting_proof: PARLIAMENT_TIMED_OVN_CASTING_PROOF_PATH_V1,
    tle_release_context_read: PARLIAMENT_TLE_RELEASE_CONTEXT_READ_PATH_V1,
    tle_partial_release: PARLIAMENT_TLE_PARTIAL_RELEASE_PATH_V1,
    transition_draft: PARLIAMENT_TRANSITION_DRAFT_PATH_V1,
  });
  assert.deepEqual(fixture.wire_ids, {
    attempt_create: PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
    transition_submit: PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
  });
  assert.equal(fixture.limits.attempt_state_bytes, PARLIAMENT_ATTEMPT_STATE_MAX_BYTES_V1);
  assert.equal(
    fixture.limits.timed_ovn_ballot_chunk_max_records,
    PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1,
  );
  assert.equal(
    fixture.limits.timed_ovn_corpus_entries,
    PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1,
  );
  assert.equal(fixture.public_transitions.length, PARLIAMENT_PUBLIC_TRANSITIONS_V1.length);
  for (const [index, layout] of PARLIAMENT_PUBLIC_TRANSITIONS_V1.entries()) {
    const entry = fixture.public_transitions[index];
    assert.deepEqual(
      [entry.norito_index, entry.json_tag, entry.json_payload === "required", entry.event_kind_index],
      [layout.noritoIndex, layout.jsonTag, layout.jsonPayloadRequired, layout.eventKindIndex],
    );
    buildParliamentTransitionDraftRequestV1(
      ATTEMPT_ID,
      transitionFixture(layout.jsonTag),
    );
  }
  assert.equal(
    fixture.automatic_execution_outcomes.length,
    PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1.length,
  );
  for (const [index, outcome] of PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1.entries()) {
    const entry = fixture.automatic_execution_outcomes[index];
    assert.deepEqual(
      [entry.norito_index, entry.json_tag, entry.event_kind, entry.event_kind_index],
      [outcome.noritoIndex, outcome.jsonTag, outcome.eventKind, outcome.eventKindIndex],
    );
    assert.throws(
      () => buildParliamentTransitionDraftRequestV1(
        ATTEMPT_ID,
        outcome.jsonPayloadRequired
          ? { transition: outcome.eventKind, payload: {} }
          : { transition: outcome.eventKind },
      ),
      /unknown, removed, or automatic-only/u,
    );
  }
  const binding = fixture.certificate_body_binding;
  assert.deepEqual(binding.norito_field_order, PARLIAMENT_CERTIFICATE_BODY_BINDING_FIELDS_V1);
  assert.deepEqual(
    binding.public_nonbinding_body.public_finding_norito_field_order,
    PARLIAMENT_PUBLIC_FINDING_CERTIFICATE_FIELDS_V1,
  );
  assert.equal(
    binding.public_nonbinding_body.endorsing_assignments,
    "strictly increasing distinct nonzero assignment ids",
  );
  assert.equal(
    binding.public_nonbinding_body.endorsements,
    "endorsing_assignments.length == quorum",
  );
  assert.deepEqual(
    fixture.no_result_kinds.map(({ norito_index, json_tag }) => [norito_index, json_tag]),
    PARLIAMENT_NO_RESULT_KINDS_V1.map(({ noritoIndex, jsonTag }) => [noritoIndex, jsonTag]),
  );
  assert.deepEqual(
    fixture.attempt_read_body_state.json_fields,
    PARLIAMENT_BODY_STATE_FIELDS_V1,
  );
});

test("request builders expose only canonical V1 fields and reject removed aliases", () => {
  const proposal = parliamentProposalFixtures()[0];
  assert.deepEqual(
    buildParliamentAttemptDraftRequestV1(proposal, 7),
    {
      version: 1,
      proposal,
      attempt_sequence: 7,
    },
  );
  assert.equal(
    parliamentAttemptReadPathV1(ATTEMPT_ID),
    `/v1/gov/parliament/attempts/${ATTEMPT_ID}`,
  );
  assert.throws(() => parliamentAttemptReadPathV1(ATTEMPT_ID.toUpperCase()), /lowercase/u);
  assert.throws(
    () => buildParliamentTransitionDraftRequestV1(
      ATTEMPT_ID,
      { transition: "ConstructCertificate", payload: {} },
    ),
    /automatic-only/u,
  );
  assert.throws(
    () => buildParliamentTransitionDraftRequestV1(
      ATTEMPT_ID,
      { transition: "CompleteQualification", payload: {} },
    ),
    /unknown, aliased, or missing/u,
  );
});

test("timed-OVN corpus transitions preflight one through 32 records per chunk", () => {
  const record = Array(PARLIAMENT_TIMED_OVN_BALLOT_RECORD_BYTES_V1).fill(1);
  for (const count of [1, PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1]) {
    assert.doesNotThrow(() => buildParliamentTransitionDraftRequestV1(
      ATTEMPT_ID,
      {
        transition: "FreezeTimedOvnCorpus",
        payload: { ballot_attempt_id: ID(1), ballot_records: Array(count).fill(record) },
      },
    ));
  }
  for (const count of [0, PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1 + 1]) {
    assert.throws(
      () => buildParliamentTransitionDraftRequestV1(
        ATTEMPT_ID,
        {
          transition: "FreezeTimedOvnCorpus",
          payload: { ballot_attempt_id: ID(1), ballot_records: Array(count).fill(record) },
        },
      ),
      /one through 32 records/u,
    );
  }
});

test("attempt drafts admit all seven exact proposal wire variants", () => {
  const proposals = parliamentProposalFixtures();
  assert.deepEqual(
    proposals.map((proposal) => (
      buildParliamentAttemptDraftRequestV1(proposal, 0).proposal.kind
    )),
    [
      "DeployContract",
      "RuntimeUpgrade",
      "SccpRouteGovernance",
      "ValidationFeePolicy",
      "ValidationFeePayoutLifecycle",
      "MusubiRegistryGovernance",
      "SorafsProviderGovernance",
    ],
  );
  const musubi = buildParliamentAttemptDraftRequestV1(proposals[5], 1).proposal;
  assert.deepEqual(musubi.payload.value.alias, ["stable-alias"]);
  assert.deepEqual(musubi.payload.value.target.name, ["governed-package"]);
  const provider = buildParliamentAttemptDraftRequestV1(proposals[6], 1).proposal;
  assert.deepEqual(provider.payload.action.value.provider_id, [Array(32).fill(0x31)]);
});

test("Parliament declarations expose the closed wire union and tuple newtypes", () => {
  const declarations = readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
  const start = declarations.indexOf("export interface ParliamentMusubiPackageIdV1");
  const end = declarations.indexOf("export type ParliamentLifecycleTransitionV1", start);
  assert.notEqual(start, -1);
  assert.notEqual(end, -1);
  const proposalDeclarations = declarations.slice(start, end);
  assert.match(proposalDeclarations, /name: readonly \[string\];/u);
  assert.match(proposalDeclarations, /alias: readonly \[string\];/u);
  assert.match(proposalDeclarations, /reason: readonly \[string\];/u);
  assert.match(
    proposalDeclarations,
    /provider_id: readonly \[ReadonlyArray<number>\];/u,
  );
  assert.doesNotMatch(proposalDeclarations, /payload: Record<string, unknown>/u);
});

test("attempt drafts reject malformed nested fields and open proposal shapes", () => {
  const mutations = [
    (proposal) => { proposal.payload.future = null; },
    (proposal) => { proposal.payload.manifest.future = null; },
    (proposal) => { proposal.payload.anchor.future = null; },
    (proposal) => { proposal.payload.policy.future = null; },
    (proposal) => { proposal.payload.payout_binding.recipients[0].future = null; },
    (proposal) => { proposal.payload.value.target.name = "governed-package"; },
    (proposal) => { proposal.payload.action.value.provider_id = Array(32).fill(0x31); },
  ];
  for (const [index, canonical] of parliamentProposalFixtures().entries()) {
    const malformed = structuredClone(canonical);
    mutations[index](malformed);
    assert.throws(
      () => buildParliamentAttemptDraftRequestV1(malformed, 0),
      /unsupported fields|exact one-field/u,
      canonical.kind,
    );
  }
  const unknown = structuredClone(parliamentProposalFixtures()[0]);
  unknown.kind = "FutureProposal";
  assert.throws(
    () => buildParliamentAttemptDraftRequestV1(unknown, 0),
    /unsupported V1 proposal variant/u,
  );
  const wrapped = { ...parliamentProposalFixtures()[0], raw: {} };
  assert.throws(
    () => buildParliamentAttemptDraftRequestV1(wrapped, 0),
    /unsupported fields/u,
  );
  const zeroKeyOwner = structuredClone(parliamentProposalFixtures()[6]);
  zeroKeyOwner.payload.action.value.owner = RUST_INVALID_ZERO_KEY_ACCOUNT;
  assert.throws(
    () => buildParliamentAttemptDraftRequestV1(zeroKeyOwner, 0),
    /small-order|all-zero public key/u,
  );
});

test("draft responses bind exact IDs, public kind, digest, and wire IDs", () => {
  const attempt = normalizeParliamentAttemptDraftResponseV1({
    version: 1,
    proposal_content_id: PROPOSAL_ID,
    governance_attempt_id: ATTEMPT_ID,
    tx_instructions: [{
      wire_id: PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1,
      payload_hex: "0102",
    }],
  }, {
    expectedProposalContentId: PROPOSAL_ID,
    expectedGovernanceAttemptId: ATTEMPT_ID,
  });
  assert.equal(attempt.governance_attempt_id, ATTEMPT_ID);

  const transition = normalizeParliamentTransitionDraftResponseV1({
    version: 1,
    governance_attempt_id: ATTEMPT_ID,
    transition_kind: { kind: "CompleteQualification" },
    transition_digest: ROOT,
    tx_instructions: [{
      wire_id: PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
      payload_hex: "0304",
    }],
  }, {
    expectedGovernanceAttemptId: ATTEMPT_ID,
    expectedTransitionKind: "CompleteQualification",
    expectedTransitionDigest: ROOT,
  });
  assert.deepEqual(transition.transition_digest, ROOT);
  assert.throws(
    () => normalizeParliamentTransitionDraftResponseV1({
      version: 1,
      governance_attempt_id: ATTEMPT_ID,
      transition_kind: { kind: "MarkEnacted" },
      transition_digest: ROOT,
      tx_instructions: [{
        wire_id: PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1,
        payload_hex: "0304",
      }],
    }, {
      expectedGovernanceAttemptId: ATTEMPT_ID,
      expectedTransitionKind: "CompleteQualification",
      expectedTransitionDigest: ROOT,
    }),
    /automatic, unknown, or consensus-owned/u,
  );
});

test("attempt read validates NRT0 and the exact canonical public-finding supporter list", () => {
  const response = readResponse();
  const parsed = normalizeParliamentAttemptReadResponseV1(response, ATTEMPT_ID);
  assert.deepEqual(
    parsed.certificate.body_bindings[0].public_finding.endorsing_assignments,
    [ID(0x11), ID(0x12)],
  );
  const unsorted = structuredClone(response);
  unsorted.certificate.body_bindings[0].public_finding.endorsing_assignments.reverse();
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(unsorted, ATTEMPT_ID),
    /strictly increasing/u,
  );
  const alias = { ...response, statePayloadHex: response.state_payload_hex };
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(alias, ATTEMPT_ID),
    /unknown, aliased, or missing/u,
  );
  const bare = { ...response, state_payload_hex: "0102" };
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(bare, ATTEMPT_ID),
    /not an NRT0 frame|shorter than/u,
  );
  const wrongDeadline = structuredClone(response);
  wrongDeadline.body_states[0].public_finding_deadline_height += 1;
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(wrongDeadline, ATTEMPT_ID),
    /frozen schedule/u,
  );
  const aliasBodyState = structuredClone(response);
  aliasBodyState.body_states[0].noResultKind = null;
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(aliasBodyState, ATTEMPT_ID),
    /unknown, aliased, or missing/u,
  );
  const automaticAsFailure = structuredClone(response);
  automaticAsFailure.body_states[0].no_result_kind = { reason: "ExecutionFailed" };
  automaticAsFailure.body_states[0].no_result_height = 9;
  automaticAsFailure.body_states[0].status = { status: "NoResult" };
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(automaticAsFailure, ATTEMPT_ID),
    /unknown/u,
  );
});

test("attempt read accepts terminal commitment chunks only inside the frozen liveness window", () => {
  const early = hiddenBallotReadResponse(10);
  const parsed = normalizeParliamentAttemptReadResponseV1(early, ATTEMPT_ID);
  assert.equal(
    parsed.certificate.body_bindings[0].ballot.commitment_closed_at_height,
    10,
  );
  for (const invalidHeight of [8, 14]) {
    assert.throws(
      () => normalizeParliamentAttemptReadResponseV1(
        hiddenBallotReadResponse(invalidHeight),
        ATTEMPT_ID,
      ),
      /frozen ballot lifecycle/u,
    );
  }
});

test("attempt read exposes only phase-bound aggregate timed-OVN progress", () => {
  const finalized = hiddenBallotReadResponse(10);
  assert.equal(
    normalizeParliamentAttemptReadResponseV1(finalized, ATTEMPT_ID)
      .body_states[0].timed_ovn_progress.accepted_ballot_prefix_count,
    3,
  );

  const active = hiddenBallotReadResponse(10);
  active.attempt.status = { status: "Active" };
  active.certificate = null;
  active.body_states[0].status = { status: "Balloting" };
  active.body_states[0].timed_ovn_progress.status = { status: "TimedCommitment" };
  active.body_states[0].timed_ovn_progress.accepted_ballot_prefix_count = 1;
  assert.equal(
    normalizeParliamentAttemptReadResponseV1(active, ATTEMPT_ID)
      .body_states[0].timed_ovn_progress.accepted_ballot_prefix_count,
    1,
  );
  active.body_states[0].timed_ovn_progress.accepted_ballot_prefix_count = 0;
  assert.doesNotThrow(() => normalizeParliamentAttemptReadResponseV1(active, ATTEMPT_ID));

  const malformed = [
    (response) => { response.body_states[0].timed_ovn_progress.accepted_ballot_prefix_count = 3; },
    (response) => { response.body_states[0].timed_ovn_progress.frozen_survivor_count = null; },
    (response) => { response.body_states[0].timed_ovn_progress.ballot_records = []; },
  ];
  for (const mutate of malformed) {
    const response = structuredClone(active);
    mutate(response);
    assert.throws(
      () => normalizeParliamentAttemptReadResponseV1(response, ATTEMPT_ID),
      /prefix|appear together|unknown, aliased, or missing/u,
    );
  }

  const forgedBinding = hiddenBallotReadResponse(10);
  forgedBinding.body_states[0].timed_ovn_progress.ballot_attempt_id = ID(0xfe);
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(forgedBinding, ATTEMPT_ID),
    /differs from timed_ovn_progress/u,
  );
});

test("attempt read rejects forged certificate cross-bindings and lifecycle fields", () => {
  const mutations = [
    [
      (response) => { response.certificate.proposal_content_id = ID(0xee); },
      /proposal_content_id differs/u,
    ],
    [
      (response) => { response.certificate.governance_attempt_sequence = 1; },
      /governance_attempt_sequence differs/u,
    ],
    [
      (response) => { response.certificate.risk_tier = { tier: "Emergency" }; },
      /risk_tier differs/u,
    ],
    [
      (response) => { response.certificate.policy_version = 2; },
      /policy_version differs/u,
    ],
    [
      (response) => { response.certificate.enact_at_height = 8; },
      /enactment height/u,
    ],
    [
      (response) => {
        response.certificate.body_bindings[0].sortition_request.beacon_session_id = ID(0xee);
      },
      /repeated certificate bindings/u,
    ],
    [
      (response) => {
        response.certificate.body_bindings[0].sortition_request.request_height = 0;
      },
      /sortition heights/u,
    ],
    [
      (response) => {
        response.certificate.body_bindings[0].sortition_request.candidate_count = 1_001;
      },
      /candidate_count/u,
    ],
    [
      (response) => {
        response.certificate.body_bindings[0].body = "agenda-council";
        response.certificate.body_bindings[0].sortition_request.body = "agenda-council";
      },
      /differs from required_bodies/u,
    ],
  ];
  for (const [mutate, pattern] of mutations) {
    const forged = readResponse();
    mutate(forged);
    assert.throws(
      () => normalizeParliamentAttemptReadResponseV1(forged, ATTEMPT_ID),
      pattern,
    );
  }

  const reorderedRequired = readResponse();
  reorderedRequired.required_bodies = [
    { body: "policy-jury", decision_mode: { mode: "HiddenBindingBallot" } },
    ...reorderedRequired.required_bodies,
  ];
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(reorderedRequired, ATTEMPT_ID),
    /canonical body order/u,
  );

  const duplicateBinding = readResponse();
  duplicateBinding.required_bodies.push({
    body: "agenda-council",
    decision_mode: { mode: "PublicFinding" },
  });
  const secondState = structuredClone(duplicateBinding.body_states[0]);
  secondState.body = "agenda-council";
  duplicateBinding.body_states.push(secondState);
  const secondBinding = structuredClone(duplicateBinding.certificate.body_bindings[0]);
  secondBinding.body = "agenda-council";
  secondBinding.sortition_request.body = "agenda-council";
  duplicateBinding.certificate.body_bindings.push(secondBinding);
  assert.throws(
    () => normalizeParliamentAttemptReadResponseV1(duplicateBinding, ATTEMPT_ID),
    /reuse bodyInstanceId/u,
  );
});

test("attempt read rejects malformed hidden-ballot retry, corpus, tally, and outcome", () => {
  const mutations = [
    [
      (response) => { response.certificate.body_bindings[0].ballot.ballot_attempt_sequence = 17; },
      /retri/u,
    ],
    [
      (response) => { response.certificate.body_bindings[0].ballot.max_corpus_entries = 2; },
      /frozen ballot lifecycle/u,
    ],
    [
      (response) => { response.certificate.body_bindings[0].ballot.tally.abstain = 1; },
      /tally/u,
    ],
    [
      (response) => { response.certificate.body_bindings[0].ballot.outcome = { outcome: "Rejected" }; },
      /approving aggregate outcome/u,
    ],
  ];
  for (const [mutate, expected] of mutations) {
    const forged = hiddenBallotReadResponse(10);
    mutate(forged);
    assert.throws(
      () => normalizeParliamentAttemptReadResponseV1(forged, ATTEMPT_ID),
      expected,
    );
  }
});

test("release context exposes a complete bounded transcript and partials remain context-bound", () => {
  const response = tleReleaseContextResponse();
  const parsed = normalizeParliamentTleReleaseContextResponseV1(response, ID(0x33));
  assert.equal(parsed.tle_key_session.public_shares.length, 4);
  assert.equal(parsed.tle_key_session.qualified_dealer_commitments.length, 2);
  assert.equal(
    parliamentTleReleaseContextReadPathV1(ID(0x33)),
    `/v1/gov/parliament/ballots/${ID(0x33)}/release-context`,
  );
  assert.equal(
    parliamentTlePartialReleasePathV1(ID(0x33)),
    `/v1/gov/parliament/ballots/${ID(0x33)}/partial-release`,
  );
  const partial = tlePartialReleaseResponse(response);
  assert.equal(normalizeParliamentTlePartialReleaseShareV1(partial, {
    expectedKeySessionId: response.tle_key_session.key_session_id,
    expectedIdentityDigest: response.identity_digest,
    committeeSize: response.tle_key_session.committee_size,
  }).participant_index, 1);

  const missingShare = structuredClone(response);
  missingShare.tle_key_session.public_shares.pop();
  assert.throws(
    () => normalizeParliamentTleReleaseContextResponseV1(missingShare, ID(0x33)),
    /complete ordered committee/u,
  );
  const aliasedDealer = structuredClone(response);
  aliasedDealer.tle_key_session.qualified_dealer_commitments[0].dealerIndex = 1;
  assert.throws(
    () => normalizeParliamentTleReleaseContextResponseV1(aliasedDealer, ID(0x33)),
    /unknown, aliased, or missing/u,
  );
  const wrongDigest = structuredClone(response);
  wrongDigest.identity_digest[0] ^= 1;
  assert.throws(
    () => normalizeParliamentTleReleaseContextResponseV1(wrongDigest, ID(0x33)),
    /threshold-session-framed/u,
  );
  const crossBound = structuredClone(partial);
  crossBound.key_session_id = ID(0x77);
  assert.throws(
    () => normalizeParliamentTlePartialReleaseShareV1(crossBound, {
      expectedKeySessionId: response.tle_key_session.key_session_id,
      expectedIdentityDigest: response.identity_digest,
      committeeSize: 4,
    }),
    /authorized release context/u,
  );
});

test("casting context is strict, phase-safe, and carries one bounded canonical archive", () => {
  const response = timedOvnCastingContextResponse();
  const parsed = normalizeParliamentTimedOvnCastingContextResponseV1(response, ID(0x33));
  assert.equal(parsed.phase, "Registered");
  assert.equal(parsed.registration_records_hex[0].length, 2 * 3_624);
  assert.equal(
    parliamentTimedOvnCastingContextReadPathV1(ID(0x33)),
    `/v1/gov/parliament/ballots/${ID(0x33)}/casting-context`,
  );
  assert.equal(
    parliamentTimedOvnCastingProofPathV1(ID(0x33)),
    `/v1/gov/parliament/ballots/${ID(0x33)}/casting-proof`,
  );
  const wrongPhase = structuredClone(response);
  wrongPhase.phase = "SurvivorsFrozen";
  assert.throws(
    () => normalizeParliamentTimedOvnCastingContextResponseV1(wrongPhase, ID(0x33)),
    /requires bounded survivor/u,
  );
  const emptyFrozen = structuredClone(response);
  emptyFrozen.phase = "SurvivorsFrozen";
  emptyFrozen.survivor_participant_hashes = [];
  emptyFrozen.release_identity = tleReleaseContextResponse().release_identity;
  assert.throws(
    () => normalizeParliamentTimedOvnCastingContextResponseV1(emptyFrozen, ID(0x33)),
    /requires bounded survivor/u,
  );
  const alias = structuredClone(response);
  alias.archiveNoritoBase64 = alias.archive_norito_base64;
  delete alias.archive_norito_base64;
  assert.throws(
    () => normalizeParliamentTimedOvnCastingContextResponseV1(alias, ID(0x33)),
    /unknown, aliased, or missing/u,
  );
  const noncanonicalArchive = structuredClone(response);
  noncanonicalArchive.archive_norito_base64 = "TlJUMA";
  assert.throws(
    () => normalizeParliamentTimedOvnCastingContextResponseV1(noncanonicalArchive, ID(0x33)),
    /canonical padded standard base64/u,
  );
});

test("ToriiClient exposes all six canonical authenticated Parliament paths", async () => {
  const canonicalAuth = { accountId: "alice-1@wonderland", privateKey: Buffer.alloc(32, 7) };
  const calls = [];
  const client = new ToriiClient("https://example.invalid");
  client._expectStatus = async () => {};
  client._readBoundedLosslessIntegerJson = async (response) => response.payload;
  client._request = async (method, path, options) => {
    calls.push({ method, path, options });
    if (path === PARLIAMENT_ATTEMPT_DRAFT_PATH_V1) {
      return { payload: {
        version: 1,
        proposal_content_id: PROPOSAL_ID,
        governance_attempt_id: ATTEMPT_ID,
        tx_instructions: [{ wire_id: PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1, payload_hex: "0102" }],
      } };
    }
    if (path === PARLIAMENT_TRANSITION_DRAFT_PATH_V1) {
      return { payload: {
        version: 1,
        governance_attempt_id: ATTEMPT_ID,
        transition_kind: { kind: "CompleteQualification" },
        transition_digest: ROOT,
        tx_instructions: [{ wire_id: PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1, payload_hex: "0304" }],
      } };
    }
    if (path.endsWith("/release-context")) {
      return { payload: tleReleaseContextResponse() };
    }
    if (path.endsWith("/casting-context")) {
      return { payload: timedOvnCastingContextResponse() };
    }
    if (path.endsWith("/partial-release")) {
      const context = tleReleaseContextResponse();
      return { payload: tlePartialReleaseResponse(context) };
    }
    return { payload: readResponse() };
  };
  await client.draftParliamentAttemptV1(
    parliamentProposalFixtures()[0],
    0,
    {
      canonicalAuth,
      expectedProposalContentId: PROPOSAL_ID,
      expectedGovernanceAttemptId: ATTEMPT_ID,
    },
  );
  await client.getParliamentAttemptV1(ATTEMPT_ID, { canonicalAuth });
  await client.getParliamentTimedOvnCastingContextV1(ID(0x33), { canonicalAuth });
  const releaseContext = await client.getParliamentTleReleaseContextV1(ID(0x33), {
    canonicalAuth,
  });
  await client.requestParliamentTlePartialReleaseV1(ID(0x33), {
    canonicalAuth,
    expectedKeySessionId: releaseContext.tle_key_session.key_session_id,
    expectedIdentityDigest: releaseContext.identity_digest,
    committeeSize: releaseContext.tle_key_session.committee_size,
  });
  await client.draftParliamentTransitionV1(
    ATTEMPT_ID,
    { transition: "CompleteQualification" },
    { canonicalAuth, expectedTransitionDigest: ROOT },
  );
  assert.deepEqual(calls.map(({ method, path }) => [method, path]), [
    ["POST", PARLIAMENT_ATTEMPT_DRAFT_PATH_V1],
    ["GET", `/v1/gov/parliament/attempts/${ATTEMPT_ID}`],
    ["GET", `/v1/gov/parliament/ballots/${ID(0x33)}/casting-context`],
    ["GET", `/v1/gov/parliament/ballots/${ID(0x33)}/release-context`],
    ["POST", `/v1/gov/parliament/ballots/${ID(0x33)}/partial-release`],
    ["POST", PARLIAMENT_TRANSITION_DRAFT_PATH_V1],
  ]);
  const attemptBody = JSON.parse(calls[0].options.body);
  assert.deepEqual(Object.keys(attemptBody), ["version", "proposal", "attempt_sequence"]);
  assert.equal(calls[4].options.body, undefined);
  const transitionBody = JSON.parse(calls[5].options.body);
  assert.deepEqual(Object.keys(transitionBody), ["version", "governance_attempt_id", "transition"]);
});

test("ToriiClient typed proposal reads use the strict local V1 parser", async () => {
  const proposals = parliamentProposalFixtures();
  const client = new ToriiClient("https://example.invalid");
  client.getGovernanceProposal = async () => ({
    found: true,
    proposal: {
      proposer: ACCOUNTS[0],
      kind: proposals.shift(),
      created_height: 1,
      status: "Proposed",
    },
  });
  const variants = [];
  while (proposals.length > 0) {
    const result = await client.getGovernanceProposalTyped(PROPOSAL_ID);
    variants.push(result.proposal.kind.variant);
  }
  assert.deepEqual(variants, [
    "DeployContract",
    "RuntimeUpgrade",
    "SccpRouteGovernance",
    "ValidationFeePolicy",
    "ValidationFeePayoutLifecycle",
    "MusubiRegistryGovernance",
    "SorafsProviderGovernance",
  ]);

  const malformed = structuredClone(parliamentProposalFixtures()[0]);
  malformed.payload.future = null;
  client.getGovernanceProposal = async () => ({
    found: true,
    proposal: {
      proposer: ACCOUNTS[0],
      kind: malformed,
      created_height: 1,
      status: "Proposed",
    },
  });
  await assert.rejects(
    client.getGovernanceProposalTyped(PROPOSAL_ID),
    /unsupported fields/u,
  );
});

function fixtureAccountId(label) {
  for (let attempt = 0; attempt < 1024; attempt += 1) {
    const publicKey = sha256(new TextEncoder().encode(`parliament:${label}:${attempt}`));
    try {
      return AccountAddress.fromAccount({ publicKey }).toI105(0x2f1);
    } catch (error) {
      if (
        !(error instanceof AccountAddressError) ||
        error.code !== AccountAddressErrorCode.INVALID_PUBLIC_KEY
      ) {
        throw error;
      }
    }
  }
  throw new Error(`unable to derive a canonical Parliament fixture account for ${label}`);
}

function parliamentProposalFixtures() {
  const [treasury, vault, ...recipients] = ACCOUNTS;
  const packageId = {
    home_dataspace: 7,
    scope: { kind: "DataspaceRoot", value: null },
    name: ["governed-package"],
  };
  const payoutBinding = {
    contract_address: CONTRACT_ADDRESS,
    code_hash: Array(32).fill(0x44),
    entrypoint: "autonomous_validation_fee_tick",
    treasury_account_id: treasury,
    ds_asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    xor_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc",
    pool_vault_account_id: vault,
    batch_ds: "10",
    min_xor_out: "4",
    max_xor_out: "100",
    recipients: recipients.map((account_id) => ({ account_id, share: "0.25" })),
  };
  return [
    {
      kind: "DeployContract",
      payload: {
        contract_address: CONTRACT_ADDRESS,
        code_hash: "aa".repeat(32),
        abi_hash: "bb".repeat(32),
        abi_version: 1,
        manifest_provenance: {
          signer: "ed012017CB79FB2B4120F2B1EC65E4198D6E08B28E813FEB01E4A400839B85E18080CE",
          signature:
            "C74557F062FDC5799D64FD2561103F6B13263B1FCE11F3148D48A34781F43D6C3ACB87C885BA666624A98D848AF3BF48A0A0C79FB3F28B244703269A52128809",
        },
      },
    },
    {
      kind: "RuntimeUpgrade",
      payload: {
        manifest: {
          name: "runtime-v1-refresh",
          description: "Canonical V1 runtime image",
          abi_version: 1,
          abi_hash: Array(32).fill(0x11),
          added_syscalls: [],
          added_pointer_types: [],
          start_height: 100,
          end_height: 120,
          sbom_digests: [{ algorithm: "sha256", digest: "AQID" }],
          slsa_attestation: "BAUG",
          provenance: [],
        },
      },
    },
    {
      kind: "SccpRouteGovernance",
      payload: {
        anchor: {
          network_id: NETWORK_ID,
          action: {
            action: "Remove",
            route: {
              lane_id: {
                source: { network: "bsc_mainnet", profile: null },
                target: { network: "sora_taira", profile: null },
              },
              route_id: "taira_bsc_xor",
              asset_key: "xor",
              revision: 1,
            },
          },
        },
      },
    },
    {
      kind: "ValidationFeePolicy",
      payload: {
        proposal_operator: treasury,
        policy: {
          schema_version: 1,
          network_id: NETWORK_ID,
          policy_version: "1",
          previous_policy_hash: null,
          ds_asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
          ds_scale: 2,
          fee: "0.1",
          treasury_account_id: treasury,
          charging_mode: {
            charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
            value: null,
          },
          effective_from_height: "121100",
          expires_after_height: null,
          exemption_classes: [],
          treasury_payout_binding: null,
        },
        payout_lifecycle_proposal_id: null,
      },
    },
    {
      kind: "ValidationFeePayoutLifecycle",
      payload: { proposal_operator: treasury, payout_binding: payoutBinding },
    },
    {
      kind: "MusubiRegistryGovernance",
      payload: {
        kind: "RetargetAlias",
        value: {
          alias: ["stable-alias"],
          target: packageId,
          expected_revision: 1,
        },
      },
    },
    {
      kind: "SorafsProviderGovernance",
      payload: {
        action: {
          action: "establish",
          value: { provider_id: [Array(32).fill(0x31)], owner: treasury },
        },
      },
    },
  ];
}

function transitionFixture(tag) {
  const one = ID(1);
  const two = ID(2);
  const payloads = {
    EscalateRisk: { target: { tier: "Standard" } },
    RegisterSortitionRequest: { sequence: 0, request: {}, candidate_snapshot: ["alice"] },
    ConsumeSortitionPulseBatch: { request_ids: [one], beacon_session_id: one, pulse_height: 1, pulse_id: two },
    BeginInvitationAcceptance: { election_attempt_id: one },
    FailBodyElectionNoRoster: { election_attempt_id: one },
    SealBodyRoster: { election_attempt_id: one },
    AdvanceBodyPhase: { body_instance_id: one, target: { phase: "Evidence" } },
    RecordAttemptAbsence: { body_instance_id: one, assignment_id: two },
    EndorsePublicFinding: { body_instance_id: one, result_root: ROOT },
    RegisterBallotAttempt: {
      body_instance_id: one,
      ballot_attempt_id: two,
      sequence: 0,
      tle_session_id: one,
      tle_key_session_id: two,
      release_beacon_session_id: one,
      release_height: 1,
    },
    CloseBallotRegistration: { ballot_attempt_id: one },
    FreezeBallotSurvivors: { ballot_attempt_id: one },
    FreezeTimedOvnCorpus: {
      ballot_attempt_id: one,
      ballot_records: [Array(2_858).fill(1)],
    },
    BeginBallotOpeningBatch: {
      ballot_attempt_ids: [one],
      release_beacon_session_id: two,
      release_height: 1,
      pulse_id: one,
    },
    FailBallotNoResult: { ballot_attempt_id: one },
    FinalizeOpenedBallot: {
      ballot_attempt_id: one,
      final_release: {
        key_session_id: two,
        identity_digest: ROOT,
        signature: Array(48).fill(1),
      },
    },
    RecordInvitationResponse: {
      election_attempt_id: one,
      body: "rules-committee",
      decision: { decision: "Accept" },
    },
    RegisterBallotParticipant: {
      ballot_attempt_id: one,
      registration_record: Array(3_624).fill(1),
    },
    RecordBallotDropout: { ballot_attempt_id: one },
    FailPublicFindingNoResult: { body_instance_id: one },
  };
  return tag === "CompleteQualification"
    ? { transition: tag }
    : { transition: tag, payload: payloads[tag] };
}

function readResponse() {
  return {
    version: 1,
    current_height: 9,
    attempt: {
      id: ATTEMPT_ID,
      proposal_content_id: PROPOSAL_ID,
      sequence: 0,
      risk_tier: { tier: "Standard" },
      stage: { stage: "Rules" },
      status: { status: "Certified" },
    },
    policy_version: 1,
    required_bodies: [{
      body: "rules-committee",
      decision_mode: { mode: "PublicFinding" },
    }],
    body_states: [{
      body: "rules-committee",
      body_instance_id: ID(1),
      status: { status: "Approved" },
      public_finding_opened_at_height: 1,
      public_finding_phase_blocks: 7,
      public_finding_deadline_height: 8,
      no_result_kind: null,
      no_result_height: null,
      timed_ovn_progress: null,
    }],
    certificate: {
      proposal_content_id: PROPOSAL_ID,
      governance_attempt_id: ATTEMPT_ID,
      governance_attempt_sequence: 0,
      risk_tier: { tier: "Standard" },
      body_bindings: [{
        body_instance_id: ID(1),
        election_attempt_id: ID(2),
        election_attempt_sequence: 0,
        sortition_request_id: ID(3),
        sortition_request: {
          id: ID(3),
          governance_attempt_id: ATTEMPT_ID,
          body_election_attempt_id: ID(2),
          body: "rules-committee",
          candidate_root: ROOT,
          candidate_count: 3,
          target_seats: 3,
          request_height: 1,
          pulse_height: 2,
          beacon_session_id: ID(4),
        },
        body: "rules-committee",
        original_seats: 3,
        beacon_session_id: ID(4),
        beacon_pulse_id: ID(5),
        roster_root: ROOT,
        assignment_root: ROOT,
        result_root: ROOT,
        result_height: 8,
        public_finding: {
          endorsement_root: ROOT,
          endorsing_assignments: [ID(0x11), ID(0x12)],
          endorsements: 2,
          quorum: 2,
        },
        ballot: null,
      }],
      policy_version: 1,
      effect_preimage_hash: ROOT,
      expected_head: { state: "Absent", head: { subject_id: ROOT } },
      certified_at_height: 8,
      enact_at_height: 10,
    },
    terminal_height: null,
    execution_failure_root: null,
    superseding_head: null,
    state_payload_hex: stateFrame().toString("hex"),
  };
}

function hiddenBallotReadResponse(commitmentClosedAtHeight) {
  const response = readResponse();
  response.current_height = 20;
  response.attempt.stage = { stage: "PolicyJury" };
  response.attempt.status = { status: "Certified" };
  response.required_bodies = [{
    body: "policy-jury",
    decision_mode: { mode: "HiddenBindingBallot" },
  }];
  response.body_states = [{
    body: "policy-jury",
    body_instance_id: ID(1),
    status: { status: "Approved" },
    public_finding_opened_at_height: null,
    public_finding_phase_blocks: null,
    public_finding_deadline_height: null,
    no_result_kind: null,
    no_result_height: null,
    timed_ovn_progress: {
      ballot_attempt_id: ID(6),
      status: { status: "Finalized" },
      frozen_survivor_count: 3,
      accepted_ballot_prefix_count: 3,
    },
  }];
  const binding = response.certificate.body_bindings[0];
  binding.body = "policy-jury";
  binding.sortition_request.body = "policy-jury";
  binding.result_height = 18;
  binding.public_finding = null;
  binding.ballot = {
    ballot_attempt_id: ID(6),
    ballot_attempt_sequence: 0,
    tle_session_id: ID(7),
    tle_key_session_id: ID(8),
    registration_root: Array(32).fill(0x41),
    dropout_root: Array(32).fill(0x42),
    survivor_root: Array(32).fill(0x43),
    corpus_root: Array(32).fill(0x44),
    no_recovery_root: Array(32).fill(0x45),
    timed_commitment_root: Array(32).fill(0x46),
    release_beacon_session_id: ID(9),
    registered_at_height: 1,
    registration_close_height: 5,
    survivor_freeze_height: 8,
    commitment_close_height: 13,
    registration_closed_at_height: 5,
    survivors_frozen_at_height: 8,
    commitment_closed_at_height: commitmentClosedAtHeight,
    max_ballot_retries: 16,
    max_corpus_entries: 3,
    release_height: 15,
    opening_deadline_height: 21,
    release_pulse_id: ID(10),
    opening_height: 16,
    opening_root: Array(32).fill(0x47),
    tally: {
      original_seats: 3,
      accepted_ballots: 3,
      aye: 2,
      nay: 1,
      abstain: 0,
    },
    outcome: { outcome: "Approved" },
  };
  response.certificate.certified_at_height = 20;
  response.certificate.enact_at_height = 30;
  return response;
}

function tleReleaseContextResponse() {
  const ballotAttemptId = ID(0x33);
  const bodyInstanceId = ID(0x22);
  const keySessionId = ID(0x44);
  const releaseHeight = 40;
  const survivorRoot = Array(32).fill(0x61);
  const noRecoveryRoot = Array(32).fill(0x62);
  const parameterHash = Array(32).fill(0x63);
  const identityPayload = Buffer.concat([
    Buffer.from("iroha.parliament.tle.identity-payload.v1\0", "utf8"),
    uint16be(1),
    Buffer.from(ATTEMPT_ID, "hex"),
    Buffer.from(bodyInstanceId, "hex"),
    Buffer.from(ballotAttemptId, "hex"),
    Buffer.from(survivorRoot),
    Buffer.from(noRecoveryRoot),
    uint64be(releaseHeight),
    Buffer.from(parameterHash),
  ]);
  assert.equal(identityPayload.length, 243);
  const tleKeySession = {
    version: 1,
    key_session_id: keySessionId,
    network_id: Array(32).fill(0x45),
    roster_hash: Array(32).fill(0x46),
    committee_size: 4,
    threshold: 2,
    generator_h: Array(96).fill(0x47),
    generator_v: Array(96).fill(0x48),
    qualified_dealers: [1, 2],
    qualified_dealer_commitments: [1, 2].map((dealerIndex) => ({
      dealer_index: dealerIndex,
      coefficient_commitments: [
        Array(96).fill(0x50 + dealerIndex),
        Array(96).fill(0x60 + dealerIndex),
      ],
      constant_pok_commitment: Array(96).fill(0x70 + dealerIndex),
      constant_pok_response: Array(32).fill(0x80 + dealerIndex),
    })),
    dkg_event_hash: Array(32).fill(0x49),
    group_public_key: Array(96).fill(0x4a),
    public_shares: [1, 2, 3, 4].map((index) => ({
      index,
      participant_hash: Array(32).fill(0x20 + index),
      public_key_share: Array(96).fill(0x30 + index),
    })),
    transcript_hash: Array(32).fill(0x4b),
  };
  const message = Buffer.concat([
    Buffer.from("iroha.threshold-bls.message.v1\0", "utf8"),
    Buffer.from("iroha.threshold-bls.session.v1\0", "utf8"),
    uint16be(1),
    Buffer.from([2]),
    Buffer.from(tleKeySession.network_id),
    Buffer.from(keySessionId, "hex"),
    Buffer.from(tleKeySession.roster_hash),
    uint16be(tleKeySession.committee_size),
    uint16be(tleKeySession.threshold),
    uint32be(identityPayload.length),
    identityPayload,
  ]);
  return {
    version: 1,
    current_height: 42,
    ballot_attempt_id: ballotAttemptId,
    governance_attempt_id: ATTEMPT_ID,
    body_instance_id: bodyInstanceId,
    status: { status: "Opening" },
    release_height: releaseHeight,
    opening_deadline_height: 45,
    tle_key_session: tleKeySession,
    release_identity: {
      tle_key_session_id: keySessionId,
      governance_attempt_id: ATTEMPT_ID,
      body_instance_id: bodyInstanceId,
      ballot_attempt_id: ballotAttemptId,
      survivor_corpus_root: survivorRoot,
      no_recovery_root: noRecoveryRoot,
      target_finalized_height: releaseHeight,
      parameter_hash: parameterHash,
    },
    identity_digest: [...sha256(message)],
    identity_payload_hex: identityPayload.toString("hex"),
  };
}

function timedOvnCastingContextResponse() {
  const release = tleReleaseContextResponse();
  return {
    version: 1,
    current_height: 20,
    phase: "Registered",
    session: {
      network_id: release.tle_key_session.network_id,
      proposal_content_id: PROPOSAL_ID,
      governance_attempt_id: ATTEMPT_ID,
      body_instance_id: release.body_instance_id,
      ballot_attempt_id: release.ballot_attempt_id,
      parameter_hash: release.release_identity.parameter_hash,
      tle_key_session_id: release.tle_key_session.key_session_id,
      tle_key_transcript_hash: release.tle_key_session.transcript_hash,
      tle_master_public_key: release.tle_key_session.group_public_key,
    },
    registration_opened_at_finalized_height: 10,
    target_finalized_height: 40,
    tle_key_session: release.tle_key_session,
    registration_records_hex: [Buffer.alloc(3_624, 0x81).toString("hex")],
    survivor_participant_hashes: null,
    release_identity: null,
    archive_norito_base64: Buffer.from("NRT0", "ascii").toString("base64"),
  };
}

function tlePartialReleaseResponse(context) {
  return {
    key_session_id: context.tle_key_session.key_session_id,
    identity_digest: context.identity_digest,
    participant_index: 1,
    sigma: Array(48).fill(0x91),
    proof_x: Array(96).fill(0x92),
    proof_y: Array(48).fill(0x93),
    z_s: Array(32).fill(0x94),
    z_r: Array(32).fill(0x95),
    z_u: Array(32).fill(0x96),
  };
}

function uint16be(value) {
  const bytes = Buffer.alloc(2);
  bytes.writeUInt16BE(value);
  return bytes;
}

function uint32be(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32BE(value);
  return bytes;
}

function uint64be(value) {
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64BE(BigInt(value));
  return bytes;
}

function stateFrame() {
  const payload = Buffer.from([1, 2]);
  const header = Buffer.alloc(40);
  header.write("NRT0", 0, "ascii");
  header[6] = 3;
  header.writeBigUInt64LE(BigInt(payload.length), 23);
  header.writeBigUInt64LE(crc64Xz(payload), 31);
  return Buffer.concat([header, payload]);
}
