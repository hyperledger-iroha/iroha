package org.hyperledger.iroha.android.client;

import java.security.KeyFactory;
import java.security.PublicKey;
import java.security.Signature;
import java.security.spec.X509EncodedKeySpec;
import java.util.Objects;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Client-side verification helper for identifier-resolution receipts. */
public final class IdentifierReceiptVerifier {
  private static final byte[] ED25519_SPKI_PREFIX =
      new byte[] {
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
      };

  private IdentifierReceiptVerifier() {}

  public static boolean verify(
      final IdentifierResolutionReceipt receipt, final IdentifierPolicySummary policy) {
    Objects.requireNonNull(receipt, "receipt");
    Objects.requireNonNull(policy, "policy");
    if (!receipt.policyId().equals(policy.policyId())) {
      throw new IllegalArgumentException("receipt policyId does not match the supplied policy");
    }
    final IdentifierResolutionPayload payload = receipt.signaturePayload();
    if (!receipt.policyId().equals(payload.policyId())
        || !receipt.opaqueId().equals(payload.opaqueId())
        || !receipt.receiptHash().equals(payload.receiptHash())
        || !receipt.uaid().equals(payload.uaid())
        || !receipt.accountId().equals(payload.accountId())
        || receipt.resolvedAtMs() != payload.resolvedAtMs()
        || !Objects.equals(receipt.expiresAtMs(), payload.expiresAtMs())) {
      throw new IllegalArgumentException("receipt top-level fields do not match signaturePayload");
    }
    final byte[] payloadBytes = hexToBytes(receipt.signaturePayloadHex());
    final byte[] message = IrohaHash.prehash(payloadBytes);
    final byte[] signatureBytes = hexToBytes(receipt.signature());
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
      final byte[] spki = new byte[ED25519_SPKI_PREFIX.length + publicKey.length];
      System.arraycopy(ED25519_SPKI_PREFIX, 0, spki, 0, ED25519_SPKI_PREFIX.length);
      System.arraycopy(publicKey, 0, spki, ED25519_SPKI_PREFIX.length, publicKey.length);
      final KeyFactory keyFactory = KeyFactory.getInstance("Ed25519");
      final PublicKey key = keyFactory.generatePublic(new X509EncodedKeySpec(spki));
      final Signature verifier = Signature.getInstance("Ed25519");
      verifier.initVerify(key);
      verifier.update(message);
      return verifier.verify(signature);
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
