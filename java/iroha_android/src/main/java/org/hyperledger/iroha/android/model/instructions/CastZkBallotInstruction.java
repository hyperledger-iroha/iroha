package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/**
 * Typed builder for {@code CastZkBallot} instructions.
 *
 * <p>Public inputs must be provided as a JSON object. The builder normalizes common alias keys
 * (camelCase to snake_case) and emits canonical JSON ordering so ballot fingerprints remain stable.
 * When any lock hint is supplied, all of {@code owner}, {@code amount}, and {@code duration_blocks}
 * are required.
 */
public final class CastZkBallotInstruction implements InstructionTemplate {

  private static final String ACTION = "CastZkBallot";

  private final String electionId;
  private final String proofBase64;
  private final String publicInputsJson;
  private final Map<String, String> arguments;

  private CastZkBallotInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CastZkBallotInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.electionId = builder.electionId;
    this.proofBase64 = builder.proofBase64;
    this.publicInputsJson = builder.publicInputsJson;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String electionId() {
    return electionId;
  }

  public String proofBase64() {
    return proofBase64;
  }

  public String publicInputsJson() {
    return publicInputsJson;
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

  public static CastZkBallotInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setElectionId(require(arguments, "election_id"))
            .setProofBase64(require(arguments, "proof_b64"))
            .setPublicInputsJson(require(arguments, "public_inputs_json"));
    return new CastZkBallotInstruction(builder, builder.canonicalArguments());
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CastZkBallotInstruction other)) {
      return false;
    }
    return Objects.equals(electionId, other.electionId)
        && Objects.equals(proofBase64, other.proofBase64)
        && Objects.equals(publicInputsJson, other.publicInputsJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(electionId, proofBase64, publicInputsJson);
  }

  public static final class Builder {
    private String electionId;
    private String proofBase64;
    private String publicInputsJson;

    private Builder() {}

    public Builder setElectionId(final String electionId) {
      if (electionId == null || electionId.isBlank()) {
        throw new IllegalArgumentException("electionId must not be blank");
      }
      this.electionId = electionId;
      return this;
    }

    public Builder setProofBase64(final String proofBase64) {
      if (proofBase64 == null || proofBase64.isBlank()) {
        throw new IllegalArgumentException("proofBase64 must not be blank");
      }
      // Validate base64 eagerly so builders catch mistakes
      try {
        Base64.getDecoder().decode(proofBase64);
      } catch (final IllegalArgumentException ex) {
        throw new IllegalArgumentException("proofBase64 must be valid base64", ex);
      }
      this.proofBase64 = proofBase64;
      return this;
    }

    public Builder setPublicInputsJson(final String publicInputsJson) {
      if (publicInputsJson == null || publicInputsJson.isBlank()) {
        throw new IllegalArgumentException("publicInputsJson must not be blank");
      }
      this.publicInputsJson = normalizePublicInputsJson(publicInputsJson);
      return this;
    }

    public CastZkBallotInstruction build() {
      if (electionId == null) {
        throw new IllegalStateException("electionId must be provided");
      }
      if (proofBase64 == null) {
        throw new IllegalStateException("proofBase64 must be provided");
      }
      if (publicInputsJson == null) {
        throw new IllegalStateException("publicInputsJson must be provided");
      }
      return new CastZkBallotInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("election_id", electionId);
      args.put("proof_b64", proofBase64);
      args.put("public_inputs_json", publicInputsJson);
      return args;
    }
  }

  private static String normalizePublicInputsJson(final String publicInputsJson) {
    final String trimmed = publicInputsJson.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException("publicInputsJson must not be blank");
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(trimmed);
    } catch (final IllegalStateException ex) {
      throw new IllegalArgumentException("publicInputsJson must be valid JSON", ex);
    }
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("publicInputsJson must be a JSON object");
    }
    @SuppressWarnings("unchecked")
    final Map<String, Object> normalized = new LinkedHashMap<>((Map<String, Object>) parsed);
    normalizePublicInputsAlias(normalized, "durationBlocks", "duration_blocks");
    normalizePublicInputsAlias(normalized, "nullifierHex", "nullifier_hex");
    normalizePublicInputsAlias(normalized, "rootHintHex", "root_hint");
    normalizePublicInputsAlias(normalized, "rootHint", "root_hint");
    ensureLockHintsComplete(normalized);
    return JsonEncoder.encode(normalized);
  }

  private static void normalizePublicInputsAlias(
      final Map<String, Object> target, final String aliasKey, final String canonicalKey) {
    if (!target.containsKey(aliasKey)) {
      return;
    }
    if (target.containsKey(canonicalKey)) {
      throw new IllegalArgumentException(
          "publicInputsJson cannot include both " + aliasKey + " and " + canonicalKey);
    }
    target.put(canonicalKey, target.remove(aliasKey));
  }

  private static void ensureLockHintsComplete(final Map<String, Object> publicInputs) {
    final boolean hasOwner = publicInputs.get("owner") != null;
    final boolean hasAmount = publicInputs.get("amount") != null;
    final boolean hasDuration = publicInputs.get("duration_blocks") != null;
    if ((hasOwner || hasAmount || hasDuration) && !(hasOwner && hasAmount && hasDuration)) {
      throw new IllegalArgumentException(
          "publicInputsJson must include owner, amount, and duration_blocks when providing lock hints");
    }
  }
}
