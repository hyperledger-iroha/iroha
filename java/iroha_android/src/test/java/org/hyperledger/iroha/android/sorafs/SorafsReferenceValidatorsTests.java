package org.hyperledger.iroha.android.sorafs;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

public final class SorafsReferenceValidatorsTests {
  private static final String MAX_SCALED_XOR =
      "6703903964971298549787012499102923063739682910296196688861780721860882015"
          + "036773488400937149083451713845015929093243025426876941405973284973216824"
          + ".503042047";

  private SorafsReferenceValidatorsTests() {}

  public static void main(final String[] args) throws IOException {
    exposesBridgeSelectors();
    rejectsGeneratedAtBeforeNativeDispatch();
    rejectsBlankLabelBeforeNativeDispatch();
    rejectsRuntimeSnapshotSigningBeforeNativeDispatch();
    rejectsBadSigningKeyBeforeNativeDispatch();
    rejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch();
    rejectsOversizedOrderbookOwnerAccountsBeforeNativeDispatch();
    rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch();
    rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch();
    rejectsNoncanonicalXorQuantitiesBeforeNativeDispatch();
    validatesOrderbookFixtureWhenNativeBridgeIsAvailable();
    signsOrderbookFixtureWhenNativeBridgeIsAvailable();
    derivesCanonicalOrderIdWhenNativeBridgeIsAvailable();
    System.out.println("[IrohaAndroid] SoraFS reference validator tests passed.");
  }

  private static void exposesBridgeSelectors() {
    assert SorafsOrderbookPayloadKind.ORDER_REQUEST.bridgeCode() == 1;
    assert SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.bridgeCode() == 6;
    assert SorafsOrderbookPayloadKind.ORDER_REQUEST.isUserSignedPayload();
    assert !SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.isUserSignedPayload();
    assert SorafsPdpPayloadKind.COMMITMENT.bridgeCode() == 1;
    assert SorafsPdpPayloadKind.PROOF.bridgeCode() == 3;
    assert SorafsPopPayloadKind.CREDENTIAL.bridgeCode() == 1;
    assert SorafsPopPayloadKind.MEMBERSHIP_PROOF.bridgeCode() == 6;
    assert SorafsPopPayloadKind.ISSUED_CREDENTIAL_BUNDLE.bridgeCode() == 7;
    assert SorafsHedgingPayloadKind.PRICE_FEED.bridgeCode() == 1;
    assert SorafsHedgingPayloadKind.BILLING_STATEMENT.bridgeCode() == 4;
    assert SorafsOrderbookSide.BID.bridgeCode() == 1;
    assert SorafsOrderbookTier.ARCHIVE.bridgeCode() == 3;
    assert SorafsOrderbookCancelReason.REPLACED.bridgeCode() == 4;
    assert SorafsReferenceValidators.REQUIRED_BRIDGE_ABI_VERSION == 19;
    assert !SorafsReferenceValidators.isBridgeAbiSupported(18);
    assert SorafsReferenceValidators.isBridgeAbiSupported(19);
    assert SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 == 256;
  }

  private static void rejectsGeneratedAtBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.validateOrderbookPayloadJson(
          SorafsOrderbookPayloadKind.ORDER_REQUEST, new byte[0], null, -1L);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("generatedAtUnix");
    }
    assert threw : "generatedAtUnix should be validated before native dispatch";
  }

  private static void rejectsBlankLabelBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.validateHedgingPayloadJson(
          SorafsHedgingPayloadKind.PRICE_FEED, new byte[0], " ", 1L);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("label");
    }
    assert threw : "label should be validated before native dispatch";
  }

  private static void rejectsRuntimeSnapshotSigningBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.signOrderbookPayload(
          SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT, new byte[0], repeatedKey(0xB7));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("cannot be signed");
    }
    assert threw : "runtime snapshots should be rejected before native dispatch";
  }

  private static void rejectsBadSigningKeyBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.signOrderbookPayload(
          SorafsOrderbookPayloadKind.ORDER_REQUEST, new byte[0], new byte[32]);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("privateKey");
    }
    assert threw : "bad signing keys should be rejected before native dispatch";
  }

  private static void rejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch() {
    boolean emptyOwnerThrew = false;
    try {
      SorafsReferenceValidators.deriveOrderbookOrderId(new byte[0], 7L);
    } catch (final IllegalArgumentException ex) {
      emptyOwnerThrew = ex.getMessage() != null && ex.getMessage().contains("ownerAccount");
    }
    assert emptyOwnerThrew : "empty owner must be rejected before native dispatch";

    boolean zeroNonceThrew = false;
    try {
      SorafsReferenceValidators.deriveOrderbookOrderId(new byte[] {1}, 0L);
    } catch (final IllegalArgumentException ex) {
      zeroNonceThrew = ex.getMessage() != null && ex.getMessage().contains("nonce");
    }
    assert zeroNonceThrew : "zero nonce must be rejected before native dispatch";
  }

  private static void rejectsOversizedOrderbookOwnerAccountsBeforeNativeDispatch() {
    final byte[] oversized =
        new byte[SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1];
    java.util.Arrays.fill(oversized, (byte) 0x45);

    boolean deriveThrew = false;
    try {
      SorafsReferenceValidators.deriveOrderbookOrderId(oversized, 7L);
    } catch (final IllegalArgumentException ex) {
      deriveThrew = ex.getMessage() != null && ex.getMessage().contains("at most 256 bytes");
    }
    assert deriveThrew : "oversized derivation owner must be rejected before native dispatch";

    boolean requestThrew = false;
    try {
      SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
          SorafsOrderbookSide.BID,
          SorafsOrderbookTier.HOT,
          "1",
          1L,
          oversized,
          1L,
          7L,
          0,
          0,
          repeatedKey(0xB7));
    } catch (final IllegalArgumentException ex) {
      requestThrew = ex.getMessage() != null && ex.getMessage().contains("at most 256 bytes");
    }
    assert requestThrew : "oversized request owner must be rejected before native dispatch";

    boolean cancelThrew = false;
    try {
      SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
          repeated(0x11),
          oversized,
          SorafsOrderbookCancelReason.OWNER_REQUESTED,
          8L,
          repeatedKey(0xB7));
    } catch (final IllegalArgumentException ex) {
      cancelThrew = ex.getMessage() != null && ex.getMessage().contains("at most 256 bytes");
    }
    assert cancelThrew : "oversized cancel owner must be rejected before native dispatch";
  }

  private static void rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
          new byte[31],
          SorafsOrderbookSide.BID,
          SorafsOrderbookTier.HOT,
          "42",
          7L,
          new byte[] {1},
          123L,
          1L,
          0,
          25,
          repeatedKey(0xB7));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("orderId");
    }
    assert threw : "order request fields should be validated before native dispatch";
  }

  private static void rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch() {
    boolean threw = false;
    try {
      SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
          repeated(0x21),
          repeated(0x22),
          repeated(0x23),
          0L,
          64L,
          repeated(0x24),
          64L,
          "not-a-decimal",
          "10",
          "1",
          123L,
          repeatedKey(0xB7));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("xorDebited");
    }
    assert threw : "settlement receipt fields should be validated before native dispatch";
  }

  private static void rejectsNoncanonicalXorQuantitiesBeforeNativeDispatch() {
    assert MAX_SCALED_XOR.length() == 155;
    for (final String value : new String[] {"1.0", "0.0000000001", decimalOnes(156)}) {
      boolean threw = false;
      try {
        SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
            repeated(0x21),
            repeated(0x22),
            repeated(0x23),
            0L,
            64L,
            repeated(0x24),
            64L,
            value,
            "0",
            "0",
            123L,
            repeatedKey(0xB7));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage() != null && ex.getMessage().contains("xorDebited");
      }
      assert threw : "noncanonical XOR quantity should be rejected before native dispatch";
    }
  }

  private static void validatesOrderbookFixtureWhenNativeBridgeIsAvailable() throws IOException {
    if (!SorafsReferenceValidators.isNativeAvailable()) {
      return;
    }
    final byte[] payload =
        fixture("sorafs_manifest", "orderbook", "order_request_v1.to");
    final String json =
        SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST, payload, null, 123L);
    assert json.contains("\"status\": \"Ok\"") : json;
    assert json.contains("\"code\": \"SFS-OK-000\"") : json;
  }

  private static void signsOrderbookFixtureWhenNativeBridgeIsAvailable() throws IOException {
    if (!SorafsReferenceValidators.isNativeAvailable()) {
      return;
    }
    final byte[] payload =
        fixture("sorafs_manifest", "orderbook", "order_request_v1.to");
    final byte[] signed =
        SorafsReferenceValidators.signOrderbookPayload(
            SorafsOrderbookPayloadKind.ORDER_REQUEST, payload, repeatedKey(0xB7));
    assert signed.length > 0 : "signed payload should be returned";
    assert !java.util.Arrays.equals(signed, payload) : "signature should change encoded payload";
  }

  private static void derivesCanonicalOrderIdWhenNativeBridgeIsAvailable() {
    if (!SorafsReferenceValidators.isNativeAvailable()) {
      return;
    }
    final byte[] owner = "buyer@sora".getBytes(StandardCharsets.UTF_8);
    final byte[] orderId = SorafsReferenceValidators.deriveOrderbookOrderId(owner, 7L);
    assert "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69"
        .equals(toHex(orderId));
    assert !java.util.Arrays.equals(
        orderId, SorafsReferenceValidators.deriveOrderbookOrderId(owner, 8L));
    assert !java.util.Arrays.equals(
        orderId,
        SorafsReferenceValidators.deriveOrderbookOrderId(
            "provider@sora".getBytes(StandardCharsets.UTF_8), 7L));

    final byte[] maximumOwner =
        new byte[SorafsReferenceValidators.ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1];
    java.util.Arrays.fill(maximumOwner, (byte) 0x45);
    final byte[] maximumOwnerOrderId =
        SorafsReferenceValidators.deriveOrderbookOrderId(maximumOwner, 9L);
    final byte[] maximumOwnerOrder =
        SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            SorafsOrderbookSide.BID,
            SorafsOrderbookTier.HOT,
            "1",
            1L,
            maximumOwner,
            1_800_000_000L,
            9L,
            0,
            0,
            repeatedKey(0xB7));
    final String maximumOwnerOrderOutcome =
        SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST, maximumOwnerOrder, null, 123L);
    assert maximumOwnerOrderOutcome.contains("\"status\": \"Ok\"")
        : maximumOwnerOrderOutcome;
    final byte[] maximumOwnerCancel =
        SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
            maximumOwnerOrderId,
            maximumOwner,
            SorafsOrderbookCancelReason.OWNER_REQUESTED,
            10L,
            repeatedKey(0xB7));
    final String maximumOwnerCancelOutcome =
        SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_CANCEL, maximumOwnerCancel, null, 123L);
    assert maximumOwnerCancelOutcome.contains("\"status\": \"Ok\"")
        : maximumOwnerCancelOutcome;

    final byte[] signed =
        SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            SorafsOrderbookSide.BID,
            SorafsOrderbookTier.HOT,
            MAX_SCALED_XOR,
            64L,
            owner,
            1_800_000_000L,
            7L,
            10,
            15,
            repeatedKey(0xB7));
    final String outcome =
        SorafsReferenceValidators.validateOrderbookPayloadJson(
            SorafsOrderbookPayloadKind.ORDER_REQUEST, signed, null, 123L);
    assert outcome.contains("\"status\": \"Ok\"") : outcome;

    boolean threw = false;
    try {
      SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
          repeated(0x11),
          SorafsOrderbookSide.BID,
          SorafsOrderbookTier.HOT,
          "0.000000001",
          64L,
          owner,
          1_800_000_000L,
          7L,
          10,
          15,
          repeatedKey(0xB7));
    } catch (final IllegalArgumentException error) {
      threw =
          error.getMessage() != null
              && error.getMessage().contains("canonical owner-and-nonce derivation");
    }
    assert threw : "noncanonical explicit order id must be rejected";
  }

  private static byte[] repeatedKey(final int value) {
    return repeated(value);
  }

  private static String decimalOnes(final int length) {
    final char[] digits = new char[length];
    java.util.Arrays.fill(digits, '1');
    return new String(digits);
  }

  private static byte[] repeated(final int value) {
    final byte[] key = new byte[32];
    java.util.Arrays.fill(key, (byte) value);
    return key;
  }

  private static String toHex(final byte[] bytes) {
    final char[] alphabet = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xFF;
      output[index * 2] = alphabet[value >>> 4];
      output[index * 2 + 1] = alphabet[value & 0x0F];
    }
    return new String(output);
  }

  private static byte[] fixture(final String... parts) throws IOException {
    Path relative = Paths.get("fixtures");
    for (final String part : parts) {
      relative = relative.resolve(part);
    }
    final Path cwd = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
    final Path[] candidates =
        new Path[] {
          cwd.resolve(relative),
          cwd.resolve("..").resolve(relative),
          cwd.resolve("..").resolve("..").resolve(relative)
        };
    for (final Path candidate : candidates) {
      final Path normalized = candidate.toAbsolutePath().normalize();
      if (Files.exists(normalized)) {
        return Files.readAllBytes(normalized);
      }
    }
    throw new IOException("missing fixture " + relative);
  }
}
