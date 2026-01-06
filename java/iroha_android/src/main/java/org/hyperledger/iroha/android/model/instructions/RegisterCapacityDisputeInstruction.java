package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RegisterCapacityDispute} instruction (SoraFS dispute registry).
 */
public final class RegisterCapacityDisputeInstruction implements InstructionTemplate {

public static final String ACTION = "RegisterCapacityDispute";

  private final String disputeIdHex;
  private final String disputePayloadBase64;
  private final String providerIdHex;
  private final String complainantIdHex;
  private final String replicationOrderIdHex;
  private final Kind kind;
  private final long submittedEpoch;
  private final String description;
  private final String requestedRemedy;
  private final Evidence evidence;
  private final Map<String, String> arguments;

  private RegisterCapacityDisputeInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterCapacityDisputeInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.disputeIdHex = builder.disputeIdHex;
    this.disputePayloadBase64 = builder.disputePayloadBase64;
    this.providerIdHex = builder.providerIdHex;
    this.complainantIdHex = builder.complainantIdHex;
    this.replicationOrderIdHex = builder.replicationOrderIdHex;
    this.kind = builder.kind;
    this.submittedEpoch = builder.submittedEpoch;
    this.description = builder.description;
    this.requestedRemedy = builder.requestedRemedy;
    this.evidence = builder.evidence;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String disputeIdHex() {
    return disputeIdHex;
  }

  public String disputePayloadBase64() {
    return disputePayloadBase64;
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public String complainantIdHex() {
    return complainantIdHex;
  }

  public String replicationOrderIdHex() {
    return replicationOrderIdHex;
  }

  public Kind disputeKind() {
    return kind;
  }

  public long submittedEpoch() {
    return submittedEpoch;
  }

  public String description() {
    return description;
  }

  public String requestedRemedy() {
    return requestedRemedy;
  }

  public Evidence evidence() {
    return evidence;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterCapacityDisputeInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDisputeIdHex(require(arguments, "dispute_id_hex"))
            .setDisputePayloadBase64(require(arguments, "dispute_b64"))
            .setProviderIdHex(require(arguments, "provider_id_hex"))
            .setComplainantIdHex(require(arguments, "complainant_id_hex"))
            .setKind(Kind.fromLabel(require(arguments, "kind")))
            .setSubmittedEpoch(requireLong(arguments, "submitted_epoch"))
            .setDescription(require(arguments, "description"));

    final String replicationOrderId = arguments.get("replication_order_id_hex");
    if (replicationOrderId != null && !replicationOrderId.isBlank()) {
      builder.setReplicationOrderIdHex(replicationOrderId);
    }
    final String remedy = arguments.get("requested_remedy");
    if (remedy != null && !remedy.isBlank()) {
      builder.setRequestedRemedy(remedy);
    }
    builder.setEvidence(
        Evidence.builder()
            .setDigestHex(require(arguments, "evidence.digest_hex"))
            .setMediaType(arguments.get("evidence.media_type"))
            .setUri(arguments.get("evidence.uri"))
            .setSizeBytes(parseOptionalLong(arguments.get("evidence.size_bytes"), "evidence.size"))
            .build());
    return new RegisterCapacityDisputeInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String raw = require(arguments, key);
    try {
      return Long.parseLong(raw);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + raw, ex);
    }
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String requireBase64(final String value, final String fieldName) {
    final String trimmed = Objects.requireNonNull(value, fieldName).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    final byte[] decoded;
    try {
      decoded = java.util.Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
  }

  private static Long parseOptionalLong(final String value, final String label) {
    if (value == null || value.isBlank()) {
      return null;
    }
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(label + " must be a number: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterCapacityDisputeInstruction other)) {
      return false;
    }
    return submittedEpoch == other.submittedEpoch
        && Objects.equals(disputeIdHex, other.disputeIdHex)
        && Objects.equals(disputePayloadBase64, other.disputePayloadBase64)
        && Objects.equals(providerIdHex, other.providerIdHex)
        && Objects.equals(complainantIdHex, other.complainantIdHex)
        && Objects.equals(replicationOrderIdHex, other.replicationOrderIdHex)
        && kind == other.kind
        && Objects.equals(description, other.description)
        && Objects.equals(requestedRemedy, other.requestedRemedy)
        && Objects.equals(evidence, other.evidence);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        disputeIdHex,
        disputePayloadBase64,
        providerIdHex,
        complainantIdHex,
        replicationOrderIdHex,
        kind,
        submittedEpoch,
        description,
        requestedRemedy,
        evidence);
  }

  public static Builder builder() {
    return new Builder();
  }

  public enum Kind {
    REPLICATION_SHORTFALL("replication_shortfall"),
    UPTIME_BREACH("uptime_breach"),
    PROOF_FAILURE("proof_failure"),
    FEE_DISPUTE("fee_dispute"),
    OTHER("other");

    private final String label;

    Kind(final String label) {
      this.label = label;
    }

    public String label() {
      return label;
    }

    public static Kind fromLabel(final String value) {
      final String normalized = value.trim().toLowerCase(Locale.ROOT);
      for (final Kind kind : values()) {
        if (kind.label.equals(normalized)) {
          return kind;
        }
      }
      throw new IllegalArgumentException("Unknown capacity dispute kind: " + value);
    }
  }

  /** Evidence metadata recorded with the dispute. */
  public static final class Evidence {
    private final String digestHex;
    private final String mediaType;
    private final String uri;
    private final Long sizeBytes;

    private Evidence(final Evidence.Builder builder) {
      this.digestHex = builder.digestHex;
      this.mediaType = builder.mediaType;
      this.uri = builder.uri;
      this.sizeBytes = builder.sizeBytes;
    }

    public String digestHex() {
      return digestHex;
    }

    public String mediaType() {
      return mediaType;
    }

    public String uri() {
      return uri;
    }

    public Long sizeBytes() {
      return sizeBytes;
    }

    void appendArguments(final Map<String, String> args) {
      args.put("evidence.digest_hex", digestHex);
      if (mediaType != null && !mediaType.isBlank()) {
        args.put("evidence.media_type", mediaType);
      }
      if (uri != null && !uri.isBlank()) {
        args.put("evidence.uri", uri);
      }
      if (sizeBytes != null) {
        args.put("evidence.size_bytes", Long.toString(sizeBytes));
      }
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof Evidence other)) {
        return false;
      }
      return Objects.equals(digestHex, other.digestHex)
          && Objects.equals(mediaType, other.mediaType)
          && Objects.equals(uri, other.uri)
          && Objects.equals(sizeBytes, other.sizeBytes);
    }

    @Override
    public int hashCode() {
      return Objects.hash(digestHex, mediaType, uri, sizeBytes);
    }

    public static Evidence.Builder builder() {
      return new Evidence.Builder();
    }

    public static final class Builder {
      private String digestHex;
      private String mediaType;
      private String uri;
      private Long sizeBytes;

      private Builder() {}

      public Builder setDigestHex(final String digestHex) {
        final String normalized = Objects.requireNonNull(digestHex, "digestHex").trim();
        if (normalized.isEmpty()) {
          throw new IllegalArgumentException("digestHex must not be blank");
        }
        this.digestHex = normalized;
        return this;
      }

      public Builder setMediaType(final String mediaType) {
        this.mediaType = mediaType;
        return this;
      }

      public Builder setUri(final String uri) {
        this.uri = uri;
        return this;
      }

      public Builder setSizeBytes(final Long sizeBytes) {
        if (sizeBytes != null && sizeBytes < 0) {
          throw new IllegalArgumentException("sizeBytes must be non-negative");
        }
        this.sizeBytes = sizeBytes;
        return this;
      }

      public Evidence build() {
        if (digestHex == null || digestHex.isBlank()) {
          throw new IllegalStateException("digestHex must be provided");
        }
        return new Evidence(this);
      }
    }
  }

  public static final class Builder {
    private String disputeIdHex;
    private String disputePayloadBase64;
    private String providerIdHex;
    private String complainantIdHex;
    private String replicationOrderIdHex;
    private Kind kind;
    private Long submittedEpoch;
    private String description;
    private String requestedRemedy;
    private Evidence evidence;

    private Builder() {}

    public Builder setDisputeIdHex(final String disputeIdHex) {
      final String normalized = Objects.requireNonNull(disputeIdHex, "disputeIdHex").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("disputeIdHex must not be blank");
      }
      this.disputeIdHex = normalized;
      return this;
    }

    public Builder setDisputeId(final byte[] disputeId) {
      Objects.requireNonNull(disputeId, "disputeId");
      final StringBuilder sb = new StringBuilder(disputeId.length * 2);
      for (final byte b : disputeId) {
        sb.append(String.format("%02x", b));
      }
      return setDisputeIdHex(sb.toString());
    }

    public Builder setDisputePayloadBase64(final String disputePayloadBase64) {
      this.disputePayloadBase64 = requireBase64(disputePayloadBase64, "disputePayloadBase64");
      return this;
    }

    public Builder setDisputePayload(final byte[] payload) {
      Objects.requireNonNull(payload, "payload");
      final String encoded = java.util.Base64.getEncoder().encodeToString(payload);
      return setDisputePayloadBase64(encoded);
    }

    public Builder setProviderIdHex(final String providerIdHex) {
      final String normalized = Objects.requireNonNull(providerIdHex, "providerIdHex").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("providerIdHex must not be blank");
      }
      this.providerIdHex = normalized;
      return this;
    }

    public Builder setComplainantIdHex(final String complainantIdHex) {
      final String normalized =
          Objects.requireNonNull(complainantIdHex, "complainantIdHex").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("complainantIdHex must not be blank");
      }
      this.complainantIdHex = normalized;
      return this;
    }

    public Builder setReplicationOrderIdHex(final String replicationOrderIdHex) {
      this.replicationOrderIdHex =
          replicationOrderIdHex == null ? null : replicationOrderIdHex.trim();
      return this;
    }

    public Builder setKind(final Kind kind) {
      this.kind = Objects.requireNonNull(kind, "kind");
      return this;
    }

    public Builder setSubmittedEpoch(final long submittedEpoch) {
      this.submittedEpoch = ensureNonNegative(submittedEpoch, "submittedEpoch");
      return this;
    }

    public Builder setDescription(final String description) {
      final String normalized = Objects.requireNonNull(description, "description").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("description must not be blank");
      }
      this.description = normalized;
      return this;
    }

    public Builder setRequestedRemedy(final String requestedRemedy) {
      this.requestedRemedy = requestedRemedy;
      return this;
    }

    public Builder setEvidence(final Evidence evidence) {
      this.evidence = Objects.requireNonNull(evidence, "evidence");
      return this;
    }

    public RegisterCapacityDisputeInstruction build() {
      if (disputeIdHex == null || disputeIdHex.isBlank()) {
        throw new IllegalStateException("disputeIdHex must be provided");
      }
      if (disputePayloadBase64 == null || disputePayloadBase64.isBlank()) {
        throw new IllegalStateException("disputePayloadBase64 must be provided");
      }
      if (providerIdHex == null || providerIdHex.isBlank()) {
        throw new IllegalStateException("providerIdHex must be provided");
      }
      if (complainantIdHex == null || complainantIdHex.isBlank()) {
        throw new IllegalStateException("complainantIdHex must be provided");
      }
      if (kind == null) {
        throw new IllegalStateException("kind must be provided");
      }
      if (submittedEpoch == null) {
        throw new IllegalStateException("submittedEpoch must be provided");
      }
      if (description == null || description.isBlank()) {
        throw new IllegalStateException("description must be provided");
      }
      if (evidence == null) {
        throw new IllegalStateException("evidence must be provided");
      }
      return new RegisterCapacityDisputeInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("dispute_id_hex", disputeIdHex);
      args.put("dispute_b64", disputePayloadBase64);
      args.put("provider_id_hex", providerIdHex);
      args.put("complainant_id_hex", complainantIdHex);
      if (replicationOrderIdHex != null && !replicationOrderIdHex.isBlank()) {
        args.put("replication_order_id_hex", replicationOrderIdHex);
      }
      args.put("kind", kind.label());
      args.put("submitted_epoch", Long.toString(submittedEpoch));
      args.put("description", description);
      if (requestedRemedy != null && !requestedRemedy.isBlank()) {
        args.put("requested_remedy", requestedRemedy);
      }
      evidence.appendArguments(args);
      return args;
    }

    private static long ensureNonNegative(final long value, final String label) {
      if (value < 0) {
        throw new IllegalArgumentException(label + " must be non-negative");
      }
      return value;
    }
  }
}
