package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RegisterPinManifest} instruction.
 *
 * <p>This surfaces the SoraFS pin-manifest metadata (chunker handle, policy, alias bindings, and
 * epoch hints) so Android tests can build deterministic fixtures aligned with the Rust
 * implementation.
 */
public final class RegisterPinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "RegisterPinManifest";

  private final String digestHex;
  private final ChunkerProfile chunkerProfile;
  private final String chunkDigestSha3Hex;
  private final PinPolicy pinPolicy;
  private final long submittedEpoch;
  private final String successorOfHex;
  private final AliasBinding aliasBinding;
  private final Map<String, String> arguments;

  private RegisterPinManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterPinManifestInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.digestHex = builder.digestHex;
    this.chunkerProfile = builder.chunkerProfile;
    this.chunkDigestSha3Hex = builder.chunkDigestSha3Hex;
    this.pinPolicy = builder.pinPolicy;
    this.submittedEpoch = builder.submittedEpoch;
    this.successorOfHex = builder.successorOfHex;
    this.aliasBinding = builder.aliasBinding;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String digestHex() {
    return digestHex;
  }

  public ChunkerProfile chunkerProfile() {
    return chunkerProfile;
  }

  public String chunkDigestSha3Hex() {
    return chunkDigestSha3Hex;
  }

  public PinPolicy pinPolicy() {
    return pinPolicy;
  }

  public long submittedEpoch() {
    return submittedEpoch;
  }

  public String successorOfHex() {
    return successorOfHex;
  }

  public AliasBinding aliasBinding() {
    return aliasBinding;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static RegisterPinManifestInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setDigestHex(require(arguments, "digest_hex"));
    builder.setChunkDigestSha3Hex(require(arguments, "chunk_digest_sha3_256_hex"));
    builder.setSubmittedEpoch(requireLong(arguments, "submitted_epoch"));
    if (arguments.containsKey("successor_of_hex")) {
      builder.setSuccessorOfHex(arguments.get("successor_of_hex"));
    }
    builder.setChunkerProfile(ChunkerProfile.fromArguments(arguments));
    builder.setPinPolicy(PinPolicy.fromArguments(arguments));
    final AliasBinding alias = AliasBinding.fromArguments(arguments, /* required= */ false);
    if (alias != null) {
      builder.setAliasBinding(alias);
    }
    return new RegisterPinManifestInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
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
    if (!(obj instanceof RegisterPinManifestInstruction other)) {
      return false;
    }
    return Objects.equals(digestHex, other.digestHex)
        && Objects.equals(chunkerProfile, other.chunkerProfile)
        && Objects.equals(chunkDigestSha3Hex, other.chunkDigestSha3Hex)
        && Objects.equals(pinPolicy, other.pinPolicy)
        && submittedEpoch == other.submittedEpoch
        && Objects.equals(successorOfHex, other.successorOfHex)
        && Objects.equals(aliasBinding, other.aliasBinding);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        digestHex,
        chunkerProfile,
        chunkDigestSha3Hex,
        pinPolicy,
        submittedEpoch,
        successorOfHex,
        aliasBinding);
  }

  public static final class Builder {
    private String digestHex;
    private ChunkerProfile chunkerProfile;
    private String chunkDigestSha3Hex;
    private PinPolicy pinPolicy;
    private Long submittedEpoch;
    private String successorOfHex;
    private AliasBinding aliasBinding;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
      return this;
    }

    public Builder setChunkerProfile(final ChunkerProfile chunkerProfile) {
      this.chunkerProfile = Objects.requireNonNull(chunkerProfile, "chunkerProfile");
      return this;
    }

    public Builder setChunkDigestSha3Hex(final String chunkDigestSha3Hex) {
      this.chunkDigestSha3Hex = Objects.requireNonNull(chunkDigestSha3Hex, "chunkDigestSha3Hex");
      return this;
    }

    public Builder setPinPolicy(final PinPolicy pinPolicy) {
      this.pinPolicy = Objects.requireNonNull(pinPolicy, "pinPolicy");
      return this;
    }

    public Builder setSubmittedEpoch(final long submittedEpoch) {
      if (submittedEpoch < 0) {
        throw new IllegalArgumentException("submittedEpoch must be non-negative");
      }
      this.submittedEpoch = submittedEpoch;
      return this;
    }

    public Builder setSuccessorOfHex(final String successorOfHex) {
      this.successorOfHex = successorOfHex;
      return this;
    }

    public Builder setAliasBinding(final AliasBinding aliasBinding) {
      this.aliasBinding = aliasBinding;
      return this;
    }

    public RegisterPinManifestInstruction build() {
      if (digestHex == null || digestHex.isBlank()) {
        throw new IllegalStateException("digestHex must be set");
      }
      if (chunkerProfile == null) {
        throw new IllegalStateException("chunkerProfile must be set");
      }
      if (chunkDigestSha3Hex == null || chunkDigestSha3Hex.isBlank()) {
        throw new IllegalStateException("chunkDigestSha3Hex must be set");
      }
      if (pinPolicy == null) {
        throw new IllegalStateException("pinPolicy must be set");
      }
      if (submittedEpoch == null) {
        throw new IllegalStateException("submittedEpoch must be set");
      }
      return new RegisterPinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
      args.put("chunk_digest_sha3_256_hex", chunkDigestSha3Hex);
      args.put("submitted_epoch", Long.toString(submittedEpoch));
      if (successorOfHex != null && !successorOfHex.isBlank()) {
        args.put("successor_of_hex", successorOfHex);
      }
      chunkerProfile.appendArguments(args);
      pinPolicy.appendArguments(args);
      if (aliasBinding != null) {
        aliasBinding.appendArguments(args);
      }
      return args;
    }
  }

  /** Chunker profile metadata recorded alongside the manifest. */
  public static final class ChunkerProfile {
    private final int profileId;
    private final String namespace;
    private final String name;
    private final String semver;
    private final String handle;
    private final long multihashCode;

    private ChunkerProfile(final Builder builder) {
      this.profileId = builder.profileId;
      this.namespace = builder.namespace;
      this.name = builder.name;
      this.semver = builder.semver;
      this.handle = builder.handle;
      this.multihashCode = builder.multihashCode;
    }

    public int profileId() {
      return profileId;
    }

    public String namespace() {
      return namespace;
    }

    public String name() {
      return name;
    }

    public String semver() {
      return semver;
    }

    public String handle() {
      return handle;
    }

    public long multihashCode() {
      return multihashCode;
    }

    private void appendArguments(final Map<String, String> arguments) {
      arguments.put("chunker.profile_id", Integer.toString(profileId));
      arguments.put("chunker.namespace", namespace);
      arguments.put("chunker.name", name);
      arguments.put("chunker.semver", semver);
      if (handle != null && !handle.isBlank()) {
        arguments.put("chunker.handle", handle);
      }
      arguments.put("chunker.multihash_code", Long.toString(multihashCode));
    }

    public static Builder builder() {
      return new Builder();
    }

    public static ChunkerProfile fromArguments(final Map<String, String> arguments) {
      return builder()
          .setProfileId(Integer.parseInt(require(arguments, "chunker.profile_id")))
          .setNamespace(require(arguments, "chunker.namespace"))
          .setName(require(arguments, "chunker.name"))
          .setSemver(require(arguments, "chunker.semver"))
          .setHandle(arguments.get("chunker.handle"))
          .setMultihashCode(Long.parseLong(require(arguments, "chunker.multihash_code")))
          .build();
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof ChunkerProfile other)) {
        return false;
      }
      return profileId == other.profileId
          && multihashCode == other.multihashCode
          && Objects.equals(namespace, other.namespace)
          && Objects.equals(name, other.name)
          && Objects.equals(semver, other.semver)
          && Objects.equals(handle, other.handle);
    }

    @Override
    public int hashCode() {
      return Objects.hash(profileId, namespace, name, semver, handle, multihashCode);
    }

    public static final class Builder {
      private Integer profileId;
      private String namespace;
      private String name;
      private String semver;
      private String handle;
      private Long multihashCode;

      private Builder() {}

      public Builder setProfileId(final int profileId) {
        this.profileId = profileId;
        return this;
      }

      public Builder setNamespace(final String namespace) {
        this.namespace = Objects.requireNonNull(namespace, "namespace");
        return this;
      }

      public Builder setName(final String name) {
        this.name = Objects.requireNonNull(name, "name");
        return this;
      }

      public Builder setSemver(final String semver) {
        this.semver = Objects.requireNonNull(semver, "semver");
        return this;
      }

      public Builder setHandle(final String handle) {
        this.handle = handle;
        return this;
      }

      public Builder setMultihashCode(final long multihashCode) {
        this.multihashCode = multihashCode;
        return this;
      }

      public ChunkerProfile build() {
        if (profileId == null) {
          throw new IllegalStateException("profileId must be set");
        }
        if (namespace == null) {
          throw new IllegalStateException("namespace must be set");
        }
        if (name == null) {
          throw new IllegalStateException("name must be set");
        }
        if (semver == null) {
          throw new IllegalStateException("semver must be set");
        }
        if (multihashCode == null) {
          throw new IllegalStateException("multihashCode must be set");
        }
        return new ChunkerProfile(this);
      }
    }
  }

  /** Pin policy metadata encoded alongside the manifest. */
  public static final class PinPolicy {
    private final int minReplicas;
    private final String storageClass;
    private final long retentionEpoch;

    private PinPolicy(final Builder builder) {
      this.minReplicas = builder.minReplicas;
      this.storageClass = builder.storageClass;
      this.retentionEpoch = builder.retentionEpoch;
    }

    public int minReplicas() {
      return minReplicas;
    }

    public String storageClass() {
      return storageClass;
    }

    public long retentionEpoch() {
      return retentionEpoch;
    }

    private void appendArguments(final Map<String, String> arguments) {
      arguments.put("policy.min_replicas", Integer.toString(minReplicas));
      arguments.put("policy.storage_class", storageClass);
      arguments.put("policy.retention_epoch", Long.toString(retentionEpoch));
    }

    public static Builder builder() {
      return new Builder();
    }

    public static PinPolicy fromArguments(final Map<String, String> arguments) {
      return builder()
          .setMinReplicas(Integer.parseInt(require(arguments, "policy.min_replicas")))
          .setStorageClass(require(arguments, "policy.storage_class"))
          .setRetentionEpoch(Long.parseLong(require(arguments, "policy.retention_epoch")))
          .build();
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PinPolicy other)) {
        return false;
      }
      return minReplicas == other.minReplicas
          && retentionEpoch == other.retentionEpoch
          && Objects.equals(storageClass, other.storageClass);
    }

    @Override
    public int hashCode() {
      return Objects.hash(minReplicas, storageClass, retentionEpoch);
    }

    public static final class Builder {
      private Integer minReplicas;
      private String storageClass;
      private Long retentionEpoch;

      private Builder() {}

      public Builder setMinReplicas(final int minReplicas) {
        this.minReplicas = minReplicas;
        return this;
      }

      public Builder setStorageClass(final String storageClass) {
        this.storageClass = Objects.requireNonNull(storageClass, "storageClass");
        return this;
      }

      public Builder setRetentionEpoch(final long retentionEpoch) {
        if (retentionEpoch < 0) {
          throw new IllegalArgumentException("retentionEpoch must be non-negative");
        }
        this.retentionEpoch = retentionEpoch;
        return this;
      }

      public PinPolicy build() {
        if (minReplicas == null) {
          throw new IllegalStateException("minReplicas must be set");
        }
        if (storageClass == null) {
          throw new IllegalStateException("storageClass must be set");
        }
        if (retentionEpoch == null) {
          throw new IllegalStateException("retentionEpoch must be set");
        }
        return new PinPolicy(this);
      }
    }
  }

  /** Optional alias binding recorded with the manifest. */
  public static final class AliasBinding {
    private final String name;
    private final String namespace;
    private final String proofHex;

    private AliasBinding(final Builder builder) {
      this.name = builder.name;
      this.namespace = builder.namespace;
      this.proofHex = builder.proofHex;
    }

    public String name() {
      return name;
    }

    public String namespace() {
      return namespace;
    }

    public String proofHex() {
      return proofHex;
    }

    void appendArguments(final Map<String, String> arguments) {
      arguments.put("alias.name", name);
      arguments.put("alias.namespace", namespace);
      arguments.put("alias.proof_hex", proofHex);
    }

    public static AliasBinding fromArguments(
        final Map<String, String> arguments, final boolean required) {
      final boolean hasAliasFields =
          arguments.containsKey("alias.name")
              || arguments.containsKey("alias.namespace")
              || arguments.containsKey("alias.proof_hex");
      if (!hasAliasFields) {
        if (required) {
          throw new IllegalArgumentException("alias binding arguments missing");
        }
        return null;
      }
      return builder()
          .setName(require(arguments, "alias.name"))
          .setNamespace(require(arguments, "alias.namespace"))
          .setProofHex(require(arguments, "alias.proof_hex"))
          .build();
    }

    public static Builder builder() {
      return new Builder();
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof AliasBinding other)) {
        return false;
      }
      return Objects.equals(name, other.name)
          && Objects.equals(namespace, other.namespace)
          && Objects.equals(proofHex, other.proofHex);
    }

    @Override
    public int hashCode() {
      return Objects.hash(name, namespace, proofHex);
    }

    public static final class Builder {
      private String name;
      private String namespace;
      private String proofHex;

      private Builder() {}

      public Builder setName(final String name) {
        this.name = Objects.requireNonNull(name, "name");
        return this;
      }

      public Builder setNamespace(final String namespace) {
        this.namespace = Objects.requireNonNull(namespace, "namespace");
        return this;
      }

      public Builder setProofHex(final String proofHex) {
        this.proofHex = Objects.requireNonNull(proofHex, "proofHex");
        return this;
      }

      public AliasBinding build() {
        if (name == null) {
          throw new IllegalStateException("name must be set");
        }
        if (namespace == null) {
          throw new IllegalStateException("namespace must be set");
        }
        if (proofHex == null) {
          throw new IllegalStateException("proofHex must be set");
        }
        return new AliasBinding(this);
      }
    }
  }
}
