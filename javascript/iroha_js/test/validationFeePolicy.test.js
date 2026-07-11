import { test } from "node:test";
import assert from "node:assert/strict";
import {
  encodeValidationFeePolicyNorito,
  validationFeePolicyHash,
  validationFeePolicyLedgerSignaturePayload,
  validationFeeQuantity,
  verifySignedValidationFeePolicy,
  verifyValidationFeePolicyRegistry,
} from "../src/validationFeePolicy.js";
import {
  VALIDATION_FEE_POLICY_CANONICAL_BYTES_HEX,
  VALIDATION_FEE_POLICY_HASH_HEX,
  VALIDATION_FEE_POLICY_SIGNATURE_PAYLOAD_HEX,
  validationFeePolicyFixture,
} from "./fixtures/validationFeePolicyV1.js";

test("ValidationFeePolicyV1 uses the canonical Norito bytes, hash, and ledger signature payload", () => {
  const fixture = validationFeePolicyFixture();
  assert.equal(
    Buffer.from(encodeValidationFeePolicyNorito(fixture.policy)).toString("hex"),
    VALIDATION_FEE_POLICY_CANONICAL_BYTES_HEX,
  );
  assert.equal(
    validationFeePolicyHash(fixture.policy),
    VALIDATION_FEE_POLICY_HASH_HEX,
  );
  assert.equal(
    Buffer.from(validationFeePolicyLedgerSignaturePayload(fixture.policy)).toString(
      "hex",
    ),
    VALIDATION_FEE_POLICY_SIGNATURE_PAYLOAD_HEX,
  );
});

test("verifySignedValidationFeePolicy verifies active registry and threshold signatures", () => {
  const fixture = validationFeePolicyFixture();
  const verified = verifySignedValidationFeePolicy(
    fixture.signedPolicy,
    fixture.verificationContext,
  );
  assert.equal(verified.policyHashHex, VALIDATION_FEE_POLICY_HASH_HEX);
  assert.equal(verified.policyVersion, 1n);
  assert.equal(verified.validSignatureCount, 2);
  assert.equal(verified.validSignatureWeight, 2n);
  assert.equal(verified.registry.registeredPolicyCount, 1);
  assert.equal(validationFeeQuantity(fixture.policy, 1), "0.10");
  assert.equal(validationFeeQuantity(fixture.policy, 3), "0.30");
});

test("verifySignedValidationFeePolicy honors weighted keyset thresholds", () => {
  const fixture = validationFeePolicyFixture();
  fixture.verificationContext.governanceKeyset = {
    keyset_id: fixture.governanceKeyset.keyset_id,
    threshold: 3,
    keys: fixture.governanceKeyset.public_keys_hex.map((public_key, index) => ({
      public_key: `ed25519:ed0120${public_key}`,
      weight: index === 0 ? 2 : 1,
    })),
  };
  const verified = verifySignedValidationFeePolicy(
    fixture.signedPolicy,
    fixture.verificationContext,
  );
  assert.equal(verified.validSignatureWeight, 3n);
});

test("verifySignedValidationFeePolicy rejects tampered policy signatures", () => {
  const fixture = validationFeePolicyFixture();
  fixture.signedPolicy.signatures[0].signature =
    `00${fixture.signedPolicy.signatures[0].signature.slice(2)}`;
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        fixture.signedPolicy,
        fixture.verificationContext,
      ),
    (error) => error?.code === "INVALID_SIGNATURE",
  );
});

test("verifySignedValidationFeePolicy rejects wrong network, genesis, and inactive policy", () => {
  for (const [contextOverride, expectedCode] of [
    [{ networkId: "other-network" }, "WRONG_NETWORK"],
    [{ genesisHash: "08".repeat(32) }, "WRONG_GENESIS"],
    [{ currentHeight: 9 }, "FUTURE_POLICY"],
    [{ currentHeight: 100 }, "EXPIRED_POLICY"],
  ]) {
    const fixture = validationFeePolicyFixture({
      verificationContext: contextOverride,
    });
    assert.throws(
      () =>
        verifySignedValidationFeePolicy(
          fixture.signedPolicy,
          fixture.verificationContext,
        ),
      (error) => error?.code === expectedCode,
      expectedCode,
    );
  }
});

test("verifySignedValidationFeePolicy always requires the active policy and rejects alias conflicts", () => {
  for (const activeOverride of [
    { requireActive: false },
    { require_active: false },
  ]) {
    const fixture = validationFeePolicyFixture({
      verificationContext: activeOverride,
    });
    assert.throws(
      () =>
        verifySignedValidationFeePolicy(
          fixture.signedPolicy,
          fixture.verificationContext,
        ),
      (error) => error?.code === "ACTIVE_POLICY_REQUIRED",
    );
  }

  const conflictingContext = validationFeePolicyFixture();
  conflictingContext.verificationContext.network_id =
    conflictingContext.verificationContext.networkId;
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        conflictingContext.signedPolicy,
        conflictingContext.verificationContext,
      ),
    (error) => error?.code === "CONFLICTING_ALIASES",
  );

  const conflictingSignature = validationFeePolicyFixture();
  conflictingSignature.signedPolicy.signatures[0].public_key =
    conflictingSignature.signedPolicy.signatures[0].signer_public_key;
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        conflictingSignature.signedPolicy,
        conflictingSignature.verificationContext,
      ),
    (error) => error?.code === "CONFLICTING_ALIASES",
  );

  const conflictingSignatureBytes = validationFeePolicyFixture();
  const signatureBytes =
    conflictingSignatureBytes.signedPolicy.signatures[0].signature;
  conflictingSignatureBytes.signedPolicy.signatures[0].signature = {
    payload: signatureBytes,
    bytes: signatureBytes,
  };
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        conflictingSignatureBytes.signedPolicy,
        conflictingSignatureBytes.verificationContext,
      ),
    (error) => error?.code === "CONFLICTING_ALIASES",
  );
});

test("verifySignedValidationFeePolicy returns an immutable policy snapshot", () => {
  const fixture = validationFeePolicyFixture();
  const mutableGenesisHash = Uint8Array.from(Buffer.alloc(32, 0x07));
  fixture.policy.genesis_hash = mutableGenesisHash;
  const verified = verifySignedValidationFeePolicy(
    fixture.signedPolicy,
    fixture.verificationContext,
  );

  mutableGenesisHash.fill(0xff);
  fixture.policy.network_id = "mutated-network";
  fixture.policy.fee_minor_units = 99;
  fixture.policy.exemption_classes[0] = "MUTATED";
  fixture.policy.exemption_classes.push("MUTATED_AGAIN");
  fixture.policyRegistry.active_policy_hash = "00".repeat(32);
  fixture.governanceKeyset.public_keys_hex[0] = "00".repeat(32);

  assert.equal(verified.policy.network_id, "boi-testnet");
  assert.equal(verified.policy.genesis_hash, "07".repeat(32));
  assert.equal(verified.policy.fee_minor_units, 10);
  assert.deepEqual(verified.policy.exemption_classes, ["TREASURY_PAYOUT"]);
  assert.equal(verified.registry.activePolicyHashHex, VALIDATION_FEE_POLICY_HASH_HEX);
  assert.equal(Object.isFrozen(verified.policy), true);
  assert.equal(Object.isFrozen(verified.policy.exemption_classes), true);
});

test("verifySignedValidationFeePolicy fixes the initial policy at scale 2 and 10 minor units", () => {
  for (const [policyOverride, expectedCode] of [
    [{ ds_scale: 3 }, "INVALID_DS_SCALE"],
    [{ fee_minor_units: 9 }, "INVALID_INITIAL_FEE_MINOR_UNITS"],
    [{ fee_minor_units: 11 }, "INVALID_INITIAL_FEE_MINOR_UNITS"],
  ]) {
    const fixture = validationFeePolicyFixture({ policy: policyOverride });
    assert.throws(
      () =>
        verifySignedValidationFeePolicy(
          fixture.signedPolicy,
          fixture.verificationContext,
        ),
      (error) => error?.code === expectedCode,
      expectedCode,
    );
  }
});

test("verifySignedValidationFeePolicy rejects malformed and insufficient keysets", () => {
  const duplicateFixture = validationFeePolicyFixture();
  duplicateFixture.verificationContext.governanceKeyset = {
    keyset_id: duplicateFixture.governanceKeyset.keyset_id,
    threshold: 1,
    public_keys_hex: [
      duplicateFixture.governanceKeyset.public_keys_hex[0],
      duplicateFixture.governanceKeyset.public_keys_hex[0],
    ],
  };
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        duplicateFixture.signedPolicy,
        duplicateFixture.verificationContext,
      ),
    (error) => error?.code === "DUPLICATE_GOVERNANCE_KEY",
  );

  const insufficientFixture = validationFeePolicyFixture();
  insufficientFixture.signedPolicy.signatures = [
    insufficientFixture.signedPolicy.signatures[0],
  ];
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        insufficientFixture.signedPolicy,
        insufficientFixture.verificationContext,
      ),
    (error) => error?.code === "INSUFFICIENT_SIGNATURE_THRESHOLD",
  );

  const weakKeyFixture = validationFeePolicyFixture();
  weakKeyFixture.verificationContext.governanceKeyset = {
    keyset_id: weakKeyFixture.governanceKeyset.keyset_id,
    threshold: 1,
    public_keys_hex: [`01${"00".repeat(31)}`],
  };
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        weakKeyFixture.signedPolicy,
        weakKeyFixture.verificationContext,
      ),
    (error) => error?.code === "MALFORMED_PUBLIC_KEY",
  );
});

test("verifySignedValidationFeePolicy rejects a small-order signature commitment", () => {
  const fixture = validationFeePolicyFixture();
  fixture.signedPolicy.signatures[0].signature =
    `01${"00".repeat(31)}${fixture.signedPolicy.signatures[0].signature.slice(64)}`;
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        fixture.signedPolicy,
        fixture.verificationContext,
      ),
    (error) => error?.code === "MALFORMED_SIGNATURE",
  );
});

test("verifyValidationFeePolicyRegistry rejects non-contiguous and redirected active tips", () => {
  const nonContiguous = validationFeePolicyFixture();
  nonContiguous.policyRegistry.registered_policies[0].policy_version = 2;
  assert.throws(
    () =>
      verifyValidationFeePolicyRegistry(
        nonContiguous.policyRegistry,
        nonContiguous.policy,
      ),
    (error) => error?.code === "UNEXPECTED_POLICY_VERSION",
  );

  const redirected = validationFeePolicyFixture();
  redirected.policyRegistry.active_policy_hash = "11".repeat(32);
  assert.throws(
    () =>
      verifyValidationFeePolicyRegistry(
        redirected.policyRegistry,
        redirected.policy,
      ),
    (error) => error?.code === "ACTIVE_POLICY_HASH_MISMATCH",
  );

  const brokenPreviousHash = validationFeePolicyFixture();
  brokenPreviousHash.policyRegistry.registered_policies[0].previous_policy_hash =
    "22".repeat(32);
  assert.throws(
    () =>
      verifyValidationFeePolicyRegistry(
        brokenPreviousHash.policyRegistry,
        brokenPreviousHash.policy,
      ),
    (error) => error?.code === "BROKEN_PREVIOUS_POLICY_HASH",
  );
});
