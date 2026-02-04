package org.hyperledger.iroha.android.offline;

/**
 * Tests for OfflineSpendReceiptPayloadEncoder JNI binding.
 *
 * <p>Set IROHA_NATIVE_REQUIRED=1 to make tests fail when native library is unavailable.
 * This is useful in CI pipelines that have the native library built.
 */
public final class OfflineSpendReceiptPayloadEncoderTest {

  private OfflineSpendReceiptPayloadEncoderTest() {}

  public static void main(final String[] args) {
    encodeReturnsNonEmptyBytes();
    System.out.println("[IrohaAndroid] OfflineSpendReceiptPayloadEncoderTest passed.");
  }

  private static void encodeReturnsNonEmptyBytes() {
    if (!OfflineSpendReceiptPayloadEncoder.isNativeAvailable()) {
      boolean nativeRequired = "1".equals(System.getenv("IROHA_NATIVE_REQUIRED"));
      if (nativeRequired) {
        throw new AssertionError(
            "IROHA_NATIVE_REQUIRED=1 but connect_norito_bridge native library is unavailable. "
                + "This indicates the .so file was not rebuilt after adding "
                + "OfflineSpendReceiptPayloadEncoder JNI function.");
      }
      System.out.println(
          "[IrohaAndroid] OfflineSpendReceiptPayloadEncoderTest skipped (native unavailable). "
              + "Set IROHA_NATIVE_REQUIRED=1 to fail instead.");
      return;
    }

    // Test values derived from Rust unit test (encode_offline_spend_receipt_payload_matches_native)
    // These use the compressed AccountAddress format
    final String sender = "RnuaJGGDLA57fKeoK1TaFQWhYLxMXY9sEqWhSviYfXxDwTkLdBw3Khq2";
    final String receiver = "RnuaJGGDL9ruds8g1c7AAz8cq1kS16u1LDptWe8FC3NLR4qs1RhLjNjk";
    final String asset = "xor##" + sender;

    // tx_id must have LSB=1 (this is the hash from Rust test)
    final String txIdHex = "e2a94e18647fe0c6283a31e40c46ae1cc5f0867650f6834e4f01e34284adc9c7";
    final String amount = "75";
    final long issuedAtMs = 1700000500000L;
    final String invoiceId = "INV-42";

    // Platform proof JSON from Rust test (hash format is special: "hash:HEX#CHECKSUM")
    final String platformProofJson =
        "{\"platform\":\"AppleAppAttest\",\"proof\":{\"key_id\":\"VEVTVF9LRVk=\",\"counter\":42,"
            + "\"assertion\":[202,254],"
            + "\"challenge_hash\":\"hash:510C7466F2A90281DF576A765517ADFC6A4C8F89FEE3E14B8EAE3A574F442C37#7C0B\"}}";

    // Certificate JSON from Rust test
    final String certificateJson =
        "{\"controller\":\"" + sender + "@default\","
            + "\"allowance\":{\"asset\":\"" + asset + "@default\",\"amount\":\"500\","
            + "\"commitment\":[66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,"
            + "66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66]},"
            + "\"spend_public_key\":\"ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4\","
            + "\"attestation_report\":[1,2,3],"
            + "\"issued_at_ms\":1700000000000,"
            + "\"expires_at_ms\":1800000000000,"
            + "\"policy\":{\"max_balance\":\"1000\",\"max_tx_value\":\"200\",\"expires_at_ms\":1800000000000},"
            + "\"operator_signature\":\"ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB"
            + "ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB\","
            + "\"metadata\":{},\"verdict_id\":null,\"attestation_nonce\":null,\"refresh_at_ms\":null}";

    final byte[] encoded =
        OfflineSpendReceiptPayloadEncoder.encode(
            txIdHex,
            sender,
            receiver,
            asset,
            amount,
            issuedAtMs,
            invoiceId,
            platformProofJson,
            certificateJson);

    if (encoded == null || encoded.length == 0) {
      throw new AssertionError("encode() returned empty bytes");
    }

    // Verify Norito header (NRT0)
    if (encoded[0] != 'N' || encoded[1] != 'R' || encoded[2] != 'T' || encoded[3] != '0') {
      throw new AssertionError(
          "encoded bytes missing Norito header (expected NRT0, got "
              + (char) encoded[0]
              + (char) encoded[1]
              + (char) encoded[2]
              + (char) encoded[3]
              + ")");
    }

    System.out.println(
        "[IrohaAndroid] OfflineSpendReceiptPayloadEncoder.encode() returned "
            + encoded.length
            + " bytes with valid Norito header");
  }
}
