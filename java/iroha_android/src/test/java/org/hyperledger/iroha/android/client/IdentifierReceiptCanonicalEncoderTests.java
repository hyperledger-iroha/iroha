package org.hyperledger.iroha.android.client;

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
            42L,
            executionExpiresAtMs),
        new RamLfeOutputOpening(
            new RamLfeOutputOpeningPayload(
                "identifier_lookup_retail",
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                "EE".repeat(32),
                84L,
                openingExpiresAtMs),
            signatureHex),
        "opaque:" + "44".repeat(32),
        "55".repeat(32),
        "uaid:" + "66".repeat(32),
        ACCOUNT_ID);
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
