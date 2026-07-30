package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Typed builder for the six-field {@code CompleteReplicationOrder} hard cut. */
public final class CompleteReplicationOrderInstruction implements InstructionTemplate {

  private static final String ACTION = "CompleteReplicationOrder";
  private static final Pattern AUTHORITY_PATTERN =
      Pattern.compile(
          "^\\{\"provider_owner\":\"([^\"\\\\]+)\",\"signer_policy\":"
              + "\\{\"policy_id\":\"([0-9a-f]{64})\",\"revision\":([1-9][0-9]*),"
              + "\"predecessor_digest\":(null|\"([0-9a-f]{64})\"),"
              + "\"policy_digest\":\"([0-9a-f]{64})\"\\}\\}$");
  private static final Pattern ANCHOR_PATTERN =
      Pattern.compile(
          "^\\{\"height\":([1-9][0-9]*),\"block_hash\":\"([0-9a-f]{64})\"\\}$");

  private final String orderId;
  private final String providerId;
  private final long completionEpoch;
  private final ProviderIngestCompletionAuthorityV1 expectedAuthority;
  private final long expectedAssignmentRevision;
  private final ProviderIngestFinalizedAnchorV1 finalizedAnchor;
  private final Map<String, String> arguments;

  private CompleteReplicationOrderInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CompleteReplicationOrderInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.orderId = builder.orderId;
    this.providerId = builder.providerId;
    this.completionEpoch = builder.completionEpoch;
    this.expectedAuthority = builder.expectedAuthority;
    this.expectedAssignmentRevision = builder.expectedAssignmentRevision;
    this.finalizedAnchor = builder.finalizedAnchor;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String orderId() {
    return orderId;
  }

  public String providerId() {
    return providerId;
  }

  public long completionEpoch() {
    return completionEpoch;
  }

  public ProviderIngestCompletionAuthorityV1 expectedAuthority() {
    return expectedAuthority;
  }

  public long expectedAssignmentRevision() {
    return expectedAssignmentRevision;
  }

  public ProviderIngestFinalizedAnchorV1 finalizedAnchor() {
    return finalizedAnchor;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static CompleteReplicationOrderInstruction fromArguments(
      final Map<String, String> arguments) {
    ReplicationOrderInstructionValidation.requireArguments(
        arguments,
        ACTION,
        "order_id",
        "provider_id",
        "completion_epoch",
        "expected_authority",
        "expected_assignment_revision",
        "finalized_anchor");
    final Builder builder =
        builder()
            .setOrderId(require(arguments, "order_id"))
            .setProviderId(require(arguments, "provider_id"))
            .setCompletionEpoch(requireLong(arguments, "completion_epoch"))
            .setExpectedAuthority(parseAuthority(require(arguments, "expected_authority")))
            .setExpectedAssignmentRevision(
                requireLong(arguments, "expected_assignment_revision"))
            .setFinalizedAnchor(parseAnchor(require(arguments, "finalized_anchor")));
    return new CompleteReplicationOrderInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static ProviderIngestCompletionAuthorityV1 parseAuthority(final String value) {
    final Matcher match = AUTHORITY_PATTERN.matcher(value);
    if (!match.matches()) {
      throw new IllegalArgumentException(
          "Instruction argument 'expected_authority' must use canonical JSON");
    }
    final ProviderIngestCompletionSignerPolicyV1 policy =
        new ProviderIngestCompletionSignerPolicyV1(
            match.group(2),
            requireLongLiteral(match.group(3), "signer policy revision"),
            match.group(5),
            match.group(6));
    final ProviderIngestCompletionAuthorityV1 authority =
        new ProviderIngestCompletionAuthorityV1(match.group(1), policy);
    if (!authority.canonicalJson().equals(value)) {
      throw new IllegalArgumentException(
          "Instruction argument 'expected_authority' must use canonical JSON");
    }
    return authority;
  }

  private static ProviderIngestFinalizedAnchorV1 parseAnchor(final String value) {
    final Matcher match = ANCHOR_PATTERN.matcher(value);
    if (!match.matches()) {
      throw new IllegalArgumentException(
          "Instruction argument 'finalized_anchor' must use canonical JSON");
    }
    final ProviderIngestFinalizedAnchorV1 anchor =
        new ProviderIngestFinalizedAnchorV1(
            requireLongLiteral(match.group(1), "finalized anchor height"), match.group(2));
    if (!anchor.canonicalJson().equals(value)) {
      throw new IllegalArgumentException(
          "Instruction argument 'finalized_anchor' must use canonical JSON");
    }
    return anchor;
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    return requireLongLiteral(require(arguments, key), "Instruction argument '" + key + "'");
  }

  private static long requireLongLiteral(final String value, final String context) {
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(context + " must be a number: " + value, ex);
    }
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CompleteReplicationOrderInstruction)) {
      return false;
    }
    final CompleteReplicationOrderInstruction other =
        (CompleteReplicationOrderInstruction) obj;
    return completionEpoch == other.completionEpoch
        && expectedAssignmentRevision == other.expectedAssignmentRevision
        && Objects.equals(orderId, other.orderId)
        && Objects.equals(providerId, other.providerId)
        && Objects.equals(expectedAuthority, other.expectedAuthority)
        && Objects.equals(finalizedAnchor, other.finalizedAnchor);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        orderId,
        providerId,
        completionEpoch,
        expectedAuthority,
        expectedAssignmentRevision,
        finalizedAnchor);
  }

  /** Builder requiring every field of the six-field hard cut. */
  public static final class Builder {
    private String orderId;
    private String providerId;
    private Long completionEpoch;
    private ProviderIngestCompletionAuthorityV1 expectedAuthority;
    private Long expectedAssignmentRevision;
    private ProviderIngestFinalizedAnchorV1 finalizedAnchor;

    private Builder() {}

    public Builder setOrderId(final String orderId) {
      this.orderId = ReplicationOrderInstructionValidation.requireOrderId(orderId);
      return this;
    }

    public Builder setProviderId(final String providerId) {
      this.providerId = ReplicationOrderInstructionValidation.requireProviderId(providerId);
      return this;
    }

    public Builder setCompletionEpoch(final long completionEpoch) {
      this.completionEpoch =
          ReplicationOrderInstructionValidation.requireEpoch(completionEpoch, "completionEpoch");
      return this;
    }

    public Builder setExpectedAuthority(
        final ProviderIngestCompletionAuthorityV1 expectedAuthority) {
      this.expectedAuthority = Objects.requireNonNull(expectedAuthority, "expectedAuthority");
      return this;
    }

    public Builder setExpectedAssignmentRevision(final long expectedAssignmentRevision) {
      this.expectedAssignmentRevision =
          ReplicationOrderInstructionValidation.requirePositiveRevision(
              expectedAssignmentRevision, "expectedAssignmentRevision");
      return this;
    }

    public Builder setFinalizedAnchor(final ProviderIngestFinalizedAnchorV1 finalizedAnchor) {
      this.finalizedAnchor = Objects.requireNonNull(finalizedAnchor, "finalizedAnchor");
      return this;
    }

    public CompleteReplicationOrderInstruction build() {
      if (orderId == null) {
        throw new IllegalStateException("orderId must be provided");
      }
      if (providerId == null) {
        throw new IllegalStateException("providerId must be provided");
      }
      if (completionEpoch == null) {
        throw new IllegalStateException("completionEpoch must be provided");
      }
      if (expectedAuthority == null) {
        throw new IllegalStateException("expectedAuthority must be provided");
      }
      if (expectedAssignmentRevision == null) {
        throw new IllegalStateException("expectedAssignmentRevision must be provided");
      }
      if (finalizedAnchor == null) {
        throw new IllegalStateException("finalizedAnchor must be provided");
      }
      return new CompleteReplicationOrderInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("order_id", orderId);
      args.put("provider_id", providerId);
      args.put("completion_epoch", Long.toString(completionEpoch));
      args.put("expected_authority", expectedAuthority.canonicalJson());
      args.put("expected_assignment_revision", Long.toString(expectedAssignmentRevision));
      args.put("finalized_anchor", finalizedAnchor.canonicalJson());
      return args;
    }
  }
}
