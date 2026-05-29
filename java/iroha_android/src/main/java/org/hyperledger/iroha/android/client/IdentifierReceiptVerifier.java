package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;

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
      default:
        return verifyNativeBacked(keyPayload.curveId(), keyPayload.keyBytes(), message, signatureBytes);
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

  private static boolean verifyNativeBacked(
      final int curveId, final byte[] publicKey, final byte[] message, final byte[] signature) {
    final SigningAlgorithm algorithm = signingAlgorithmForCurveId(curveId);
    if (algorithm == null || !NativeSignerBridge.isNativeAvailable()) {
      return false;
    }
    try {
      return NativeSignerBridge.verifyDetached(algorithm, publicKey, message, signature);
    } catch (final RuntimeException ex) {
      return false;
    }
  }

  private static SigningAlgorithm signingAlgorithmForCurveId(final int curveId) {
    switch (curveId) {
      case 0x02:
        return SigningAlgorithm.ML_DSA;
      case 0x03:
        return SigningAlgorithm.BLS_NORMAL;
      case 0x04:
        return SigningAlgorithm.SECP256K1;
      case 0x05:
        return SigningAlgorithm.BLS_SMALL;
      case 0x0A:
        return SigningAlgorithm.GOST_2012_256_A;
      case 0x0B:
        return SigningAlgorithm.GOST_2012_256_B;
      case 0x0C:
        return SigningAlgorithm.GOST_2012_256_C;
      case 0x0D:
        return SigningAlgorithm.GOST_2012_512_A;
      case 0x0E:
        return SigningAlgorithm.GOST_2012_512_B;
      case 0x0F:
        return SigningAlgorithm.SM2;
      default:
        return null;
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
