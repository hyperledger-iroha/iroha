package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code CreateKaigi} instructions. */
public final class CreateKaigiInstruction implements InstructionTemplate {

  private static final String ACTION = "CreateKaigi";

  private final KaigiInstructionUtils.CallId callId;
  private final String host;
  private final String title;
  private final String description;
  private final Integer maxParticipants;
  private final long gasRatePerMinute;
  private final Map<String, String> metadata;
  private final Long scheduledStartMs;
  private final String billingAccount;
  private final KaigiInstructionUtils.PrivacyMode privacyMode;
  private final KaigiInstructionUtils.RelayManifest relayManifest;
  private final Map<String, String> arguments;

  private CreateKaigiInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CreateKaigiInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.host = builder.host;
    this.title = builder.title;
    this.description = builder.description;
    this.maxParticipants = builder.maxParticipants;
    this.gasRatePerMinute = builder.gasRatePerMinute;
    this.metadata =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(builder.metadata)));
    this.scheduledStartMs = builder.scheduledStartMs;
    this.billingAccount = builder.billingAccount;
    this.privacyMode = builder.privacyMode;
    this.relayManifest = builder.buildRelayManifest();
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public String host() {
    return host;
  }

  public String title() {
    return title;
  }

  public String description() {
    return description;
  }

  public Integer maxParticipants() {
    return maxParticipants;
  }

  public long gasRatePerMinute() {
    return gasRatePerMinute;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public Long scheduledStartMs() {
    return scheduledStartMs;
  }

  public String billingAccount() {
    return billingAccount;
  }

  public KaigiInstructionUtils.PrivacyMode privacyMode() {
    return privacyMode;
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

  public static CreateKaigiInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    builder.setHost(KaigiInstructionUtils.require(arguments, "host"));
    builder.setTitle(arguments.get("title"));
    builder.setDescription(arguments.get("description"));

    final Integer participants =
        KaigiInstructionUtils.parseOptionalPositiveInt(
            arguments.get("max_participants"), "max_participants");
    if (participants != null) {
      builder.setMaxParticipants(participants);
    }

    final long gasRate =
        KaigiInstructionUtils.parseUnsignedLong(
            arguments.getOrDefault("gas_rate_per_minute", "0"), "gas_rate_per_minute");
    builder.setGasRatePerMinute(gasRate);

    builder.setMetadata(KaigiInstructionUtils.extractMetadata(arguments, "metadata"));

    final Long scheduled =
        KaigiInstructionUtils.parseOptionalUnsignedLong(
            arguments.get("scheduled_start_ms"), "scheduled_start_ms");
    builder.setScheduledStartMs(scheduled);
    builder.setBillingAccount(arguments.get("billing_account"));
    builder.setPrivacyMode(KaigiInstructionUtils.parsePrivacyMode(arguments, "privacy"));

    final KaigiInstructionUtils.RelayManifest manifest =
        KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest");
    builder.setRelayManifest(manifest);

    return new CreateKaigiInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CreateKaigiInstruction other)) {
      return false;
    }
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && Objects.equals(host, other.host)
        && Objects.equals(title, other.title)
        && Objects.equals(description, other.description)
        && Objects.equals(maxParticipants, other.maxParticipants)
        && gasRatePerMinute == other.gasRatePerMinute
        && metadata.equals(other.metadata)
        && Objects.equals(scheduledStartMs, other.scheduledStartMs)
        && Objects.equals(billingAccount, other.billingAccount)
        && Objects.equals(privacyMode.mode(), other.privacyMode.mode())
        && Objects.equals(privacyMode.state(), other.privacyMode.state())
        && relayManifestEquals(relayManifest, other.relayManifest);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        callId.domainId(),
        callId.callName(),
        host,
        title,
        description,
        maxParticipants,
        gasRatePerMinute,
        metadata,
        scheduledStartMs,
        billingAccount,
        privacyMode.mode(),
        privacyMode.state(),
        relayManifestHash(relayManifest));
  }

  static boolean relayManifestEquals(
      final KaigiInstructionUtils.RelayManifest first, final KaigiInstructionUtils.RelayManifest second) {
    if (first == second) {
      return true;
    }
    if (first == null || second == null) {
      return false;
    }
    if (!Objects.equals(first.expiryMs(), second.expiryMs())) {
      return false;
    }
    final List<KaigiInstructionUtils.RelayManifestHop> hops1 = first.hops();
    final List<KaigiInstructionUtils.RelayManifestHop> hops2 = second.hops();
    if (hops1.size() != hops2.size()) {
      return false;
    }
    for (int index = 0; index < hops1.size(); index++) {
      final KaigiInstructionUtils.RelayManifestHop left = hops1.get(index);
      final KaigiInstructionUtils.RelayManifestHop right = hops2.get(index);
      if (!Objects.equals(left.relayId(), right.relayId())
          || !Objects.equals(left.hpkePublicKey(), right.hpkePublicKey())
          || !Objects.equals(left.weight(), right.weight())) {
        return false;
      }
    }
    return true;
  }

  static int relayManifestHash(final KaigiInstructionUtils.RelayManifest manifest) {
    if (manifest == null) {
      return 0;
    }
    int result = Objects.hash(manifest.expiryMs());
    for (final KaigiInstructionUtils.RelayManifestHop hop : manifest.hops()) {
      result =
          31 * result
              + Objects.hash(hop.relayId(), hop.hpkePublicKey(), hop.weight());
    }
    return result;
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private String host;
    private String title;
    private String description;
    private Integer maxParticipants;
    private long gasRatePerMinute;
    private final Map<String, String> metadata = new LinkedHashMap<>();
    private Long scheduledStartMs;
    private String billingAccount;
    private KaigiInstructionUtils.PrivacyMode privacyMode =
        new KaigiInstructionUtils.PrivacyMode("Transparent", null);
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

    public Builder setHost(final String host) {
      if (host == null || host.isBlank()) {
        throw new IllegalArgumentException("host must not be blank");
      }
      this.host = host;
      return this;
    }

    public Builder setTitle(final String title) {
      this.title = title;
      return this;
    }

    public Builder setDescription(final String description) {
      this.description = description;
      return this;
    }

    public Builder setMaxParticipants(final Integer maxParticipants) {
      if (maxParticipants != null && maxParticipants <= 0) {
        throw new IllegalArgumentException("maxParticipants must be greater than zero when provided");
      }
      this.maxParticipants = maxParticipants;
      return this;
    }

    public Builder setGasRatePerMinute(final long gasRatePerMinute) {
      if (gasRatePerMinute < 0) {
        throw new IllegalArgumentException("gasRatePerMinute must be non-negative");
      }
      this.gasRatePerMinute = gasRatePerMinute;
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(
          Objects.requireNonNull(key, "metadata key"),
          Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach(this::putMetadata);
      }
      return this;
    }

    public Builder setScheduledStartMs(final Long scheduledStartMs) {
      if (scheduledStartMs != null && scheduledStartMs < 0) {
        throw new IllegalArgumentException("scheduledStartMs must be non-negative");
      }
      this.scheduledStartMs = scheduledStartMs;
      return this;
    }

    public Builder setBillingAccount(final String billingAccount) {
      this.billingAccount = billingAccount;
      return this;
    }

    public Builder setPrivacyMode(final String mode) {
      this.privacyMode = new KaigiInstructionUtils.PrivacyMode(mode, null);
      return this;
    }

    public Builder setPrivacyMode(final KaigiInstructionUtils.PrivacyMode privacyMode) {
      this.privacyMode = Objects.requireNonNull(privacyMode, "privacyMode");
      return this;
    }

    public Builder setPrivacyMode(final String mode, final String state) {
      this.privacyMode = new KaigiInstructionUtils.PrivacyMode(mode, state);
      return this;
    }

    public Builder clearRelayManifest() {
      this.relayManifestExpiry = null;
      this.relayManifestHops.clear();
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
      final KaigiInstructionUtils.RelayManifestHop hop =
          new KaigiInstructionUtils.RelayManifestHop()
              .withRelayId(relayId)
              .withHpkePublicKey(normalizedKey)
              .withWeight(weight);
      relayManifestHops.add(hop);
      return this;
    }

    public Builder addRelayManifestHop(
        final String relayId, final byte[] hpkePublicKey, final int weight) {
      return addRelayManifestHop(
          relayId, KaigiInstructionUtils.toBase64(hpkePublicKey), weight);
    }

    public Builder setRelayManifest(final KaigiInstructionUtils.RelayManifest manifest) {
      relayManifestHops.clear();
      relayManifestExpiry = null;
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

    public CreateKaigiInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (host == null) {
        throw new IllegalStateException("host must be provided");
      }
      return new CreateKaigiInstruction(this);
    }

    private KaigiInstructionUtils.RelayManifest buildRelayManifest() {
      if (relayManifestHops.isEmpty() && relayManifestExpiry == null) {
        return null;
      }
      final List<KaigiInstructionUtils.RelayManifestHop> copy = new ArrayList<>(relayManifestHops.size());
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
      args.put("host", host);
      if (title != null) {
        args.put("title", title);
      }
      if (description != null) {
        args.put("description", description);
      }
      if (maxParticipants != null) {
        args.put("max_participants", Integer.toString(maxParticipants));
      }
      args.put("gas_rate_per_minute", Long.toUnsignedString(gasRatePerMinute));
      KaigiInstructionUtils.appendMetadata(metadata, args, "metadata");
      if (scheduledStartMs != null) {
        args.put("scheduled_start_ms", Long.toUnsignedString(scheduledStartMs));
      }
      if (billingAccount != null) {
        args.put("billing_account", billingAccount);
      }
      KaigiInstructionUtils.appendPrivacyMode(privacyMode, args, "privacy");
      KaigiInstructionUtils.appendRelayManifest(buildRelayManifest(), args, "relay_manifest");
      return args;
    }
  }
}
