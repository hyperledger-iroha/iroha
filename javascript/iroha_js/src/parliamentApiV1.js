// SPDX-License-Identifier: Apache-2.0

import { validateNoritoFrame } from "./norito.js";
import { sha256 } from "@noble/hashes/sha2";
import { normalizeGovernanceProposalWireV1 } from "./governanceProposalV1.js";

export const PARLIAMENT_API_VERSION_V1 = 1;
export const PARLIAMENT_ATTEMPT_DRAFT_PATH_V1 =
  "/v1/gov/parliament/attempts/draft";
export const PARLIAMENT_ATTEMPT_READ_PATH_V1 =
  "/v1/gov/parliament/attempts/{governance_attempt_id}";
export const PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_READ_PATH_V1 =
  "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_PATH_V1 =
  "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof";
export const PARLIAMENT_TLE_RELEASE_CONTEXT_READ_PATH_V1 =
  "/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context";
export const PARLIAMENT_TLE_PARTIAL_RELEASE_PATH_V1 =
  "/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release";
export const PARLIAMENT_TRANSITION_DRAFT_PATH_V1 =
  "/v1/gov/parliament/transitions/draft";
export const PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1 =
  "iroha.governance.parliament.attempt.create.v1";
export const PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1 =
  "iroha.governance.parliament.transition.submit.v1";
export const PARLIAMENT_ATTEMPT_STATE_MAX_BYTES_V1 = 16 * 1024 * 1024;
export const PARLIAMENT_TIMED_OVN_REGISTRATION_RECORD_BYTES_V1 = 3_624;
export const PARLIAMENT_TIMED_OVN_BALLOT_RECORD_BYTES_V1 = 2_858;
export const PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1 = 1_000;
export const PARLIAMENT_TLE_MAX_COMMITTEE_SIZE_V1 = 31;
const PARLIAMENT_BODY_TARGET_SEATS_MAX_V1 = 1_000;
const PARLIAMENT_BALLOT_RETRIES_MAX_V1 = 16;
export const PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1 =
  4 * 1024 * 1024;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_MAX_BYTES_V1 =
  8 * 1024 * 1024;

export const PARLIAMENT_PUBLIC_TRANSITIONS_V1 = Object.freeze([
  [0, "EscalateRisk", true, 0],
  [1, "CompleteQualification", false, 1],
  [2, "RegisterSortitionRequest", true, 2],
  [3, "ConsumeSortitionPulseBatch", true, 3],
  [4, "BeginInvitationAcceptance", true, 4],
  [5, "FailBodyElectionNoRoster", true, 5],
  [6, "SealBodyRoster", true, 6],
  [7, "AdvanceBodyPhase", true, 7],
  [8, "RecordAttemptAbsence", true, 8],
  [9, "EndorsePublicFinding", true, 9],
  [10, "RegisterBallotAttempt", true, 10],
  [11, "CloseBallotRegistration", true, 11],
  [12, "FreezeBallotSurvivors", true, 12],
  [13, "FreezeTimedOvnCorpus", true, 13],
  [14, "BeginBallotOpeningBatch", true, 14],
  [15, "FailBallotNoResult", true, 15],
  [16, "FinalizeOpenedBallot", true, 16],
  [17, "RecordInvitationResponse", true, 20],
  [18, "RegisterBallotParticipant", true, 21],
  [19, "RecordBallotDropout", true, 22],
  [20, "FailPublicFindingNoResult", true, 23],
].map(([noritoIndex, jsonTag, jsonPayloadRequired, eventKindIndex]) =>
  Object.freeze({ noritoIndex, jsonTag, jsonPayloadRequired, eventKindIndex }),
));

// These outcomes are exported for read/audit decoding only. The transition
// builder below deliberately accepts only PARLIAMENT_PUBLIC_TRANSITIONS_V1.
export const PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1 = Object.freeze([
  [0, "Enacted", false, "MarkEnacted", 17],
  [1, "Superseded", true, "MarkSuperseded", 18],
  [2, "ExecutionFailed", true, "MarkExecutionFailed", 19],
].map(([noritoIndex, jsonTag, jsonPayloadRequired, eventKind, eventKindIndex]) =>
  Object.freeze({
    noritoIndex,
    jsonTag,
    jsonPayloadRequired,
    eventKind,
    eventKindIndex,
  }),
));

export const PARLIAMENT_NO_RESULT_KINDS_V1 = Object.freeze([
  [0, "PublicFindingQuorumUnreachable"],
  [1, "PublicFindingDeadlineExpired"],
  [2, "BallotRegistrationDeadlineExpired"],
  [3, "BallotSurvivorDeadlineExpired"],
  [4, "BallotCommitmentDeadlineExpired"],
  [5, "BallotReleasePulseUnavailable"],
  [6, "BallotOpeningDeadlineExpired"],
].map(([noritoIndex, jsonTag]) => Object.freeze({ noritoIndex, jsonTag })));

export const PARLIAMENT_BODY_STATE_FIELDS_V1 = Object.freeze([
  "body",
  "body_instance_id",
  "status",
  "public_finding_opened_at_height",
  "public_finding_phase_blocks",
  "public_finding_deadline_height",
  "no_result_kind",
  "no_result_height",
]);

export const PARLIAMENT_CERTIFICATE_BODY_BINDING_FIELDS_V1 = Object.freeze([
  "body_instance_id",
  "election_attempt_id",
  "election_attempt_sequence",
  "sortition_request_id",
  "sortition_request",
  "body",
  "original_seats",
  "beacon_session_id",
  "beacon_pulse_id",
  "roster_root",
  "assignment_root",
  "result_root",
  "result_height",
  "public_finding",
  "ballot",
]);

export const PARLIAMENT_PUBLIC_FINDING_CERTIFICATE_FIELDS_V1 = Object.freeze([
  "endorsement_root",
  "endorsing_assignments",
  "endorsements",
  "quorum",
]);

const PUBLIC_TRANSITIONS_BY_TAG = new Map(
  PARLIAMENT_PUBLIC_TRANSITIONS_V1.map((entry) => [entry.jsonTag, entry]),
);
const TRANSITION_PAYLOAD_FIELDS = new Map([
  ["EscalateRisk", ["target"]],
  ["RegisterSortitionRequest", ["sequence", "request", "candidate_snapshot"]],
  ["ConsumeSortitionPulseBatch", ["request_ids", "beacon_session_id", "pulse_height", "pulse_id"]],
  ["BeginInvitationAcceptance", ["election_attempt_id"]],
  ["FailBodyElectionNoRoster", ["election_attempt_id"]],
  ["SealBodyRoster", ["election_attempt_id"]],
  ["AdvanceBodyPhase", ["body_instance_id", "target"]],
  ["RecordAttemptAbsence", ["body_instance_id", "assignment_id"]],
  ["EndorsePublicFinding", ["body_instance_id", "result_root"]],
  ["RegisterBallotAttempt", [
    "body_instance_id", "ballot_attempt_id", "sequence", "tle_session_id",
    "tle_key_session_id", "release_beacon_session_id", "release_height",
  ]],
  ["CloseBallotRegistration", ["ballot_attempt_id"]],
  ["FreezeBallotSurvivors", ["ballot_attempt_id"]],
  ["FreezeTimedOvnCorpus", ["ballot_attempt_id", "ballot_records"]],
  ["BeginBallotOpeningBatch", [
    "ballot_attempt_ids", "release_beacon_session_id", "release_height", "pulse_id",
  ]],
  ["FailBallotNoResult", ["ballot_attempt_id"]],
  ["FinalizeOpenedBallot", ["ballot_attempt_id", "final_release"]],
  ["RecordInvitationResponse", ["election_attempt_id", "body", "decision"]],
  ["RegisterBallotParticipant", ["ballot_attempt_id", "registration_record"]],
  ["RecordBallotDropout", ["ballot_attempt_id"]],
  ["FailPublicFindingNoResult", ["body_instance_id"]],
]);
const ATTEMPT_RESPONSE_FIELDS = [
  "version", "proposal_content_id", "governance_attempt_id", "tx_instructions",
];
const TRANSITION_RESPONSE_FIELDS = [
  "version", "governance_attempt_id", "transition_kind", "transition_digest",
  "tx_instructions",
];
const READ_RESPONSE_FIELDS = [
  "version", "current_height", "attempt", "policy_version", "required_bodies",
  "body_states", "certificate", "terminal_height", "execution_failure_root", "superseding_head",
  "state_payload_hex",
];
const TLE_RELEASE_CONTEXT_FIELDS = [
  "version", "current_height", "ballot_attempt_id", "governance_attempt_id",
  "body_instance_id", "status", "release_height", "opening_deadline_height",
  "tle_key_session", "release_identity", "identity_digest", "identity_payload_hex",
];
const TIMED_OVN_CASTING_CONTEXT_FIELDS = [
  "version", "current_height", "phase", "session",
  "registration_opened_at_finalized_height", "target_finalized_height",
  "tle_key_session", "registration_records_hex", "survivor_participant_hashes",
  "release_identity", "archive_norito_base64",
];
const TIMED_OVN_SESSION_FIELDS = [
  "network_id", "proposal_content_id", "governance_attempt_id", "body_instance_id",
  "ballot_attempt_id", "parameter_hash", "tle_key_session_id",
  "tle_key_transcript_hash", "tle_master_public_key",
];
const TLE_KEY_SESSION_BINDING_FIELDS = [
  "version", "key_session_id", "network_id", "roster_hash", "committee_size", "threshold",
  "generator_h", "generator_v", "qualified_dealers", "qualified_dealer_commitments",
  "dkg_event_hash", "group_public_key", "public_shares", "transcript_hash",
];
const TLE_DEALER_COMMITMENT_FIELDS = [
  "dealer_index", "coefficient_commitments", "constant_pok_commitment",
  "constant_pok_response",
];
const TLE_PUBLIC_SHARE_FIELDS = ["index", "participant_hash", "public_key_share"];
const TLE_PARTIAL_RELEASE_FIELDS = [
  "key_session_id", "identity_digest", "participant_index", "sigma", "proof_x", "proof_y",
  "z_s", "z_r", "z_u",
];
const TLE_RELEASE_IDENTITY_FIELDS = [
  "tle_key_session_id", "governance_attempt_id", "body_instance_id",
  "ballot_attempt_id", "survivor_corpus_root", "no_recovery_root",
  "target_finalized_height", "parameter_hash",
];
const ATTEMPT_FIELDS = [
  "id", "proposal_content_id", "sequence", "risk_tier", "stage", "status",
];
const CERTIFICATE_FIELDS = [
  "proposal_content_id", "governance_attempt_id", "governance_attempt_sequence",
  "risk_tier", "body_bindings", "policy_version", "effect_preimage_hash",
  "expected_head", "certified_at_height", "enact_at_height",
];
const SORTITION_REQUEST_FIELDS = [
  "id", "governance_attempt_id", "body_election_attempt_id", "body",
  "candidate_root", "candidate_count", "target_seats", "request_height",
  "pulse_height", "beacon_session_id",
];
const BALLOT_CERTIFICATE_FIELDS = [
  "ballot_attempt_id", "ballot_attempt_sequence", "tle_session_id",
  "tle_key_session_id", "registration_root", "dropout_root", "survivor_root",
  "corpus_root", "no_recovery_root", "timed_commitment_root",
  "release_beacon_session_id", "registered_at_height", "registration_close_height",
  "survivor_freeze_height", "commitment_close_height",
  "registration_closed_at_height", "survivors_frozen_at_height",
  "commitment_closed_at_height", "max_ballot_retries", "max_corpus_entries",
  "release_height", "opening_deadline_height", "release_pulse_id", "opening_height",
  "opening_root", "tally", "outcome",
];
const BALLOT_TALLY_FIELDS = [
  "original_seats", "accepted_ballots", "aye", "nay", "abstain",
];
const PARLIAMENT_BODY_ORDER_V1 = [
  "rules-committee", "agenda-council", "interest-panel", "review-panel",
  "coordination-council", "mpc-committee", "fma-committee",
  "oversight-committee", "policy-jury", "confirmation-jury",
];
const PRIVATE_KEY_FIELDS = new Set([
  "private_key", "privateKey", "seed", "mnemonic", "private_key_seed",
]);
const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const LOWER_HEX_BYTES = /^(?:[0-9a-f]{2})+$/u;
const STANDARD_PADDED_BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;

/** Replace the sole read-path placeholder after exact non-zero ID validation. */
export function parliamentAttemptReadPathV1(governanceAttemptId) {
  return PARLIAMENT_ATTEMPT_READ_PATH_V1.replace(
    "{governance_attempt_id}",
    canonicalId(governanceAttemptId, "governanceAttemptId"),
  );
}

/** Replace the sole casting-context placeholder after exact non-zero ID validation. */
export function parliamentTimedOvnCastingContextReadPathV1(ballotAttemptId) {
  return PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_READ_PATH_V1.replace(
    "{ballot_attempt_id}",
    canonicalId(ballotAttemptId, "ballotAttemptId"),
  );
}

/** Replace the sole casting-proof placeholder after exact non-zero ID validation. */
export function parliamentTimedOvnCastingProofPathV1(ballotAttemptId) {
  return PARLIAMENT_TIMED_OVN_CASTING_PROOF_PATH_V1.replace(
    "{ballot_attempt_id}",
    canonicalId(ballotAttemptId, "ballotAttemptId"),
  );
}

/** Replace the sole release-context placeholder after exact non-zero ID validation. */
export function parliamentTleReleaseContextReadPathV1(ballotAttemptId) {
  return PARLIAMENT_TLE_RELEASE_CONTEXT_READ_PATH_V1.replace(
    "{ballot_attempt_id}",
    canonicalId(ballotAttemptId, "ballotAttemptId"),
  );
}

/** Replace the sole partial-release placeholder after exact non-zero ID validation. */
export function parliamentTlePartialReleasePathV1(ballotAttemptId) {
  return PARLIAMENT_TLE_PARTIAL_RELEASE_PATH_V1.replace(
    "{ballot_attempt_id}",
    canonicalId(ballotAttemptId, "ballotAttemptId"),
  );
}

/** Build the exact closed V1 attempt-draft request body. */
export function buildParliamentAttemptDraftRequestV1(proposal, attemptSequence) {
  rejectPrivateKeyFields(proposal, "proposal");
  const tagged = normalizeGovernanceProposalWireV1(proposal, "proposal");
  return {
    version: PARLIAMENT_API_VERSION_V1,
    proposal: tagged,
    attempt_sequence: uint(attemptSequence, 0xffff_ffff, "attemptSequence"),
  };
}

/** Build the exact closed V1 public lifecycle-transition draft request body. */
export function buildParliamentTransitionDraftRequestV1(
  governanceAttemptId,
  transition,
) {
  const tagged = normalizePublicTransition(transition);
  return {
    version: PARLIAMENT_API_VERSION_V1,
    governance_attempt_id: canonicalId(governanceAttemptId, "governanceAttemptId"),
    transition: tagged,
  };
}

/** Strictly admit and bind an attempt-draft response. */
export function normalizeParliamentAttemptDraftResponseV1(
  value,
  { expectedProposalContentId, expectedGovernanceAttemptId },
) {
  const root = exactObject(value, ATTEMPT_RESPONSE_FIELDS, "Parliament attempt draft response");
  version(root.version);
  const proposalContentId = canonicalId(root.proposal_content_id, "proposal_content_id");
  const governanceAttemptId = canonicalId(root.governance_attempt_id, "governance_attempt_id");
  if (proposalContentId !== canonicalId(expectedProposalContentId, "expectedProposalContentId")) {
    throw new Error("proposal_content_id differs from the exact request binding");
  }
  if (governanceAttemptId !== canonicalId(expectedGovernanceAttemptId, "expectedGovernanceAttemptId")) {
    throw new Error("governance_attempt_id differs from the exact request binding");
  }
  return {
    version: PARLIAMENT_API_VERSION_V1,
    proposal_content_id: proposalContentId,
    governance_attempt_id: governanceAttemptId,
    tx_instructions: [instruction(root.tx_instructions, PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1)],
  };
}

/** Strictly admit and bind a transition-draft response. */
export function normalizeParliamentTransitionDraftResponseV1(
  value,
  { expectedGovernanceAttemptId, expectedTransitionKind, expectedTransitionDigest },
) {
  if (!PUBLIC_TRANSITIONS_BY_TAG.has(expectedTransitionKind)) {
    throw new TypeError("expectedTransitionKind is not a submit-able Parliament transition");
  }
  const expectedDigest = bytes(expectedTransitionDigest, 32, "expectedTransitionDigest", true);
  const root = exactObject(value, TRANSITION_RESPONSE_FIELDS, "Parliament transition draft response");
  version(root.version);
  const attemptId = canonicalId(root.governance_attempt_id, "governance_attempt_id");
  if (attemptId !== canonicalId(expectedGovernanceAttemptId, "expectedGovernanceAttemptId")) {
    throw new Error("governance_attempt_id differs from the exact request binding");
  }
  const kindObject = exactObject(root.transition_kind, ["kind"], "transition_kind");
  if (!PUBLIC_TRANSITIONS_BY_TAG.has(kindObject.kind)) {
    throw new TypeError("transition_kind is automatic, unknown, or consensus-owned");
  }
  if (kindObject.kind !== expectedTransitionKind) {
    throw new Error("transition_kind differs from the exact request binding");
  }
  const digest = bytes(root.transition_digest, 32, "transition_digest", true);
  if (!digest.equals(expectedDigest)) {
    throw new Error("transition_digest differs from the exact request binding");
  }
  return {
    version: PARLIAMENT_API_VERSION_V1,
    governance_attempt_id: attemptId,
    transition_kind: { kind: kindObject.kind },
    transition_digest: [...digest],
    tx_instructions: [instruction(root.tx_instructions, PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1)],
  };
}

/** Strictly admit one bounded attempt-read projection and its certificate. */
export function normalizeParliamentAttemptReadResponseV1(value, expectedGovernanceAttemptId) {
  const root = exactObject(value, READ_RESPONSE_FIELDS, "Parliament attempt read response");
  version(root.version);
  unsigned(root.current_height, "current_height");
  unsigned(root.policy_version, "policy_version");
  optionalUnsigned(root.terminal_height, "terminal_height");
  optionalBytes32(root.execution_failure_root, "execution_failure_root");
  const attempt = exactObject(root.attempt, ATTEMPT_FIELDS, "attempt");
  const attemptId = canonicalId(attempt.id, "attempt.id");
  if (attemptId !== canonicalId(expectedGovernanceAttemptId, "expectedGovernanceAttemptId")) {
    throw new Error("attempt.id differs from the requested canonical identifier");
  }
  canonicalId(attempt.proposal_content_id, "attempt.proposal_content_id");
  uint(attempt.sequence, 0xffff_ffff, "attempt.sequence");
  validateTaggedUnit(attempt.risk_tier, "tier", ["Routine", "Standard", "Constitutional", "Emergency"], "attempt.risk_tier");
  validateTaggedUnit(attempt.stage, "stage", [
    "Qualification", "Rules", "Agenda", "Interest", "Review", "Coordination",
    "Mpc", "Fma", "Oversight", "PolicyJury", "ConfirmationJury",
    "Certification", "Enactment",
  ], "attempt.stage");
  validateTaggedUnit(attempt.status, "status", [
    "Active", "Certified", "Rejected", "Enacted", "Superseded", "ExecutionFailed",
  ], "attempt.status");
  if (!Array.isArray(root.required_bodies) || root.required_bodies.length < 1 || root.required_bodies.length > 10) {
    throw new TypeError("required_bodies must contain one through ten exact body projections");
  }
  for (const [index, body] of root.required_bodies.entries()) {
    exactObject(body, ["body", "decision_mode"], `required_bodies[${index}]`);
  }
  validateBodyStates(root.body_states, root.required_bodies);
  if (root.certificate !== null) {
    validateCertificate(root.certificate, attemptId);
  }
  if (root.superseding_head !== null) {
    validateExpectedHead(root.superseding_head, "superseding_head");
  }
  const statePayloadHex = canonicalHex(root.state_payload_hex, "state_payload_hex");
  const stateBytes = Buffer.from(statePayloadHex, "hex");
  if (stateBytes.length > PARLIAMENT_ATTEMPT_STATE_MAX_BYTES_V1) {
    throw new RangeError("state_payload_hex exceeds the 16 MiB Parliament bound");
  }
  validateNoritoFrame(stateBytes, {
    context: "Parliament attempt state_payload_hex",
    requireNonEmptyPayload: true,
  });
  return { ...root, attempt: { ...attempt }, state_payload_hex: statePayloadHex };
}

/** Strictly admit one Core-authorized bounded public TLE release context. */
export function normalizeParliamentTleReleaseContextResponseV1(value, expectedBallotAttemptId) {
  const root = exactObject(
    value,
    TLE_RELEASE_CONTEXT_FIELDS,
    "Parliament TLE release-context response",
  );
  version(root.version);
  const currentHeight = unsigned(root.current_height, "current_height");
  const ballotAttemptId = canonicalId(root.ballot_attempt_id, "ballot_attempt_id");
  if (ballotAttemptId !== canonicalId(expectedBallotAttemptId, "expectedBallotAttemptId")) {
    throw new Error("ballot_attempt_id differs from the requested canonical identifier");
  }
  const governanceAttemptId = canonicalId(root.governance_attempt_id, "governance_attempt_id");
  const bodyInstanceId = canonicalId(root.body_instance_id, "body_instance_id");
  validateTaggedUnit(root.status, "status", ["Opening"], "status");
  const releaseHeight = unsigned(root.release_height, "release_height");
  const openingDeadline = unsigned(root.opening_deadline_height, "opening_deadline_height");
  if (BigInt(currentHeight) < BigInt(releaseHeight)
    || BigInt(currentHeight) > BigInt(openingDeadline)
    || BigInt(openingDeadline) < BigInt(releaseHeight)) {
    throw new Error("TLE release context lies outside its inclusive opening window");
  }

  const { keySession, keySessionId } = normalizeTleKeySessionBinding(
    root.tle_key_session,
  );

  const releaseIdentity = exactObject(
    root.release_identity,
    TLE_RELEASE_IDENTITY_FIELDS,
    "release_identity",
  );
  if (canonicalId(releaseIdentity.tle_key_session_id, "release_identity.tle_key_session_id") !== keySessionId
    || canonicalId(releaseIdentity.governance_attempt_id, "release_identity.governance_attempt_id") !== governanceAttemptId
    || canonicalId(releaseIdentity.body_instance_id, "release_identity.body_instance_id") !== bodyInstanceId
    || canonicalId(releaseIdentity.ballot_attempt_id, "release_identity.ballot_attempt_id") !== ballotAttemptId) {
    throw new Error("release_identity differs from the top-level Parliament/TLE bindings");
  }
  for (const field of ["survivor_corpus_root", "no_recovery_root", "parameter_hash"]) {
    bytes(releaseIdentity[field], 32, `release_identity.${field}`, true);
  }
  const targetHeight = unsigned(
    releaseIdentity.target_finalized_height,
    "release_identity.target_finalized_height",
  );
  if (BigInt(targetHeight) !== BigInt(releaseHeight)) {
    throw new Error("release_identity target differs from release_height");
  }
  const identityDigest = bytes(root.identity_digest, 32, "identity_digest", true);
  const identityPayloadHex = canonicalHex(root.identity_payload_hex, "identity_payload_hex");
  if (identityPayloadHex.length !== 486) {
    throw new TypeError("identity_payload_hex must encode the exact 243-byte identity payload");
  }
  validateTleIdentityPayload(
    Buffer.from(identityPayloadHex, "hex"),
    governanceAttemptId,
    bodyInstanceId,
    ballotAttemptId,
    releaseIdentity,
    releaseHeight,
  );
  const expectedDigest = tleReleaseMessageDigest(keySession, Buffer.from(identityPayloadHex, "hex"));
  if (!identityDigest.equals(expectedDigest)) {
    throw new Error("identity_digest differs from the exact threshold-session-framed release message");
  }
  return { ...root, identity_payload_hex: identityPayloadHex };
}

/** Strictly admit one replay-validated public timed-OVN wallet context. */
export function normalizeParliamentTimedOvnCastingContextResponseV1(
  value,
  expectedBallotAttemptId,
) {
  const root = exactObject(
    value,
    TIMED_OVN_CASTING_CONTEXT_FIELDS,
    "Parliament timed-OVN casting-context response",
  );
  version(root.version);
  const currentHeight = unsigned(root.current_height, "current_height");
  if (BigInt(currentHeight) === 0n) throw new TypeError("current_height must be non-zero");
  if (!["Registered", "RegistrationClosed", "SurvivorsFrozen"].includes(root.phase)) {
    throw new TypeError("phase is not a cast-capable Parliament timed-OVN phase");
  }
  const session = exactObject(root.session, TIMED_OVN_SESSION_FIELDS, "session");
  bytes(session.network_id, 32, "session.network_id", true);
  canonicalId(session.proposal_content_id, "session.proposal_content_id");
  const governanceAttemptId = canonicalId(
    session.governance_attempt_id,
    "session.governance_attempt_id",
  );
  const bodyInstanceId = canonicalId(session.body_instance_id, "session.body_instance_id");
  const ballotAttemptId = canonicalId(session.ballot_attempt_id, "session.ballot_attempt_id");
  if (ballotAttemptId !== canonicalId(expectedBallotAttemptId, "expectedBallotAttemptId")) {
    throw new Error("session.ballot_attempt_id differs from the requested identifier");
  }
  bytes(session.parameter_hash, 32, "session.parameter_hash", true);
  const sessionKeyId = canonicalId(session.tle_key_session_id, "session.tle_key_session_id");
  bytes(session.tle_key_transcript_hash, 32, "session.tle_key_transcript_hash", true);
  bytes(session.tle_master_public_key, 96, "session.tle_master_public_key", true);

  const registrationOpened = unsigned(
    root.registration_opened_at_finalized_height,
    "registration_opened_at_finalized_height",
  );
  const targetHeight = unsigned(root.target_finalized_height, "target_finalized_height");
  if (BigInt(registrationOpened) === 0n
    || BigInt(registrationOpened) > BigInt(currentHeight)
    || BigInt(targetHeight) <= BigInt(registrationOpened)) {
    throw new Error("casting-context height schedule is inconsistent");
  }

  const { keySession, keySessionId } = normalizeTleKeySessionBinding(root.tle_key_session);
  if (keySessionId !== sessionKeyId
    || !bytes(session.tle_key_transcript_hash, 32, "session.tle_key_transcript_hash").equals(
      bytes(keySession.transcript_hash, 32, "tle_key_session.transcript_hash"),
    )
    || !bytes(session.tle_master_public_key, 96, "session.tle_master_public_key").equals(
      bytes(keySession.group_public_key, 96, "tle_key_session.group_public_key"),
    )) {
    throw new Error("timed-OVN session differs from the complete public TLE transcript");
  }

  if (!Array.isArray(root.registration_records_hex)
    || root.registration_records_hex.length > PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1
    || (root.phase !== "Registered" && root.registration_records_hex.length === 0)) {
    throw new RangeError("registration_records_hex violates the casting-phase corpus bound");
  }
  const registrations = new Set();
  const registrationRecordsHex = root.registration_records_hex.map((value, index) => {
    const record = canonicalHex(value, `registration_records_hex[${index}]`);
    if (record.length !== PARLIAMENT_TIMED_OVN_REGISTRATION_RECORD_BYTES_V1 * 2
      || registrations.has(record)) {
      throw new TypeError("registration_records_hex is not an exact unique canonical corpus");
    }
    registrations.add(record);
    return record;
  });

  if (root.phase === "SurvivorsFrozen") {
    if (!Array.isArray(root.survivor_participant_hashes)
      || root.survivor_participant_hashes.length === 0
      || root.survivor_participant_hashes.length > registrationRecordsHex.length
      || root.release_identity === null) {
      throw new TypeError("SurvivorsFrozen requires bounded survivor hashes and release identity");
    }
    const survivors = new Set();
    root.survivor_participant_hashes.forEach((value, index) => {
      const hash = bytes(value, 32, `survivor_participant_hashes[${index}]`, true).toString("hex");
      if (survivors.has(hash)) throw new TypeError("survivor participant hashes must be unique");
      survivors.add(hash);
    });
    const releaseIdentity = exactObject(
      root.release_identity,
      TLE_RELEASE_IDENTITY_FIELDS,
      "release_identity",
    );
    if (canonicalId(releaseIdentity.tle_key_session_id, "release_identity.tle_key_session_id") !== sessionKeyId
      || canonicalId(releaseIdentity.governance_attempt_id, "release_identity.governance_attempt_id") !== governanceAttemptId
      || canonicalId(releaseIdentity.body_instance_id, "release_identity.body_instance_id") !== bodyInstanceId
      || canonicalId(releaseIdentity.ballot_attempt_id, "release_identity.ballot_attempt_id") !== ballotAttemptId
      || BigInt(unsigned(releaseIdentity.target_finalized_height, "release_identity.target_finalized_height")) !== BigInt(targetHeight)) {
      throw new Error("frozen release identity differs from the timed-OVN session");
    }
    for (const field of ["survivor_corpus_root", "no_recovery_root", "parameter_hash"]) {
      bytes(releaseIdentity[field], 32, `release_identity.${field}`, true);
    }
    if (!bytes(releaseIdentity.parameter_hash, 32, "release_identity.parameter_hash").equals(
      bytes(session.parameter_hash, 32, "session.parameter_hash"))) {
      throw new Error("frozen release identity parameter hash differs from the session");
    }
  } else if (root.survivor_participant_hashes !== null || root.release_identity !== null) {
    throw new TypeError("pre-freeze casting context must not expose frozen state");
  }

  const archiveNoritoBase64 = canonicalBoundedStandardBase64(
    root.archive_norito_base64,
    PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1,
    "archive_norito_base64",
  );
  return {
    ...root,
    registration_records_hex: registrationRecordsHex,
    archive_norito_base64: archiveNoritoBase64,
  };
}

/** Strictly admit one proof-carrying public partial and bind it to a fetched context. */
export function normalizeParliamentTlePartialReleaseShareV1(
  value,
  { expectedKeySessionId, expectedIdentityDigest, committeeSize },
) {
  const root = exactObject(value, TLE_PARTIAL_RELEASE_FIELDS, "Parliament TLE partial release");
  const keySessionId = canonicalId(root.key_session_id, "key_session_id");
  if (keySessionId !== canonicalId(expectedKeySessionId, "expectedKeySessionId")) {
    throw new Error("partial key_session_id differs from the authorized release context");
  }
  const identityDigest = bytes(root.identity_digest, 32, "identity_digest", true);
  if (!identityDigest.equals(bytes(expectedIdentityDigest, 32, "expectedIdentityDigest", true))) {
    throw new Error("partial identity_digest differs from the authorized release context");
  }
  const size = uint(
    committeeSize,
    PARLIAMENT_TLE_MAX_COMMITTEE_SIZE_V1,
    "committeeSize",
    4,
  );
  uint(root.participant_index, size, "participant_index", 1);
  bytes(root.sigma, 48, "sigma", true);
  bytes(root.proof_x, 96, "proof_x", true);
  bytes(root.proof_y, 48, "proof_y", true);
  bytes(root.z_s, 32, "z_s");
  bytes(root.z_r, 32, "z_r");
  bytes(root.z_u, 32, "z_u");
  return root;
}

function normalizeTleKeySessionBinding(value) {
  const keySession = exactObject(
    value,
    TLE_KEY_SESSION_BINDING_FIELDS,
    "tle_key_session",
  );
  version(keySession.version);
  const keySessionId = canonicalId(keySession.key_session_id, "tle_key_session.key_session_id");
  for (const field of ["network_id", "roster_hash", "dkg_event_hash", "transcript_hash"]) {
    bytes(keySession[field], 32, `tle_key_session.${field}`, true);
  }
  bytes(keySession.group_public_key, 96, "tle_key_session.group_public_key", true);
  bytes(keySession.generator_h, 96, "tle_key_session.generator_h", true);
  bytes(keySession.generator_v, 96, "tle_key_session.generator_v", true);
  const committeeSize = uint(
    keySession.committee_size,
    PARLIAMENT_TLE_MAX_COMMITTEE_SIZE_V1,
    "tle_key_session.committee_size",
    4,
  );
  const threshold = uint(keySession.threshold, 11, "tle_key_session.threshold", 2);
  if ((committeeSize - 1) % 3 !== 0
    || threshold !== Math.floor((committeeSize - 1) / 3) + 1) {
    throw new Error("tle_key_session committee_size/threshold is not an exact 3f+1/f+1 binding");
  }
  validateTlePublicTranscript(keySession, committeeSize, threshold);
  return { keySession, keySessionId, committeeSize, threshold };
}

function validateTlePublicTranscript(keySession, committeeSize, threshold) {
  if (!Array.isArray(keySession.qualified_dealers)
    || keySession.qualified_dealers.length < threshold
    || keySession.qualified_dealers.length > committeeSize) {
    throw new RangeError("tle_key_session.qualified_dealers violates the threshold/committee bound");
  }
  let previous = 0;
  for (const [index, dealer] of keySession.qualified_dealers.entries()) {
    uint(dealer, committeeSize, `tle_key_session.qualified_dealers[${index}]`, 1);
    if (dealer <= previous) throw new TypeError("qualified dealer indices must be strictly increasing");
    previous = dealer;
  }
  if (!Array.isArray(keySession.qualified_dealer_commitments)
    || keySession.qualified_dealer_commitments.length !== keySession.qualified_dealers.length) {
    throw new TypeError("qualified dealer commitments must align exactly with qualified_dealers");
  }
  for (const [index, value] of keySession.qualified_dealer_commitments.entries()) {
    const dealer = exactObject(
      value,
      TLE_DEALER_COMMITMENT_FIELDS,
      `tle_key_session.qualified_dealer_commitments[${index}]`,
    );
    if (uint(dealer.dealer_index, committeeSize, `dealer[${index}].dealer_index`, 1)
      !== keySession.qualified_dealers[index]) {
      throw new Error("dealer commitment index differs from the canonical qualified set");
    }
    if (!Array.isArray(dealer.coefficient_commitments)
      || dealer.coefficient_commitments.length !== threshold) {
      throw new TypeError("each dealer must carry the exact degree-f coefficient commitment set");
    }
    dealer.coefficient_commitments.forEach((commitment, coefficientIndex) =>
      bytes(commitment, 96, `dealer[${index}].coefficient_commitments[${coefficientIndex}]`, true));
    bytes(dealer.constant_pok_commitment, 96, `dealer[${index}].constant_pok_commitment`, true);
    bytes(dealer.constant_pok_response, 32, `dealer[${index}].constant_pok_response`);
  }
  if (!Array.isArray(keySession.public_shares)
    || keySession.public_shares.length !== committeeSize) {
    throw new TypeError("public_shares must contain the complete ordered committee");
  }
  keySession.public_shares.forEach((value, offset) => {
    const share = exactObject(value, TLE_PUBLIC_SHARE_FIELDS, `tle_key_session.public_shares[${offset}]`);
    if (uint(share.index, committeeSize, `public_shares[${offset}].index`, 1) !== offset + 1) {
      throw new TypeError("public share indices must be the exact one-based committee sequence");
    }
    bytes(share.participant_hash, 32, `public_shares[${offset}].participant_hash`, true);
    bytes(share.public_key_share, 96, `public_shares[${offset}].public_key_share`, true);
  });
}

function tleReleaseMessageDigest(keySession, identityPayload) {
  const u16 = (value) => {
    const bytes = Buffer.alloc(2);
    bytes.writeUInt16BE(value);
    return bytes;
  };
  const payloadLength = Buffer.alloc(4);
  payloadLength.writeUInt32BE(identityPayload.length);
  const framed = Buffer.concat([
    Buffer.from("iroha.threshold-bls.message.v1\0", "utf8"),
    Buffer.from("iroha.threshold-bls.session.v1\0", "utf8"),
    u16(1),
    Buffer.from([2]),
    Buffer.from(keySession.network_id),
    Buffer.from(keySession.key_session_id, "hex"),
    Buffer.from(keySession.roster_hash),
    u16(keySession.committee_size),
    u16(keySession.threshold),
    payloadLength,
    identityPayload,
  ]);
  return Buffer.from(sha256(framed));
}

function validateTleIdentityPayload(
  payload,
  governanceAttemptId,
  bodyInstanceId,
  ballotAttemptId,
  releaseIdentity,
  releaseHeight,
) {
  const domain = Buffer.from("iroha.parliament.tle.identity-payload.v1\0", "utf8");
  if (payload.length !== 243 || !payload.subarray(0, domain.length).equals(domain)) {
    throw new TypeError("identity_payload_hex has the wrong domain or canonical width");
  }
  let offset = domain.length;
  if (payload.readUInt16BE(offset) !== 1) throw new TypeError("identity payload version must equal one");
  offset += 2;
  for (const [expected, field] of [
    [governanceAttemptId, "governance_attempt_id"],
    [bodyInstanceId, "body_instance_id"],
    [ballotAttemptId, "ballot_attempt_id"],
    [Buffer.from(releaseIdentity.survivor_corpus_root).toString("hex"), "survivor_corpus_root"],
    [Buffer.from(releaseIdentity.no_recovery_root).toString("hex"), "no_recovery_root"],
  ]) {
    const actual = payload.subarray(offset, offset + 32).toString("hex");
    if (actual !== expected) throw new Error(`identity payload ${field} binding differs`);
    offset += 32;
  }
  const target = payload.readBigUInt64BE(offset);
  if (target !== BigInt(releaseHeight)) throw new Error("identity payload release height differs");
  offset += 8;
  if (payload.subarray(offset, offset + 32).toString("hex")
    !== Buffer.from(releaseIdentity.parameter_hash).toString("hex")) {
    throw new Error("identity payload parameter_hash binding differs");
  }
}

function validateBodyStates(value, requiredBodies) {
  if (!Array.isArray(value) || value.length !== requiredBodies.length || value.length < 1 || value.length > 10) {
    throw new TypeError("body_states must exactly match the required body pipeline");
  }
  const admittedStatuses = [
    "AwaitingSortition", "AcceptingInvitations", "RosterSealed", "Deliberating",
    "Balloting", "Approved", "Rejected", "NoQuorum", "NoResult", "Superseded",
  ];
  const admittedPhases = [
    "Orientation", "Evidence", "Questions", "Responses", "Deliberation", "Reflection", "Vote",
  ];
  const noResultKinds = new Set(PARLIAMENT_NO_RESULT_KINDS_V1.map(({ jsonTag }) => jsonTag));
  for (const [index, raw] of value.entries()) {
    const context = `body_states[${index}]`;
    const body = exactObject(raw, PARLIAMENT_BODY_STATE_FIELDS_V1, context);
    if (body.body !== requiredBodies[index].body) {
      throw new Error(`${context}.body differs from required_bodies order`);
    }
    if ((body.body_instance_id === null) !== (body.status === null)) {
      throw new TypeError(`${context} must bind body_instance_id and status together`);
    }
    if (body.body_instance_id !== null) canonicalId(body.body_instance_id, `${context}.body_instance_id`);
    let status = null;
    if (body.status !== null) {
      const tagged = plainObject(body.status, `${context}.status`);
      status = tagged.status;
      if (!admittedStatuses.includes(status)) throw new TypeError(`${context}.status is unknown`);
      exactObject(tagged, status === "Deliberating" ? ["status", "phase"] : ["status"], `${context}.status`);
      if (status === "Deliberating") {
        validateTaggedUnit(tagged.phase, "phase", admittedPhases, `${context}.status.phase`);
      }
    }
    const opened = body.public_finding_opened_at_height;
    const phaseBlocks = body.public_finding_phase_blocks;
    const deadline = body.public_finding_deadline_height;
    if ((opened === null) !== (phaseBlocks === null) || (opened === null) !== (deadline === null)) {
      throw new TypeError(`${context} must expose the complete public-finding schedule or none`);
    }
    if (opened !== null) {
      unsigned(opened, `${context}.public_finding_opened_at_height`);
      uint(phaseBlocks, Number.MAX_SAFE_INTEGER, `${context}.public_finding_phase_blocks`, 1);
      unsigned(deadline, `${context}.public_finding_deadline_height`);
      if (BigInt(deadline) !== BigInt(opened) + BigInt(phaseBlocks)) {
        throw new Error(`${context} public-finding deadline does not match its frozen schedule`);
      }
    }
    if ((body.no_result_kind === null) !== (body.no_result_height === null)) {
      throw new TypeError(`${context} must bind no-result kind and height together`);
    }
    if (body.no_result_kind !== null) {
      const kind = validateTaggedUnit(
        body.no_result_kind,
        "reason",
        [...noResultKinds],
        `${context}.no_result_kind`,
      );
      unsigned(body.no_result_height, `${context}.no_result_height`);
      if (status !== "NoResult") throw new Error(`${context} no-result facts require NoResult status`);
      const publicFailure = kind === "PublicFindingQuorumUnreachable" || kind === "PublicFindingDeadlineExpired";
      const privateBody = body.body === "policy-jury" || body.body === "confirmation-jury";
      if (publicFailure === privateBody) {
        throw new Error(`${context} no-result kind does not match the body's decision protocol`);
      }
    }
  }
}

function normalizePublicTransition(value) {
  rejectPrivateKeyFields(value, "transition");
  const root = plainObject(value, "transition");
  const tag = root.transition;
  if (typeof tag !== "string" || !PUBLIC_TRANSITIONS_BY_TAG.has(tag)) {
    throw new TypeError("transition tag is unknown, removed, or automatic-only");
  }
  const layout = PUBLIC_TRANSITIONS_BY_TAG.get(tag);
  exactObject(root, layout.jsonPayloadRequired ? ["transition", "payload"] : ["transition"], "transition");
  if (layout.jsonPayloadRequired) {
    exactObject(root.payload, TRANSITION_PAYLOAD_FIELDS.get(tag), `transition.${tag}.payload`);
    validateTransitionPayload(tag, root.payload);
  }
  return root;
}

function validateTransitionPayload(tag, payload) {
  const idFields = [
    "assignment_id", "ballot_attempt_id", "beacon_session_id", "body_instance_id",
    "election_attempt_id", "pulse_id", "release_beacon_session_id", "tle_key_session_id",
    "tle_session_id",
  ];
  for (const field of idFields) {
    if (Object.hasOwn(payload, field)) canonicalId(payload[field], `${tag}.${field}`);
  }
  for (const field of ["sequence"]) {
    if (Object.hasOwn(payload, field)) uint(payload[field], 0xffff_ffff, `${tag}.${field}`);
  }
  for (const field of ["pulse_height", "release_height"]) {
    if (Object.hasOwn(payload, field)) unsigned(payload[field], `${tag}.${field}`);
  }
  if (Object.hasOwn(payload, "result_root")) bytes(payload.result_root, 32, `${tag}.result_root`, true);
  if (tag === "RegisterBallotParticipant") {
    bytes(payload.registration_record, PARLIAMENT_TIMED_OVN_REGISTRATION_RECORD_BYTES_V1, `${tag}.registration_record`);
  }
  if (tag === "FreezeTimedOvnCorpus") {
    if (!Array.isArray(payload.ballot_records) || payload.ballot_records.length < 1 || payload.ballot_records.length > PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1) {
      throw new RangeError(`${tag}.ballot_records must contain one through 1000 records`);
    }
    payload.ballot_records.forEach((record, index) =>
      bytes(record, PARLIAMENT_TIMED_OVN_BALLOT_RECORD_BYTES_V1, `${tag}.ballot_records[${index}]`));
  }
  for (const field of ["request_ids", "ballot_attempt_ids"]) {
    if (Object.hasOwn(payload, field)) validateStrictIdList(payload[field], `${tag}.${field}`);
  }
  if (tag === "FinalizeOpenedBallot") {
    const release = exactObject(payload.final_release, ["key_session_id", "identity_digest", "signature"], `${tag}.final_release`);
    canonicalId(release.key_session_id, `${tag}.final_release.key_session_id`);
    bytes(release.identity_digest, 32, `${tag}.final_release.identity_digest`, true);
    bytes(release.signature, 48, `${tag}.final_release.signature`, true);
  }
}

/**
 * Validate one complete GovernanceCertificateV1 embedded outside an attempt response.
 *
 * The returned object retains the exact canonical JSON wire shape. Certificate
 * identifiers are content hashes computed by Rust; callers must bind those
 * separately because this JavaScript boundary does not reimplement Norito
 * fingerprinting.
 */
export function normalizeParliamentGovernanceCertificateV1(value) {
  return validateCertificate(value, undefined, { requirePolicyJury: true });
}

function validateCertificate(value, expectedAttemptId, options = {}) {
  const certificate = exactObject(value, CERTIFICATE_FIELDS, "certificate");
  canonicalId(certificate.proposal_content_id, "certificate.proposal_content_id");
  const attemptId = canonicalId(
    certificate.governance_attempt_id,
    "certificate.governance_attempt_id",
  );
  if (expectedAttemptId !== undefined && attemptId !== expectedAttemptId) {
    throw new Error("certificate.governance_attempt_id differs from attempt.id");
  }
  uint(certificate.governance_attempt_sequence, 0xffff_ffff, "certificate.governance_attempt_sequence");
  validateTaggedUnit(
    certificate.risk_tier,
    "tier",
    ["Routine", "Standard", "Constitutional", "Emergency"],
    "certificate.risk_tier",
  );
  if (!Array.isArray(certificate.body_bindings) || certificate.body_bindings.length < 1 || certificate.body_bindings.length > 10) {
    throw new TypeError("certificate.body_bindings must contain one through ten bindings");
  }
  const certifiedAtHeight = unsigned(
    certificate.certified_at_height,
    "certificate.certified_at_height",
  );
  const enactAtHeight = unsigned(certificate.enact_at_height, "certificate.enact_at_height");
  if (
    BigInt(certifiedAtHeight) === 0n ||
    BigInt(enactAtHeight) <= BigInt(certifiedAtHeight)
  ) {
    throw new TypeError("certificate enactment height must follow a positive certified height");
  }
  const bindings = certificate.body_bindings.map((binding, index) =>
    validateBodyCertificateBinding(binding, index, attemptId, certifiedAtHeight));
  for (let index = 1; index < bindings.length; index += 1) {
    if (bindings[index - 1].bodyIndex >= bindings[index].bodyIndex) {
      throw new TypeError("certificate.body_bindings must use strict canonical body order");
    }
  }
  for (const field of [
    "bodyInstanceId", "electionAttemptId", "sortitionRequestId",
    "ballotAttemptId", "tleSessionId", "releasePulseId", "releaseSlot",
  ]) {
    const values = bindings.map((binding) => binding[field]).filter((item) => item !== null);
    if (new Set(values).size !== values.length) {
      throw new TypeError(`certificate.body_bindings reuse ${field}`);
    }
  }
  const sortitionPulseIds = new Set(
    bindings.map((binding) => binding.sortitionPulseId),
  );
  if (
    bindings.some((binding) =>
      binding.releasePulseId !== null &&
      sortitionPulseIds.has(binding.releasePulseId))
  ) {
    throw new TypeError(
      "certificate must keep sortition and release pulse identifiers disjoint",
    );
  }
  if (options.requirePolicyJury === true) {
    const policyJuries = bindings.filter((binding) => binding.body === "policy-jury");
    const confirmationJuries = bindings.filter(
      (binding) => binding.body === "confirmation-jury",
    );
    if (policyJuries.length !== 1 || policyJuries[0].ballot === null) {
      throw new TypeError("certificate must contain exactly one approving policy-jury ballot");
    }
    const policy = policyJuries[0];
    const decisive = policy.ballot.tally.aye + policy.ballot.tally.nay;
    const narrow = decisive > 0 && Math.abs(
      policy.ballot.tally.aye - policy.ballot.tally.nay,
    ) * 100 < decisive * 5;
    if (
      (narrow && confirmationJuries.length !== 1) ||
      (!narrow && confirmationJuries.length !== 0)
    ) {
      throw new TypeError("certificate confirmation-jury presence differs from the policy margin");
    }
    if (narrow) {
      const confirmation = confirmationJuries[0];
      if (
        confirmation.sortitionRequest.requestHeight <= BigInt(policy.resultHeight) ||
        (
          confirmation.beaconSessionId === policy.beaconSessionId &&
          confirmation.sortitionPulseId === policy.sortitionPulseId
        )
      ) {
        throw new TypeError(
          "certificate confirmation-jury request and pulse must follow the policy result",
        );
      }
    }
  }
  bytes(certificate.effect_preimage_hash, 32, "certificate.effect_preimage_hash", true);
  validateExpectedHead(certificate.expected_head, "certificate.expected_head");
  if (BigInt(unsigned(certificate.policy_version, "certificate.policy_version")) === 0n) {
    throw new TypeError("certificate.policy_version must be positive");
  }
  return certificate;
}

function validateBodyCertificateBinding(value, index, governanceAttemptId, certifiedAtHeight) {
  const context = `certificate.body_bindings[${index}]`;
  const binding = exactObject(value, PARLIAMENT_CERTIFICATE_BODY_BINDING_FIELDS_V1, context);
  const seats = uint(
    binding.original_seats,
    0xffff_ffff,
    `${context}.original_seats`,
    1,
  );
  for (const field of [
    "body_instance_id", "election_attempt_id", "sortition_request_id",
    "beacon_session_id", "beacon_pulse_id",
  ]) canonicalId(binding[field], `${context}.${field}`);
  for (const field of ["roster_root", "assignment_root", "result_root"]) {
    bytes(binding[field], 32, `${context}.${field}`, true);
  }
  uint(binding.election_attempt_sequence, 0xffff_ffff, `${context}.election_attempt_sequence`);
  const resultHeight = unsigned(binding.result_height, `${context}.result_height`);
  const sortitionRequest = validateSortitionRequest(
    binding.sortition_request,
    binding,
    governanceAttemptId,
    `${context}.sortition_request`,
  );
  if (
    BigInt(resultHeight) <= sortitionRequest.pulseHeight ||
    BigInt(resultHeight) > BigInt(certifiedAtHeight)
  ) {
    throw new TypeError(`${context}.result_height violates the certificate lifecycle`);
  }
  const bodyIndex = PARLIAMENT_BODY_ORDER_V1.indexOf(binding.body);
  if (bodyIndex < 0) throw new TypeError(`${context}.body is unknown`);
  const isPrivateJury = binding.body === "policy-jury" || binding.body === "confirmation-jury";
  let ballot = null;
  if (isPrivateJury) {
    if (binding.public_finding !== null || binding.ballot === null) {
      throw new TypeError(`${context} private jury must carry ballot only`);
    }
    ballot = validateBallotCertificate(binding.ballot, binding, `${context}.ballot`);
  } else {
    if (binding.public_finding === null || binding.ballot !== null) {
      throw new TypeError(`${context} public body must carry public_finding only`);
    }
    validatePublicFinding(binding.public_finding, seats, `${context}.public_finding`);
  }
  return {
    ballot,
    ballotAttemptId: ballot?.ballotAttemptId ?? null,
    beaconSessionId: binding.beacon_session_id,
    body: binding.body,
    bodyIndex,
    bodyInstanceId: binding.body_instance_id,
    electionAttemptId: binding.election_attempt_id,
    releasePulseId: ballot?.releasePulseId ?? null,
    releaseSlot: ballot?.releaseSlot ?? null,
    resultHeight,
    sortitionPulseId: binding.beacon_pulse_id,
    sortitionRequest,
    sortitionRequestId: binding.sortition_request_id,
    tleSessionId: ballot?.tleSessionId ?? null,
  };
}

function validateSortitionRequest(value, binding, governanceAttemptId, context) {
  const request = exactObject(value, SORTITION_REQUEST_FIELDS, context);
  const id = canonicalId(request.id, `${context}.id`);
  const requestAttemptId = canonicalId(
    request.governance_attempt_id,
    `${context}.governance_attempt_id`,
  );
  const electionAttemptId = canonicalId(
    request.body_election_attempt_id,
    `${context}.body_election_attempt_id`,
  );
  const beaconSessionId = canonicalId(
    request.beacon_session_id,
    `${context}.beacon_session_id`,
  );
  bytes(request.candidate_root, 32, `${context}.candidate_root`, true);
  uint(request.candidate_count, 0xffff_ffff, `${context}.candidate_count`, 1);
  uint(
    request.target_seats,
    PARLIAMENT_BODY_TARGET_SEATS_MAX_V1,
    `${context}.target_seats`,
    1,
  );
  const requestHeight = BigInt(unsigned(request.request_height, `${context}.request_height`));
  const pulseHeight = BigInt(unsigned(request.pulse_height, `${context}.pulse_height`));
  if (pulseHeight === 0n || pulseHeight <= requestHeight) {
    throw new TypeError(`${context}.pulse_height must be strictly after request_height`);
  }
  if (
    id !== binding.sortition_request_id ||
    requestAttemptId !== governanceAttemptId ||
    electionAttemptId !== binding.election_attempt_id ||
    request.body !== binding.body ||
    beaconSessionId !== binding.beacon_session_id
  ) {
    throw new TypeError(`${context} differs from its repeated certificate bindings`);
  }
  return { pulseHeight, requestHeight };
}

function validateBallotCertificate(value, binding, context) {
  const ballot = exactObject(value, BALLOT_CERTIFICATE_FIELDS, context);
  const ballotAttemptId = canonicalId(ballot.ballot_attempt_id, `${context}.ballot_attempt_id`);
  const tleSessionId = canonicalId(ballot.tle_session_id, `${context}.tle_session_id`);
  canonicalId(ballot.tle_key_session_id, `${context}.tle_key_session_id`);
  canonicalId(
    ballot.release_beacon_session_id,
    `${context}.release_beacon_session_id`,
  );
  const releasePulseId = canonicalId(ballot.release_pulse_id, `${context}.release_pulse_id`);
  for (const field of [
    "registration_root", "dropout_root", "survivor_root", "corpus_root",
    "no_recovery_root", "timed_commitment_root", "opening_root",
  ]) bytes(ballot[field], 32, `${context}.${field}`, true);
  const sequence = uint(
    ballot.ballot_attempt_sequence,
    0xffff_ffff,
    `${context}.ballot_attempt_sequence`,
  );
  const maxRetries = uint(
    ballot.max_ballot_retries,
    PARLIAMENT_BALLOT_RETRIES_MAX_V1,
    `${context}.max_ballot_retries`,
  );
  if (sequence > maxRetries) {
    throw new TypeError(`${context}.ballot_attempt_sequence exceeds max_ballot_retries`);
  }
  const maxCorpusEntries = uint(
    ballot.max_corpus_entries,
    PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1,
    `${context}.max_corpus_entries`,
    1,
  );
  const heights = {};
  for (const field of [
    "registered_at_height", "registration_close_height", "survivor_freeze_height",
    "commitment_close_height", "registration_closed_at_height",
    "survivors_frozen_at_height", "commitment_closed_at_height", "release_height",
    "opening_deadline_height", "opening_height",
  ]) heights[field] = BigInt(unsigned(ballot[field], `${context}.${field}`));
  if (
    heights.registered_at_height === 0n ||
    heights.registration_close_height <= heights.registered_at_height ||
    heights.survivor_freeze_height <= heights.registration_close_height ||
    heights.commitment_close_height <= heights.survivor_freeze_height ||
    heights.release_height <= heights.commitment_close_height ||
    heights.opening_deadline_height <= heights.release_height ||
    heights.registration_closed_at_height !== heights.registration_close_height ||
    heights.survivors_frozen_at_height !== heights.survivor_freeze_height ||
    heights.commitment_closed_at_height !== heights.commitment_close_height ||
    heights.opening_height < heights.release_height ||
    heights.opening_height > heights.opening_deadline_height ||
    BigInt(binding.result_height) < heights.opening_height ||
    BigInt(binding.result_height) > heights.opening_deadline_height
  ) {
    throw new TypeError(`${context} violates the frozen ballot lifecycle`);
  }
  const tally = exactObject(ballot.tally, BALLOT_TALLY_FIELDS, `${context}.tally`);
  const normalizedTally = {};
  for (const field of BALLOT_TALLY_FIELDS) {
    normalizedTally[field] = uint(tally[field], 0xffff_ffff, `${context}.tally.${field}`);
  }
  if (
    normalizedTally.original_seats !== binding.original_seats ||
    normalizedTally.accepted_ballots > maxCorpusEntries ||
    normalizedTally.aye + normalizedTally.nay + normalizedTally.abstain !==
      normalizedTally.accepted_ballots ||
    normalizedTally.accepted_ballots > normalizedTally.original_seats
  ) {
    throw new TypeError(`${context}.tally violates count conservation`);
  }
  const quorum = Math.floor((2 * normalizedTally.original_seats + 2) / 3);
  const outcome = validateTaggedUnit(
    ballot.outcome,
    "outcome",
    ["Approved", "Rejected", "NoQuorum", "NoResult"],
    `${context}.outcome`,
  );
  if (
    outcome !== "Approved" ||
    normalizedTally.accepted_ballots < quorum ||
    normalizedTally.aye <= normalizedTally.nay
  ) {
    throw new TypeError(`${context} must contain an approving aggregate outcome`);
  }
  return {
    ballotAttemptId,
    releasePulseId,
    releaseSlot: `${ballot.release_beacon_session_id}:${heights.release_height}`,
    tally: normalizedTally,
    tleSessionId,
  };
}

function validatePublicFinding(value, originalSeats, context) {
  const finding = exactObject(value, PARLIAMENT_PUBLIC_FINDING_CERTIFICATE_FIELDS_V1, context);
  bytes(finding.endorsement_root, 32, `${context}.endorsement_root`, true);
  validateStrictIdList(finding.endorsing_assignments, `${context}.endorsing_assignments`);
  const endorsements = uint(finding.endorsements, 1_000, `${context}.endorsements`, 1);
  const quorum = uint(finding.quorum, 1_000, `${context}.quorum`, 1);
  const expectedQuorum = Math.floor((2 * originalSeats + 2) / 3);
  if (endorsements !== finding.endorsing_assignments.length || endorsements !== quorum || quorum !== expectedQuorum) {
    throw new Error(`${context} must contain the exact canonical 2/3 supporter list`);
  }
}

function validateExpectedHead(value, context) {
  const root = exactObject(value, ["state", "head"], context);
  if (root.state === "Absent") {
    const head = exactObject(root.head, ["subject_id"], `${context}.head`);
    bytes(head.subject_id, 32, `${context}.head.subject_id`, true);
  } else if (root.state === "Present") {
    const head = exactObject(root.head, ["subject_id", "version", "head_root"], `${context}.head`);
    bytes(head.subject_id, 32, `${context}.head.subject_id`, true);
    unsigned(head.version, `${context}.head.version`);
    bytes(head.head_root, 32, `${context}.head.head_root`, true);
  } else {
    throw new TypeError(`${context}.state must be Absent or Present`);
  }
}

function instruction(value, expectedWireId) {
  if (!Array.isArray(value) || value.length !== 1) {
    throw new TypeError("Parliament draft response must contain exactly one instruction");
  }
  const draft = exactObject(value[0], ["wire_id", "payload_hex"], "tx_instructions[0]");
  if (draft.wire_id !== expectedWireId) throw new Error("instruction draft has the wrong wire_id");
  return { wire_id: expectedWireId, payload_hex: canonicalHex(draft.payload_hex, "payload_hex") };
}

function validateStrictIdList(value, context) {
  if (!Array.isArray(value) || value.length < 1 || value.length > 1_000) {
    throw new RangeError(`${context} must contain one through 1000 identifiers`);
  }
  let previous = null;
  for (const [index, item] of value.entries()) {
    const current = canonicalId(item, `${context}[${index}]`);
    if (previous !== null && previous >= current) {
      throw new TypeError(`${context} must be strictly increasing and distinct`);
    }
    previous = current;
  }
}

function validateTaggedUnit(value, tagField, accepted, context) {
  const root = exactObject(value, [tagField], context);
  if (!accepted.includes(root[tagField])) throw new TypeError(`${context}.${tagField} is unknown`);
  return root[tagField];
}

function exactObject(value, fields, context) {
  const root = plainObject(value, context);
  const actual = Object.keys(root);
  if (actual.length !== fields.length || actual.some((key) => !fields.includes(key))) {
    throw new TypeError(`${context} contains unknown, aliased, or missing fields`);
  }
  return root;
}

function plainObject(value, context) {
  const prototype = value === null || typeof value !== "object"
    ? undefined
    : Object.getPrototypeOf(value);
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    (prototype !== Object.prototype && prototype !== null)
  ) {
    throw new TypeError(`${context} must be a plain object`);
  }
  return value;
}

function canonicalId(value, context) {
  if (typeof value !== "string" || !LOWER_HEX_32.test(value) || /^0{64}$/u.test(value)) {
    throw new TypeError(`${context} must be exactly 64 lowercase non-zero hexadecimal characters`);
  }
  return value;
}

function canonicalHex(value, context) {
  if (typeof value !== "string" || !LOWER_HEX_BYTES.test(value)) {
    throw new TypeError(`${context} must be non-empty complete lowercase hexadecimal bytes`);
  }
  return value;
}

function canonicalBoundedStandardBase64(value, maximumBytes, context) {
  if (typeof value !== "string" || value.length === 0 || !STANDARD_PADDED_BASE64.test(value)) {
    throw new TypeError(`${context} must be non-empty canonical padded standard base64`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.length === 0 || decoded.length > maximumBytes || decoded.toString("base64") !== value) {
    throw new RangeError(`${context} exceeds its byte bound or is not canonical base64`);
  }
  return value;
}

function bytes(value, length, context, nonZero = false) {
  const buffer = Buffer.isBuffer(value)
    ? Buffer.from(value)
    : Array.isArray(value) && value.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)
      ? Buffer.from(value)
      : null;
  if (buffer === null || buffer.length !== length) {
    throw new TypeError(`${context} must contain exactly ${length} bytes`);
  }
  if (nonZero && buffer.every((byte) => byte === 0)) throw new TypeError(`${context} must be non-zero`);
  return buffer;
}

function uint(value, maximum, context, minimum = 0) {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new TypeError(`${context} must be an integer from ${minimum} through ${maximum}`);
  }
  return value;
}

function unsigned(value, context) {
  if ((typeof value === "number" && Number.isSafeInteger(value) && value >= 0)
    || (typeof value === "bigint" && value >= 0n)) return value;
  throw new TypeError(`${context} must be a losslessly decoded unsigned integer`);
}

function optionalUnsigned(value, context) {
  if (value !== null) unsigned(value, context);
}

function optionalBytes32(value, context) {
  if (value !== null) bytes(value, 32, context, true);
}

function version(value) {
  if (value !== PARLIAMENT_API_VERSION_V1) throw new TypeError("unsupported Parliament API version");
}

function rejectPrivateKeyFields(value, context) {
  const pending = [[value, context]];
  const seen = new WeakSet();
  while (pending.length > 0) {
    const [current, path] = pending.pop();
    if (current === null || typeof current !== "object") continue;
    if (seen.has(current)) continue;
    seen.add(current);
    if (Array.isArray(current)) {
      current.forEach((item, index) => pending.push([item, `${path}[${index}]`]));
      continue;
    }
    for (const [key, item] of Object.entries(current)) {
      if (PRIVATE_KEY_FIELDS.has(key)) throw new TypeError(`${path}.${key} is forbidden; sign drafts locally`);
      pending.push([item, `${path}.${key}`]);
    }
  }
}
