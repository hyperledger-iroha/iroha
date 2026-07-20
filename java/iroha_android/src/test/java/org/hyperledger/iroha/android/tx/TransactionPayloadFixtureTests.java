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
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.junit.Test;

public final class TransactionPayloadFixtureTests {

  private static final String SAMPLE_AUTHORITY = sampleAuthority((byte) 0x11);

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
    payload.put("chain", "00000001");
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 0L);
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap = new LinkedHashMap<>();
    fixtureMap.put("name", "wire_instruction_fixture");
    fixtureMap.put("chain", "00000001");
    fixtureMap.put("authority", SAMPLE_AUTHORITY);
    fixtureMap.put("creation_time_ms", 0L);
    fixtureMap.put("payload", payload);

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
    payload.put("chain", "00000001");
    payload.put("authority", SAMPLE_AUTHORITY);
    payload.put("creation_time_ms", 0L);
    payload.put("executable", executable);
    payload.put("metadata", Collections.emptyMap());

    final Map<String, Object> fixtureMap = new LinkedHashMap<>();
    fixtureMap.put("name", "wire_instruction_arguments_fixture");
    fixtureMap.put("chain", "00000001");
    fixtureMap.put("authority", SAMPLE_AUTHORITY);
    fixtureMap.put("creation_time_ms", 0L);
    fixtureMap.put("payload", payload);

    final TransactionPayloadFixtures.Fixture fixture =
        TransactionPayloadFixtures.Fixture.fromObject(fixtureMap);
    assertThrows(
        fixture::toPayload,
        "expected wire payload arguments to be rejected");
  }

  public static void main(final String[] args) throws Exception {
    runFixtures();
  }

  private static void runFixtures() throws Exception {
    final Path path = TransactionPayloadFixtures.resolveFixturePath();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    for (TransactionPayloadFixtures.Fixture fixture : TransactionPayloadFixtures.load(path)) {
      final String name = fixture.name();
      if (!fixture.isDecodable()) {
        throw new IllegalStateException(name + ": encoded payload not decodable");
      }
      final TransactionPayload payload = fixture.toPayload();
      assert Objects.equals(fixture.chain(), payload.chainId())
          : name + ": chain mismatch vs fixture metadata";
      assert Objects.equals(fixture.authority(), payload.authority())
          : name + ": authority mismatch vs fixture metadata";
      assert fixture.creationTimeMs() == payload.creationTimeMs()
          : name + ": creation_time_ms mismatch vs fixture metadata";
      assert Objects.equals(fixture.timeToLiveMs(), payload.timeToLiveMs())
          : name + ": TTL mismatch vs fixture metadata";
      assert Objects.equals(fixture.nonce(), payload.nonce())
          : name + ": nonce mismatch vs fixture metadata";
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
        assert !payload.metadata().containsKey("gas_asset_id")
            : name + ": legacy gas asset metadata must be absent";
        assert !payload.metadata().containsKey("gas_limit")
            : name + ": legacy gas limit metadata must be absent";
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
        assert "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
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
      if ("ivm_transfer".equals(name)) {
        assert payload.executable().isIvm() : name + ": legacy executable must remain IVM";
        assert payload.feePayment().gasLimit() == null : name + ": legacy fixture must remain gasless";
        try {
          adapter.encodeTransaction(payload);
          throw new AssertionError(name + ": gasless IVM payload must not be re-encoded");
        } catch (final NoritoException expected) {
          // Historical wire data remains decodable but cannot enter a new signing flow.
        }
        continue;
      }
      final byte[] encoded = adapter.encodeTransaction(payload);
      fixture.encoded().ifPresent(expected -> {
        if (payload.executable().isInstructions()) {
          assert allInstructionsWire(payload)
              : name + ": instruction fixtures must use wire payloads";
          final String actual = Base64.getEncoder().encodeToString(encoded);
          assert expected.equals(actual) : name + ": encoded payload mismatch";
          return;
        }
        final String actual = Base64.getEncoder().encodeToString(encoded);
        assert expected.equals(actual) : name + ": encoded payload mismatch";
      });
      if (!fixture.encoded().isPresent()) {
        final String base64 = Base64.getEncoder().encodeToString(encoded);
        System.out.println("[fixture] " + name + "=" + base64);
      }
      final TransactionPayload decoded = adapter.decodeTransaction(encoded);
      assertPayloadEquals(name, payload, decoded);
    }
    System.out.println("[IrohaAndroid] Transaction payload fixture tests passed.");
  }

  private static void assertPayloadEquals(
      final String name, final TransactionPayload expected, final TransactionPayload actual) {
    assert Objects.equals(expected.chainId(), actual.chainId())
        : name + ": chain mismatch";
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

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException ex) {
      return;
    }
    throw new AssertionError(message);
  }

  private static String sampleAuthority(final byte fill) {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, fill);
    try {
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build sample authority", ex);
    }
  }
}
