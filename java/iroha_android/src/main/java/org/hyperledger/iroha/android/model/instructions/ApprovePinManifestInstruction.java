package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code ApprovePinManifest} instruction (SoraFS manifest lifecycle).
 * The recorded approval epoch comes exclusively from the block consensus timestamp.
 */
public final class ApprovePinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "ApprovePinManifest";

  private final String digestHex;
  private final String councilEnvelopeBase64;
  private final String councilEnvelopeDigestHex;
  private final Map<String, String> arguments;

  private ApprovePinManifestInstruction(final Builder builder) {
    this.digestHex = builder.digestHex;
    this.councilEnvelopeBase64 = builder.councilEnvelopeBase64;
    this.councilEnvelopeDigestHex = builder.councilEnvelopeDigestHex;
    this.arguments = Map.copyOf(builder.canonicalArguments());
  }

  public String digestHex() {
    return digestHex;
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
    Objects.requireNonNull(arguments, "arguments");
    if (!ACTION.equals(arguments.get("action"))) {
      throw new IllegalArgumentException("Instruction argument 'action' must be " + ACTION);
    }
    for (final String key : arguments.keySet()) {
      if (!key.equals("action")
          && !key.equals("digest_hex")
          && !key.equals("council_envelope_base64")
          && !key.equals("council_envelope_digest_hex")) {
        throw new IllegalArgumentException("Unsupported ApprovePinManifest argument: " + key);
      }
    }
    final Builder builder = builder().setDigestHex(require(arguments, "digest_hex"));
    final String envelope = arguments.get("council_envelope_base64");
    if (envelope != null && !envelope.isBlank()) {
      builder.setCouncilEnvelopeBase64(envelope);
    }
    final String envelopeDigest = arguments.get("council_envelope_digest_hex");
    if (envelopeDigest != null && !envelopeDigest.isBlank()) {
      builder.setCouncilEnvelopeDigestHex(envelopeDigest);
    }
    return builder.build();
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

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ApprovePinManifestInstruction other)) {
      return false;
    }
    return Objects.equals(digestHex, other.digestHex)
        && Objects.equals(councilEnvelopeBase64, other.councilEnvelopeBase64)
        && Objects.equals(councilEnvelopeDigestHex, other.councilEnvelopeDigestHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(digestHex, councilEnvelopeBase64, councilEnvelopeDigestHex);
  }

  public static final class Builder {
    private String digestHex;
    private String councilEnvelopeBase64;
    private String councilEnvelopeDigestHex;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
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
      return new ApprovePinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
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
