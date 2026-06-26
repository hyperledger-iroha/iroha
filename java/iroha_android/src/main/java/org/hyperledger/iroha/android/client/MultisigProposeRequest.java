package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;

/** Request body for `POST /v1/multisig/propose`. */
public final class MultisigProposeRequest {
  private final String multisigAccountId;
  private final String multisigAccountAlias;
  private final String signerAccountId;
  private final List<byte[]> instructions;
  private final String publicKeyHex;
  private final String signatureB64;
  private final Long creationTimeMs;
  private final String feeSponsor;
  private final String memo;
  private final Long validationFeePolicyVersion;
  private final String validationFeePolicyHash;
  private final Long validationFeeInstructionIndex;

  private MultisigProposeRequest(final Builder builder) {
    this.multisigAccountId = builder.multisigAccountId;
    this.multisigAccountAlias = builder.multisigAccountAlias;
    this.signerAccountId = Objects.requireNonNull(builder.signerAccountId, "signerAccountId");
    this.instructions = copyInstructions(builder.instructions);
    this.publicKeyHex = builder.publicKeyHex;
    this.signatureB64 = builder.signatureB64;
    this.creationTimeMs = builder.creationTimeMs;
    this.feeSponsor = builder.feeSponsor;
    this.memo = builder.memo;
    this.validationFeePolicyVersion = builder.validationFeePolicyVersion;
    this.validationFeePolicyHash = builder.validationFeePolicyHash;
    this.validationFeeInstructionIndex = builder.validationFeeInstructionIndex;
  }

  public static Builder builder() {
    return new Builder();
  }

  public String multisigAccountId() { return multisigAccountId; }
  public String multisigAccountAlias() { return multisigAccountAlias; }
  public String signerAccountId() { return signerAccountId; }
  public List<byte[]> instructions() { return copyInstructions(instructions); }
  public String publicKeyHex() { return publicKeyHex; }
  public String signatureB64() { return signatureB64; }
  public Long creationTimeMs() { return creationTimeMs; }
  public String feeSponsor() { return feeSponsor; }
  public String memo() { return memo; }
  public Long validationFeePolicyVersion() { return validationFeePolicyVersion; }
  public String validationFeePolicyHash() { return validationFeePolicyHash; }
  public Long validationFeeInstructionIndex() { return validationFeeInstructionIndex; }

  private static List<byte[]> copyInstructions(final List<byte[]> source) {
    final List<byte[]> copy = new ArrayList<>();
    if (source != null) {
      for (final byte[] instruction : source) {
        copy.add(instruction == null ? null : instruction.clone());
      }
    }
    return Collections.unmodifiableList(copy);
  }

  public static final class Builder {
    private String multisigAccountId;
    private String multisigAccountAlias;
    private String signerAccountId;
    private final List<byte[]> instructions = new ArrayList<>();
    private String publicKeyHex;
    private String signatureB64;
    private Long creationTimeMs;
    private String feeSponsor;
    private String memo;
    private Long validationFeePolicyVersion;
    private String validationFeePolicyHash;
    private Long validationFeeInstructionIndex;

    public Builder setMultisigAccountId(final String value) {
      this.multisigAccountId = value;
      return this;
    }

    public Builder setMultisigAccountAlias(final String value) {
      this.multisigAccountAlias = value;
      return this;
    }

    public Builder setSignerAccountId(final String value) {
      this.signerAccountId = value;
      return this;
    }

    public Builder addInstructionBytes(final byte[] value) {
      this.instructions.add(value == null ? null : value.clone());
      return this;
    }

    public Builder setInstructionBytes(final List<byte[]> values) {
      this.instructions.clear();
      if (values != null) {
        for (final byte[] value : values) {
          addInstructionBytes(value);
        }
      }
      return this;
    }

    public Builder setInstructionBoxes(final List<InstructionBox> values) throws NoritoException {
      this.instructions.clear();
      if (values != null) {
        for (final InstructionBox value : values) {
          this.instructions.add(NoritoJavaCodecAdapter.encodeInstructionBox(value));
        }
      }
      return this;
    }

    public Builder setPublicKeyHex(final String value) {
      this.publicKeyHex = value;
      return this;
    }

    public Builder setSignatureB64(final String value) {
      this.signatureB64 = value;
      return this;
    }

    public Builder setCreationTimeMs(final Long value) {
      this.creationTimeMs = value;
      return this;
    }

    public Builder setFeeSponsor(final String value) {
      this.feeSponsor = value;
      return this;
    }

    public Builder setMemo(final String value) {
      this.memo = value;
      return this;
    }

    public Builder setValidationFeePolicyVersion(final Long value) {
      this.validationFeePolicyVersion = value;
      return this;
    }

    public Builder setValidationFeePolicyHash(final String value) {
      this.validationFeePolicyHash = value;
      return this;
    }

    public Builder setValidationFeeInstructionIndex(final Long value) {
      this.validationFeeInstructionIndex = value;
      return this;
    }

    public MultisigProposeRequest build() {
      return new MultisigProposeRequest(this);
    }
  }
}
