package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed representation of {@code zk::Shield}. */
public final class ShieldInstruction implements InstructionTemplate {
  private final String asset;
  private final String from;
  private final String amount;
  private final byte[] noteCommitment;
  private final ConfidentialEncryptedPayload encryptedPayload;
  private final Map<String, String> arguments;

  private ShieldInstruction(final Builder builder) {
    this.asset = builder.asset;
    this.from = builder.from;
    this.amount = builder.amount;
    this.noteCommitment = builder.noteCommitment.clone();
    this.encryptedPayload = builder.encryptedPayload;
    final LinkedHashMap<String, String> args = new LinkedHashMap<>();
    args.put("action", "Shield");
    args.put("asset", asset);
    args.put("from", from);
    args.put("amount", amount);
    args.put("note_commitment", ZkInstructionUtils.hexLower(noteCommitment));
    args.put("payload_ephemeral", ZkInstructionUtils.hexLower(encryptedPayload.ephemeralPublicKey()));
    args.put("payload_nonce", ZkInstructionUtils.hexLower(encryptedPayload.nonce()));
    args.put(
        "payload_ciphertext",
        Base64.getEncoder().encodeToString(encryptedPayload.ciphertext()));
    this.arguments = Collections.unmodifiableMap(args);
  }

  public String asset() {
    return asset;
  }

  public String from() {
    return from;
  }

  public String amount() {
    return amount;
  }

  public byte[] noteCommitment() {
    return noteCommitment.clone();
  }

  public ConfidentialEncryptedPayload encryptedPayload() {
    return encryptedPayload;
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

  /**
   * Intentionally unsupported. {@code zk::Shield} carries a 32-byte note commitment and a binary
   * X25519/XChaCha20-Poly1305 encrypted payload that cannot be reconstructed from a generic string
   * argument map. Build instances through {@link #builder()} instead.
   */
  public static ShieldInstruction fromArguments(final Map<String, String> arguments) {
    throw new UnsupportedOperationException(
        "ShieldInstruction cannot be built from an argument map: its note commitment and "
            + "encrypted payload are binary fields. Use ShieldInstruction.builder().");
  }

  public static final class Builder {
    private String asset;
    private String from;
    private String amount;
    private byte[] noteCommitment;
    private ConfidentialEncryptedPayload encryptedPayload;

    private Builder() {}

    public Builder setAsset(final String asset) {
      this.asset = ZkInstructionUtils.requireText(asset, "asset");
      return this;
    }

    public Builder setFrom(final String from) {
      this.from = ZkInstructionUtils.requireText(from, "from");
      return this;
    }

    public Builder setAmount(final String amount) {
      this.amount = ZkInstructionUtils.canonicalU128(amount, "amount");
      return this;
    }

    public Builder setAmount(final Number amount) {
      return setAmount(amount == null ? null : amount.toString());
    }

    public Builder setNoteCommitment(final byte[] noteCommitment) {
      this.noteCommitment =
          ZkInstructionUtils.fixedNonZeroBytes(noteCommitment, 32, "noteCommitment");
      return this;
    }

    public Builder setEncryptedPayload(final ConfidentialEncryptedPayload encryptedPayload) {
      this.encryptedPayload = Objects.requireNonNull(encryptedPayload, "encryptedPayload");
      return this;
    }

    public ShieldInstruction build() {
      if (asset == null) {
        throw new IllegalStateException("asset must be provided");
      }
      if (from == null) {
        throw new IllegalStateException("from must be provided");
      }
      if (amount == null) {
        throw new IllegalStateException("amount must be provided");
      }
      if (noteCommitment == null) {
        throw new IllegalStateException("noteCommitment must be provided");
      }
      if (encryptedPayload == null) {
        throw new IllegalStateException("encryptedPayload must be provided");
      }
      return new ShieldInstruction(this);
    }
  }
}
