package org.hyperledger.iroha.android.tx;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.ContractInvocation;
import org.hyperledger.iroha.android.model.ExecutableBatchItem;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

public final class TransactionPayloadFixtureTests {

  private static final String SAMPLE_AUTHORITY = sampleAuthority((byte) 0x11);
  private static final String TEST_NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
  private static final String TRANSACTION_PAYLOAD_TYPE =
      "iroha_data_model::transaction::signed::model::TransactionPayload";

  @Test
  public void validatePayloadFixtures() throws Exception {
    runFixtures();
  }

  @Test
  public void fixtureLoaderAcceptsWireInstructionEntries() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-fixture", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final String payloadBase64 = Base64.getEncoder().encodeToString(wirePayload);

    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("wire_name", "iroha.custom");
    instruction.put("payload_base64", payloadBase64);
    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("network_id", TEST_NETWORK_ID);
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 0L);
    payload.put("time_to_live_ms", 100_000L);
    payload.put("nonce", null);
    payload.put("fee_payment", authorityFeePayment());
    payload.put("admission_intent", admissionIntent("ordinary"));
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap =
        firstReleaseFixture("wire_instruction_fixture", payload);

    final TransactionPayloadFixtures.Fixture fixture =
        TransactionPayloadFixtures.Fixture.fromObject(fixtureMap);
    final TransactionPayload decoded = fixture.toPayload();
    assert decoded.executable().isInstructions() : "Expected instruction executable";
    final List<InstructionBox> boxes = decoded.executable().instructions();
    assert boxes.size() == 1 : "Expected one instruction";
    final InstructionBox box = boxes.get(0);
    assert box.payload() instanceof InstructionBox.WirePayload : "Expected wire payload";
    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
    assert "iroha.custom".equals(wire.wireName()) : "Wire name should round-trip";
    assert Arrays.equals(wirePayload, wire.payloadBytes()) : "Wire payload bytes should round-trip";
  }

  @Test
  public void fixtureLoaderRejectsWireInstructionArguments() {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-arguments", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final String payloadBase64 = Base64.getEncoder().encodeToString(wirePayload);

    final Map<String, Object> wireArgs = new LinkedHashMap<>();
    wireArgs.put("wire_name", "iroha.custom");
    wireArgs.put("payload_base64", payloadBase64);

    final Map<String, Object> instruction = new LinkedHashMap<>();
    instruction.put("arguments", wireArgs);
    final List<Object> instructions = new ArrayList<>();
    instructions.add(instruction);

    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", instructions);

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("network_id", TEST_NETWORK_ID);
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 0L);
    payload.put("time_to_live_ms", 100_000L);
    payload.put("nonce", null);
    payload.put("fee_payment", authorityFeePayment());
    payload.put("admission_intent", admissionIntent("ordinary"));
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap =
        firstReleaseFixture("wire_instruction_arguments_fixture", payload);

    final TransactionPayloadFixtures.Fixture fixture =
        TransactionPayloadFixtures.Fixture.fromObject(fixtureMap);
    assertThrows(
        fixture::toPayload,
        "expected wire payload arguments to be rejected");
  }

  @Test
  public void fixtureLoaderRequiresMatchingPositiveIntegerTtl() {
    final TransactionPayloadFixtures.Fixture accepted =
        TransactionPayloadFixtures.Fixture.fromObject(ttlFixture(100_000L, 100_000L));
    assert accepted.timeToLiveMs().orElseThrow() == 100_000L;

    for (Object invalid :
        Arrays.asList(null, 0L, -1L, true, false, 1.0d, "100000")) {
      assertThrows(
          () -> TransactionPayloadFixtures.Fixture.fromObject(ttlFixture(invalid, 100_000L)),
          "expected invalid top-level TTL to be rejected: " + invalid);
      assertThrows(
          () -> TransactionPayloadFixtures.Fixture.fromObject(ttlFixture(100_000L, invalid)),
          "expected invalid payload TTL to be rejected: " + invalid);
    }

    final Map<String, Object> missingTopLevel = ttlFixture(100_000L, 100_000L);
    missingTopLevel.remove("time_to_live_ms");
    assertThrows(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingTopLevel),
        "expected missing top-level TTL to be rejected");

    final Map<String, Object> missingPayload = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> nested = (Map<String, Object>) missingPayload.get("payload");
    nested.remove("time_to_live_ms");
    assertThrows(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingPayload),
        "expected missing payload TTL to be rejected");

    assertThrows(
        () -> TransactionPayloadFixtures.Fixture.fromObject(ttlFixture(100_000L, 99_999L)),
        "expected mismatched TTL copies to be rejected");
  }

  @Test
  public void fixtureLoaderRejectsEncodedAlias() {
    final Map<String, Object> fixture = ttlFixture(100_000L, 100_000L);
    fixture.put("encoded", fixture.get("payload_base64"));
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(fixture),
        "unknown top-level field 'encoded'",
        "encoded alias must be rejected");
  }

  @Test
  public void transactionFixtureSchemasRejectChainChainIdAndChainIdSnakeCase() {
    for (final String legacyField : Arrays.asList("chain", "chainId", "chain_id")) {
      final Map<String, Object> legacyTopLevel = ttlFixture(100_000L, 100_000L);
      legacyTopLevel.put(legacyField, "legacy");
      assertThrowsContaining(
          () -> TransactionPayloadFixtures.Fixture.fromObject(legacyTopLevel),
          "unknown top-level field '" + legacyField + "'",
          "legacy top-level " + legacyField + " field must be rejected");

      final Map<String, Object> legacyPayload = ttlFixture(100_000L, 100_000L);
      @SuppressWarnings("unchecked")
      final Map<String, Object> nestedLegacy =
          (Map<String, Object>) legacyPayload.get("payload");
      nestedLegacy.put(legacyField, "legacy");
      assertThrowsContaining(
          () -> TransactionPayloadFixtures.Fixture.fromObject(legacyPayload),
          "unknown payload field '" + legacyField + "'",
          "legacy payload " + legacyField + " field must be rejected");
    }
  }

  @Test
  public void fixtureLoaderRejectsNonCanonicalNetworkIdentity() {
    final Map<String, Object> lowercaseTopLevel = ttlFixture(100_000L, 100_000L);
    lowercaseTopLevel.put("network_id", TEST_NETWORK_ID.toLowerCase(java.util.Locale.ROOT));
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(lowercaseTopLevel),
        "canonical network hash identity",
        "top-level network_id must use canonical hash text");

    final Map<String, Object> lowercasePayload = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> nestedLowercase =
        (Map<String, Object>) lowercasePayload.get("payload");
    nestedLowercase.put("network_id", TEST_NETWORK_ID.toLowerCase(java.util.Locale.ROOT));
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(lowercasePayload),
        "canonical network hash identity",
        "payload network_id must use canonical hash text");
  }

  @Test
  public void fixtureLoaderRequiresPayloadBase64AndStructuredPayload() {
    final Map<String, Object> missingPayloadBase64 = ttlFixture(100_000L, 100_000L);
    missingPayloadBase64.remove("payload_base64");
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingPayloadBase64),
        "missing required top-level field 'payload_base64'",
        "payload_base64 must be required");

    final Map<String, Object> missingPayload = ttlFixture(100_000L, 100_000L);
    missingPayload.remove("payload");
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingPayload),
        "missing required top-level field 'payload'",
        "structured payload must be required");

    final Map<String, Object> nullPayload = ttlFixture(100_000L, 100_000L);
    nullPayload.put("payload", null);
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(nullPayload),
        "expected object for payload",
        "structured payload must not be null");

    final Map<String, Object> missingMetadata = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> payloadWithoutMetadata =
        (Map<String, Object>) missingMetadata.get("payload");
    payloadWithoutMetadata.remove("metadata");
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingMetadata),
        "missing required payload field 'metadata'",
        "structured metadata must be required");

    final Map<String, Object> missingFeePayment = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> payloadWithoutFeePayment =
        (Map<String, Object>) missingFeePayment.get("payload");
    payloadWithoutFeePayment.remove("fee_payment");
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingFeePayment),
        "missing required payload field 'fee_payment'",
        "structured fee payment must be required");

    final Map<String, Object> missingAdmissionIntent = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> payloadWithoutAdmissionIntent =
        (Map<String, Object>) missingAdmissionIntent.get("payload");
    payloadWithoutAdmissionIntent.remove("admission_intent");
    assertThrowsContaining(
        () -> TransactionPayloadFixtures.Fixture.fromObject(missingAdmissionIntent),
        "missing required payload field 'admission_intent'",
        "structured admission intent must be required");
  }

  @Test
  public void fixtureLoaderRequiresExactAdmissionIntent() {
    final Map<String, Object> queuePlanFixture = ttlFixture(100_000L, 100_000L);
    @SuppressWarnings("unchecked")
    final Map<String, Object> queuePlanPayload =
        (Map<String, Object>) queuePlanFixture.get("payload");
    queuePlanPayload.put("admission_intent", admissionIntent("queue_plan_synced"));
    assert TransactionPayloadFixtures.Fixture.fromObject(queuePlanFixture)
            .toPayload()
            .admissionIntent()
        == TransactionAdmissionIntent.QUEUE_PLAN_SYNCED;

    final List<Map<String, Object>> invalid = new ArrayList<>();
    final Map<String, Object> missingValue = new LinkedHashMap<>();
    missingValue.put("intent", "ordinary");
    invalid.add(missingValue);
    final Map<String, Object> nonNullValue = admissionIntent("ordinary");
    nonNullValue.put("value", 0L);
    invalid.add(nonNullValue);
    invalid.add(admissionIntent("legacy"));
    for (final Map<String, Object> intent : invalid) {
      final Map<String, Object> fixture = ttlFixture(100_000L, 100_000L);
      @SuppressWarnings("unchecked")
      final Map<String, Object> payload = (Map<String, Object>) fixture.get("payload");
      payload.put("admission_intent", intent);
      assertThrows(
          () -> TransactionPayloadFixtures.Fixture.fromObject(fixture).toPayload(),
          "non-exact admission intent must be rejected");
    }
  }

  public static void main(final String[] args) throws Exception {
    runFixtures();
  }

  private static void runFixtures() throws Exception {
    final Path path = TransactionPayloadFixtures.resolveFixturePath();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);
    for (TransactionPayloadFixtures.Fixture fixture : TransactionPayloadFixtures.load(path)) {
      final String name = fixture.name();
      final TransactionPayload payload = fixture.toPayload();
      assert Objects.equals(fixture.networkId(), payload.networkId())
          : name + ": network_id mismatch vs fixture metadata";
      assert Objects.equals(fixture.authority(), payload.authority())
          : name + ": authority mismatch vs fixture metadata";
      assert fixture.creationTimeMs() == payload.creationTimeMs()
          : name + ": creation_time_ms mismatch vs fixture metadata";
      assert Objects.equals(fixture.timeToLiveMs(), payload.timeToLiveMs())
          : name + ": TTL mismatch vs fixture metadata";
      assert Objects.equals(fixture.nonce(), payload.nonce())
          : name + ": nonce mismatch vs fixture metadata";
      assert payload.admissionIntent() == TransactionAdmissionIntent.ORDINARY
          : name + ": canonical fixture admission intent mismatch";
      if ("typed_fee_payment_gas_limit".equals(name)) {
        assert payload.feePayment() instanceof FeePaymentIntent.Authority
            : name + ": payer must be authority";
        assert Long.valueOf(1000L).equals(payload.feePayment().gasLimit())
            : name + ": typed gas limit mismatch";
        assert payload.feePayment().chargeLimits().size() == 1
            : name + ": expected one typed fee charge";
        final FeeChargeLimit charge = payload.feePayment().chargeLimits().get(0);
        assert charge.kind() == FeeChargeKind.PIPELINE_GAS
            : name + ": fee component mismatch";
        assert "7EAD8EFYUx1aVKZPUU1fyKvr8dF1".equals(charge.assetDefinitionId())
            : name + ": fee asset mismatch";
        assert "1000".equals(charge.maxAmount()) : name + ": fee maximum mismatch";
        assert JsonValue.bool(true).equals(payload.metadata().get("checked"))
            : name + ": boolean metadata must be preserved";
      }
      if ("mixed_executable_batch".equals(name)) {
        assert payload.executable().isBatch() : name + ": executable must be a Batch";
        final List<ExecutableBatchItem> items = payload.executable().batchItems();
        assert items.size() == 3 : name + ": expected three ordered batch items";
        assert items.get(0).isInstruction() : name + ": first item must be an Instruction";
        assert items.get(1).isContractCall() : name + ": second item must be a ContractCall";
        assert items.get(2).isInstruction() : name + ": third item must be an Instruction";
        final ContractInvocation invocation = items.get(1).contractInvocation();
        assert "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .equals(invocation.contractAddress()) : name + ": contract address mismatch";
        assert Arrays.equals(
                HashLiteral.decode(
                    "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22"),
                invocation.expectedCodeHash()) : name + ": expected code hash mismatch";
        assert "run".equals(invocation.entrypoint()) : name + ": entrypoint mismatch";
        assert Arrays.equals(new byte[] {1, 2, 3, 4}, invocation.arguments())
            : name + ": argument record mismatch";
        assert Long.valueOf(100_000L).equals(payload.feePayment().gasLimit())
            : name + ": gas limit mismatch";
      }
      final byte[] payloadBytes = adapter.encodeTransaction(payload);
      if (payload.executable().isInstructions()) {
        assert allInstructionsWire(payload)
            : name + ": instruction fixtures must use wire payloads";
      }
      final byte[] payloadFrame = Base64.getDecoder().decode(fixture.payloadBase64());
      final byte[] framedPayload =
          decodeCanonicalFrame(name + ".payload", payloadFrame, TRANSACTION_PAYLOAD_TYPE);
      assert Arrays.equals(framedPayload, payloadBytes) : name + ": payload_base64 mismatch";
      final TransactionPayload decoded = adapter.decodeTransaction(payloadBytes);
      assertPayloadEquals(name, payload, decoded);
    }
    System.out.println("[IrohaAndroid] Transaction payload fixture tests passed.");
  }

  private static void assertPayloadEquals(
      final String name, final TransactionPayload expected, final TransactionPayload actual) {
    assert Objects.equals(expected.networkId(), actual.networkId())
        : name + ": network_id mismatch";
    assert Objects.equals(expected.authority(), actual.authority())
        : name + ": authority mismatch";
    assert expected.creationTimeMs() == actual.creationTimeMs()
        : name + ": creation time mismatch";
    if (expected.executable().isIvm()) {
      assert actual.executable().isIvm() : name + ": executable type mismatch";
      assert java.util.Arrays.equals(expected.executable().ivmBytes(), actual.executable().ivmBytes())
          : name + ": IVM bytes mismatch";
    } else if (expected.executable().isInstructions()) {
      assert actual.executable().isInstructions() : name + ": executable type mismatch";
      final java.util.List<InstructionBox> expectedInstr = expected.executable().instructions();
      final java.util.List<InstructionBox> actualInstr = actual.executable().instructions();
      assert expectedInstr.size() == actualInstr.size() : name + ": instruction count mismatch";
      for (int i = 0; i < expectedInstr.size(); i++) {
        assertInstructionBoxEquals(name, i, expectedInstr.get(i), actualInstr.get(i));
      }
    } else if (expected.executable().isContractCall()) {
      assert actual.executable().isContractCall() : name + ": executable type mismatch";
      assertContractInvocationEquals(
          name,
          expected.executable().contractInvocation(),
          actual.executable().contractInvocation());
    } else if (expected.executable().isBatch()) {
      assert actual.executable().isBatch() : name + ": executable type mismatch";
      final List<ExecutableBatchItem> expectedItems = expected.executable().batchItems();
      final List<ExecutableBatchItem> actualItems = actual.executable().batchItems();
      assert expectedItems.size() == actualItems.size() : name + ": batch item count mismatch";
      for (int index = 0; index < expectedItems.size(); index++) {
        final ExecutableBatchItem expectedItem = expectedItems.get(index);
        final ExecutableBatchItem actualItem = actualItems.get(index);
        if (expectedItem.isInstruction()) {
          assert actualItem.isInstruction() : name + ": batch item tag mismatch at index " + index;
          assertInstructionBoxEquals(
              name + ": batch", index, expectedItem.instruction(), actualItem.instruction());
        } else {
          assert actualItem.isContractCall()
              : name + ": batch item tag mismatch at index " + index;
          assertContractInvocationEquals(
              name + ": batch item " + index,
              expectedItem.contractInvocation(),
              actualItem.contractInvocation());
        }
      }
    } else {
      throw new AssertionError(name + ": unsupported executable type");
    }
    assert Objects.equals(expected.timeToLiveMs(), actual.timeToLiveMs())
        : name + ": TTL mismatch";
    assert Objects.equals(expected.nonce(), actual.nonce())
        : name + ": nonce mismatch";
    assert Objects.equals(expected.feePayment(), actual.feePayment())
        : name + ": fee payment mismatch";
    assert Objects.equals(expected.metadata(), actual.metadata())
        : name + ": metadata mismatch";
  }

  private static void assertInstructionBoxEquals(
      final String name,
      final int index,
      final InstructionBox expected,
      final InstructionBox actual) {
    assert expected.kind() == actual.kind()
        : name + ": instruction kind mismatch at index " + index;
    assert Objects.equals(expected.arguments(), actual.arguments())
        : name + ": instruction arguments mismatch at index " + index;
    assert expected.payload().getClass().equals(actual.payload().getClass())
        : name + ": instruction payload type mismatch at index " + index;
    if (expected.payload() instanceof InstructionBox.WirePayload) {
      final InstructionBox.WirePayload expectedWire =
          (InstructionBox.WirePayload) expected.payload();
      final InstructionBox.WirePayload actualWire =
          (InstructionBox.WirePayload) actual.payload();
      assert expectedWire.wireName().equals(actualWire.wireName())
          : name + ": instruction wire name mismatch at index " + index;
      assert Arrays.equals(expectedWire.payloadBytes(), actualWire.payloadBytes())
          : name + ": instruction wire payload mismatch at index " + index;
    }
  }

  private static void assertContractInvocationEquals(
      final String name,
      final ContractInvocation expected,
      final ContractInvocation actual) {
    assert expected.contractAddress().equals(actual.contractAddress())
        : name + ": contract address mismatch";
    assert Arrays.equals(expected.expectedCodeHash(), actual.expectedCodeHash())
        : name + ": expected code hash mismatch";
    assert expected.entrypoint().equals(actual.entrypoint())
        : name + ": entrypoint mismatch";
    assert Arrays.equals(expected.arguments(), actual.arguments())
        : name + ": contract arguments mismatch";
  }

  private static boolean allInstructionsWire(final TransactionPayload payload) {
    if (!payload.executable().isInstructions()) {
      return false;
    }
    for (InstructionBox box : payload.executable().instructions()) {
      if (!(box.payload() instanceof InstructionBox.WirePayload)) {
        return false;
      }
    }
    return true;
  }

  private static byte[] decodeCanonicalFrame(
      final String name, final byte[] frame, final String typeName) {
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(frame, SchemaHash.hash16(typeName));
    if (decoded.header().compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalStateException(name + ": compressed fixture frames are not canonical");
    }
    if (decoded.header().flags() != NoritoHeader.COMPACT_LEN) {
      throw new IllegalStateException(name + ": fixture frame does not use exact canonical flags");
    }
    if (frame.length != NoritoHeader.HEADER_LENGTH + decoded.header().payloadLength()) {
      throw new IllegalStateException(name + ": fixture frame does not use exact zero padding");
    }
    decoded.header().validateChecksum(decoded.payload());
    try {
      NoritoHeader.decode(decoded.payload(), null);
      throw new IllegalStateException(name + ": bare fixture payload was accepted as a frame");
    } catch (final IllegalArgumentException expected) {
      // The SDK codec consumes inner bytes, but fixture transport always requires this frame.
    }
    return decoded.payload();
  }

  private static Map<String, Object> ttlFixture(
      final Object topLevelTtl, final Object payloadTtl) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("network_id", TEST_NETWORK_ID);
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 0L);
    payload.put("time_to_live_ms", payloadTtl);
    payload.put("nonce", null);
    payload.put("fee_payment", authorityFeePayment());
    payload.put("admission_intent", admissionIntent("ordinary"));
    payload.put("metadata", Collections.emptyMap());
    final Map<String, Object> executable = new LinkedHashMap<>();
    executable.put("Instructions", Collections.emptyList());
    payload.put("executable", executable);

    final Map<String, Object> fixture = firstReleaseFixture("ttl_fixture", payload);
    fixture.put("time_to_live_ms", topLevelTtl);
    return fixture;
  }

  private static Map<String, Object> firstReleaseFixture(
      final String name, final Map<String, Object> payload) {
    final Map<String, Object> fixture = new LinkedHashMap<>();
    fixture.put("name", name);
    fixture.put("network_id", payload.get("network_id"));
    fixture.put("authority", payload.get("authority"));
    fixture.put("creation_time_ms", payload.get("creation_time_ms"));
    fixture.put("time_to_live_ms", payload.get("time_to_live_ms"));
    fixture.put("nonce", payload.get("nonce"));
    fixture.put("payload_base64", "AA==");
    fixture.put("signed_base64", "AQ==");
    fixture.put("payload_hash", String.join("", Collections.nCopies(64, "0")));
    fixture.put("signed_hash", String.join("", Collections.nCopies(64, "1")));
    fixture.put("payload", payload);
    return fixture;
  }

  private static Map<String, Object> authorityFeePayment() {
    final Map<String, Object> value = new LinkedHashMap<>();
    value.put("charge_limits", Collections.emptyList());
    value.put("gas_limit", null);
    final Map<String, Object> payment = new LinkedHashMap<>();
    payment.put("payer", "authority");
    payment.put("value", value);
    return payment;
  }

  private static Map<String, Object> admissionIntent(final String intent) {
    final Map<String, Object> value = new LinkedHashMap<>();
    value.put("intent", intent);
    value.put("value", null);
    return value;
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException ex) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertThrowsContaining(
      final Runnable runnable, final String expectedMessage, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException ex) {
      if (ex.getMessage() != null && ex.getMessage().contains(expectedMessage)) {
        return;
      }
      throw new AssertionError(message + ": unexpected diagnostic: " + ex.getMessage(), ex);
    }
    throw new AssertionError(message);
  }

  private static String sampleAuthority(final byte fill) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill & 0xff), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build sample authority", ex);
    }
  }
}
