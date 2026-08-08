package org.hyperledger.iroha.android.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.instructions.ProofAttachment;

/**
 * Representation of a transaction payload prior to Norito encoding.
 *
 * <p>The structure mirrors the Rust data model for encoding and signing native instructions,
 * deployed-contract calls, flat mixed batches, and IVM bytecode. The optional nonce uses a
 * {@link Long} carrier so the full nonzero unsigned 32-bit wire range remains representable.
 * Proof attachments are part of the signed payload and therefore affect both authorization
 * signatures and the canonical transaction identifier.
 */
public final class TransactionPayload {

  private final String chainId;
  private final String authority;
  private final long creationTimeMs;
  private final Executable executable;
  private final Optional<Long> timeToLiveMs;
  private final Optional<Long> nonce;
  private final FeePaymentIntent feePayment;
  private final Map<String, JsonValue> metadata;
  private final Optional<List<ProofAttachment>> attachments;

  private TransactionPayload(final Builder builder) {
    this.chainId = builder.chainId;
    this.authority = builder.authority;
    this.creationTimeMs = builder.creationTimeMs;
    this.executable = builder.executable;
    this.timeToLiveMs = builder.timeToLiveMs;
    this.nonce = builder.nonce;
    this.feePayment = Objects.requireNonNull(builder.feePayment, "feePayment");
    this.metadata = Collections.unmodifiableMap(new LinkedHashMap<>(builder.metadata));
    this.attachments =
        builder.attachments.map(
            value -> Collections.unmodifiableList(new ArrayList<>(value)));
  }

  public String chainId() {
    return chainId;
  }

  public String authority() {
    return authority;
  }

  public long creationTimeMs() {
    return creationTimeMs;
  }

  public Executable executable() {
    return executable;
  }

  public Optional<Long> timeToLiveMs() {
    return timeToLiveMs;
  }

  public Optional<Long> nonce() {
    return nonce;
  }

  public FeePaymentIntent feePayment() {
    return feePayment;
  }

  public Map<String, JsonValue> metadata() {
    return metadata;
  }

  /** Returns ordered execution proof attachments included in the signed transaction intent. */
  public Optional<List<ProofAttachment>> attachments() {
    return attachments;
  }

  public Builder toBuilder() {
    return builder()
        .setChainId(chainId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(executable)
        .setTimeToLiveMs(timeToLiveMs.orElse(null))
        .setNonce(nonce.orElse(null))
        .setFeePayment(feePayment)
        .setMetadata(metadata)
        .setAttachments(attachments.orElse(null));
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private static final long MAX_U32 = 0xffff_ffffL;
    private static final long DEFAULT_TRANSACTION_TTL_MS = 100_000L;
    private static final int MAX_CHAIN_ID_BYTES = 128;

    private String chainId;
    private String authority;
    private long creationTimeMs = System.currentTimeMillis();
    private Executable executable = Executable.ivm(new byte[0]);
    private Optional<Long> timeToLiveMs = Optional.of(DEFAULT_TRANSACTION_TTL_MS);
    private Optional<Long> nonce = Optional.empty();
    private FeePaymentIntent feePayment;
    private final Map<String, JsonValue> metadata = new LinkedHashMap<>();
    private Optional<List<ProofAttachment>> attachments = Optional.empty();

    public Builder setChainId(final String chainId) {
      this.chainId = requireCanonicalChainId(chainId);
      return this;
    }

    public Builder setAuthority(final String authority) {
      this.authority = AccountIdLiteral.requireCanonicalI105Address(authority, "authority");
      return this;
    }

    public Builder setCreationTimeMs(final long creationTimeMs) {
      if (creationTimeMs < 0) {
        throw new IllegalArgumentException("creationTimeMs must be non-negative");
      }
      this.creationTimeMs = creationTimeMs;
      return this;
    }

    public Builder setExecutable(final Executable executable) {
      this.executable = Objects.requireNonNull(executable, "executable");
      return this;
    }

    public Builder setInstructionBytes(final byte[] instructionBytes) {
      return setExecutable(Executable.ivm(instructionBytes));
    }

    public Builder setInstructions(final List<InstructionBox> instructions) {
      return setExecutable(Executable.instructions(instructions));
    }

    public Builder setContractCall(final ContractInvocation invocation) {
      return setExecutable(Executable.contractCall(invocation));
    }

    public Builder setBatch(final List<? extends ExecutableBatchItem> items) {
      return setExecutable(Executable.batch(items));
    }

    public Builder setTimeToLiveMs(final Long ttlMs) {
      if (ttlMs == null) {
        throw new IllegalArgumentException(
            "timeToLiveMs must be a positive signature-bound lifetime");
      } else if (ttlMs <= 0) {
        throw new IllegalArgumentException("timeToLiveMs must be positive");
      } else {
        this.timeToLiveMs = Optional.of(ttlMs);
      }
      return this;
    }

    /** Sets an optional transaction nonce in the exact nonzero unsigned 32-bit wire range. */
    public Builder setNonce(final Long nonce) {
      if (nonce == null) {
        this.nonce = Optional.empty();
      } else if (nonce <= 0 || nonce > MAX_U32) {
        throw new IllegalArgumentException("nonce must fit in the nonzero u32 range");
      } else {
        this.nonce = Optional.of(nonce);
      }
      return this;
    }

    /** Convenience overload for callers whose nonce already fits in a signed {@code int}. */
    public Builder setNonce(final int nonce) {
      return setNonce((long) nonce);
    }

    public Builder setFeePayment(final FeePaymentIntent feePayment) {
      this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      return putMetadata(key, JsonValue.string(Objects.requireNonNull(value, "metadata value")));
    }

    public Builder putMetadata(final String key, final JsonValue value) {
      metadata.put(normalize(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, ?> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach((key, value) -> putMetadata(key, metadataValue(value)));
      }
      return this;
    }

    /**
     * Sets the ordered proof attachments that form part of the signed transaction intent.
     *
     * <p>Passing {@code null} encodes Rust {@code Option::None}; an empty list encodes
     * {@code Some(ProofAttachmentList([]))}.
     */
    public Builder setAttachments(final List<ProofAttachment> attachments) {
      if (attachments == null) {
        this.attachments = Optional.empty();
      } else {
        final List<ProofAttachment> snapshot = new ArrayList<>(attachments.size());
        for (final ProofAttachment attachment : attachments) {
          snapshot.add(Objects.requireNonNull(attachment, "attachments must not contain null"));
        }
        this.attachments = Optional.of(Collections.unmodifiableList(snapshot));
      }
      return this;
    }

    private static JsonValue metadataValue(final Object value) {
      if (value instanceof JsonValue) {
        return (JsonValue) value;
      }
      if (value instanceof String) {
        return JsonValue.string((String) value);
      }
      if (value instanceof Boolean) {
        return JsonValue.bool((Boolean) value);
      }
      if (value instanceof Number) {
        if (value instanceof Double && !Double.isFinite((Double) value)) {
          throw new IllegalArgumentException("metadata number must be finite");
        }
        if (value instanceof Float && !Float.isFinite((Float) value)) {
          throw new IllegalArgumentException("metadata number must be finite");
        }
        return JsonValue.parse(value.toString());
      }
      throw new IllegalArgumentException("Unsupported metadata value type: " + value);
    }

    public TransactionPayload build() {
      return build(true);
    }

    /**
     * Decoder-only construction hook.
     *
     * <p>This permits the codec to materialize a wire payload before the encoder performs its
     * admissibility check. Required payload fields remain mandatory, and encoding/signing still
     * rejects gasless IVM or contract-call executables.
     */
    public TransactionPayload buildDecodedForCodec() {
      return build(false);
    }

    private TransactionPayload build(final boolean validateExecutableGas) {
      if (chainId == null) {
        throw new IllegalStateException("chainId must be set explicitly");
      }
      if (authority == null) {
        throw new IllegalStateException("authority must be set explicitly");
      }
      if (feePayment == null) {
        throw new IllegalStateException("feePayment must be set explicitly");
      }
      if (validateExecutableGas
          && executable.requiresTransactionGasLimit()
          && feePayment.gasLimit() == null) {
        throw new IllegalStateException(
            "feePayment.gasLimit is required for IVM and contract-call executables");
      }
      return new TransactionPayload(this);
    }

    private static String normalize(final String value, final String field) {
      if (value == null || value.trim().isEmpty()) {
        throw new IllegalArgumentException(field + " must not be blank");
      }
      return value;
    }

    private static String normalizeExact(final String value, final String field) {
      final String normalized = normalize(value, field);
      if (!normalized.trim().equals(normalized)) {
        throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
      }
      return normalized;
    }

    private static String requireCanonicalChainId(final String value) {
      if (value == null || value.isEmpty() || value.length() > MAX_CHAIN_ID_BYTES) {
        throw new IllegalArgumentException(
            "chainId must contain 1.." + MAX_CHAIN_ID_BYTES + " ASCII bytes");
      }
      if (!isAsciiLetterOrDigit(value.charAt(0))
          || !isAsciiLetterOrDigit(value.charAt(value.length() - 1))) {
        throw new IllegalArgumentException(
            "chainId must begin and end with an ASCII alphanumeric character");
      }
      for (int index = 0; index < value.length(); index++) {
        final char character = value.charAt(index);
        if (!isAsciiLetterOrDigit(character)
            && character != '.'
            && character != '_'
            && character != ':'
            && character != '-') {
          throw new IllegalArgumentException("chainId contains a non-canonical character");
        }
      }
      return value;
    }

    private static boolean isAsciiLetterOrDigit(final char value) {
      return (value >= 'a' && value <= 'z')
          || (value >= 'A' && value <= 'Z')
          || (value >= '0' && value <= '9');
    }

  }
}
