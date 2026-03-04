package org.hyperledger.iroha.android.model.zk;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/**
 * Describes the fields embedded in {@code RegisterVerifyingKey} and
 * {@code UpdateVerifyingKey} instructions.
 *
 * <p>The builder enforces the same invariants as the server-side DTO handling:
 * either inline verifying key bytes must be provided or the commitment + length
 * pair must be supplied. Gas schedule identifiers are required, and the builder
 * defaults optional fields to sensible values when omitted.
 */
public final class VerifyingKeyRecordDescription {

  private final int version;
  private final String circuitId;
  private final VerifyingKeyBackendTag backendTag;
  private final String curve;
  private final String schemaHashHex;
  private final String commitmentHex;
  private final byte[] inlineKeyBytes;
  private final Integer vkLength;
  private final Integer maxProofBytes;
  private final String gasScheduleId;
  private final String metadataUriCid;
  private final String vkBytesCid;
  private final Long activationHeight;
  private final Long withdrawHeight;
  private final VerifyingKeyStatus status;

  private VerifyingKeyRecordDescription(final Builder builder, final String computedCommitmentHex) {
    this.version = builder.version;
    this.circuitId = builder.circuitId;
    this.backendTag = builder.backendTag;
    this.curve = builder.curve;
    this.schemaHashHex = builder.schemaHashHex;
    this.commitmentHex = computedCommitmentHex;
    this.inlineKeyBytes = builder.inlineKeyBytes;
    this.vkLength = builder.vkLength;
    this.maxProofBytes = builder.maxProofBytes;
    this.gasScheduleId = builder.gasScheduleId;
    this.metadataUriCid = builder.metadataUriCid;
    this.vkBytesCid = builder.vkBytesCid;
    this.activationHeight = builder.activationHeight;
    this.withdrawHeight = builder.withdrawHeight;
    this.status = builder.status;
  }

  public int version() {
    return version;
  }

  public String circuitId() {
    return circuitId;
  }

  public VerifyingKeyBackendTag backendTag() {
    return backendTag;
  }

  public String schemaHashHex() {
    return schemaHashHex;
  }

  public String commitmentHex() {
    return commitmentHex;
  }

  public byte[] inlineKeyBytes() {
    return inlineKeyBytes == null ? null : inlineKeyBytes.clone();
  }

  public Integer vkLength() {
    return vkLength;
  }

  public Integer maxProofBytes() {
    return maxProofBytes;
  }

  public String gasScheduleId() {
    return gasScheduleId;
  }

  public VerifyingKeyStatus status() {
    return status;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof VerifyingKeyRecordDescription other)) {
      return false;
    }
    return version == other.version
        && Objects.equals(circuitId, other.circuitId)
        && backendTag == other.backendTag
        && Objects.equals(curve, other.curve)
        && Objects.equals(schemaHashHex, other.schemaHashHex)
        && Objects.equals(commitmentHex, other.commitmentHex)
        && Arrays.equals(inlineKeyBytes, other.inlineKeyBytes)
        && Objects.equals(vkLength, other.vkLength)
        && Objects.equals(maxProofBytes, other.maxProofBytes)
        && Objects.equals(gasScheduleId, other.gasScheduleId)
        && Objects.equals(metadataUriCid, other.metadataUriCid)
        && Objects.equals(vkBytesCid, other.vkBytesCid)
        && Objects.equals(activationHeight, other.activationHeight)
        && Objects.equals(withdrawHeight, other.withdrawHeight)
        && status == other.status;
  }

  @Override
  public int hashCode() {
    int result =
        Objects.hash(
            version,
            circuitId,
            backendTag,
            curve,
            schemaHashHex,
            commitmentHex,
            vkLength,
            maxProofBytes,
            gasScheduleId,
            metadataUriCid,
            vkBytesCid,
            activationHeight,
            withdrawHeight,
            status);
    result = 31 * result + Arrays.hashCode(inlineKeyBytes);
    return result;
  }

  /**
    * Serialises the record into Norito-style arguments using the provided backend identifier for
    * inline verifying key bytes.
    */
  public Map<String, String> toArguments(final String backend) {
    Objects.requireNonNull(backend, "backend");
    final Map<String, String> args = new LinkedHashMap<>();
    args.put("record.version", Integer.toUnsignedString(version));
    args.put("record.circuit_id", circuitId);
    args.put("record.backend_tag", backendTag.noritoValue());
    args.put("record.curve", curve);
    args.put("record.public_inputs_schema_hash_hex", schemaHashHex);
    args.put("record.commitment_hex", commitmentHex);
    if (inlineKeyBytes != null) {
      args.put(
          "record.vk_bytes_b64",
          Base64.getEncoder().encodeToString(inlineKeyBytes));
      args.put("record.vk_len", Integer.toUnsignedString(vkLength));
    } else if (vkLength != null) {
      args.put("record.vk_len", Integer.toUnsignedString(vkLength));
    }
    if (maxProofBytes != null) {
      args.put("record.max_proof_bytes", Integer.toUnsignedString(maxProofBytes));
    }
    args.put("record.gas_schedule_id", gasScheduleId);
    if (metadataUriCid != null) {
      args.put("record.metadata_uri_cid", metadataUriCid);
    }
    if (vkBytesCid != null) {
      args.put("record.vk_bytes_cid", vkBytesCid);
    }
    if (activationHeight != null) {
      args.put("record.activation_height", Long.toUnsignedString(activationHeight));
    }
    if (withdrawHeight != null) {
      args.put("record.withdraw_height", Long.toUnsignedString(withdrawHeight));
    }
    args.put("record.status", status.wireName());
    return args;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private Integer version;
    private String circuitId;
    private VerifyingKeyBackendTag backendTag = VerifyingKeyBackendTag.UNSUPPORTED;
    private String curve = "unknown";
    private String schemaHashHex;
    private String commitmentHex;
    private byte[] inlineKeyBytes;
    private Integer vkLength;
    private Integer maxProofBytes;
    private String gasScheduleId;
    private String metadataUriCid;
    private String vkBytesCid;
    private Long activationHeight;
    private Long withdrawHeight;
    private VerifyingKeyStatus status = VerifyingKeyStatus.ACTIVE;

    private Builder() {}

    public Builder setVersion(final int version) {
      if (version < 0) {
        throw new IllegalArgumentException("version must be non-negative");
      }
      this.version = version;
      return this;
    }

    public Builder setCircuitId(final String circuitId) {
      if (circuitId == null || circuitId.trim().isEmpty()) {
        throw new IllegalArgumentException("circuitId must not be blank");
      }
      this.circuitId = circuitId.trim();
      return this;
    }

    public Builder setBackendTag(final VerifyingKeyBackendTag backendTag) {
      this.backendTag = Objects.requireNonNull(backendTag, "backendTag");
      return this;
    }

    public Builder setCurve(final String curve) {
      if (curve != null && !curve.trim().isEmpty()) {
        this.curve = curve.trim();
      }
      return this;
    }

    public Builder setSchemaHashHex(final String schemaHashHex) {
      this.schemaHashHex = validateHex(schemaHashHex, "public_inputs_schema_hash_hex");
      return this;
    }

    public Builder setCommitmentHex(final String commitmentHex) {
      this.commitmentHex = validateHex(commitmentHex, "commitment_hex");
      return this;
    }

    public Builder setInlineKeyBytes(final byte[] inlineKeyBytes) {
      if (inlineKeyBytes == null) {
        this.inlineKeyBytes = null;
      } else {
        this.inlineKeyBytes = inlineKeyBytes.clone();
      }
      return this;
    }

    public Builder setVkLength(final Integer vkLength) {
      if (vkLength != null && vkLength <= 0) {
        throw new IllegalArgumentException("vkLength must be greater than zero");
      }
      this.vkLength = vkLength;
      return this;
    }

    public Builder setMaxProofBytes(final Integer maxProofBytes) {
      if (maxProofBytes != null && maxProofBytes < 0) {
        throw new IllegalArgumentException("maxProofBytes must be non-negative");
      }
      this.maxProofBytes = maxProofBytes;
      return this;
    }

    public Builder setGasScheduleId(final String gasScheduleId) {
      if (gasScheduleId == null || gasScheduleId.trim().isEmpty()) {
        throw new IllegalArgumentException("gasScheduleId must not be blank");
      }
      this.gasScheduleId = gasScheduleId.trim();
      return this;
    }

    public Builder setMetadataUriCid(final String metadataUriCid) {
      this.metadataUriCid = normalizeOptional(metadataUriCid);
      return this;
    }

    public Builder setVkBytesCid(final String vkBytesCid) {
      this.vkBytesCid = normalizeOptional(vkBytesCid);
      return this;
    }

    public Builder setActivationHeight(final Long activationHeight) {
      if (activationHeight != null && activationHeight < 0) {
        throw new IllegalArgumentException("activationHeight must be non-negative");
      }
      this.activationHeight = activationHeight;
      return this;
    }

    public Builder setWithdrawHeight(final Long withdrawHeight) {
      if (withdrawHeight != null && withdrawHeight < 0) {
        throw new IllegalArgumentException("withdrawHeight must be non-negative");
      }
      this.withdrawHeight = withdrawHeight;
      return this;
    }

    public Builder setStatus(final VerifyingKeyStatus status) {
      this.status = Objects.requireNonNull(status, "status");
      return this;
    }

    public VerifyingKeyRecordDescription build(final String backend) {
      Objects.requireNonNull(backend, "backend");
      if (version == null) {
        throw new IllegalStateException("version must be provided");
      }
      if (circuitId == null) {
        throw new IllegalStateException("circuitId must be provided");
      }
      if (schemaHashHex == null) {
        throw new IllegalStateException("public_inputs_schema_hash_hex must be provided");
      }
      if (gasScheduleId == null) {
        throw new IllegalStateException("gasScheduleId must be provided");
      }
      final byte[] inlineBytes = inlineKeyBytes == null ? null : inlineKeyBytes.clone();
      final Integer lengthValue;
      if (inlineBytes != null) {
        lengthValue = inlineBytes.length;
        if (vkLength != null && !vkLength.equals(lengthValue)) {
          throw new IllegalStateException("vkLength does not match length of inline key bytes");
        }
      } else {
        if (commitmentHex == null) {
          throw new IllegalStateException("commitmentHex must be provided when vk bytes are absent");
        }
        if (vkLength == null) {
          throw new IllegalStateException("vkLength must be provided when vk bytes are absent");
        }
        lengthValue = vkLength;
      }
      if (withdrawHeight != null && activationHeight != null
          && withdrawHeight < activationHeight) {
        throw new IllegalStateException("withdrawHeight must be >= activationHeight");
      }
      final String computedCommitment =
          inlineBytes != null
              ? computeCommitmentHex(backend, inlineBytes)
              : commitmentHex.toLowerCase(Locale.ROOT);
      if (commitmentHex != null
          && inlineBytes != null
          && !commitmentHex.equalsIgnoreCase(computedCommitment)) {
        throw new IllegalStateException("commitmentHex does not match computed inline key commitment");
      }
      this.vkLength = lengthValue;
      return new VerifyingKeyRecordDescription(this, computedCommitment);
    }

    private static String validateHex(final String value, final String field) {
      if (value == null) {
        return null;
      }
      final String normalized = value.trim();
      if (normalized.isEmpty()) {
        return null;
      }
      if (normalized.length() != 64) {
        throw new IllegalArgumentException(field + " must contain exactly 64 hexadecimal characters");
      }
      for (int index = 0; index < normalized.length(); index++) {
        final char c = normalized.charAt(index);
        if (!((c >= '0' && c <= '9')
            || (c >= 'a' && c <= 'f')
            || (c >= 'A' && c <= 'F'))) {
          throw new IllegalArgumentException(field + " must contain only hexadecimal characters");
        }
      }
      return normalized.toLowerCase(Locale.ROOT);
    }

    private static String normalizeOptional(final String value) {
      if (value == null) {
        return null;
      }
      final String trimmed = value.trim();
      return trimmed.isEmpty() ? null : trimmed;
    }

    private static String computeCommitmentHex(final String backend, final byte[] bytes) {
      try {
        final MessageDigest digest = MessageDigest.getInstance("SHA-256");
        digest.update(backend.getBytes());
        digest.update(bytes);
        final byte[] hash = digest.digest();
        final StringBuilder builder = new StringBuilder(hash.length * 2);
        for (final byte b : hash) {
          builder.append(String.format("%02x", b));
        }
        return builder.toString();
      } catch (final NoSuchAlgorithmException ex) {
        throw new IllegalStateException("SHA-256 algorithm unavailable", ex);
      }
    }
  }
}
