package org.hyperledger.iroha.android.model;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/**
 * Representation of a transaction payload prior to Norito encoding.
 *
 * <p>The structure mirrors the Rust data model sufficiently for encoding and signing. Instruction
 * handling currently focuses on the IVM bytecode variant; support for general instruction lists will
 * be added alongside dedicated builders.
 */
public final class TransactionPayload {

  private final String chainId;
  private final String authority;
  private final long creationTimeMs;
  private final Executable executable;
  private final Optional<Long> timeToLiveMs;
  private final Optional<Integer> nonce;
  private final Map<String, String> metadata;

  private TransactionPayload(final Builder builder) {
    this.chainId = builder.chainId;
    this.authority = builder.authority;
    this.creationTimeMs = builder.creationTimeMs;
    this.executable = builder.executable;
    this.timeToLiveMs = builder.timeToLiveMs;
    this.nonce = builder.nonce;
    this.metadata = Collections.unmodifiableMap(new LinkedHashMap<>(builder.metadata));
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

  public Optional<Integer> nonce() {
    return nonce;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public Builder toBuilder() {
    return builder()
        .setChainId(chainId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(executable)
        .setTimeToLiveMs(timeToLiveMs.orElse(null))
        .setNonce(nonce.orElse(null))
        .setMetadata(metadata);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private static final String DEFAULT_AUTHORITY = defaultAuthority();
    private String chainId = "00000000";
    private String authority = DEFAULT_AUTHORITY;
    private long creationTimeMs = System.currentTimeMillis();
    private Executable executable = Executable.ivm(new byte[0]);
    private Optional<Long> timeToLiveMs = Optional.empty();
    private Optional<Integer> nonce = Optional.empty();
    private final Map<String, String> metadata = new LinkedHashMap<>();

    public Builder setChainId(final String chainId) {
      this.chainId = normalize(chainId, "chainId");
      return this;
    }

    public Builder setAuthority(final String authority) {
      this.authority = AccountIdLiteral.extractI105Address(normalize(authority, "authority"));
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

    public Builder setTimeToLiveMs(final Long ttlMs) {
      if (ttlMs == null) {
        this.timeToLiveMs = Optional.empty();
      } else if (ttlMs <= 0) {
        throw new IllegalArgumentException("timeToLiveMs must be positive when present");
      } else {
        this.timeToLiveMs = Optional.of(ttlMs);
      }
      return this;
    }

    public Builder setNonce(final Integer nonce) {
      if (nonce == null) {
        this.nonce = Optional.empty();
      } else if (nonce <= 0) {
        throw new IllegalArgumentException("nonce must be positive when present");
      } else {
        this.nonce = Optional.of(nonce);
      }
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(normalize(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach(this::putMetadata);
      }
      return this;
    }

    public TransactionPayload build() {
      return new TransactionPayload(this);
    }

    private static String normalize(final String value, final String field) {
      if (value == null || value.trim().isEmpty()) {
        throw new IllegalArgumentException(field + " must not be blank");
      }
      return value;
    }

    private static String defaultAuthority() {
      final byte[] publicKey = new byte[32];
      try {
        return AccountAddress.fromAccount(publicKey, "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      } catch (final AccountAddress.AccountAddressException ex) {
        throw new IllegalStateException("Failed to construct default encoded authority", ex);
      }
    }
  }
}
