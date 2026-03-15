package org.hyperledger.iroha.android.offline;

/**
 * Tests for OfflineSpendReceiptPayloadEncoder JNI binding.
 */
public final class OfflineSpendReceiptPayloadEncoderTest {

  private OfflineSpendReceiptPayloadEncoderTest() {}

  public static void main(final String[] args) {
    encodeReturnsNonEmptyBytes();
    System.out.println("[IrohaAndroid] OfflineSpendReceiptPayloadEncoderTest passed.");
  }

  private static void encodeReturnsNonEmptyBytes() {
    if (!OfflineSpendReceiptPayloadEncoder.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] OfflineSpendReceiptPayloadEncoderTest skipped (native unavailable).");
      return;
    }

    // Valid fixtures from fixtures/offline_allowance/android-demo.
    final String sender = "6cmzPVPX8DcdUnE1nGLZBU1opw24wjxczQNqhCCYvMzKfJR2rGs9tan";
    final String receiver = "6cmzPVPX8j6hZ4dceSGxHX65vr3iK4RG3em7w24HeJ9BbQXmKFVrykC";
    final String asset =
        "norito:4e52543000000eaf5ef05db6ed320eaf5ef05db6ed3200c0000000000000005c0d942ec37b449c00810000000000000017000000000000000f00000000000000070000000000000064656661756c745a00000000000000000000004e000000000000004600000000000000656430313230413938424146423036363343453038443735454244353036464543333841383445353736413743394230383937363933454434423034464439454632443138442f0000000000000014000000000000000c000000000000000400000000000000736f72610b000000000000000300000000000000757364";

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

    // Deterministic 32-byte certificate id hex.
    final String senderCertificateIdHex =
        "8f4c5cc60e2f8cb2cbec6db861f2f923fbe46362b55ef8f40bbd8fa54f6b6f31";

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
            senderCertificateIdHex);

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
