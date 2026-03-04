package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code PublishSpaceDirectoryManifest} instruction.
 *
 * <p>The manifest payload is stored as canonical JSON so Android callers can reuse the same
 * fixtures and workflow described in {@code docs/space-directory.md} without inventing bespoke
 * encodings.
 */
public final class PublishSpaceDirectoryManifestInstruction implements InstructionTemplate {

  private static final String ACTION = "PublishSpaceDirectoryManifest";

  private final String manifestJson;
  private final Map<String, String> arguments;

  private PublishSpaceDirectoryManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private PublishSpaceDirectoryManifestInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.manifestJson = builder.manifestJson;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  /** Returns the canonical JSON representation of the manifest. */
  public String manifestJson() {
    return manifestJson;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static PublishSpaceDirectoryManifestInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder().setManifestJson(require(arguments, "manifest_json"));
    return new PublishSpaceDirectoryManifestInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof PublishSpaceDirectoryManifestInstruction other)) {
      return false;
    }
    return Objects.equals(manifestJson, other.manifestJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(manifestJson);
  }

  public static final class Builder {
    private String manifestJson;

    private Builder() {}

    /**
     * Sets the canonical JSON representation of the manifest. Callers should pass the exact JSON
     * that {@code cargo xtask space-directory encode --json ...} would consume so Norito payloads
     * stay deterministic across toolchains.
     */
    public Builder setManifestJson(final String manifestJson) {
      this.manifestJson = Objects.requireNonNull(manifestJson, "manifestJson");
      return this;
    }

    public PublishSpaceDirectoryManifestInstruction build() {
      if (manifestJson == null || manifestJson.isBlank()) {
        throw new IllegalStateException("manifestJson must be provided");
      }
      return new PublishSpaceDirectoryManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("manifest_json", manifestJson);
      return args;
    }
  }
}
