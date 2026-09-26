package org.hyperledger.iroha.sdk.core.model;

import static org.junit.jupiter.api.Assertions.*;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.hyperledger.iroha.sdk.core.model.instructions.UpdatePlainConvictionInstruction;
import org.hyperledger.iroha.sdk.norito.NoritoHeader;
import org.hyperledger.iroha.sdk.norito.SchemaHash;
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter;
import org.junit.jupiter.api.Test;

/** Java-source callers submit the Kotlin-owned choice-free native instruction. */
final class UpdatePlainConvictionJavaConsumerTest {
  @Test
  void updateUsesOneCanonicalFourFieldNoritoInstruction() throws Exception {
    final String owner = OWNER;
    final UpdatePlainConvictionInstruction update =
        new UpdatePlainConvictionInstruction(
            "referendum_1", owner, "20", "18446744073709551615");
    assertEquals("18446744073709551615", update.getDurationBlocks());
    assertEquals(4, update.getArguments().size());
    assertFalse(update.getArguments().containsKey("direction"));

    final InstructionBox box = update.toInstructionBox();
    final WirePayload wire = (WirePayload) box.getPayload();
    assertEquals(UpdatePlainConvictionInstruction.WIRE_NAME, wire.getWireName());
    assertEquals(
        update,
        UpdatePlainConvictionInstruction.fromWirePayload(
            wire.getPayloadBytes(), AccountAddress.DEFAULT_I105_DISCRIMINANT));
    final byte[] encoded = NoritoJavaCodecAdapter.encodeInstructionBox(box);
    assertArrayEquals(
        encoded,
        NoritoJavaCodecAdapter.encodeInstructionBox(
            NoritoJavaCodecAdapter.decodeInstructionBox(encoded)));

    final Map<String, String> forged = new LinkedHashMap<>(update.getArguments());
    forged.put("direction", "1");
    assertThrows(
        IllegalArgumentException.class,
        () -> UpdatePlainConvictionInstruction.fromCanonicalFields(forged));
  }

  @Test
  void javaSourceMatchesRustOwnedDirectConvictionGolden() throws Exception {
    final Map<?, ?> fixture = rustGolden();
    final Map<?, ?> inputs = (Map<?, ?>) fixture.get("inputs");
    assertEquals(4, inputs.size());
    assertEquals(1L, ((Number) fixture.get("version")).longValue());
    final Map<String, String> fields = new LinkedHashMap<>();
    fields.put("referendum_id", (String) inputs.get("referendum_id"));
    fields.put("owner", (String) inputs.get("owner"));
    fields.put("amount", (String) inputs.get("amount"));
    fields.put("duration_blocks", ((Number) inputs.get("duration_blocks")).toString());
    final UpdatePlainConvictionInstruction update =
        UpdatePlainConvictionInstruction.fromCanonicalFields(fields);
    assertEquals(fields, update.getArguments());
    assertEquals(UpdatePlainConvictionInstruction.WIRE_NAME, fixture.get("wire_id"));
    final String schema = (String) fixture.get("concrete_schema_name");
    assertEquals("iroha_data_model::isi::governance::UpdatePlainConviction", schema);
    assertEquals(fixture.get("concrete_schema_hash"), hexText(SchemaHash.hash16(schema)));

    final WirePayload wire = (WirePayload) update.toInstructionBox().getPayload();
    assertEquals(fixture.get("wire_id"), wire.getWireName());
    final byte[] frame = wire.getPayloadBytes();
    assertArrayEquals(hex((String) fixture.get("concrete_frame_hex")), frame);
    assertArrayEquals(Base64.getDecoder().decode((String) fixture.get("framed_instruction_base64")), frame);
    assertEquals(((Number) fixture.get("framed_instruction_len")).intValue(), frame.length);
    final NoritoHeader.DecodeResult concrete = NoritoHeader.decode(frame, SchemaHash.hash16(schema));
    concrete.getHeader().validateChecksum(concrete.getPayload());
    assertEquals(((Number) fixture.get("header_flags")).intValue(), concrete.getHeader().flags);
    assertArrayEquals(hex((String) fixture.get("bare_payload_hex")), concrete.getPayload());
    final UpdatePlainConvictionInstruction decoded = UpdatePlainConvictionInstruction.fromWirePayload(
        frame, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assertEquals(update, decoded);
    assertArrayEquals(frame, ((WirePayload) decoded.toInstructionBox().getPayload()).getPayloadBytes());

    final byte[] standalone = NoritoJavaCodecAdapter.encodeInstructionBox(update.toInstructionBox());
    assertArrayEquals(hex((String) fixture.get("standalone_instruction_box_frame_hex")), standalone);
    final NoritoHeader.DecodeResult boxed = NoritoHeader.decode(
        standalone, SchemaHash.hash16("(alloc::string::String, alloc::vec::Vec<u8>)"));
    boxed.getHeader().validateChecksum(boxed.getPayload());
    assertArrayEquals(hex((String) fixture.get("instruction_box_pair_hex")), boxed.getPayload());
    final InstructionBox decodedBox = NoritoJavaCodecAdapter.decodeInstructionBox(standalone);
    final WirePayload decodedWire = (WirePayload) decodedBox.getPayload();
    assertEquals(fixture.get("wire_id"), decodedWire.getWireName());
    assertArrayEquals(frame, decodedWire.getPayloadBytes());
    assertArrayEquals(standalone, NoritoJavaCodecAdapter.encodeInstructionBox(decodedBox));
  }

  private static Map<?, ?> rustGolden() throws Exception {
    final String relative = "fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json";
    File directory = new File(".").getCanonicalFile();
    while (directory != null && !new File(directory, relative).isFile()) {
      directory = directory.getParentFile();
    }
    assertNotNull(directory);
    final byte[] bytes = Files.readAllBytes(new File(directory, relative).toPath());
    return (Map<?, ?>) JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
  }

  private static byte[] hex(String value) {
    assertEquals(0, value.length() % 2);
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      final int high = Character.digit(value.charAt(index * 2), 16);
      final int low = Character.digit(value.charAt(index * 2 + 1), 16);
      assertTrue(high >= 0 && low >= 0);
      bytes[index] = (byte) ((high << 4) | low);
    }
    return bytes;
  }

  private static String hexText(byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      output[index * 2] = digits[(bytes[index] >>> 4) & 0x0f];
      output[index * 2 + 1] = digits[bytes[index] & 0x0f];
    }
    return new String(output);
  }

  private static final String OWNER =
      "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
}
