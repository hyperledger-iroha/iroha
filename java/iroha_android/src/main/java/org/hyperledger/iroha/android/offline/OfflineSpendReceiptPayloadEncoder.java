package org.hyperledger.iroha.android.offline;

/**
 * Encodes OfflineSpendReceiptPayload to Norito bytes for signing.
 *
 * <p>This encoder serializes the receipt payload structure using the Norito codec, producing
 * bytes that can be signed by the sender's spend key.
 */
public final class OfflineSpendReceiptPayloadEncoder {
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available;
    try {
      System.loadLibrary("connect_norito_bridge");
      available = true;
    } catch (UnsatisfiedLinkError error) {
      available = false;
    }
    NATIVE_AVAILABLE = available;
  }

  private OfflineSpendReceiptPayloadEncoder() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  /**
   * Encode OfflineSpendReceiptPayload to Norito bytes for signing.
   *
   * @param txIdHex 32-byte transaction ID as hex (64 chars)
   * @param fromAccountId sender canonical Katakana i105 AccountId
   * @param toAccountId receiver canonical Katakana i105 AccountId
   * @param assetId canonical internal asset balance-bucket literal
   *     ({@code <base58-asset-definition-id>#<katakana-i105-account-id>} with an optional
   *     {@code #dataspace:<id>} suffix; canonical asset-definition ids are Base58)
   * @param amount decimal amount string
   * @param issuedAtMs timestamp in milliseconds
   * @param invoiceId invoice identifier
   * @param platformProofJson JSON-serialized OfflinePlatformProof
   * @param senderCertificateIdHex 32-byte certificate identifier as hex (64 chars)
   * @return Norito-encoded bytes
   * @throws IllegalStateException if native library is not available
   * @throws IllegalArgumentException if any parameter is invalid
   */
  public static byte[] encode(
      final String txIdHex,
      final String fromAccountId,
      final String toAccountId,
      final String assetId,
      final String amount,
      final long issuedAtMs,
      final String invoiceId,
      final String platformProofJson,
      final String senderCertificateIdHex) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    if (txIdHex == null || txIdHex.length() != 64) {
      throw new IllegalArgumentException("txIdHex must be 64 hex characters");
    }
    if (fromAccountId == null || fromAccountId.isEmpty()) {
      throw new IllegalArgumentException("fromAccountId must not be empty");
    }
    if (toAccountId == null || toAccountId.isEmpty()) {
      throw new IllegalArgumentException("toAccountId must not be empty");
    }
    if (assetId == null || assetId.isEmpty()) {
      throw new IllegalArgumentException("assetId must not be empty");
    }
    if (amount == null || amount.isEmpty()) {
      throw new IllegalArgumentException("amount must not be empty");
    }
    if (invoiceId == null) {
      throw new IllegalArgumentException("invoiceId must not be null");
    }
    if (platformProofJson == null || platformProofJson.isEmpty()) {
      throw new IllegalArgumentException("platformProofJson must not be empty");
    }
    if (senderCertificateIdHex == null || senderCertificateIdHex.length() != 64) {
      throw new IllegalArgumentException("senderCertificateIdHex must be 64 hex characters");
    }
    final byte[] result =
        nativeEncode(
            txIdHex,
            fromAccountId,
            toAccountId,
            assetId,
            amount,
            issuedAtMs,
            invoiceId,
            platformProofJson,
            senderCertificateIdHex);
    if (result == null) {
      throw new IllegalStateException("nativeEncode returned null");
    }
    return result;
  }

  private static native byte[] nativeEncode(
      String txIdHex,
      String fromAccountId,
      String toAccountId,
      String assetId,
      String amount,
      long issuedAtMs,
      String invoiceId,
      String platformProofJson,
      String senderCertificateIdHex);
}
