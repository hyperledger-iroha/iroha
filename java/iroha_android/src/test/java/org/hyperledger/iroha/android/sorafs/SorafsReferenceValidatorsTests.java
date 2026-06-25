package org.hyperledger.iroha.android.sorafs;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

public final class SorafsReferenceValidatorsTests {

  private SorafsReferenceValidatorsTests() {}

  public static void main(final String[] args) throws IOException {
    exposesBridgeSelectors();
    rejectsGeneratedAtBeforeNativeDispatch();
    rejectsBlankLabelBeforeNativeDispatch();
    rejectsRuntimeSnapshotSigningBeforeNativeDispatch();
    rejectsBadSigningKeyBeforeNativeDispatch();
    rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch();
    rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch();
    validatesOrderbookFixtureWhenNativeBridgeIsAvailable();
    signsOrderbookFixtureWhenNativeBridgeIsAvailable();
    System.out.println("[IrohaAndroid] SoraFS reference validator tests passed.");
  }

  private static void exposesBridgeSelectors() {
    assert SorafsOrderbookPayloadKind.ORDER_REQUEST.bridgeCode() == 1;
    assert SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.bridgeCode() == 6;
    assert SorafsOrderbookPayloadKind.ORDER_REQUEST.isUserSignedPayload();
    assert !SorafsOrderbookPayloadKind.RUNTIME_SNAPSHOT.isUserSignedPayload();
    assert SorafsPdpPayloadKind.COMMITMENT.bridgeCode() == 1;
    assert SorafsPdpPayloadKind.PROOF.bridgeCode() == 3;
    assert SorafsOrderbookSide.BID.bridgeCode() == 1;
    assert SorafsOrderbookTier.ARCHIVE.bridgeCode() == 3;
    assert SorafsOrderbookCancelReason.REPLACED.bridgeCode() == 4;
    assert SorafsReferenceValidators.REQUIRED_BRIDGE_ABI_VERSION == 10;
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
      SorafsReferenceValidators.validatePdpPayloadJson(
          SorafsPdpPayloadKind.PROOF, new byte[0], " ", 1L);
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
      threw = ex.getMessage() != null && ex.getMessage().contains("xorDebitedMicroXor");
    }
    assert threw : "settlement receipt fields should be validated before native dispatch";
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

  private static byte[] repeatedKey(final int value) {
    return repeated(value);
  }

  private static byte[] repeated(final int value) {
    final byte[] key = new byte[32];
    java.util.Arrays.fill(key, (byte) value);
    return key;
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
