package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code RegisterNft} instructions. */
public final class RegisterNftInstruction implements InstructionTemplate {

  private final String nftId;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterNftInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterNftInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.nftId = builder.nftId;
    this.metadata = Collections.unmodifiableMap(new LinkedHashMap<>(builder.metadata));
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(argumentOrder));
  }

  /** Returns the NFT identifier (`name$domain`). */
  public String nftId() {
    return nftId;
  }

  /** Returns the metadata map associated with the NFT. */
  public Map<String, String> metadata() {
    return metadata;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterNftInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder().setNftId(require(arguments, "nft"));
    arguments.forEach((key, value) -> {
      if (key.startsWith("metadata.")) {
        final String metadataKey = key.substring("metadata.".length());
        if (!metadataKey.isEmpty()) {
          builder.putMetadata(metadataKey, value);
        }
      }
    });
    return new RegisterNftInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterNftInstruction other)) {
      return false;
    }
    return nftId.equals(other.nftId) && metadata.equals(other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(nftId, metadata);
  }

  public static final class Builder {
    private String nftId;
    private final Map<String, String> metadata = new LinkedHashMap<>();

    private Builder() {}

    public Builder setNftId(final String nftId) {
      if (nftId == null || nftId.isBlank()) {
        throw new IllegalArgumentException("nftId must not be blank");
      }
      this.nftId = nftId;
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      if (key == null || key.isBlank()) {
        throw new IllegalArgumentException("metadata key must not be blank");
      }
      metadata.put(key, Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterNftInstruction build() {
      if (nftId == null) {
        throw new IllegalStateException("nftId must be provided");
      }
      return new RegisterNftInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "RegisterNft");
      args.put("nft", nftId);
      metadata.forEach((key, value) -> args.put("metadata." + key, value));
      return args;
    }
  }
}
