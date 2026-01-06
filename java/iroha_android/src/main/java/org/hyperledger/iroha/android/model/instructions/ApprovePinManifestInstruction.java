package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code ApprovePinManifest} instruction (SoraFS manifest lifecycle).
 */
public final class ApprovePinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "ApprovePinManifest";

  private final String digestHex;
  private final long approvedEpoch;
  private final String councilEnvelopeBase64;
  private final String councilEnvelopeDigestHex;
  private final Map<String, String> arguments;

  private ApprovePinManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ApprovePinManifestInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.digestHex = builder.digestHex;
    this.approvedEpoch = builder.approvedEpoch;
    this.councilEnvelopeBase64 = builder.councilEnvelopeBase64;
    this.councilEnvelopeDigestHex = builder.councilEnvelopeDigestHex;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String digestHex() {
    return digestHex;
  }

  public long approvedEpoch() {
    return approvedEpoch;
  }

  public String councilEnvelopeBase64() {
    return councilEnvelopeBase64;
  }

  public String councilEnvelopeDigestHex() {
    return councilEnvelopeDigestHex;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static ApprovePinManifestInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDigestHex(require(arguments, "digest_hex"))
            .setApprovedEpoch(requireLong(arguments, "approved_epoch"));
    final String envelope = arguments.get("council_envelope_base64");
    if (envelope != null && !envelope.isBlank()) {
      builder.setCouncilEnvelopeBase64(envelope);
    }
    final String envelopeDigest = arguments.get("council_envelope_digest_hex");
    if (envelopeDigest != null && !envelopeDigest.isBlank()) {
      builder.setCouncilEnvelopeDigestHex(envelopeDigest);
    }
    return new ApprovePinManifestInstruction(builder, new LinkedHashMap<>(arguments));
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
      decoded = Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ApprovePinManifestInstruction other)) {
      return false;
    }
    return approvedEpoch == other.approvedEpoch
        && Objects.equals(digestHex, other.digestHex)
        && Objects.equals(councilEnvelopeBase64, other.councilEnvelopeBase64)
        && Objects.equals(councilEnvelopeDigestHex, other.councilEnvelopeDigestHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(digestHex, approvedEpoch, councilEnvelopeBase64, councilEnvelopeDigestHex);
  }

  public static final class Builder {
    private String digestHex;
    private Long approvedEpoch;
    private String councilEnvelopeBase64;
    private String councilEnvelopeDigestHex;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
      return this;
    }

    public Builder setApprovedEpoch(final long approvedEpoch) {
      if (approvedEpoch < 0) {
        throw new IllegalArgumentException("approvedEpoch must be non-negative");
      }
      this.approvedEpoch = approvedEpoch;
      return this;
    }

    public Builder setCouncilEnvelopeBase64(final String councilEnvelopeBase64) {
      this.councilEnvelopeBase64 =
          requireBase64(councilEnvelopeBase64, "councilEnvelopeBase64");
      return this;
    }

    public Builder setCouncilEnvelopeBytes(final byte[] councilEnvelopeBytes) {
      Objects.requireNonNull(councilEnvelopeBytes, "councilEnvelopeBytes");
      return setCouncilEnvelopeBase64(
          Base64.getEncoder().encodeToString(councilEnvelopeBytes));
    }

    public Builder setCouncilEnvelopeDigestHex(final String digestHex) {
      this.councilEnvelopeDigestHex = Objects.requireNonNull(digestHex, "digestHex");
      return this;
    }

    public ApprovePinManifestInstruction build() {
      if (digestHex == null || digestHex.isBlank()) {
        throw new IllegalStateException("digestHex must be set");
      }
      if (approvedEpoch == null) {
        throw new IllegalStateException("approvedEpoch must be set");
      }
      return new ApprovePinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
      args.put("approved_epoch", Long.toString(approvedEpoch));
      if (councilEnvelopeBase64 != null && !councilEnvelopeBase64.isBlank()) {
        args.put("council_envelope_base64", councilEnvelopeBase64);
      }
      if (councilEnvelopeDigestHex != null && !councilEnvelopeDigestHex.isBlank()) {
        args.put("council_envelope_digest_hex", councilEnvelopeDigestHex);
      }
      return args;
    }
  }
}
