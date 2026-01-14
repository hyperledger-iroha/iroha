package org.hyperledger.iroha.android.tx;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.SimpleJson;

final class TransactionPayloadFixtures {

  private static final NoritoJavaCodecAdapter DECODER = new NoritoJavaCodecAdapter();

  private TransactionPayloadFixtures() {}

  static List<Fixture> load(final Path path) throws IOException {
    final String json = new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
    final Object parsed = SimpleJson.parse(json);
    if (!(parsed instanceof List)) {
      throw new IllegalStateException("Fixture root must be an array");
    }
    @SuppressWarnings("unchecked")
    final List<Object> fixturesRaw = (List<Object>) parsed;
    final List<Fixture> fixtures = new ArrayList<>();
    for (Object entry : fixturesRaw) {
      fixtures.add(Fixture.fromObject(entry));
    }
    return fixtures;
  }

  static Path resolveFixturePath() throws IOException {
    final List<Path> candidates =
        Arrays.asList(
            Paths.get("java/iroha_android/src/test/resources/transaction_payloads.json"),
            Paths.get("src/test/resources/transaction_payloads.json"),
            Paths.get("../src/test/resources/transaction_payloads.json"),
            Paths.get("../../src/test/resources/transaction_payloads.json"));
    for (final Path candidate : candidates) {
      if (Files.exists(candidate)) {
        return candidate;
      }
    }
    throw new IOException("transaction_payloads.json not found (tried " + candidates + ")");
  }

  static final class Fixture {
    private final String name;
    private final Map<String, Object> payload;
    private final String encoded;
    private final TransactionPayload decodedPayload;

    private Fixture(final String name, final Map<String, Object> payload, final String encoded) {
      this.name = name;
      this.payload = payload;
      this.encoded = encoded;
      this.decodedPayload = payload == null && encoded != null ? decodePayload(name, encoded) : null;
    }

    static Fixture fromObject(final Object value) {
      if (!(value instanceof Map)) {
        throw new IllegalStateException("Fixture entries must be objects");
      }
      @SuppressWarnings("unchecked")
      final Map<Object, Object> map = (Map<Object, Object>) value;
      final String name = Objects.toString(map.get("name"), "<unnamed>");
      final Map<String, Object> payload =
          map.containsKey("payload") ? asMap(map.get("payload"), "payload", name) : null;
      final Object encoded = map.get("encoded");
      final Object payloadBase64 = map.get("payload_base64");
      final String resolvedEncoded =
          encoded != null
              ? Objects.toString(encoded)
              : payloadBase64 == null ? null : Objects.toString(payloadBase64);
      return new Fixture(name, payload, resolvedEncoded);
    }

    String name() {
      return name;
    }

    boolean isDecodable() {
      return payload != null || decodedPayload != null;
    }

    Optional<String> encoded() {
      return Optional.ofNullable(encoded);
    }

    TransactionPayload toPayload() {
      if (payload == null) {
        if (decodedPayload != null) {
          return decodedPayload;
        }
        throw new IllegalStateException(name + ": fixture missing payload and encoded data");
      }
      final TransactionPayload.Builder builder =
          TransactionPayload.builder()
              .setChainId(asString(payload.get("chain"), "chain"))
              .setAuthority(asString(payload.get("authority"), "authority"))
              .setCreationTimeMs(asNumber(payload.get("creation_time_ms"), "creation_time_ms").longValue());

      final Map<String, Object> exec = asMap(payload.get("executable"), "executable", name);
      if (exec.containsKey("Ivm")) {
        final String base64 = asString(exec.get("Ivm"), "executable.Ivm");
        builder.setExecutable(Executable.ivm(java.util.Base64.getDecoder().decode(base64)));
      } else if (exec.containsKey("Instructions")) {
        final List<?> instructionsRaw = asList(exec.get("Instructions"), "executable.Instructions");
        final List<InstructionBox> instructions = new ArrayList<>(instructionsRaw.size());
        for (Object entry : instructionsRaw) {
          final Map<String, Object> instructionMap = asMap(entry, "instruction", name);
          InstructionKind kind = InstructionKind.CUSTOM;
          if (instructionMap.containsKey("kind")) {
            kind =
                InstructionKind.fromDisplayName(asString(instructionMap.get("kind"), "instruction.kind"));
          }
          final Map<String, Object> args = instructionMap.get("arguments") == null
              ? Collections.emptyMap()
              : asMap(instructionMap.get("arguments"), "instruction.arguments", name);
          final Map<String, String> convertedArgs = new LinkedHashMap<>();
          args.forEach((key, value) -> convertedArgs.put(key, Objects.toString(value)));
          if (instructionMap.containsKey("name")) {
            final String customName = asString(instructionMap.get("name"), "instruction.name");
            convertedArgs.putIfAbsent("action", customName);
            try {
              kind = InstructionKind.fromDisplayName(customName);
            } catch (final IllegalArgumentException ignored) {
              kind = InstructionKind.CUSTOM;
            }
          }
          instructions.add(InstructionBox.fromNorito(kind, convertedArgs));
        }
        builder.setExecutable(Executable.instructions(instructions));
      } else {
        throw new IllegalStateException("Executable variant missing");
      }

      final Object ttl = payload.get("time_to_live_ms");
      builder.setTimeToLiveMs(ttl == null ? null : asNumber(ttl, "time_to_live_ms").longValue());

      final Object nonce = payload.get("nonce");
      builder.setNonce(nonce == null ? null : asNumber(nonce, "nonce").intValue());

      final Map<String, Object> metadataRaw = payload.get("metadata") == null
          ? Collections.emptyMap()
          : asMap(payload.get("metadata"), "metadata", name);
      final Map<String, String> metadata = new LinkedHashMap<>();
      metadataRaw.forEach((key, value) -> metadata.put(key, Objects.toString(value)));
      builder.setMetadata(metadata);
      return builder.build();
    }

    private static TransactionPayload decodePayload(final String name, final String encoded) {
      try {
        final byte[] bytes = java.util.Base64.getDecoder().decode(encoded);
        return DECODER.decodeTransaction(bytes);
      } catch (final Exception ex) {
        System.err.println("[fixture] " + name + ": failed to decode encoded payload (" + ex.getMessage() + ")");
        return null;
      }
    }
  }

  private static List<?> asList(final Object value, final String field) {
    if (!(value instanceof List)) {
      throw new IllegalStateException("Expected array for " + field);
    }
    return (List<?>) value;
  }

  private static Map<String, Object> asMap(
      final Object value, final String field, final String fixtureName) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException(
          fixtureName + ": expected object for " + field + " but found " + value);
    }
    final Map<String, Object> checked = new LinkedHashMap<>();
    @SuppressWarnings("unchecked")
    final Map<Object, Object> raw = (Map<Object, Object>) value;
    raw.forEach((k, v) -> checked.put(Objects.toString(k), v));
    return checked;
  }

  private static String asString(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalStateException("Expected string for " + field);
    }
    return (String) value;
  }

  private static Number asNumber(final Object value, final String field) {
    if (!(value instanceof Number)) {
      throw new IllegalStateException("Expected number for " + field);
    }
    return (Number) value;
  }

}
