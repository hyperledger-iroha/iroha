package org.hyperledger.iroha.android.tx;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import org.hyperledger.iroha.android.model.ContractInvocation;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.ExecutableBatchItem;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.testing.SimpleJson;
import org.hyperledger.iroha.android.util.HashLiteral;

final class TransactionPayloadFixtures {

  private static final List<String> FIXTURE_FIELDS =
      Collections.unmodifiableList(
          Arrays.asList(
              "name",
              "network_id",
              "authority",
              "creation_time_ms",
              "time_to_live_ms",
              "nonce",
              "payload_base64",
              "signed_base64",
              "payload_hash",
              "signed_hash",
              "payload"));
  private static final List<String> PAYLOAD_FIELDS =
      Collections.unmodifiableList(
          Arrays.asList(
              "network_id",
              "authority",
              "creation_time_ms",
              "time_to_live_ms",
              "nonce",
              "fee_payment",
              "metadata",
              "executable"));

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
    final Set<String> names = new HashSet<>();
    final Set<ByteBuffer> payloadBytes = new HashSet<>();
    for (Object entry : fixturesRaw) {
      final Fixture fixture = Fixture.fromObject(entry);
      if (!names.add(fixture.name())) {
        throw new IllegalStateException("Duplicate fixture name: " + fixture.name());
      }
      final ByteBuffer identity =
          ByteBuffer.wrap(
                  decodeCanonicalBase64(
                      fixture.payloadBase64(), fixture.name() + ".payload_base64"))
              .asReadOnlyBuffer();
      if (!payloadBytes.add(identity)) {
        throw new IllegalStateException("Duplicate fixture payload bytes: " + fixture.name());
      }
      fixtures.add(fixture);
    }
    return fixtures;
  }

  private static byte[] decodeCanonicalBase64(final String value, final String context) {
    try {
      final byte[] decoded = Base64.getDecoder().decode(value);
      if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
        throw new IllegalStateException(context + " is not canonical base64");
      }
      return decoded;
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(context + " is not valid base64", ex);
    }
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
    private final NetworkId networkId;
    private final String authority;
    private final long creationTimeMs;
    private final Optional<Long> timeToLiveMs;
    private final Optional<Long> nonce;
    private final Map<String, Object> payload;
    private final String payloadBase64;

    private Fixture(
        final String name,
        final NetworkId networkId,
        final String authority,
        final long creationTimeMs,
        final Optional<Long> timeToLiveMs,
        final Optional<Long> nonce,
        final Map<String, Object> payload,
        final String payloadBase64) {
      this.name = name;
      this.networkId = networkId;
      this.authority = authority;
      this.creationTimeMs = creationTimeMs;
      this.timeToLiveMs = timeToLiveMs;
      this.nonce = nonce;
      this.payload = payload;
      this.payloadBase64 = payloadBase64;
    }

    static Fixture fromObject(final Object value) {
      if (!(value instanceof Map)) {
        throw new IllegalStateException("Fixture entries must be objects");
      }
      final Map<String, Object> map = asMap(value, "fixture", "<unnamed>");
      final String name = asString(map.get("name"), "fixture.name");
      requireExactFields(map, FIXTURE_FIELDS, name, "top-level");
      final Map<String, Object> payload = asMap(map.get("payload"), "payload", name);
      requireExactFields(payload, PAYLOAD_FIELDS, name, "payload");
      final NetworkId networkId =
          requireNetworkId(map.get("network_id"), name + ".network_id");
      final String authority = asString(map.get("authority"), name + ".authority");
      final long creationTimeMs =
          asNumber(map.get("creation_time_ms"), name + ".creation_time_ms").longValue();
      final long timeToLiveMs =
          requiredPositiveInteger(map, "time_to_live_ms", name + ".time_to_live_ms");
      final Optional<Long> nonce = optionalLong(map.get("nonce"), name + ".nonce");
      final String payloadBase64 =
          asString(map.get("payload_base64"), name + ".payload_base64");
      decodeCanonicalBase64(payloadBase64, name + ".payload_base64");
      decodeCanonicalBase64(
          asString(map.get("signed_base64"), name + ".signed_base64"),
          name + ".signed_base64");
      asString(map.get("payload_hash"), name + ".payload_hash");
      asString(map.get("signed_hash"), name + ".signed_hash");

      final NetworkId payloadNetworkId =
          requireNetworkId(payload.get("network_id"), name + ".payload.network_id");
      final String payloadAuthority =
          asString(payload.get("authority"), name + ".payload.authority");
      final long payloadCreationTimeMs =
          asNumber(payload.get("creation_time_ms"), name + ".payload.creation_time_ms")
              .longValue();
      final long payloadTimeToLiveMs =
          requiredPositiveInteger(
              payload, "time_to_live_ms", name + ".payload.time_to_live_ms");
      final Optional<Long> payloadNonce =
          optionalLong(payload.get("nonce"), name + ".payload.nonce");
      if (!networkId.equals(payloadNetworkId)
          || !authority.equals(payloadAuthority)
          || creationTimeMs != payloadCreationTimeMs
          || timeToLiveMs != payloadTimeToLiveMs
          || !nonce.equals(payloadNonce)) {
        throw new IllegalStateException(
            name + ": top-level transaction metadata must exactly match payload metadata");
      }
      return new Fixture(
          name,
          networkId,
          authority,
          creationTimeMs,
          Optional.of(timeToLiveMs),
          nonce,
          payload,
          payloadBase64);
    }

    String name() {
      return name;
    }

    NetworkId networkId() {
      return networkId;
    }

    String authority() {
      return authority;
    }

    long creationTimeMs() {
      return creationTimeMs;
    }

    Optional<Long> timeToLiveMs() {
      return timeToLiveMs;
    }

    Optional<Long> nonce() {
      return nonce;
    }

    String payloadBase64() {
      return payloadBase64;
    }

    TransactionPayload toPayload() {
      final TransactionPayload.Builder builder =
          TransactionPayload.builder()
              .setFeePayment(parseFeePayment(payload.get("fee_payment"), name))
              .setNetworkId(
                  requireNetworkId(
                      payload.get("network_id"), name + ".payload.network_id"))
              .setAuthority(asString(payload.get("authority"), "authority"))
              .setCreationTimeMs(
                  asNumber(payload.get("creation_time_ms"), "creation_time_ms").longValue());

      final Map<String, Object> exec = asMap(payload.get("executable"), "executable", name);
      if (exec.size() != 1) {
        throw new IllegalStateException(
            name + ": executable must contain exactly one externally tagged variant");
      }
      if (exec.containsKey("Ivm")) {
        final String base64 = asString(exec.get("Ivm"), "executable.Ivm");
        builder.setExecutable(
            Executable.ivm(decodeCanonicalBase64(base64, name + ".executable.Ivm")));
      } else if (exec.containsKey("Instructions")) {
        final List<?> instructionsRaw = asList(exec.get("Instructions"), "executable.Instructions");
        final List<InstructionBox> instructions = new ArrayList<>(instructionsRaw.size());
        for (int index = 0; index < instructionsRaw.size(); index++) {
          instructions.add(
              parseInstruction(
                  instructionsRaw.get(index), "executable.Instructions[" + index + "]", name));
        }
        builder.setExecutable(Executable.instructions(instructions));
      } else if (exec.containsKey("ContractCall")) {
        builder.setContractCall(
            parseContractInvocation(exec.get("ContractCall"), "executable.ContractCall", name));
      } else if (exec.containsKey("Batch")) {
        final List<?> itemsRaw = asList(exec.get("Batch"), "executable.Batch");
        final List<ExecutableBatchItem> items = new ArrayList<>(itemsRaw.size());
        for (int index = 0; index < itemsRaw.size(); index++) {
          final String context = "executable.Batch[" + index + "]";
          final Map<String, Object> item = asMap(itemsRaw.get(index), context, name);
          if (item.size() != 1) {
            throw new IllegalStateException(
                name + ": " + context + " must contain exactly one externally tagged variant");
          }
          if (item.containsKey("Instruction")) {
            items.add(
                ExecutableBatchItem.instruction(
                    parseInstruction(item.get("Instruction"), context + ".Instruction", name)));
          } else if (item.containsKey("ContractCall")) {
            items.add(
                ExecutableBatchItem.contractCall(
                    parseContractInvocation(
                        item.get("ContractCall"), context + ".ContractCall", name)));
          } else {
            throw new IllegalStateException(
                name + ": " + context + " has an unknown executable batch item variant");
          }
        }
        builder.setBatch(items);
      } else {
        throw new IllegalStateException("Executable variant missing");
      }

      builder.setTimeToLiveMs(
          requiredPositiveInteger(
              payload, "time_to_live_ms", name + ".payload.time_to_live_ms"));

      final Object nonce = payload.get("nonce");
      builder.setNonce(nonce == null ? null : asNumber(nonce, "nonce").longValue());

      final Map<String, Object> metadataRaw =
          asMap(payload.get("metadata"), "metadata", name);
      final Map<String, JsonValue> metadata = new LinkedHashMap<>();
      metadataRaw.forEach((key, value) -> metadata.put(key, jsonValue(value)));
      builder.setMetadata(metadata);
      return builder.build();
    }

    private static InstructionBox parseInstruction(
        final Object value, final String context, final String fixtureName) {
      final Map<String, Object> instructionMap = asMap(value, context, fixtureName);
      final Object wireNameRaw = instructionMap.get("wire_name");
      final Object wirePayloadRaw = instructionMap.get("payload_base64");
      if (wireNameRaw == null || wirePayloadRaw == null) {
        throw new IllegalStateException(
            fixtureName + ": " + context + " requires wire_name and payload_base64");
      }
      if (instructionMap.size() != 2) {
        throw new IllegalStateException(
            fixtureName + ": " + context + " must only include wire_name and payload_base64");
      }
      final String wireName = asString(wireNameRaw, context + ".wire_name");
      final String payloadBase64 = asString(wirePayloadRaw, context + ".payload_base64");
      final byte[] wirePayload;
      try {
        wirePayload =
            decodeCanonicalBase64(
                payloadBase64, fixtureName + "." + context + ".payload_base64");
      } catch (final IllegalStateException ex) {
        throw new IllegalStateException(
            fixtureName + ": " + context + ".payload_base64 is not valid base64", ex);
      }
      return InstructionBox.fromWirePayload(wireName, wirePayload);
    }

    private static ContractInvocation parseContractInvocation(
        final Object value, final String context, final String fixtureName) {
      final Map<String, Object> invocation = asMap(value, context, fixtureName);
      final Set<String> allowed =
          new HashSet<>(
              Arrays.asList(
                  "contract_address", "expected_code_hash", "entrypoint", "arguments"));
      if (!allowed.containsAll(invocation.keySet())) {
        throw new IllegalStateException(
            fixtureName + ": " + context + " contains unknown fields");
      }
      final String contractAddress =
          asString(invocation.get("contract_address"), context + ".contract_address");
      final String expectedCodeHash =
          asString(invocation.get("expected_code_hash"), context + ".expected_code_hash");
      final String entrypoint = asString(invocation.get("entrypoint"), context + ".entrypoint");
      final byte[] arguments =
          parseByteArray(invocation.get("arguments"), context + ".arguments", fixtureName);
      try {
        return new ContractInvocation(
            contractAddress, HashLiteral.decode(expectedCodeHash), entrypoint, arguments);
      } catch (final IllegalArgumentException ex) {
        throw new IllegalStateException(
            fixtureName + ": invalid " + context + " payload: " + ex.getMessage(), ex);
      }
    }

    private static byte[] parseByteArray(
        final Object value, final String context, final String fixtureName) {
      if (value == null) {
        return null;
      }
      final List<?> values = asList(value, context);
      if (values.size() > ContractInvocation.MAX_ARGUMENT_BYTES) {
        throw new IllegalStateException(
            fixtureName + ": " + context + " exceeds the signed wire limit");
      }
      final byte[] bytes = new byte[values.size()];
      for (int index = 0; index < values.size(); index++) {
        final long byteValue = asNumber(values.get(index), context + "[" + index + "]").longValue();
        if (byteValue < 0L || byteValue > 255L) {
          throw new IllegalStateException(
              fixtureName + ": " + context + "[" + index + "] must be an unsigned byte");
        }
        bytes[index] = (byte) byteValue;
      }
      return bytes;
    }

    private static FeePaymentIntent parseFeePayment(
        final Object value, final String fixtureName) {
      final Map<String, Object> payment = asMap(value, "fee_payment", fixtureName);
      final String payer = asString(payment.get("payer"), "fee_payment.payer");
      final Map<String, Object> paymentValue =
          asMap(payment.get("value"), "fee_payment.value", fixtureName);
      final List<?> limitsRaw =
          asList(paymentValue.get("charge_limits"), "fee_payment.value.charge_limits");
      final List<FeeChargeLimit> limits = new ArrayList<>(limitsRaw.size());
      for (int index = 0; index < limitsRaw.size(); index++) {
        final String context = "fee_payment.value.charge_limits[" + index + "]";
        final Map<String, Object> limit = asMap(limitsRaw.get(index), context, fixtureName);
        final Map<String, Object> kindValue =
            asMap(limit.get("kind"), context + ".kind", fixtureName);
        final String kindName = asString(kindValue.get("kind"), context + ".kind.kind");
        final FeeChargeKind kind;
        if ("nexus".equals(kindName)) {
          kind = FeeChargeKind.NEXUS;
        } else if ("pipeline_gas".equals(kindName)) {
          kind = FeeChargeKind.PIPELINE_GAS;
        } else {
          throw new IllegalStateException(
              fixtureName + ": " + context + " has unknown fee charge kind " + kindName);
        }
        limits.add(
            new FeeChargeLimit(
                kind,
                asString(limit.get("asset_definition_id"), context + ".asset_definition_id"),
                asString(limit.get("max_amount"), context + ".max_amount")));
      }
      final Object gasLimitRaw = paymentValue.get("gas_limit");
      final Long gasLimit =
          gasLimitRaw == null
              ? null
              : Long.valueOf(asNumber(gasLimitRaw, "fee_payment.value.gas_limit").longValue());
      if ("authority".equals(payer)) {
        return FeePaymentIntent.authority(limits, gasLimit);
      }
      if ("sponsor".equals(payer)) {
        final Map<String, Object> programId =
            asMap(paymentValue.get("program_id"), "fee_payment.value.program_id", fixtureName);
        final FeeSponsorProgramId parsedProgramId =
            new FeeSponsorProgramId(
                asString(programId.get("sponsor"), "fee_payment.value.program_id.sponsor"),
                asString(programId.get("name"), "fee_payment.value.program_id.name"));
        final long revision =
            asNumber(
                    paymentValue.get("program_revision"),
                    "fee_payment.value.program_revision")
                .longValue();
        return FeePaymentIntent.sponsor(parsedProgramId, revision, limits, gasLimit);
      }
      throw new IllegalStateException(fixtureName + ": unknown fee payer " + payer);
    }

    private static JsonValue jsonValue(final Object value) {
      if (value == null) {
        return JsonValue.nullValue();
      }
      if (value instanceof String) {
        return JsonValue.string((String) value);
      }
      if (value instanceof Number) {
        return JsonValue.parse(value.toString());
      }
      if (value instanceof Boolean) {
        return JsonValue.bool((Boolean) value);
      }
      throw new IllegalStateException("Unsupported metadata JSON value type: " + value.getClass());
    }

  }

  private static void requireExactFields(
      final Map<String, Object> map,
      final List<String> expectedFields,
      final String fixtureName,
      final String context) {
    for (final String field : map.keySet()) {
      if (!expectedFields.contains(field)) {
        throw new IllegalStateException(
            "fixture '"
                + fixtureName
                + "' contains unknown "
                + context
                + " field '"
                + field
                + "'");
      }
    }
    for (final String field : expectedFields) {
      if (!map.containsKey(field)) {
        throw new IllegalStateException(
            "fixture '"
                + fixtureName
                + "' missing required "
                + context
                + " field '"
                + field
                + "'");
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

  private static NetworkId requireNetworkId(final Object value, final String field) {
    final String networkId = asString(value, field);
    try {
      return NetworkId.parse(networkId);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(field + " must be a canonical network hash identity", ex);
    }
  }

  private static Number asNumber(final Object value, final String field) {
    if (!(value instanceof Number)) {
      throw new IllegalStateException("Expected number for " + field);
    }
    return (Number) value;
  }

  private static Optional<Long> optionalLong(final Object value, final String field) {
    if (value == null) {
      return Optional.empty();
    }
    return Optional.of(asNumber(value, field).longValue());
  }

  private static long requiredPositiveInteger(
      final Map<?, ?> map, final String key, final String field) {
    if (!map.containsKey(key)) {
      throw new IllegalStateException(field + " is required");
    }
    final Object value = map.get(key);
    if (!(value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long)) {
      throw new IllegalStateException(field + " must be an integer");
    }
    final long decoded = ((Number) value).longValue();
    if (decoded <= 0) {
      throw new IllegalStateException(field + " must be positive");
    }
    return decoded;
  }

}
