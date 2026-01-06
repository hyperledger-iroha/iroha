package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code SetKaigiRelayManifest} instructions. */
public final class SetKaigiRelayManifestInstruction implements InstructionTemplate {

  private static final String ACTION = "SetKaigiRelayManifest";

  private final KaigiInstructionUtils.CallId callId;
  private final KaigiInstructionUtils.RelayManifest relayManifest;
  private final Map<String, String> arguments;

  private SetKaigiRelayManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private SetKaigiRelayManifestInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.relayManifest = builder.buildRelayManifest();
    this.arguments = Map.copyOf(argumentOrder);
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public KaigiInstructionUtils.RelayManifest relayManifest() {
    return relayManifest;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static SetKaigiRelayManifestInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    builder.setRelayManifest(KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest"));
    return new SetKaigiRelayManifestInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof SetKaigiRelayManifestInstruction other)) {
      return false;
    }
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && CreateKaigiInstruction.relayManifestEquals(relayManifest, other.relayManifest);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        callId.domainId(),
        callId.callName(),
        CreateKaigiInstruction.relayManifestHash(relayManifest));
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private Long relayManifestExpiry;
    private final List<KaigiInstructionUtils.RelayManifestHop> relayManifestHops = new ArrayList<>();

    private Builder() {}

    public Builder setCallId(final String domainId, final String callName) {
      this.callId = new KaigiInstructionUtils.CallId(domainId, callName);
      return this;
    }

    public Builder setCallId(final KaigiInstructionUtils.CallId callId) {
      this.callId = Objects.requireNonNull(callId, "callId");
      return this;
    }

    public Builder clearRelayManifest() {
      relayManifestExpiry = null;
      relayManifestHops.clear();
      return this;
    }

    public Builder setRelayManifestExpiryMs(final Long expiryMs) {
      if (expiryMs != null && expiryMs < 0) {
        throw new IllegalArgumentException("relay manifest expiry must be non-negative");
      }
      this.relayManifestExpiry = expiryMs;
      return this;
    }

    public Builder addRelayManifestHop(
        final String relayId, final String hpkePublicKeyBase64, final int weight) {
      if (relayId == null || relayId.isBlank()) {
        throw new IllegalArgumentException("relayId must not be blank");
      }
      final String normalizedKey =
          KaigiInstructionUtils.requireBase64(hpkePublicKeyBase64, "hpkePublicKey");
      if (weight < 0 || weight > 0xFF) {
        throw new IllegalArgumentException("relay hop weight must fit in an unsigned byte");
      }
      relayManifestHops.add(
          new KaigiInstructionUtils.RelayManifestHop()
              .withRelayId(relayId)
              .withHpkePublicKey(normalizedKey)
              .withWeight(weight));
      return this;
    }

    public Builder addRelayManifestHop(
        final String relayId, final byte[] hpkePublicKey, final int weight) {
      return addRelayManifestHop(
          relayId, KaigiInstructionUtils.toBase64(hpkePublicKey), weight);
    }

    public Builder setRelayManifest(final KaigiInstructionUtils.RelayManifest manifest) {
      relayManifestExpiry = null;
      relayManifestHops.clear();
      if (manifest != null) {
        relayManifestExpiry = manifest.expiryMs();
        for (KaigiInstructionUtils.RelayManifestHop hop : manifest.hops()) {
          relayManifestHops.add(
              new KaigiInstructionUtils.RelayManifestHop(
                  hop.relayId(), hop.hpkePublicKey(), hop.weight()));
        }
      }
      return this;
    }

    public SetKaigiRelayManifestInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      return new SetKaigiRelayManifestInstruction(this);
    }

    private KaigiInstructionUtils.RelayManifest buildRelayManifest() {
      if (relayManifestHops.isEmpty() && relayManifestExpiry == null) {
        return null;
      }
      final List<KaigiInstructionUtils.RelayManifestHop> copy =
          new ArrayList<>(relayManifestHops.size());
      for (KaigiInstructionUtils.RelayManifestHop hop : relayManifestHops) {
        copy.add(
            new KaigiInstructionUtils.RelayManifestHop(
                hop.relayId(), hop.hpkePublicKey(), hop.weight()));
      }
      return new KaigiInstructionUtils.RelayManifest(relayManifestExpiry, Collections.unmodifiableList(copy));
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, args, "call");
      KaigiInstructionUtils.appendRelayManifest(buildRelayManifest(), args, "relay_manifest");
      return args;
    }
  }
}
