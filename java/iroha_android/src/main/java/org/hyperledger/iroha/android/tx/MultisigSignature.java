package org.hyperledger.iroha.android.tx;

import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.address.PublicKeyCodec;

/** Signature produced by a multisig member. */
public final class MultisigSignature {
  private final int curveId;
  private final int algorithmTag;
  private final byte[] publicKey;
  private final byte[] signature;

  private MultisigSignature(
      final int curveId, final int algorithmTag, final byte[] publicKey, final byte[] signature) {
    if (curveId < 0 || curveId > 0xFF) {
      throw new IllegalArgumentException("curveId must fit in u8");
    }
    if (algorithmTag < 0 || algorithmTag > 0xFF) {
      throw new IllegalArgumentException("algorithm tag must fit in u8");
    }
    this.curveId = curveId;
    this.algorithmTag = algorithmTag;
    this.publicKey =
        Arrays.copyOf(Objects.requireNonNull(publicKey, "publicKey"), publicKey.length);
    this.signature =
        Arrays.copyOf(Objects.requireNonNull(signature, "signature"), signature.length);
    if (this.publicKey.length == 0) {
      throw new IllegalArgumentException("publicKey must not be empty");
    }
    if (this.signature.length == 0) {
      throw new IllegalArgumentException("signature must not be empty");
    }
  }

  /**
   * Construct a multisig signature from the curve id, raw public key, and signature bytes.
   */
  public static MultisigSignature fromCurveId(
      final int curveId, final byte[] publicKey, final byte[] signature) {
    final int algorithmTag = algorithmTagForCurveId(curveId);
    if (algorithmTag < 0) {
      throw new IllegalArgumentException("Unsupported curve id: " + curveId);
    }
    return new MultisigSignature(curveId, algorithmTag, publicKey, signature);
  }

  /**
   * Construct a multisig signature from a multihash public key literal.
   */
  public static MultisigSignature fromPublicKeyLiteral(
      final String publicKeyLiteral, final byte[] signature) {
    final PublicKeyCodec.PublicKeyPayload payload =
        PublicKeyCodec.decodePublicKeyLiteral(publicKeyLiteral);
    if (payload == null) {
      throw new IllegalArgumentException("Invalid public key literal");
    }
    return fromCurveId(payload.curveId(), payload.keyBytes(), signature);
  }

  /** Curve id associated with the signer. */
  public int curveId() {
    return curveId;
  }

  /** Algorithm tag used in Norito public key encoding. */
  public int algorithmTag() {
    return algorithmTag;
  }

  /** Raw public key bytes (payload only, without algorithm tag). */
  public byte[] publicKey() {
    return Arrays.copyOf(publicKey, publicKey.length);
  }

  /** Raw signature bytes. */
  public byte[] signature() {
    return Arrays.copyOf(signature, signature.length);
  }

  /** Public key literal (multihash hex) derived from the payload. */
  public String publicKeyMultihash() {
    return PublicKeyCodec.encodePublicKeyMultihash(curveId, publicKey);
  }

  /** Norito public key payload (algorithm tag + raw key). */
  public byte[] publicKeyNoritoPayload() {
    final byte[] payload = new byte[publicKey.length + 1];
    payload[0] = (byte) algorithmTag;
    System.arraycopy(publicKey, 0, payload, 1, publicKey.length);
    return payload;
  }

  private static int algorithmTagForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return 0;
      case 0x02:
        return 4;
      case 0x0A:
        return 5;
      case 0x0B:
        return 6;
      case 0x0C:
        return 7;
      case 0x0D:
        return 8;
      case 0x0E:
        return 9;
      case 0x0F:
        return 10;
      default:
        return -1;
    }
  }
}
