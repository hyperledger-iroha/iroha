package org.hyperledger.iroha.sdk.core.model;

import static org.junit.jupiter.api.Assertions.*;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.hyperledger.iroha.sdk.core.model.instructions.*;
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter;
import org.junit.jupiter.api.Test;

/** Java-source callers use the Kotlin-owned Kaigi scalar, wire, and transaction implementations. */
final class KaigiWireJavaConsumerTest {
  @Test
  void fiveTypedInstructionsEncodeAndDecodeThroughTheRealTransactionAdapter() throws Exception {
    final String account = fixtureAccount();
    final KaigiInstructionUtils.CallId call = new KaigiInstructionUtils.CallId("wonderland.sora", "java-wire");
    final List<KaigiWireInstructionV1> instructions = Arrays.asList(
        CreateKaigiInstruction.create(call, account), new JoinKaigiInstruction(call, account),
        new LeaveKaigiInstruction(call, account), new EndKaigiInstruction(call),
        new RecordKaigiUsageInstruction(call, 1L));
    final List<InstructionBox> boxes = new ArrayList<>();
    for (final KaigiWireInstructionV1 instruction : instructions) {
      final InstructionBox box = instruction.toInstructionBox();
      assertTrue(box.getPayload() instanceof WirePayload);
      final byte[] canonical = NoritoJavaCodecAdapter.encodeInstructionBox(box);
      assertArrayEquals(canonical, NoritoJavaCodecAdapter.encodeInstructionBox(
          NoritoJavaCodecAdapter.decodeInstructionBox(canonical)));
      assertArrayEquals(instruction.getPayloadBytes(), KaigiWirePayloadEncoderV1.decode(
          instruction.getWireName(), instruction.getPayloadBytes()).getPayloadBytes());
      boxes.add(box);
    }
    final byte[] networkBytes = new byte[32]; Arrays.fill(networkBytes, (byte) 1);
    final TransactionPayload payload = new TransactionPayload(
        NetworkId.fromBytes(networkBytes), account, 1L, Executable.instructions(boxes),
        1000L, null, FeePaymentIntent.authority(Collections.emptyList()),
        TransactionAdmissionIntent.ORDINARY, Collections.emptyMap(), null);
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final byte[] encoded = codec.encodeTransaction(payload);
    assertArrayEquals(encoded, codec.encodeTransaction(codec.decodeTransaction(encoded)));
  }

  @Test
  void privateLeaveUsesRawScalarSnapshotsAndRetiredHintsCannotBeParsed() throws Exception {
    final byte[] source = new byte[32]; Arrays.fill(source, (byte) 0x22);
    final KaigiAuthorizationScalarV1 scalar = KaigiAuthorizationScalarV1.fromLeBytes(source);
    final byte[] retained = source.clone(); Arrays.fill(source, (byte) 0xff);
    assertArrayEquals(retained, scalar.toLeBytes());
    scalar.toLeBytes()[0] = 0;
    assertArrayEquals(retained, scalar.toLeBytes());
    final String root = repeat("55", 32);
    final LeaveKaigiInstruction leave = new LeaveKaigiInstruction(
        new KaigiInstructionUtils.CallId("wonderland.sora", "java-private-leave"), fixtureAccount(),
        scalar, scalar, root, "AQID");
    final byte[] encoded = NoritoJavaCodecAdapter.encodeInstructionBox(leave.toInstructionBox());
    assertArrayEquals(encoded, NoritoJavaCodecAdapter.encodeInstructionBox(
        NoritoJavaCodecAdapter.decodeInstructionBox(encoded)));
    for (final String key : Arrays.asList("commitment.alias_tag", "nullifier.issued_at_ms")) {
      final Map<String, String> arguments = new LinkedHashMap<>(leave.getArguments());
      arguments.put(key, "0");
      assertThrows(IllegalArgumentException.class, () -> LeaveKaigiInstruction.fromArguments(arguments));
    }
    final Map<String, String> oldHash = new LinkedHashMap<>(leave.getArguments());
    oldHash.put("commitment.commitment", "hash:" + repeat("22", 32) + "#0000");
    assertThrows(IllegalArgumentException.class, () -> LeaveKaigiInstruction.fromArguments(oldHash));
    assertThrows(IllegalArgumentException.class, () -> KaigiAuthorizationScalarV1.fromLeBytes(source));
  }

  private static String fixtureAccount() throws Exception {
    File directory = new File(".").getCanonicalFile();
    final String relative = "python/iroha_python/tests/fixtures/kaigi_instruction_wire_v1.json";
    while (directory != null && !new File(directory, relative).isFile()) directory = directory.getParentFile();
    assertNotNull(directory);
    final String json = new String(Files.readAllBytes(new File(directory, relative).toPath()), StandardCharsets.UTF_8);
    final Map<?, ?> fixture = (Map<?, ?>) JsonParser.parse(json);
    return (String) ((List<?>) fixture.get("accounts")).get(0);
  }

  private static String repeat(String value, int count) {
    final StringBuilder text = new StringBuilder();
    for (int index = 0; index < count; index++) text.append(value);
    return text.toString();
  }
}
