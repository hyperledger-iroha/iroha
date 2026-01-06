package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code RegisterKaigiRelay} instructions. */
public final class RegisterKaigiRelayInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterKaigiRelay";

  private final String relayId;
  private final String hpkePublicKeyBase64;
  private final int bandwidthClass;
  private final Map<String, String> arguments;

  private RegisterKaigiRelayInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterKaigiRelayInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.relayId = builder.relayId;
    this.hpkePublicKeyBase64 = builder.hpkePublicKeyBase64;
    this.bandwidthClass = builder.bandwidthClass;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String relayId() {
    return relayId;
  }

  public String hpkePublicKeyBase64() {
    return hpkePublicKeyBase64;
  }

  public int bandwidthClass() {
    return bandwidthClass;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterKaigiRelayInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setRelayId(KaigiInstructionUtils.require(arguments, "relay.relay_id"));
    builder.setHpkePublicKeyBase64(KaigiInstructionUtils.require(arguments, "relay.hpke_public_key"));
    builder.setBandwidthClass(
        KaigiInstructionUtils.parseNonNegativeInt(
            KaigiInstructionUtils.require(arguments, "relay.bandwidth_class"),
            "relay.bandwidth_class"));
    return new RegisterKaigiRelayInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterKaigiRelayInstruction other)) {
      return false;
    }
    return Objects.equals(relayId, other.relayId)
        && Objects.equals(hpkePublicKeyBase64, other.hpkePublicKeyBase64)
        && bandwidthClass == other.bandwidthClass;
  }

  @Override
  public int hashCode() {
    return Objects.hash(relayId, hpkePublicKeyBase64, bandwidthClass);
  }

  public static final class Builder {
    private String relayId;
    private String hpkePublicKeyBase64;
    private int bandwidthClass;

    private Builder() {}

    public Builder setRelayId(final String relayId) {
      if (relayId == null || relayId.isBlank()) {
        throw new IllegalArgumentException("relayId must not be blank");
      }
      this.relayId = relayId;
      return this;
    }

    public Builder setHpkePublicKey(final byte[] hpkePublicKey) {
      this.hpkePublicKeyBase64 = KaigiInstructionUtils.toBase64(hpkePublicKey);
      return this;
    }

    public Builder setHpkePublicKeyBase64(final String base64) {
      this.hpkePublicKeyBase64 =
          KaigiInstructionUtils.requireBase64(base64, "hpkePublicKey");
      return this;
    }

    public Builder setBandwidthClass(final int bandwidthClass) {
      if (bandwidthClass < 0 || bandwidthClass > 0xFF) {
        throw new IllegalArgumentException("bandwidthClass must be between 0 and 255");
      }
      this.bandwidthClass = bandwidthClass;
      return this;
    }

    public RegisterKaigiRelayInstruction build() {
      if (relayId == null) {
        throw new IllegalStateException("relayId must be provided");
      }
      if (hpkePublicKeyBase64 == null) {
        throw new IllegalStateException("hpkePublicKey must be provided");
      }
      return new RegisterKaigiRelayInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("relay.relay_id", relayId);
      args.put("relay.hpke_public_key", hpkePublicKeyBase64);
      args.put("relay.bandwidth_class", Integer.toUnsignedString(bandwidthClass));
      return args;
    }
  }
}
