package org.hyperledger.iroha.android.offline;

/** Tests for OfflineSpendReceiptPayloadEncoder JNI binding. */
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

    // Test values derived from the live account-address fixtures and an owner-qualified
    // asset-holding id in `<base58-asset-definition-id>#<katakana-i105-account-id>` form.
    final String sender = "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE";
    final String receiver = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D";
    final String asset = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#" + sender;

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
