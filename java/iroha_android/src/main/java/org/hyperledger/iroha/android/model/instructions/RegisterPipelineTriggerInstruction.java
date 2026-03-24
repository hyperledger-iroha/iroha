package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code RegisterPipelineTrigger} instructions. */
public final class RegisterPipelineTriggerInstruction implements InstructionTemplate {

  public static final String ACTION = "RegisterPipelineTrigger";

  private final String triggerId;
  private final String authority;
  private final PipelineFilter filter;
  private final Integer repeats;
  private final List<InstructionBox> instructions;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterPipelineTriggerInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterPipelineTriggerInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.triggerId = builder.triggerId;
    this.authority = builder.authority;
    this.filter = builder.filter;
    this.repeats = builder.repeats;
    this.instructions =
        Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(builder.instructions)));
    this.metadata =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(builder.metadata)));
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String triggerId() {
    return triggerId;
  }

  public String authority() {
    return authority;
  }

  public PipelineFilter filter() {
    return filter;
  }

  public Integer repeats() {
    return repeats;
  }

  public List<InstructionBox> instructions() {
    return instructions;
  }

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

  public static RegisterPipelineTriggerInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setTriggerId(require(arguments, "trigger"))
            .setAuthority(require(arguments, "authority"))
            .setFilter(PipelineFilter.fromArguments(arguments))
            .setInstructions(TriggerInstructionUtils.parseInstructions(arguments))
            .setMetadata(TriggerInstructionUtils.extractMetadata(arguments));
    final Integer repeats = TriggerInstructionUtils.parseRepeats(arguments.get("repeats"));
    if (repeats != null) {
      builder.setRepeats(repeats);
    }
    return new RegisterPipelineTriggerInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterPipelineTriggerInstruction other)) {
      return false;
    }
    return Objects.equals(triggerId, other.triggerId)
        && Objects.equals(authority, other.authority)
        && Objects.equals(filter, other.filter)
        && Objects.equals(repeats, other.repeats)
        && instructions.equals(other.instructions)
        && metadata.equals(other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(triggerId, authority, filter, repeats, instructions, metadata);
  }

  public static final class Builder {
    private String triggerId;
    private String authority;
    private PipelineFilter filter;
    private Integer repeats;
    private final List<InstructionBox> instructions = new ArrayList<>();
    private final Map<String, String> metadata = new LinkedHashMap<>();

    private Builder() {}

    public Builder setTriggerId(final String triggerId) {
      if (triggerId == null || triggerId.isBlank()) {
        throw new IllegalArgumentException("triggerId must not be blank");
      }
      this.triggerId = triggerId;
      return this;
    }

    public Builder setAuthority(final String authority) {
      this.authority =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              authority, "authority");
      return this;
    }

    public Builder setFilter(final PipelineFilter filter) {
      this.filter = Objects.requireNonNull(filter, "filter");
      return this;
    }

    public Builder setRepeats(final Integer repeats) {
      if (repeats == null) {
        this.repeats = null;
        return this;
      }
      if (repeats <= 0) {
        throw new IllegalArgumentException("repeats must be greater than zero when provided");
      }
      this.repeats = repeats;
      return this;
    }

    public Builder addInstruction(final InstructionBox instruction) {
      instructions.add(Objects.requireNonNull(instruction, "instruction"));
      return this;
    }

    public Builder setInstructions(final List<InstructionBox> newInstructions) {
      instructions.clear();
      if (newInstructions != null) {
        newInstructions.forEach(this::addInstruction);
      }
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(
          Objects.requireNonNull(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> entries) {
      metadata.clear();
      if (entries != null) {
        entries.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterPipelineTriggerInstruction build() {
      if (triggerId == null) {
        throw new IllegalStateException("triggerId must be set");
      }
      if (authority == null) {
        throw new IllegalStateException("authority must be set");
      }
      if (filter == null) {
        throw new IllegalStateException("filter must be set");
      }
      if (instructions.isEmpty()) {
        throw new IllegalStateException("at least one instruction must be provided");
      }
      return new RegisterPipelineTriggerInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("trigger", triggerId);
      args.put("authority", authority);
      args.put(
          "repeats",
          repeats == null
              ? RegisterTimeTriggerInstruction.REPEATS_INDEFINITE
              : Integer.toUnsignedString(repeats));
      filter.appendArguments(args);
      TriggerInstructionUtils.appendInstructions(instructions, args);
      TriggerInstructionUtils.appendMetadata(metadata, args);
      return args;
    }
  }

  public static final class PipelineFilter {

    private final Kind kind;
    private String transactionHash;
    private Long transactionBlockHeight;
    private String transactionStatus;
    private Long blockHeight;
    private String blockStatus;
    private Long mergeEpochId;
    private String witnessBlockHash;
    private Long witnessHeight;
    private Long witnessView;

    private PipelineFilter(final Kind kind) {
      this.kind = kind;
    }

    public static PipelineFilter transaction() {
      return new PipelineFilter(Kind.TRANSACTION);
    }

    public static PipelineFilter block() {
      return new PipelineFilter(Kind.BLOCK);
    }

    public static PipelineFilter merge() {
      return new PipelineFilter(Kind.MERGE);
    }

    public static PipelineFilter witness() {
      return new PipelineFilter(Kind.WITNESS);
    }

    public Kind kind() {
      return kind;
    }

    public PipelineFilter setTransactionHash(final String hashHex) {
      ensureKind(Kind.TRANSACTION);
      if (hashHex == null || hashHex.isBlank()) {
        throw new IllegalArgumentException("transaction hash must not be blank");
      }
      this.transactionHash = hashHex;
      return this;
    }

    public PipelineFilter setTransactionBlockHeight(final Long blockHeight) {
      ensureKind(Kind.TRANSACTION);
      if (blockHeight != null && blockHeight <= 0) {
        throw new IllegalArgumentException("transaction block height must be positive when provided");
      }
      this.transactionBlockHeight = blockHeight;
      return this;
    }

    public PipelineFilter setTransactionStatus(final String status) {
      ensureKind(Kind.TRANSACTION);
      if (status != null && status.isBlank()) {
        throw new IllegalArgumentException("transaction status must not be blank when provided");
      }
      this.transactionStatus = status;
      return this;
    }

    public PipelineFilter setBlockHeight(final Long height) {
      ensureKind(Kind.BLOCK);
      if (height != null && height <= 0) {
        throw new IllegalArgumentException("block height must be positive when provided");
      }
      this.blockHeight = height;
      return this;
    }

    public PipelineFilter setBlockStatus(final String status) {
      ensureKind(Kind.BLOCK);
      if (status != null && status.isBlank()) {
        throw new IllegalArgumentException("block status must not be blank when provided");
      }
      this.blockStatus = status;
      return this;
    }

    public PipelineFilter setMergeEpochId(final Long epochId) {
      ensureKind(Kind.MERGE);
      if (epochId != null && epochId < 0) {
        throw new IllegalArgumentException("epoch id must be non-negative when provided");
      }
      this.mergeEpochId = epochId;
      return this;
    }

    public PipelineFilter setWitnessBlockHash(final String blockHashHex) {
      ensureKind(Kind.WITNESS);
      if (blockHashHex == null || blockHashHex.isBlank()) {
        throw new IllegalArgumentException("witness block hash must not be blank");
      }
      this.witnessBlockHash = blockHashHex;
      return this;
    }

    public PipelineFilter setWitnessHeight(final Long height) {
      ensureKind(Kind.WITNESS);
      if (height != null && height <= 0) {
        throw new IllegalArgumentException("witness height must be positive when provided");
      }
      this.witnessHeight = height;
      return this;
    }

    public PipelineFilter setWitnessView(final Long view) {
      ensureKind(Kind.WITNESS);
      if (view != null && view < 0) {
        throw new IllegalArgumentException("witness view must be non-negative when provided");
      }
      this.witnessView = view;
      return this;
    }

    private void ensureKind(final Kind expected) {
      if (kind != expected) {
        throw new IllegalStateException("Filter variant mismatch: expected " + expected + " but was " + kind);
      }
    }

    private void appendArguments(final Map<String, String> target) {
      target.put("filter.kind", kind.displayName);
      switch (kind) {
        case TRANSACTION -> {
          if (transactionHash != null) {
            target.put("filter.transaction.hash", transactionHash);
          }
          if (transactionBlockHeight != null) {
            target.put("filter.transaction.block_height", Long.toUnsignedString(transactionBlockHeight));
          }
          if (transactionStatus != null) {
            target.put("filter.transaction.status", transactionStatus);
          }
        }
        case BLOCK -> {
          if (blockHeight != null) {
            target.put("filter.block.height", Long.toUnsignedString(blockHeight));
          }
          if (blockStatus != null) {
            target.put("filter.block.status", blockStatus);
          }
        }
        case MERGE -> {
          if (mergeEpochId != null) {
            target.put("filter.merge.epoch_id", Long.toUnsignedString(mergeEpochId));
          }
        }
        case WITNESS -> {
          if (witnessBlockHash != null) {
            target.put("filter.witness.block_hash", witnessBlockHash);
          }
          if (witnessHeight != null) {
            target.put("filter.witness.height", Long.toUnsignedString(witnessHeight));
          }
          if (witnessView != null) {
            target.put("filter.witness.view", Long.toUnsignedString(witnessView));
          }
        }
      }
    }

    private static PipelineFilter fromArguments(final Map<String, String> arguments) {
      final String kindValue = arguments.get("filter.kind");
      if (kindValue == null || kindValue.isBlank()) {
        throw new IllegalArgumentException("filter.kind is required for RegisterPipelineTrigger");
      }
      final Kind kind = Kind.fromDisplayName(kindValue);
      final PipelineFilter filter = new PipelineFilter(kind);
      switch (kind) {
        case TRANSACTION -> {
          final String hash = arguments.get("filter.transaction.hash");
          if (hash != null) {
            filter.setTransactionHash(hash);
          }
          final String blockHeight = arguments.get("filter.transaction.block_height");
          if (blockHeight != null) {
            filter.setTransactionBlockHeight(parseUnsignedLong(blockHeight, "filter.transaction.block_height"));
          }
          final String status = arguments.get("filter.transaction.status");
          if (status != null) {
            filter.setTransactionStatus(status);
          }
        }
        case BLOCK -> {
          final String height = arguments.get("filter.block.height");
          if (height != null) {
            filter.setBlockHeight(parseUnsignedLong(height, "filter.block.height"));
          }
          final String status = arguments.get("filter.block.status");
          if (status != null) {
            filter.setBlockStatus(status);
          }
        }
        case MERGE -> {
          final String epoch = arguments.get("filter.merge.epoch_id");
          if (epoch != null) {
            filter.setMergeEpochId(parseUnsignedLong(epoch, "filter.merge.epoch_id"));
          }
        }
        case WITNESS -> {
          final String blockHash = arguments.get("filter.witness.block_hash");
          if (blockHash != null) {
            filter.setWitnessBlockHash(blockHash);
          }
          final String height = arguments.get("filter.witness.height");
          if (height != null) {
            filter.setWitnessHeight(parseUnsignedLong(height, "filter.witness.height"));
          }
          final String view = arguments.get("filter.witness.view");
          if (view != null) {
            filter.setWitnessView(parseUnsignedLong(view, "filter.witness.view"));
          }
        }
      }
      return filter;
    }

    private static long parseUnsignedLong(final String value, final String field) {
      try {
        final long parsed = Long.parseUnsignedLong(value);
        if (parsed < 0) {
          throw new NumberFormatException("negative");
        }
        return parsed;
      } catch (final NumberFormatException ex) {
        throw new IllegalArgumentException(field + " must be an unsigned integer", ex);
      }
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PipelineFilter other)) {
        return false;
      }
      return kind == other.kind
          && Objects.equals(transactionHash, other.transactionHash)
          && Objects.equals(transactionBlockHeight, other.transactionBlockHeight)
          && Objects.equals(transactionStatus, other.transactionStatus)
          && Objects.equals(blockHeight, other.blockHeight)
          && Objects.equals(blockStatus, other.blockStatus)
          && Objects.equals(mergeEpochId, other.mergeEpochId)
          && Objects.equals(witnessBlockHash, other.witnessBlockHash)
          && Objects.equals(witnessHeight, other.witnessHeight)
          && Objects.equals(witnessView, other.witnessView);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          kind,
          transactionHash,
          transactionBlockHeight,
          transactionStatus,
          blockHeight,
          blockStatus,
          mergeEpochId,
          witnessBlockHash,
          witnessHeight,
          witnessView);
    }

    private enum Kind {
      TRANSACTION("Transaction"),
      BLOCK("Block"),
      MERGE("Merge"),
      WITNESS("Witness");

      private final String displayName;

      Kind(final String displayName) {
        this.displayName = displayName;
      }

      static Kind fromDisplayName(final String value) {
        for (final Kind kind : values()) {
          if (kind.displayName.equalsIgnoreCase(value)) {
            return kind;
          }
        }
        throw new IllegalArgumentException("Unknown pipeline filter kind: " + value);
      }
    }
  }
}
