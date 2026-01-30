package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Internal helpers shared by trigger registration instruction builders. */
final class TriggerInstructionUtils {

  private static final String INSTRUCTION_PREFIX = "instruction.";
  private static final String KIND_SUFFIX = ".kind";
  private static final String ARG_PREFIX = ".arg.";
  private static final String METADATA_PREFIX = "metadata.";

  private TriggerInstructionUtils() {}

  static Map<String, String> extractMetadata(final Map<String, String> arguments) {
    final Map<String, String> metadata = new LinkedHashMap<>();
    arguments.forEach(
        (key, value) -> {
          if (key.startsWith(METADATA_PREFIX)) {
            final String metadataKey = key.substring(METADATA_PREFIX.length());
            if (!metadataKey.isEmpty()) {
              metadata.put(metadataKey, value);
            }
          }
        });
    return metadata;
  }

  static void appendMetadata(final Map<String, String> metadata, final Map<String, String> target) {
    metadata.forEach((key, value) -> target.put(METADATA_PREFIX + key, value));
  }

  static void appendInstructions(final List<InstructionBox> instructions, final Map<String, String> target) {
    for (int index = 0; index < instructions.size(); index++) {
      final InstructionBox instruction = instructions.get(index);
      if (!(instruction.payload() instanceof InstructionBox.WirePayload)) {
        throw new IllegalArgumentException(
            "Trigger registration requires wire-framed instruction payloads");
      }
      target.put(INSTRUCTION_PREFIX + index + KIND_SUFFIX, instruction.kind().displayName());
      for (final Map.Entry<String, String> entry : instruction.arguments().entrySet()) {
        target.put(
            INSTRUCTION_PREFIX + index + ARG_PREFIX + entry.getKey(), Objects.requireNonNull(entry.getValue()));
      }
    }
  }

  static List<InstructionBox> parseInstructions(final Map<String, String> arguments) {
    final Map<Integer, InstructionParts> grouped = new TreeMap<>();
    int parsedInstructionKeys = 0;
    for (final Map.Entry<String, String> entry : arguments.entrySet()) {
      final String key = entry.getKey();
      if (!key.startsWith(INSTRUCTION_PREFIX)) {
        continue;
      }
      parsedInstructionKeys++;
      final String remainder = key.substring(INSTRUCTION_PREFIX.length());
      final int separatorIndex = remainder.indexOf('.');
      if (separatorIndex <= 0) {
        throw new IllegalArgumentException("Malformed trigger instruction key: " + key);
      }
      final int index;
      try {
        index = Integer.parseInt(remainder.substring(0, separatorIndex));
      } catch (final NumberFormatException ex) {
        throw new IllegalArgumentException("Trigger instruction index must be numeric: " + key, ex);
      }
      final InstructionParts parts =
          grouped.computeIfAbsent(index, ignored -> new InstructionParts());
      final String suffix = remainder.substring(separatorIndex);
      if (KIND_SUFFIX.equals(suffix)) {
        parts.kind = entry.getValue();
      } else if (suffix.startsWith(ARG_PREFIX)) {
        final String argumentKey = suffix.substring(ARG_PREFIX.length());
        if (argumentKey.isEmpty()) {
          throw new IllegalArgumentException("Trigger instruction argument key must not be empty");
        }
        parts.arguments.put(argumentKey, entry.getValue());
      } else {
        throw new IllegalArgumentException("Unrecognised trigger instruction key: " + key);
      }
    }
    if (grouped.isEmpty()) {
      throw new IllegalArgumentException("Trigger registration must include at least one instruction");
    }
    final List<InstructionBox> instructions = new ArrayList<>(grouped.size());
    for (final Map.Entry<Integer, InstructionParts> entry : grouped.entrySet()) {
      final InstructionParts parts = entry.getValue();
      if (parts.kind == null || parts.kind.isBlank()) {
        throw new IllegalArgumentException(
            "instruction." + entry.getKey() + ".kind is required for trigger registration");
      }
      final InstructionKind nestedKind = InstructionKind.fromDisplayName(parts.kind);
      if (!parts.arguments.containsKey("wire_name")
          || !parts.arguments.containsKey("payload_base64")) {
        throw new IllegalArgumentException(
            "instruction." + entry.getKey() + " must include wire_name and payload_base64");
      }
      if (parts.arguments.size() != 2) {
        throw new IllegalArgumentException(
            "instruction." + entry.getKey()
                + " must not include extra arguments when using wire payloads");
      }
      instructions.add(
          InstructionBox.fromNorito(
              nestedKind,
              Collections.unmodifiableMap(new LinkedHashMap<>(parts.arguments))));
    }
    return Collections.unmodifiableList(new ArrayList<>(instructions));
  }

  static Integer parseRepeats(final String value) {
    if (value == null || value.isBlank() || RegisterTimeTriggerInstruction.REPEATS_INDEFINITE.equalsIgnoreCase(value)) {
      return null;
    }
    try {
      final int parsed = Integer.parseUnsignedInt(value);
      if (parsed <= 0) {
        throw new IllegalArgumentException("repeats must be greater than zero when provided");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException("repeats must be an unsigned integer", ex);
    }
  }

  private static final class InstructionParts {
    private String kind;
    private final LinkedHashMap<String, String> arguments = new LinkedHashMap<>();
  }
}
