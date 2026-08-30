/** Closed public model contract for authenticated Exact12 action submission. */

/** Taira V1 `max_tx_bytes`, shared with native Exact12 action inspection. */
export const PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1 =
  10 * 1024 * 1024;

/** Canonical public order of the thirteen Exact12 operations. */
export const PRIVACY_EXACT12_ACTION_OPERATIONS_V1 = Object.freeze([
  "zk_ace_authorization_action_v1",
  "anonymous_pgc_payment_action_v1",
  "verange_range_proof_v1",
  "zk_ams_batch_admission_action_v1",
  "zk_ams_provision_account_action_v1",
  "vega_credential_presentation_v1",
  "zk_x509_identity_presentation_v1",
  "jindo_polynomial_evaluation_v1",
  "bootle_lantern_credential_presentation_v1",
  "orchard_note_action_v1",
  "fcmp_membership_payment_v1",
  "ivm_private_note_action_v1",
  "pq_masp_note_action_v1",
]);

/** Closed typed ledger-effect vocabulary for successful Exact12 operations. */
export const PRIVACY_LEDGER_EFFECT_KINDS_V1 = Object.freeze([
  "verification_only",
  "zk_ace_transparent_transfer",
  "anonymous_pgc_account_state_transition",
  "zk_ams_batch_admission",
  "zk_ams_provision_account",
  "zk_x509_certificate_nullifier",
  "orchard_note_state_transition",
  "fcmp_membership_payment",
  "ivm_private_note_state_transition",
  "pq_masp_note_state_transition",
]);

const ACTION_SPECS_V1 = Object.freeze({
  zk_ace_authorization_action_v1: Object.freeze({
    protocolId: "zk-ace-pq-authorization-v0",
    ledgerEffectKind: "zk_ace_transparent_transfer",
  }),
  anonymous_pgc_payment_action_v1: Object.freeze({
    protocolId: "anonymous-pgc-k-out-of-n-v1",
    ledgerEffectKind: "anonymous_pgc_account_state_transition",
  }),
  verange_range_proof_v1: Object.freeze({
    protocolId: "verange-transparent-range-v1",
    ledgerEffectKind: "verification_only",
  }),
  zk_ams_batch_admission_action_v1: Object.freeze({
    protocolId: "iroha-zk-ams-v1",
    ledgerEffectKind: "zk_ams_batch_admission",
  }),
  zk_ams_provision_account_action_v1: Object.freeze({
    protocolId: "iroha-zk-ams-v1",
    ledgerEffectKind: "zk_ams_provision_account",
  }),
  vega_credential_presentation_v1: Object.freeze({
    protocolId: "vega-existing-credential-zk-v0",
    ledgerEffectKind: "verification_only",
  }),
  zk_x509_identity_presentation_v1: Object.freeze({
    protocolId: "iroha-zk-x509-stark-p256-v0",
    ledgerEffectKind: "zk_x509_certificate_nullifier",
  }),
  jindo_polynomial_evaluation_v1: Object.freeze({
    protocolId: "iroha-jindo-polynomial-commitment-v0",
    ledgerEffectKind: "verification_only",
  }),
  bootle_lantern_credential_presentation_v1: Object.freeze({
    protocolId: "iroha-bootle-lantern-anoncred-v1",
    ledgerEffectKind: "verification_only",
  }),
  orchard_note_action_v1: Object.freeze({
    protocolId: "orchard-halo2-actions-v1",
    ledgerEffectKind: "orchard_note_state_transition",
  }),
  fcmp_membership_payment_v1: Object.freeze({
    protocolId: "monero-fcmp-plus-plus-v1",
    ledgerEffectKind: "fcmp_membership_payment",
  }),
  ivm_private_note_action_v1: Object.freeze({
    protocolId: "iroha-ivm-private-note-stark-v1",
    ledgerEffectKind: "ivm_private_note_state_transition",
  }),
  pq_masp_note_action_v1: Object.freeze({
    protocolId: "pq-masp-stark-v0",
    ledgerEffectKind: "pq_masp_note_state_transition",
  }),
});

const MAX_U64 = (1n << 64n) - 1n;
const requestState = new WeakMap();
const viewState = new WeakMap();

/** Fail-closed validation error for an Exact12 public action model. */
export class PrivacyExact12ActionModelErrorV1 extends TypeError {
  constructor(message, path = "Exact12 action model") {
    super(`${path}: ${message}`);
    this.name = "PrivacyExact12ActionModelErrorV1";
    this.path = path;
  }
}

function fail(message, path) {
  throw new PrivacyExact12ActionModelErrorV1(message, path);
}

function operationSpec(operation, path = "operation") {
  if (
    typeof operation !== "string"
    || !Object.hasOwn(ACTION_SPECS_V1, operation)
  ) {
    fail("must be one exact PrivacyExact12ActionOperationV1", path);
  }
  return ACTION_SPECS_V1[operation];
}

function copyBytes(value, path) {
  if (!(value instanceof Uint8Array)) {
    fail("must be a Uint8Array", path);
  }
  return Uint8Array.from(value);
}

function nonzeroFixed32(value, path) {
  const bytes = copyBytes(value, path);
  if (bytes.byteLength !== 32 || !bytes.some((byte) => byte !== 0)) {
    fail("must contain exactly 32 non-zero bytes", path);
  }
  return bytes;
}

function positiveU64(value, path) {
  let exact;
  if (typeof value === "bigint") {
    exact = value;
  } else if (Number.isSafeInteger(value) && !Object.is(value, -0)) {
    exact = BigInt(value);
  } else {
    fail("must be an exact positive u64 integer", path);
  }
  if (exact <= 0n || exact > MAX_U64) {
    fail("must be an exact positive u64 integer", path);
  }
  return exact;
}

function optionalValue(value) {
  return value === undefined ? null : value;
}

/** Return the sole retained protocol that executes an Exact12 operation. */
export function privacyExact12ProtocolIdV1(operation) {
  return operationSpec(operation).protocolId;
}

/** Return the typed ledger effect committed by an Exact12 operation. */
export function privacyExact12LedgerEffectKindV1(operation) {
  return operationSpec(operation).ledgerEffectKind;
}

/** JavaScript-friendly access to the closed operation mappings. */
export const PrivacyExact12ActionContractV1 = Object.freeze({
  protocolId: privacyExact12ProtocolIdV1,
  ledgerEffectKind: privacyExact12LedgerEffectKindV1,
});

/**
 * One closed Exact12 operation and its already-signed versioned transaction.
 *
 * The model snapshots and bounds public wire bytes. It performs no local proof
 * acceptance and grants no capability or submission authority. Its optional
 * expectedManifestDigest is only a pre-submit observation check; it is not a
 * signed consensus precondition. The finalized receipt reports the manifest
 * actually admitted by native execution.
 */
export class PrivacyExact12ActionRequestV1 {
  constructor(optionsOrOperation, signedTransactionVersioned, expectedManifestDigest) {
    let operation;
    let wire;
    let digest;
    if (typeof optionsOrOperation === "string") {
      operation = optionsOrOperation;
      wire = signedTransactionVersioned;
      digest = optionalValue(expectedManifestDigest);
    } else {
      if (
        optionsOrOperation === null
        || typeof optionsOrOperation !== "object"
        || Array.isArray(optionsOrOperation)
      ) {
        fail("must be an options object or an exact operation", "request");
      }
      operation = optionsOrOperation.operation;
      wire = optionsOrOperation.signedTransactionVersioned;
      digest = optionalValue(optionsOrOperation.expectedManifestDigest);
    }

    operationSpec(operation);
    const canonicalWire = copyBytes(wire, "signedTransactionVersioned");
    if (
      canonicalWire.byteLength === 0
      || canonicalWire.byteLength > PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1
    ) {
      fail(
        `must contain 1..${PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1} bytes`,
        "signedTransactionVersioned",
      );
    }
    const canonicalDigest = digest === null
      ? null
      : nonzeroFixed32(digest, "expectedManifestDigest");

    this.operation = operation;
    requestState.set(this, {
      signedTransactionVersioned: canonicalWire,
      expectedManifestDigest: canonicalDigest,
    });
    Object.freeze(this);
  }

  get signedTransactionVersioned() {
    const state = requestState.get(this);
    if (!state) fail("invalid receiver", "PrivacyExact12ActionRequestV1");
    return Uint8Array.from(state.signedTransactionVersioned);
  }

  get expectedManifestDigest() {
    const state = requestState.get(this);
    if (!state) fail("invalid receiver", "PrivacyExact12ActionRequestV1");
    return state.expectedManifestDigest === null
      ? null
      : Uint8Array.from(state.expectedManifestDigest);
  }
}

/**
 * Immutable public state of one authenticated Exact12 action submission.
 *
 * Construction validates the closed mappings, non-zero hashes, exact heights,
 * and the complete submitted/terminal state relationship.
 */
export class PrivacyActionOperationViewV1 {
  constructor(options) {
    if (options === null || typeof options !== "object" || Array.isArray(options)) {
      fail("must be an options object", "operation view");
    }
    const {
      protocolId,
      operationSchema,
      transactionHash,
      transactionIntentDigest,
      statementDigest,
      proofEnvelopeHash,
      localState,
      ledgerEffectKind,
      capabilityManifestDigest,
    } = options;
    const terminalChainState = optionalValue(options.terminalChainState);
    const committedHeight = optionalValue(options.committedHeight);
    const rejectionReason = optionalValue(options.rejectionReason);
    const executionCapabilityManifestDigest = optionalValue(
      options.executionCapabilityManifestDigest,
    );
    const executionCapabilityCommittedHeight = optionalValue(
      options.executionCapabilityCommittedHeight,
    );
    const executionReceiptFinalizedHeight = optionalValue(
      options.executionReceiptFinalizedHeight,
    );
    const executionReceiptFinalizedBlockHash = optionalValue(
      options.executionReceiptFinalizedBlockHash,
    );
    const spec = operationSpec(operationSchema, "operationSchema");

    if (protocolId !== spec.protocolId) {
      fail("does not match operationSchema", "protocolId");
    }
    if (ledgerEffectKind !== spec.ledgerEffectKind) {
      fail("does not match operationSchema", "ledgerEffectKind");
    }

    const bytes = {
      transactionHash: nonzeroFixed32(transactionHash, "transactionHash"),
      transactionIntentDigest: nonzeroFixed32(
        transactionIntentDigest,
        "transactionIntentDigest",
      ),
      statementDigest: nonzeroFixed32(statementDigest, "statementDigest"),
      proofEnvelopeHash: nonzeroFixed32(proofEnvelopeHash, "proofEnvelopeHash"),
      capabilityManifestDigest: nonzeroFixed32(
        capabilityManifestDigest,
        "capabilityManifestDigest",
      ),
      executionCapabilityManifestDigest:
        executionCapabilityManifestDigest === null
          ? null
          : nonzeroFixed32(
              executionCapabilityManifestDigest,
              "executionCapabilityManifestDigest",
            ),
      executionReceiptFinalizedBlockHash:
        executionReceiptFinalizedBlockHash === null
          ? null
          : nonzeroFixed32(
              executionReceiptFinalizedBlockHash,
              "executionReceiptFinalizedBlockHash",
            ),
    };
    const capabilityCommittedHeight = positiveU64(
      options.capabilityCommittedHeight,
      "capabilityCommittedHeight",
    );
    const exactCommittedHeight = committedHeight === null
      ? null
      : positiveU64(committedHeight, "committedHeight");
    const exactExecutionCapabilityCommittedHeight =
      executionCapabilityCommittedHeight === null
        ? null
        : positiveU64(
            executionCapabilityCommittedHeight,
            "executionCapabilityCommittedHeight",
          );
    const exactExecutionReceiptFinalizedHeight =
      executionReceiptFinalizedHeight === null
        ? null
        : positiveU64(
            executionReceiptFinalizedHeight,
            "executionReceiptFinalizedHeight",
          );
    const hasCompleteExecutionEvidence =
      bytes.executionCapabilityManifestDigest !== null
      && exactExecutionCapabilityCommittedHeight !== null
      && exactExecutionReceiptFinalizedHeight !== null
      && bytes.executionReceiptFinalizedBlockHash !== null;
    const hasAnyExecutionEvidence =
      bytes.executionCapabilityManifestDigest !== null
      || exactExecutionCapabilityCommittedHeight !== null
      || exactExecutionReceiptFinalizedHeight !== null
      || bytes.executionReceiptFinalizedBlockHash !== null;

    if (localState === "submitted") {
      if (
        terminalChainState !== null
        || exactCommittedHeight !== null
        || rejectionReason !== null
        || hasAnyExecutionEvidence
      ) {
        fail("submitted actions cannot carry terminal fields", "localState");
      }
    } else if (localState === "terminal") {
      if (terminalChainState === "Committed") {
        if (exactCommittedHeight === null || rejectionReason !== null) {
          fail(
            "successful actions require only an authenticated committed height",
            "terminalChainState",
          );
        }
        if (hasAnyExecutionEvidence) {
          fail(
            "legacy committed actions cannot carry finalized execution evidence",
            "terminalChainState",
          );
        }
      } else if (terminalChainState === "Applied") {
        if (
          exactCommittedHeight === null
          || rejectionReason !== null
          || !hasCompleteExecutionEvidence
        ) {
          fail(
            "applied actions require committed and finalized execution evidence",
            "terminalChainState",
          );
        }
        if (
          exactExecutionCapabilityCommittedHeight > exactCommittedHeight
          || exactExecutionReceiptFinalizedHeight < exactCommittedHeight
        ) {
          fail(
            "applied execution-receipt heights are inconsistent",
            "terminalChainState",
          );
        }
      } else if (terminalChainState === "Rejected") {
        if (exactCommittedHeight === null) {
          fail(
            "rejected actions require an authenticated committed height",
            "terminalChainState",
          );
        }
        if (
          typeof rejectionReason !== "string"
          || rejectionReason.length === 0
          || new TextEncoder().encode(rejectionReason).byteLength > 1_024
          || rejectionReason.trim() !== rejectionReason
          || /[\u0000-\u001f\u007f-\u009f]/u.test(rejectionReason)
        ) {
          fail(
            "rejected actions require one canonical non-empty reason",
            "rejectionReason",
          );
        }
        if (hasAnyExecutionEvidence) {
          fail(
            "rejected actions cannot carry successful execution evidence",
            "terminalChainState",
          );
        }
      } else if (terminalChainState === "Expired") {
        if (
          exactCommittedHeight !== null
          || rejectionReason !== null
          || hasAnyExecutionEvidence
        ) {
          fail("expired actions cannot carry committed fields", "terminalChainState");
        }
      } else {
        fail(
          "terminal actions require one supported terminal chain state",
          "terminalChainState",
        );
      }
    } else {
      fail("must be submitted or terminal", "localState");
    }
    if (
      localState === "terminal"
      && exactCommittedHeight !== null
      && exactCommittedHeight < capabilityCommittedHeight
    ) {
      fail(
        "terminal committed height precedes the finalized pre-submit capability snapshot",
        "committedHeight",
      );
    }

    this.protocolId = protocolId;
    this.operationSchema = operationSchema;
    this.localState = localState;
    this.terminalChainState = terminalChainState;
    this.committedHeight = exactCommittedHeight;
    this.rejectionReason = rejectionReason;
    this.ledgerEffectKind = ledgerEffectKind;
    // This pair is the fresh, pre-submit admission snapshot. It is deliberately
    // distinct from the capability manifest recorded by native execution.
    this.capabilityCommittedHeight = capabilityCommittedHeight;
    this.executionCapabilityCommittedHeight =
      exactExecutionCapabilityCommittedHeight;
    this.executionReceiptFinalizedHeight = exactExecutionReceiptFinalizedHeight;
    viewState.set(this, bytes);
    Object.freeze(this);
  }

  get transactionHash() {
    return copyViewBytes(this, "transactionHash");
  }

  get transactionIntentDigest() {
    return copyViewBytes(this, "transactionIntentDigest");
  }

  get statementDigest() {
    return copyViewBytes(this, "statementDigest");
  }

  get proofEnvelopeHash() {
    return copyViewBytes(this, "proofEnvelopeHash");
  }

  get capabilityManifestDigest() {
    return copyViewBytes(this, "capabilityManifestDigest");
  }

  get executionCapabilityManifestDigest() {
    return copyOptionalViewBytes(this, "executionCapabilityManifestDigest");
  }

  get executionReceiptFinalizedBlockHash() {
    return copyOptionalViewBytes(this, "executionReceiptFinalizedBlockHash");
  }
}

function copyViewBytes(receiver, field) {
  const state = viewState.get(receiver);
  if (!state) fail("invalid receiver", "PrivacyActionOperationViewV1");
  return Uint8Array.from(state[field]);
}

function copyOptionalViewBytes(receiver, field) {
  const state = viewState.get(receiver);
  if (!state) fail("invalid receiver", "PrivacyActionOperationViewV1");
  return state[field] === null ? null : Uint8Array.from(state[field]);
}
