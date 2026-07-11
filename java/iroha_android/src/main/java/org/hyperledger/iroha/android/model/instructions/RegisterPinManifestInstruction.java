package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.regex.Pattern;

/** Typed first-release builder for the consensus {@code RegisterPinManifest} instruction. */
public final class RegisterPinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "RegisterPinManifest";
  private static final int MAX_MANIFEST_PAYLOAD_BYTES = 512 * 1024;
  private static final int MAX_ALIAS_PROOF_BYTES = 1024 * 1024;
  private static final Pattern CANONICAL_HEX = Pattern.compile("^[0-9a-f]+$");
  private static final Set<String> MANDATORY_ARGUMENTS =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList(
                  "action",
                  "manifest_payload_base64",
                  "submitted_epoch")));
  private static final Set<String> OPTIONAL_ARGUMENTS =
      Collections.unmodifiableSet(
          new HashSet<>(
              Arrays.asList(
                  "successor_of_hex", "alias.name", "alias.namespace", "alias.proof_hex")));

  private final String manifestPayloadBase64;
  private final long submittedEpoch;
  private final String successorOfHex;
  private final AliasBinding aliasBinding;
  private final Map<String, String> arguments;

  private RegisterPinManifestInstruction(final Builder builder) {
    manifestPayloadBase64 = builder.manifestPayloadBase64;
    submittedEpoch = builder.submittedEpoch;
    successorOfHex = builder.successorOfHex;
    aliasBinding = builder.aliasBinding;
    arguments = Collections.unmodifiableMap(builder.canonicalArguments());
  }

  public String manifestPayloadBase64() {
    return manifestPayloadBase64;
  }

  /** Returns a fresh copy of the canonical Norito manifest payload. */
  public byte[] manifestPayloadBytes() {
    return Base64.getDecoder().decode(manifestPayloadBase64);
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
    Objects.requireNonNull(arguments, "arguments");
    if (!ACTION.equals(arguments.get("action"))) {
      throw new IllegalArgumentException("Instruction argument 'action' must be " + ACTION);
    }
    for (final String key : arguments.keySet()) {
      if (!MANDATORY_ARGUMENTS.contains(key) && !OPTIONAL_ARGUMENTS.contains(key)) {
        throw new IllegalArgumentException("Unsupported RegisterPinManifest argument: " + key);
      }
    }
    if (!arguments.keySet().containsAll(MANDATORY_ARGUMENTS)) {
      throw new IllegalArgumentException("RegisterPinManifest arguments are missing required fields");
    }
    return builder()
        .setManifestPayloadBase64(require(arguments, "manifest_payload_base64"))
        .setSubmittedEpoch(requireLong(arguments, "submitted_epoch"))
        .setSuccessorOfHex(arguments.get("successor_of_hex"))
        .setAliasBinding(AliasBinding.fromArguments(arguments))
        .build();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterPinManifestInstruction)) {
      return false;
    }
    final RegisterPinManifestInstruction other = (RegisterPinManifestInstruction) obj;
    return submittedEpoch == other.submittedEpoch
        && Objects.equals(manifestPayloadBase64, other.manifestPayloadBase64)
        && Objects.equals(successorOfHex, other.successorOfHex)
        && Objects.equals(aliasBinding, other.aliasBinding);
  }

  @Override
  public int hashCode() {
    return Objects.hash(manifestPayloadBase64, submittedEpoch, successorOfHex, aliasBinding);
  }

  public static final class Builder {
    private String manifestPayloadBase64;
    private Long submittedEpoch;
    private String successorOfHex;
    private AliasBinding aliasBinding;

    private Builder() {}

    public Builder setManifestPayloadBase64(final String manifestPayloadBase64) {
      this.manifestPayloadBase64 = requireCanonicalManifestPayload(manifestPayloadBase64);
      return this;
    }

    public Builder setManifestPayload(final byte[] manifestPayload) {
      Objects.requireNonNull(manifestPayload, "manifestPayload");
      if (manifestPayload.length == 0 || manifestPayload.length > MAX_MANIFEST_PAYLOAD_BYTES) {
        throw new IllegalArgumentException(
            "manifestPayload must contain 1.." + MAX_MANIFEST_PAYLOAD_BYTES + " bytes");
      }
      this.manifestPayloadBase64 =
          Base64.getEncoder().encodeToString(Arrays.copyOf(manifestPayload, manifestPayload.length));
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
      this.successorOfHex =
          successorOfHex == null
              ? null
              : requireNonzeroDigest(successorOfHex, "successorOfHex");
      return this;
    }

    public Builder setAliasBinding(final AliasBinding aliasBinding) {
      this.aliasBinding = aliasBinding;
      return this;
    }

    public RegisterPinManifestInstruction build() {
      if (manifestPayloadBase64 == null) {
        throw new IllegalStateException("manifestPayload must be set");
      }
      if (submittedEpoch == null) {
        throw new IllegalStateException("submittedEpoch must be set");
      }
      return new RegisterPinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> result = new LinkedHashMap<>();
      result.put("action", ACTION);
      result.put("manifest_payload_base64", manifestPayloadBase64);
      result.put("submitted_epoch", Long.toString(submittedEpoch));
      if (successorOfHex != null) {
        result.put("successor_of_hex", successorOfHex);
      }
      if (aliasBinding != null) {
        aliasBinding.appendArguments(result);
      }
      return result;
    }
  }

  /** Optional manifest alias binding. */
  public static final class AliasBinding {
    private final String name;
    private final String namespace;
    private final String proofHex;

    private AliasBinding(final Builder builder) {
      name = builder.name;
      namespace = builder.namespace;
      proofHex = builder.proofHex;
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

    private void appendArguments(final Map<String, String> target) {
      target.put("alias.name", name);
      target.put("alias.namespace", namespace);
      target.put("alias.proof_hex", proofHex);
    }

    static AliasBinding fromArguments(final Map<String, String> arguments) {
      return fromArguments(arguments, false);
    }

    static AliasBinding fromArguments(
        final Map<String, String> arguments, final boolean required) {
      int present = 0;
      if (arguments.containsKey("alias.name")) {
        present++;
      }
      if (arguments.containsKey("alias.namespace")) {
        present++;
      }
      if (arguments.containsKey("alias.proof_hex")) {
        present++;
      }
      if (present == 0) {
        if (required) {
          throw new IllegalArgumentException(
              "Alias binding requires alias.name, alias.namespace, and alias.proof_hex");
        }
        return null;
      }
      if (present != 3) {
        throw new IllegalArgumentException(
            "Alias binding requires alias.name, alias.namespace, and alias.proof_hex together");
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
      if (!(obj instanceof AliasBinding)) {
        return false;
      }
      final AliasBinding other = (AliasBinding) obj;
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
        this.name = requireAliasText(name, "alias.name");
        return this;
      }

      public Builder setNamespace(final String namespace) {
        this.namespace = requireAliasText(namespace, "alias.namespace");
        return this;
      }

      public Builder setProofHex(final String proofHex) {
        this.proofHex =
            requireCanonicalHex(proofHex, "alias.proofHex", null, MAX_ALIAS_PROOF_BYTES);
        return this;
      }

      public AliasBinding build() {
        if (name == null || namespace == null || proofHex == null) {
          throw new IllegalStateException("alias name, namespace, and proof must be set");
        }
        return new AliasBinding(this);
      }
    }
  }

  private static String requireCanonicalManifestPayload(final String value) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException("manifestPayloadBase64 must not be empty");
    }
    final int maximumEncodedLength = ((MAX_MANIFEST_PAYLOAD_BYTES + 2) / 3) * 4;
    if (value.length() > maximumEncodedLength) {
      throw new IllegalArgumentException("manifestPayloadBase64 exceeds the manifest size limit");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("manifestPayloadBase64 must be valid base64", ex);
    }
    if (decoded.length == 0 || decoded.length > MAX_MANIFEST_PAYLOAD_BYTES) {
      throw new IllegalArgumentException("manifestPayloadBase64 decoded size is outside limits");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException("manifestPayloadBase64 must use canonical padded base64");
    }
    return value;
  }

  private static String requireNonzeroDigest(final String value, final String field) {
    final String digest = requireCanonicalHex(value, field, 32, null);
    boolean nonzero = false;
    for (int index = 0; index < digest.length(); index++) {
      if (digest.charAt(index) != '0') {
        nonzero = true;
        break;
      }
    }
    if (!nonzero) {
      throw new IllegalArgumentException(field + " must not be the all-zero digest");
    }
    return digest;
  }

  private static String requireCanonicalHex(
      final String value,
      final String field,
      final Integer expectedBytes,
      final Integer maximumBytes) {
    if (value == null
        || value.isEmpty()
        || value.length() % 2 != 0
        || !CANONICAL_HEX.matcher(value).matches()) {
      throw new IllegalArgumentException(
          field + " must be canonical lowercase even-length hex without a prefix");
    }
    if (expectedBytes != null && value.length() != expectedBytes * 2) {
      throw new IllegalArgumentException(field + " must encode exactly " + expectedBytes + " bytes");
    }
    if (maximumBytes != null && value.length() > maximumBytes * 2) {
      throw new IllegalArgumentException(field + " exceeds the encoded byte limit");
    }
    return value;
  }

  private static String requireAliasText(final String value, final String field) {
    if (value == null
        || value.isEmpty()
        || !value.equals(value.trim())
        || value.getBytes(StandardCharsets.UTF_8).length > 128) {
      throw new IllegalArgumentException(field + " must be unpadded UTF-8 of at most 128 bytes");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(field + " must not contain control characters");
      }
    }
    return value;
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isEmpty()) {
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
}
