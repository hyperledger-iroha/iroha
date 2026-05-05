package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Client-side verification helper for identifier-resolution receipts. */
public final class IdentifierReceiptVerifier {
  private IdentifierReceiptVerifier() {}

  public static boolean verify(
      final IdentifierResolutionReceipt receipt, final IdentifierPolicySummary policy) {
    Objects.requireNonNull(receipt, "receipt");
    Objects.requireNonNull(policy, "policy");
    if (!receipt.policyId().equals(policy.policyId())) {
      throw new IllegalArgumentException("receipt policyId does not match the supplied policy");
    }
    if (!"signed".equals(receipt.attestation().kind())) {
      throw new IllegalArgumentException(
          "only signed identifier receipt attestations can be verified with a resolver public key");
    }
    final byte[] payloadBytes = IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload());
    final byte[] message = IrohaHash.prehash(payloadBytes);
    final byte[] signatureBytes =
        hexToBytes(
            Objects.requireNonNull(
                receipt.attestation().signature(), "signed attestation is missing signature"));
    final PublicKeyCodec.PublicKeyPayload keyPayload =
        PublicKeyCodec.decodePublicKeyLiteral(policy.resolverPublicKey());
    if (keyPayload == null) {
      throw new IllegalArgumentException("resolverPublicKey is not a valid multihash literal");
    }
    switch (keyPayload.curveId()) {
      case 0x01:
        return verifyEd25519(keyPayload.keyBytes(), message, signatureBytes);
      case 0x0F:
        throw new UnsupportedOperationException(
            "SM2 receipt verification is not available in the Android SDK");
      case 0x02:
        throw new UnsupportedOperationException(
            "ML-DSA receipt verification is not available in the Android SDK");
      default:
        throw new UnsupportedOperationException(
            "Unsupported resolver key curve id: " + keyPayload.curveId());
    }
  }

  private static boolean verifyEd25519(
      final byte[] publicKey, final byte[] message, final byte[] signature) {
    try {
      final Ed25519Signer verifier = new Ed25519Signer();
      verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
      verifier.update(message, 0, message.length);
      return verifier.verifySignature(signature);
    } catch (final Exception ex) {
      throw new IllegalArgumentException("failed to verify Ed25519 identifier receipt", ex);
    }
  }

  private static byte[] hexToBytes(final String hex) {
    Objects.requireNonNull(hex, "hex");
    String trimmed = hex.trim();
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) == 1) {
      throw new IllegalArgumentException("hex value must contain an even number of characters");
    }
    final byte[] out = new byte[trimmed.length() / 2];
    for (int i = 0; i < trimmed.length(); i += 2) {
      final int high = Character.digit(trimmed.charAt(i), 16);
      final int low = Character.digit(trimmed.charAt(i + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException("hex value contains non-hex characters");
      }
      out[i / 2] = (byte) ((high << 4) | low);
    }
    return out;
  }
}
