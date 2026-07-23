package org.hyperledger.iroha.android.alias;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Objects;
import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;

/** Canonical hash and onboarding-authority signature verification for stateless receipts. */
public final class AccountOnboardingReceiptVerifier {
  private static final byte[] HASH_DOMAIN =
      "iroha:account-onboarding-plan-receipt:v1\0".getBytes(StandardCharsets.UTF_8);

  private AccountOnboardingReceiptVerifier() {}

  /** Computes the exact domain-separated hash signed by the onboarding authority. */
  public static byte[] canonicalHash(final AccountOnboardingPlanBodyV1 body) {
    final byte[] encoded = AliasNoritoCodec.encodeOnboardingPlanBody(body);
    final byte[] preimage = new byte[HASH_DOMAIN.length + encoded.length];
    System.arraycopy(HASH_DOMAIN, 0, preimage, 0, HASH_DOMAIN.length);
    System.arraycopy(encoded, 0, preimage, HASH_DOMAIN.length, encoded.length);
    return IrohaHash.prehash(preimage);
  }

  /** Verifies the canonical body hash and the signature of the authority embedded in the body. */
  public static boolean verify(final AccountOnboardingPlanReceiptV1 receipt) {
    return verify(receipt, null);
  }

  /** Verifies the receipt and optionally pins its signer to a configured onboarding authority. */
  public static boolean verify(
      final AccountOnboardingPlanReceiptV1 receipt, final String expectedAuthority) {
    Objects.requireNonNull(receipt, "receipt");
    if (expectedAuthority != null) {
      final String canonicalExpected;
      try {
        canonicalExpected =
            org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
                expectedAuthority, "expectedAuthority");
      } catch (final IllegalArgumentException ignored) {
        return false;
      }
      if (!receipt.body().authority().equals(canonicalExpected)) return false;
    }
    final byte[] carriedHash = AliasNameSupport.decodeHash(receipt.planHash());
    if (carriedHash == null
        || !MessageDigest.isEqual(carriedHash, canonicalHash(receipt.body()))) {
      return false;
    }
    final byte[] signature = decodeHex(receipt.signature());
    if (signature == null) return false;
    try {
      final AccountAddress address = AccountAddress.fromI105(receipt.body().authority(), null);
      final Optional<AccountAddress.SingleKeyPayload> signatory =
          address.singleKeyPayloadIgnoringCurveSupport();
      if (!signatory.isPresent()) return false;
      final AccountAddress.SingleKeyPayload key = signatory.get();
      if (key.curveId() == 0x01) {
        return verifyEd25519(key.publicKey(), carriedHash, signature);
      }
      return verifyNative(key.curveId(), key.publicKey(), carriedHash, signature);
    } catch (final Exception ignored) {
      return false;
    }
  }

  /** Requires a valid hash and signature from the authority embedded in the receipt. */
  public static AccountOnboardingPlanReceiptV1 requireValid(
      final AccountOnboardingPlanReceiptV1 receipt) {
    return requireValid(receipt, null);
  }

  /** Requires a valid receipt signed by the expected configured authority when supplied. */
  public static AccountOnboardingPlanReceiptV1 requireValid(
      final AccountOnboardingPlanReceiptV1 receipt, final String expectedAuthority) {
    if (!verify(receipt, expectedAuthority)) {
      throw new IllegalArgumentException(
          "account onboarding receipt hash or authority signature is invalid");
    }
    return receipt;
  }

  /** Also binds a verified receipt to the exact canonical request sent by the caller. */
  public static AccountOnboardingPlanReceiptV1 requireValidForRequest(
      final AccountOnboardingPlanRequestV1 request,
      final AccountOnboardingPlanReceiptV1 receipt) {
    return requireValidForRequest(request, receipt, null);
  }

  /** Binds a pinned receipt to the exact canonical request sent by the caller. */
  public static AccountOnboardingPlanReceiptV1 requireValidForRequest(
      final AccountOnboardingPlanRequestV1 request,
      final AccountOnboardingPlanReceiptV1 receipt,
      final String expectedAuthority) {
    if (!Objects.requireNonNull(request, "request").equals(receipt.body().request())) {
      throw new IllegalArgumentException(
          "account onboarding receipt does not match the exact normalized request");
    }
    return requireValid(receipt, expectedAuthority);
  }

  private static boolean verifyEd25519(
      final byte[] publicKey, final byte[] message, final byte[] signature) {
    if (!Ed25519PublicKeyAdmission.isValid(publicKey)) return false;
    try {
      final Ed25519Signer verifier = new Ed25519Signer();
      verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
      verifier.update(message, 0, message.length);
      return verifier.verifySignature(signature);
    } catch (final RuntimeException ignored) {
      return false;
    }
  }

  private static boolean verifyNative(
      final int curveId,
      final byte[] publicKey,
      final byte[] message,
      final byte[] signature) {
    final SigningAlgorithm algorithm = signingAlgorithm(curveId);
    if (algorithm == null || !NativeSignerBridge.isNativeAvailable()) return false;
    try {
      return NativeSignerBridge.verifyDetached(algorithm, publicKey, message, signature);
    } catch (final RuntimeException ignored) {
      return false;
    }
  }

  private static SigningAlgorithm signingAlgorithm(final int curveId) {
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

  private static byte[] decodeHex(final String value) {
    if (value == null || value.isEmpty() || (value.length() & 1) != 0) return null;
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      final int high = Character.digit(value.charAt(index * 2), 16);
      final int low = Character.digit(value.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) return null;
      result[index] = (byte) ((high << 4) | low);
    }
    return result;
  }
}
