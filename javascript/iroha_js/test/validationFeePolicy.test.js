import { test } from "node:test";
import assert from "node:assert/strict";
import { ed25519 } from "@noble/curves/ed25519";
import { sha512 } from "@noble/hashes/sha512";
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

function concatBytes(...chunks) {
  const output = new Uint8Array(
    chunks.reduce((length, chunk) => length + chunk.length, 0),
  );
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.length;
  }
  return output;
}

function littleEndianBigInt(bytes) {
  let value = 0n;
  for (let index = bytes.length - 1; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[index]);
  }
  return value;
}

function littleEndianBytes(value, length) {
  const output = new Uint8Array(length);
  let remaining = value;
  for (let index = 0; index < output.length; index += 1) {
    output[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return output;
}

function mixedTorsionSignature(signature, publicKey, message, privateKeySeed) {
  const expanded = sha512(privateKeySeed);
  expanded[0] &= 248;
  expanded[31] &= 127;
  expanded[31] |= 64;
  const secretScalar = littleEndianBigInt(expanded.subarray(0, 32));
  const encodedR = signature.subarray(0, 32);
  const orderTwoPoint = ed25519.ExtendedPoint.fromHex(
    Buffer.from(`ec${"ff".repeat(30)}7f`, "hex"),
    false,
  );
  const mixedR = ed25519.ExtendedPoint.fromHex(encodedR, false)
    .add(orderTwoPoint)
    .toRawBytes();
  const order = ed25519.CURVE.n;
  const challenge =
    littleEndianBigInt(sha512(concatBytes(encodedR, publicKey, message))) % order;
  const mixedChallenge =
    littleEndianBigInt(sha512(concatBytes(mixedR, publicKey, message))) % order;
  const scalar = littleEndianBigInt(signature.subarray(32));
  const mixedScalar =
    (scalar + (mixedChallenge - challenge) * secretScalar) % order;
  return concatBytes(
    mixedR,
    littleEndianBytes((mixedScalar + order) % order, 32),
  );
}

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
  assert.equal(verified.validSignatureCount, 1);
  assert.equal(verified.validSignatureWeight, 1n);
  assert.equal(verified.registry.registeredPolicyCount, 1);
  assert.equal(validationFeeQuantity(fixture.policy, 1), "0.1");
  assert.equal(validationFeeQuantity(fixture.policy, 3), "0.3");
});

test("verifySignedValidationFeePolicy honors weighted keyset thresholds", () => {
  const fixture = validationFeePolicyFixture();
  fixture.verificationContext.governanceKeyset = {
    keyset_id: fixture.governanceKeyset.keyset_id,
    threshold: 3,
    keys: fixture.governanceKeyset.public_keys_hex.map((public_key) => ({
      public_key: `ed25519:ed0120${public_key}`,
      weight: 3,
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
  const tampered = Buffer.from(
    fixture.signedPolicy.signatures[0].signature,
    "hex",
  );
  tampered[32] ^= 0x01;
  fixture.signedPolicy.signatures[0].signature = tampered.toString("hex");
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
  fixture.policy.fee = "99";
  fixture.policy.exemption_classes[0] = "MUTATED";
  fixture.policy.exemption_classes.push("MUTATED_AGAIN");
  fixture.policy.treasury_payout_binding.recipients[0].share = "1";
  fixture.policyRegistry.active_policy_hash = "00".repeat(32);
  fixture.governanceKeyset.public_keys_hex[0] = "00".repeat(32);

  assert.equal(verified.policy.network_id, "generic-testnet");
  assert.equal(verified.policy.genesis_hash, "07".repeat(32));
  assert.equal(verified.policy.fee, "0.1");
  assert.deepEqual(verified.policy.exemption_classes, ["TREASURY_PAYOUT"]);
  assert.equal(
    verified.policy.treasury_payout_binding.recipients[0].share,
    "0.25",
  );
  assert.equal(verified.registry.activePolicyHashHex, VALIDATION_FEE_POLICY_HASH_HEX);
  assert.equal(Object.isFrozen(verified.policy), true);
  assert.equal(Object.isFrozen(verified.policy.exemption_classes), true);
  assert.equal(Object.isFrozen(verified.policy.treasury_payout_binding), true);
  assert.equal(
    Object.isFrozen(verified.policy.treasury_payout_binding.recipients),
    true,
  );
});

test("verifySignedValidationFeePolicy rejects incomplete or altered treasury payout bindings", () => {
  const cases = [
    { policy: { treasury_payout_binding: null } },
    { treasuryPayoutBinding: { code_hash: "00".repeat(32) } },
    {
      treasuryPayoutBinding: {
        entrypoint: "different_validation_fee_tick",
      },
    },
    {
      treasuryPayoutBinding: {
        treasury_account_id:
          "sorauﾛ1Q2ﾜﾓ4bｳEDHﾏﾇｿSyFﾘk6bX74ﾙｦRﾙﾄM2GｻSｳｴpPヰｱ6HEY4T",
      },
    },
    {
      treasuryPayoutBinding: {
        xor_asset_id: "55tBnqydRcQTDJmz6Z7wkjRr83XZ",
      },
    },
    { treasuryPayoutBinding: { batch_sbd: "0" } },
    {
      treasuryPayoutBinding: { min_xor_out: "101", max_xor_out: "100" },
    },
    {
      treasuryPayoutBinding: {
        recipients: Array.from({ length: 4 }, () => ({
          account_id:
            "sorauﾛ1Q2ﾜﾓ4bｳEDHﾏﾇｿSyFﾘk6bX74ﾙｦRﾙﾄM2GｻSｳｴpPヰｱ6HEY4T",
          share: "0.25",
        })),
      },
    },
    {
      treasuryPayoutBinding: {
        recipients: validationFeePolicyFixture().policy.treasury_payout_binding.recipients.map(
          (recipient, index) => ({
            ...recipient,
            share: index === 3 ? "0.24" : recipient.share,
          }),
        ),
      },
    },
  ];
  for (const overrides of cases) {
    const fixture = validationFeePolicyFixture(overrides);
    assert.throws(
      () =>
        verifySignedValidationFeePolicy(
          fixture.signedPolicy,
          fixture.verificationContext,
        ),
      (error) => error?.code === "INVALID_TREASURY_PAYOUT_BINDING",
    );
  }
});

test("verifySignedValidationFeePolicy fixes the initial policy at scale 2 and an exact 0.1 fee", () => {
  for (const [policyOverride, expectedCode] of [
    [{ ds_scale: 3 }, "INVALID_DS_SCALE"],
    [{ fee: "0.09" }, "INVALID_INITIAL_FEE"],
    [{ fee: "0.11" }, "INVALID_INITIAL_FEE"],
    [{ fee: "0.10" }, "INVALID_QUANTITY"],
    [{ fee: "-0.1" }, "INVALID_QUANTITY"],
    [{ fee: 0.1 }, "INVALID_QUANTITY"],
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
  insufficientFixture.verificationContext.governanceKeyset = {
    keyset_id: insufficientFixture.governanceKeyset.keyset_id,
    threshold: 2,
    public_keys_hex: [
      ...insufficientFixture.governanceKeyset.public_keys_hex,
      "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
    ],
  };
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

test("verifySignedValidationFeePolicy rejects duplicate keyset ids before selection", () => {
  const fixture = validationFeePolicyFixture();
  const duplicate = {
    ...fixture.governanceKeyset,
    public_keys_hex: [...fixture.governanceKeyset.public_keys_hex],
  };
  delete fixture.verificationContext.governanceKeyset;
  fixture.verificationContext.governanceKeysets = [
    duplicate,
    fixture.governanceKeyset,
  ];
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        fixture.signedPolicy,
        fixture.verificationContext,
      ),
    (error) => error?.code === "DUPLICATE_GOVERNANCE_KEYSET_ID",
  );
  fixture.verificationContext.governanceKeysets.reverse();
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        fixture.signedPolicy,
        fixture.verificationContext,
      ),
    (error) => error?.code === "DUPLICATE_GOVERNANCE_KEYSET_ID",
  );
});

test("verifySignedValidationFeePolicy applies bounds before materializing adversarial inputs", () => {
  const tooManySignatures = validationFeePolicyFixture();
  tooManySignatures.signedPolicy.signatures = Array.from(
    { length: tooManySignatures.governanceKeyset.public_keys_hex.length + 1 },
    () => ({ ...tooManySignatures.signedPolicy.signatures[0] }),
  );
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        tooManySignatures.signedPolicy,
        tooManySignatures.verificationContext,
      ),
    (error) => error?.code === "TOO_MANY_SIGNATURES",
  );

  const oversizedSignature = validationFeePolicyFixture();
  oversizedSignature.signedPolicy.signatures[0].signature = new Uint8Array(65);
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        oversizedSignature.signedPolicy,
        oversizedSignature.verificationContext,
      ),
    (error) => error?.code === "INPUT_TOO_LARGE",
  );

  const tooManyKeysets = validationFeePolicyFixture();
  delete tooManyKeysets.verificationContext.governanceKeyset;
  tooManyKeysets.verificationContext.governanceKeysets = Array.from(
    { length: 65 },
    (_, index) => ({
      ...tooManyKeysets.governanceKeyset,
      keyset_id: `keyset-${index}`,
    }),
  );
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        tooManyKeysets.signedPolicy,
        tooManyKeysets.verificationContext,
      ),
    (error) => error?.code === "INPUT_TOO_LARGE",
  );

  const tooManyKeys = validationFeePolicyFixture();
  tooManyKeys.verificationContext.governanceKeyset = {
    ...tooManyKeys.governanceKeyset,
    public_keys_hex: Array(257).fill(
      tooManyKeys.governanceKeyset.public_keys_hex[0],
    ),
  };
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        tooManyKeys.signedPolicy,
        tooManyKeys.verificationContext,
      ),
    (error) => error?.code === "INPUT_TOO_LARGE",
  );

  const tooManyRegistryEntries = validationFeePolicyFixture();
  tooManyRegistryEntries.policyRegistry.registered_policies = Array(4097).fill(
    tooManyRegistryEntries.policyRegistry.registered_policies[0],
  );
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        tooManyRegistryEntries.signedPolicy,
        tooManyRegistryEntries.verificationContext,
      ),
    (error) => error?.code === "INPUT_TOO_LARGE",
  );

  const oversizedNetwork = validationFeePolicyFixture();
  oversizedNetwork.signedPolicy.policy.network_id = "x".repeat(1025);
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        oversizedNetwork.signedPolicy,
        oversizedNetwork.verificationContext,
      ),
    (error) => error?.code === "INPUT_TOO_LARGE",
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

test("verifySignedValidationFeePolicy rejects mixed-torsion signatures accepted by cofactored verify", () => {
  const fixture = validationFeePolicyFixture();
  const signature = Uint8Array.from(
    Buffer.from(fixture.signedPolicy.signatures[0].signature, "hex"),
  );
  const publicKey = Uint8Array.from(
    Buffer.from(fixture.signedPolicy.signatures[0].signer_public_key, "hex"),
  );
  const privateKeySeed = new Uint8Array(32).fill(77);
  assert.deepEqual(ed25519.getPublicKey(privateKeySeed), publicKey);
  const message = validationFeePolicyLedgerSignaturePayload(fixture.policy);
  const mixedSignature = mixedTorsionSignature(
    signature,
    publicKey,
    message,
    privateKeySeed,
  );
  assert.equal(
    ed25519.verify(mixedSignature, message, publicKey, { zip215: false }),
    true,
    "negative control must remain a signature accepted by cofactored verification",
  );
  fixture.signedPolicy.signatures[0].signature =
    Buffer.from(mixedSignature).toString("hex");
  assert.throws(
    () =>
      verifySignedValidationFeePolicy(
        fixture.signedPolicy,
        fixture.verificationContext,
      ),
    (error) => error?.code === "INVALID_SIGNATURE",
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
