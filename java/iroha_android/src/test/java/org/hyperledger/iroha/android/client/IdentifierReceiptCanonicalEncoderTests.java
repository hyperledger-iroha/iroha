package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Locale;
import org.hyperledger.iroha.android.testing.TestAccountIds;

/** Tests canonical Norito identifier receipt payload decoding. */
public final class IdentifierReceiptCanonicalEncoderTests {
  private static final String ACCOUNT_ID = TestAccountIds.ed25519Authority(0x41);

  private IdentifierReceiptCanonicalEncoderTests() {}

  public static void main(final String[] args) {
    decodePayloadRoundTripsCanonicalReceipt();
    decodeSignedAttestationRoundTrips();
    decodeProofAttestationRoundTrips();
    encodePayloadRejectsNonExactExecutionTags();
    encodeAndDecodeProofAttestationRejectsNonExactBackend();
    decodePayloadRejectsTrailingBytes();
    decodeAttestationRejectsTrailingBytes();
    decodeAttestationRejectsUnknownTag();
    System.out.println("[IrohaAndroid] IdentifierReceiptCanonicalEncoder tests passed.");
  }

  private static void decodePayloadRoundTripsCanonicalReceipt() {
    final IdentifierResolutionPayload payload = samplePayload("A1B2C3D4", 142L, 242L);
    final IdentifierResolutionPayload decoded =
        IdentifierReceiptCanonicalEncoder.decodePayload(
            IdentifierReceiptCanonicalEncoder.encodePayload(payload));

    assert payload.policyId().equals(decoded.policyId()) : "policy id mismatch";
    assert payload.opaqueId().equals(decoded.opaqueId()) : "opaque id mismatch";
    assert payload.receiptHash().toLowerCase(Locale.ROOT).equals(decoded.receiptHash())
        : "receipt hash mismatch";
    assert payload.uaid().equals(decoded.uaid()) : "uaid mismatch";
    assert payload.accountId().equals(decoded.accountId()) : "account id mismatch";
    assert payload.execution().programId().equals(decoded.execution().programId())
        : "execution program id mismatch";
    assert payload.execution().backend().equals(decoded.execution().backend())
        : "execution backend mismatch";
    assert payload.execution().verificationMode().equals(decoded.execution().verificationMode())
        : "execution verification mode mismatch";
    assert payload.execution().executedAtMs() == decoded.execution().executedAtMs()
        : "executed_at_ms mismatch";
    assert payload.execution().expiresAtMs().equals(decoded.execution().expiresAtMs())
        : "execution expires_at_ms mismatch";
    assert payload.opening().payload().programId().equals(decoded.opening().payload().programId())
        : "opening program id mismatch";
    assert payload.opening().payload().expiresAtMs().equals(decoded.opening().payload().expiresAtMs())
        : "opening expires_at_ms mismatch";
    assert payload.opening().signature().toLowerCase(Locale.ROOT).equals(decoded.opening().signature())
        : "opening signature mismatch";
  }

  private static void decodeSignedAttestationRoundTrips() {
    final IdentifierReceiptAttestation attestation =
        new IdentifierReceiptAttestation("signed", "A1B2C3D4", null, null);
    final IdentifierReceiptAttestation decoded =
        IdentifierReceiptCanonicalEncoder.decodeAttestation(
            IdentifierReceiptCanonicalEncoder.encodeAttestation(attestation));

    assert "signed".equals(decoded.kind()) : "signed attestation kind mismatch";
    assert "a1b2c3d4".equals(decoded.signature()) : "signed attestation signature mismatch";
    assert decoded.proofBackend() == null : "signed attestation proof backend must be absent";
    assert decoded.proofB64() == null : "signed attestation proof bytes must be absent";
  }

  private static void decodeProofAttestationRoundTrips() {
    final IdentifierReceiptAttestation attestation =
        new IdentifierReceiptAttestation("proof", null, "halo2/ipa", "AQID");
    final IdentifierReceiptAttestation decoded =
        IdentifierReceiptCanonicalEncoder.decodeAttestation(
            IdentifierReceiptCanonicalEncoder.encodeAttestation(attestation));

    assert "proof".equals(decoded.kind()) : "proof attestation kind mismatch";
    assert decoded.signature() == null : "proof attestation signature must be absent";
    assert "halo2/ipa".equals(decoded.proofBackend()) : "proof backend mismatch";
    assert "AQID".equals(decoded.proofB64()) : "proof payload mismatch";
  }

  private static void encodePayloadRejectsNonExactExecutionTags() {
    for (final String policyId :
        new String[] {" phone#retail", "phone#retail ", "phone #retail", "phone# retail"}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithPolicyId(policyId)),
          "policy_id exactness must fail");
    }

    for (final String programId :
        new String[] {" identifier_lookup_retail", "identifier_lookup_retail "}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithProgramId(programId)),
          "execution program_id exactness must fail");
      assertThrows(
          () ->
              IdentifierReceiptCanonicalEncoder.encodePayload(
                  samplePayloadWithOpeningProgramId(programId)),
          "opening program_id exactness must fail");
    }

    for (final String accountId : new String[] {" " + ACCOUNT_ID, ACCOUNT_ID + " "}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithAccountId(accountId)),
          "account_id exactness must fail");
    }

    assertThrows(
        () ->
            IdentifierReceiptCanonicalEncoder.encodePayload(
                samplePayloadWithHashFields(
                    " " + "opaque:" + "44".repeat(32),
                    "55".repeat(32),
                    "uaid:" + "66".repeat(32),
                    "11".repeat(32),
                    "EE".repeat(32))),
        "opaque_id exactness must fail");
    assertThrows(
        () ->
            IdentifierReceiptCanonicalEncoder.encodePayload(
                samplePayloadWithHashFields(
                    "opaque:" + "44".repeat(32),
                    "55".repeat(32) + " ",
                    "uaid:" + "66".repeat(32),
                    "11".repeat(32),
                    "EE".repeat(32))),
        "receipt_hash exactness must fail");
    assertThrows(
        () ->
            IdentifierReceiptCanonicalEncoder.encodePayload(
                samplePayloadWithHashFields(
                    "opaque:" + "44".repeat(32),
                    "55".repeat(32),
                    " " + "uaid:" + "66".repeat(32),
                    "11".repeat(32),
                    "EE".repeat(32))),
        "uaid exactness must fail");
    assertThrows(
        () ->
            IdentifierReceiptCanonicalEncoder.encodePayload(
                samplePayloadWithHashFields(
                    "opaque:" + "44".repeat(32),
                    "55".repeat(32),
                    "uaid:" + "66".repeat(32),
                    " " + "11".repeat(32),
                    "EE".repeat(32))),
        "program_digest exactness must fail");
    assertThrows(
        () ->
            IdentifierReceiptCanonicalEncoder.encodePayload(
                samplePayloadWithHashFields(
                    "opaque:" + "44".repeat(32),
                    "55".repeat(32),
                    "uaid:" + "66".repeat(32),
                    "11".repeat(32),
                    "EE".repeat(32) + " ")),
        "opening input_ciphertext_hash exactness must fail");

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithTimestamps(-1L, 142L, 84L, 242L)),
        "timestamp u64 executed_at_ms must fail");
    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithTimestamps(42L, -1L, 84L, 242L)),
        "timestamp u64 execution expires_at_ms must fail");
    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithTimestamps(42L, 142L, -1L, 242L)),
        "timestamp u64 opened_at_ms must fail");
    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayloadWithTimestamps(42L, 142L, 84L, -1L)),
        "timestamp u64 opening expires_at_ms must fail");

    for (final String backend :
        new String[] {" bfv-affine-sha3-256-v1", "bfv-affine-sha3-256-v1 ", "BFV-AFFINE-SHA3-256-V1"}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(backend, "signed")),
          "non-exact execution backend must fail");
    }

    for (final String mode : new String[] {" signed", "signed ", "Signed"}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(
              samplePayload("bfv-affine-sha3-256-v1", mode)),
          "non-exact execution verification mode must fail");
    }

    for (final String signature : new String[] {" A1B2C3D4", "A1B2C3D4 "}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload(signature, 142L, 242L)),
          "non-exact opening signature must fail");
    }
  }

  private static void encodeAndDecodeProofAttestationRejectsNonExactBackend() {
    for (final String kind : new String[] {" signed", "signed ", "Signed"}) {
      assertThrows(
          () ->
              IdentifierReceiptCanonicalEncoder.encodeAttestation(
                  new IdentifierReceiptAttestation(kind, "A1B2C3D4", null, null)),
          "non-exact attestation kind must fail during encode");
    }

    for (final String signature : new String[] {" A1B2C3D4", "A1B2C3D4 "}) {
      assertThrows(
          () ->
              IdentifierReceiptCanonicalEncoder.encodeAttestation(
                  new IdentifierReceiptAttestation("signed", signature, null, null)),
          "non-exact attestation signature must fail during encode");
    }

    for (final String proofBackend : new String[] {" halo2/ipa", "halo2/ipa ", " "}) {
      assertThrows(
          () -> IdentifierReceiptCanonicalEncoder.encodeAttestation(
              new IdentifierReceiptAttestation("proof", null, proofBackend, "AQID")),
          "non-exact proof backend must fail during encode");
    }

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.encodeAttestation(
            new IdentifierReceiptAttestation("proof", null, "halo2/ipa", "@@@")),
        "malformed proof_b64 must fail during encode");

    for (final String proofB64 : new String[] {" AQID", "AQID "}) {
      assertThrows(
          () ->
              IdentifierReceiptCanonicalEncoder.encodeAttestation(
                  new IdentifierReceiptAttestation("proof", null, "halo2/ipa", proofB64)),
          "non-exact proof_b64 must fail during encode");
    }

    final byte[] encoded =
        IdentifierReceiptCanonicalEncoder.encodeAttestation(
            new IdentifierReceiptAttestation("proof", null, "halo2/ipa", "AQID"));
    final byte[] needle = "halo2/ipa".getBytes(StandardCharsets.UTF_8);
    final int offset = indexOf(encoded, needle);
    assert offset >= 0 : "encoded proof backend must be present";
    encoded[offset + needle.length - 1] = (byte) ' ';

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.decodeAttestation(encoded),
        "non-exact proof backend must fail during decode");
  }

  private static void decodePayloadRejectsTrailingBytes() {
    final byte[] encoded =
        IdentifierReceiptCanonicalEncoder.encodePayload(samplePayload("A1B2C3D4", null, null));
    final byte[] mutated = Arrays.copyOf(encoded, encoded.length + 1);

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.decodePayload(mutated),
        "identifier receipt payload trailing bytes must fail");
  }

  private static void decodeAttestationRejectsTrailingBytes() {
    final byte[] encoded =
        IdentifierReceiptCanonicalEncoder.encodeAttestation(
            new IdentifierReceiptAttestation("signed", "A1B2C3D4", null, null));
    final byte[] mutated = Arrays.copyOf(encoded, encoded.length + 1);

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.decodeAttestation(mutated),
        "identifier attestation trailing bytes must fail");
  }

  private static void decodeAttestationRejectsUnknownTag() {
    final byte[] encoded =
        IdentifierReceiptCanonicalEncoder.encodeAttestation(
            new IdentifierReceiptAttestation("signed", "A1B2C3D4", null, null));
    encoded[0] = 9;

    assertThrows(
        () -> IdentifierReceiptCanonicalEncoder.decodeAttestation(encoded),
        "unknown identifier attestation tag must fail");
  }

  private static IdentifierResolutionPayload samplePayload(
      final String signatureHex, final Long executionExpiresAtMs, final Long openingExpiresAtMs) {
    return samplePayload(
        signatureHex,
        executionExpiresAtMs,
        openingExpiresAtMs,
        "bfv-affine-sha3-256-v1",
        "signed");
  }

  private static IdentifierResolutionPayload samplePayloadWithPolicyId(final String policyId) {
    return samplePayload(
        "A1B2C3D4",
        142L,
        242L,
        "bfv-affine-sha3-256-v1",
        "signed",
        policyId,
        "identifier_lookup_retail",
        "identifier_lookup_retail",
        ACCOUNT_ID);
  }

  private static IdentifierResolutionPayload samplePayloadWithProgramId(final String programId) {
    return samplePayload(
        "A1B2C3D4",
        142L,
        242L,
        "bfv-affine-sha3-256-v1",
        "signed",
        "phone#retail",
        programId,
        "identifier_lookup_retail",
        ACCOUNT_ID);
  }

  private static IdentifierResolutionPayload samplePayloadWithOpeningProgramId(
      final String programId) {
    return samplePayload(
        "A1B2C3D4",
        142L,
        242L,
        "bfv-affine-sha3-256-v1",
        "signed",
        "phone#retail",
        "identifier_lookup_retail",
        programId,
        ACCOUNT_ID);
  }

  private static IdentifierResolutionPayload samplePayloadWithAccountId(final String accountId) {
    return samplePayload(
        "A1B2C3D4",
        142L,
        242L,
        "bfv-affine-sha3-256-v1",
        "signed",
        "phone#retail",
        "identifier_lookup_retail",
        "identifier_lookup_retail",
        accountId);
  }

  private static IdentifierResolutionPayload samplePayloadWithHashFields(
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String programDigest,
      final String openingInputCiphertextHash) {
    return samplePayload(
        "A1B2C3D4",
        142L,
        242L,
        "bfv-affine-sha3-256-v1",
        "signed",
        "phone#retail",
        "identifier_lookup_retail",
        "identifier_lookup_retail",
        ACCOUNT_ID,
        opaqueId,
        receiptHash,
        uaid,
        programDigest,
        openingInputCiphertextHash);
  }

  private static IdentifierResolutionPayload samplePayloadWithTimestamps(
      final long executedAtMs,
      final Long executionExpiresAtMs,
      final long openedAtMs,
      final Long openingExpiresAtMs) {
    return new IdentifierResolutionPayload(
        "phone#retail",
        new IdentifierResolutionExecutionPayload(
            "identifier_lookup_retail",
            "11".repeat(32),
            "bfv-affine-sha3-256-v1",
            "signed",
            "AA".repeat(32),
            "BB".repeat(32),
            "CC".repeat(32),
            "DD".repeat(32),
            "22".repeat(32),
            "33".repeat(32),
            executedAtMs,
            executionExpiresAtMs),
        new RamLfeOutputOpening(
            new RamLfeOutputOpeningPayload(
                "identifier_lookup_retail",
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                openedAtMs,
                openingExpiresAtMs),
            "A1B2C3D4"),
        "opaque:" + "44".repeat(32),
        "55".repeat(32),
        "uaid:" + "66".repeat(32),
        ACCOUNT_ID);
  }

  private static IdentifierResolutionPayload samplePayload(
      final String backend, final String verificationMode) {
    return samplePayload("A1B2C3D4", 142L, 242L, backend, verificationMode);
  }

  private static IdentifierResolutionPayload samplePayload(
      final String signatureHex,
      final Long executionExpiresAtMs,
      final Long openingExpiresAtMs,
      final String backend,
      final String verificationMode) {
    return samplePayload(
        signatureHex,
        executionExpiresAtMs,
        openingExpiresAtMs,
        backend,
        verificationMode,
        "phone#retail",
        "identifier_lookup_retail",
        "identifier_lookup_retail",
        ACCOUNT_ID);
  }

  private static IdentifierResolutionPayload samplePayload(
      final String signatureHex,
      final Long executionExpiresAtMs,
      final Long openingExpiresAtMs,
      final String backend,
      final String verificationMode,
      final String policyId,
      final String programId,
      final String openingProgramId,
      final String accountId) {
    return samplePayload(
        signatureHex,
        executionExpiresAtMs,
        openingExpiresAtMs,
        backend,
        verificationMode,
        policyId,
        programId,
        openingProgramId,
        accountId,
        "opaque:" + "44".repeat(32),
        "55".repeat(32),
        "uaid:" + "66".repeat(32),
        "11".repeat(32),
        "EE".repeat(32));
  }

  private static IdentifierResolutionPayload samplePayload(
      final String signatureHex,
      final Long executionExpiresAtMs,
      final Long openingExpiresAtMs,
      final String backend,
      final String verificationMode,
      final String policyId,
      final String programId,
      final String openingProgramId,
      final String accountId,
      final String opaqueId,
      final String receiptHash,
      final String uaid,
      final String programDigest,
      final String openingInputCiphertextHash) {
    return new IdentifierResolutionPayload(
        policyId,
        new IdentifierResolutionExecutionPayload(
            programId,
            programDigest,
            backend,
            verificationMode,
            "AA".repeat(32),
            "BB".repeat(32),
            "CC".repeat(32),
            "DD".repeat(32),
            "22".repeat(32),
            "33".repeat(32),
            42L,
            executionExpiresAtMs),
        new RamLfeOutputOpening(
            new RamLfeOutputOpeningPayload(
                openingProgramId,
                openingInputCiphertextHash,
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                84L,
                openingExpiresAtMs),
            signatureHex),
        opaqueId,
        receiptHash,
        uaid,
        accountId);
  }

  private static int indexOf(final byte[] haystack, final byte[] needle) {
    if (needle.length == 0 || haystack.length < needle.length) {
      return -1;
    }
    for (int offset = 0; offset <= haystack.length - needle.length; offset++) {
      boolean matches = true;
      for (int index = 0; index < needle.length; index++) {
        if (haystack[offset + index] != needle[index]) {
          matches = false;
          break;
        }
      }
      if (matches) {
        return offset;
      }
    }
    return -1;
  }

  private static void assertThrows(final ThrowingRunnable runnable, final String message) {
    boolean threw = false;
    try {
      runnable.run();
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : message;
  }

  private interface ThrowingRunnable {
    void run();
  }
}
