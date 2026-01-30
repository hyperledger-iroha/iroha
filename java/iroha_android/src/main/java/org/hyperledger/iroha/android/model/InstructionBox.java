package org.hyperledger.iroha.android.model;

import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;

/**
 * Typed representation of an instruction scheduled for execution within a transaction.
 *
 * <p>The container wraps strongly typed instruction payloads where available (for example, register
 * domain/account/asset definition builders) and can wrap opaque key/value payloads for local use.
 * Transaction encoding requires wire-framed instruction payloads so Norito parity is preserved
 * whenever raw instruction bytes are available.
 */
public final class InstructionBox {

  private static final String ARG_WIRE_NAME = "wire_name";
  private static final String ARG_PAYLOAD_BASE64 = "payload_base64";

  private final InstructionPayload payload;

  private InstructionBox(final InstructionPayload payload) {
    this.payload = Objects.requireNonNull(payload, "payload");
  }

  /** Instruction display name (matches `InstructionType` tag). */
  public String name() {
    if (payload instanceof WirePayload wire) {
      return wire.wireName();
    }
    return kind().displayName();
  }

  public InstructionKind kind() {
    return payload.kind();
  }

  public InstructionPayload payload() {
    return payload;
  }

  /**
   * Returns Norito-ready arguments describing the instruction. The returned map is immutable and
   * safe to cache by callers.
   */
  public Map<String, String> arguments() {
    return payload.toArguments();
  }

  public Builder toBuilder() {
    return builder().setKind(kind()).setArguments(arguments());
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof InstructionBox other)) {
      return false;
    }
    return kind() == other.kind() && arguments().equals(other.arguments());
  }

  @Override
  public int hashCode() {
    return Objects.hash(kind(), arguments());
  }

  public static InstructionBox of(final InstructionPayload payload) {
    return new InstructionBox(payload);
  }

  /**
   * Builds an {@link InstructionBox} from a canonical wire identifier and its Norito-framed payload.
   *
   * <p>The payload must include the Norito header with a valid checksum.
   */
  public static InstructionBox fromWirePayload(final String wireName, final byte[] payloadBytes) {
    return new InstructionBox(new WireInstructionPayload(wireName, payloadBytes));
  }

  /**
   * Builds an {@link InstructionBox} from Norito decoded components.
   *
   * <p>The arguments must include {@code wire_name} and {@code payload_base64} fields carrying a
   * Norito-framed instruction payload.
   */
  public static InstructionBox fromNorito(
      final InstructionKind kind, final Map<String, String> arguments) {
    return new InstructionBox(resolvePayload(kind, arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  private static InstructionPayload resolvePayload(
      final InstructionKind kind, final Map<String, String> arguments) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(arguments, "arguments");
    final String wireName = arguments.get(ARG_WIRE_NAME);
    final String payloadBase64 = arguments.get(ARG_PAYLOAD_BASE64);
    if (wireName == null && payloadBase64 == null) {
      throw new IllegalArgumentException(
          "instruction arguments must include wire_name and payload_base64 fields");
    }
    if (wireName == null || payloadBase64 == null) {
      throw new IllegalArgumentException(
          "wire payload requires both wire_name and payload_base64 fields");
    }
    final byte[] payloadBytes;
    try {
      payloadBytes = Base64.getDecoder().decode(payloadBase64);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("wire payload base64 is invalid", ex);
    }
    return new WireInstructionPayload(wireName, payloadBytes);
  }

  private static InstructionKind wireKindForName(final String wireName) {
    if (wireName == null) {
      return InstructionKind.CUSTOM;
    }
    final String normalized = wireName.toLowerCase(Locale.ROOT);
    if (normalized.startsWith("iroha.register")) {
      return InstructionKind.REGISTER;
    }
    if (normalized.startsWith("iroha.unregister")) {
      return InstructionKind.UNREGISTER;
    }
    if (normalized.startsWith("iroha.transfer")) {
      return InstructionKind.TRANSFER;
    }
    if (normalized.startsWith("iroha.mint")) {
      return InstructionKind.MINT;
    }
    if (normalized.startsWith("iroha.burn")) {
      return InstructionKind.BURN;
    }
    if (normalized.startsWith("iroha.grant")) {
      return InstructionKind.GRANT;
    }
    if (normalized.startsWith("iroha.revoke")) {
      return InstructionKind.REVOKE;
    }
    if (normalized.startsWith("iroha.set_key_value")) {
      return InstructionKind.SET_KEY_VALUE;
    }
    if (normalized.startsWith("iroha.remove_key_value")) {
      return InstructionKind.REMOVE_KEY_VALUE;
    }
    if (normalized.startsWith("iroha.set_parameter")) {
      return InstructionKind.SET_PARAMETER;
    }
    if (normalized.startsWith("iroha.execute_trigger")) {
      return InstructionKind.EXECUTE_TRIGGER;
    }
    if (normalized.startsWith("iroha.log")) {
      return InstructionKind.LOG;
    }
    if (normalized.startsWith("iroha.runtime_upgrade") || normalized.startsWith("iroha.upgrade")) {
      return InstructionKind.UPGRADE;
    }
    return InstructionKind.CUSTOM;
  }

  public interface InstructionPayload {
    InstructionKind kind();

    Map<String, String> toArguments();
  }

  public interface WirePayload extends InstructionPayload {
    String wireName();

    byte[] payloadBytes();
  }

  private static final class WireInstructionPayload implements WirePayload {
    private final InstructionKind kind;
    private final String wireName;
    private final byte[] payloadBytes;
    private final Map<String, String> arguments;

    private WireInstructionPayload(final String wireName, final byte[] payloadBytes) {
      if (wireName == null || wireName.isBlank()) {
        throw new IllegalArgumentException("wireName must not be blank");
      }
      if (payloadBytes == null || payloadBytes.length == 0) {
        throw new IllegalArgumentException("payloadBytes must not be empty");
      }
      this.wireName = wireName;
      this.payloadBytes = payloadBytes.clone();
      this.kind = wireKindForName(wireName);
      final LinkedHashMap<String, String> args = new LinkedHashMap<>();
      args.put(ARG_WIRE_NAME, wireName);
      args.put(ARG_PAYLOAD_BASE64, Base64.getEncoder().encodeToString(this.payloadBytes));
      this.arguments = Collections.unmodifiableMap(args);
    }

    @Override
    public InstructionKind kind() {
      return kind;
    }

    @Override
    public Map<String, String> toArguments() {
      return arguments;
    }

    @Override
    public String wireName() {
      return wireName;
    }

    @Override
    public byte[] payloadBytes() {
      return payloadBytes.clone();
    }
  }

  public static final class Builder {
    private InstructionKind kind = InstructionKind.CUSTOM;
    private final Map<String, String> arguments = new LinkedHashMap<>();

    public Builder setKind(final InstructionKind kind) {
      this.kind = Objects.requireNonNull(kind, "kind");
      return this;
    }

    public Builder setName(final String name) {
      Objects.requireNonNull(name, "name");
      try {
        this.kind = InstructionKind.fromDisplayName(name);
      } catch (final IllegalArgumentException ignored) {
        this.kind = InstructionKind.CUSTOM;
      }
      arguments.putIfAbsent("action", name);
      return this;
    }

    public Builder putArgument(final String key, final String value) {
      arguments.put(Objects.requireNonNull(key, "key"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setArguments(final Map<String, String> arguments) {
      this.arguments.clear();
      if (arguments != null) {
        arguments.forEach(this::putArgument);
      }
      return this;
    }

    public InstructionBox build() {
      return InstructionBox.fromNorito(kind, arguments);
    }
  }
}
