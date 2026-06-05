package org.hyperledger.iroha.android.client;

/** Request body for {@code POST /v1/zk/vk/register}. */
public final class VerifyingKeyRegisterRequest {
  private final String authority;
  private final String privateKey;
  private final String backend;
  private final String name;
  private final long version;
  private final String circuitId;
  private final String publicInputsSchemaHashHex;
  private final String gasScheduleId;
  private final String curve;
  private final Long maxProofBytes;
  private final String metadataUriCid;
  private final String verifyingKeyBytesCid;
  private final Long activationHeight;
  private final Long withdrawHeight;
  private final String commitmentHex;
  private final byte[] verifyingKeyBytes;
  private final Long verifyingKeyLength;
  private final String status;

  private VerifyingKeyRegisterRequest(final Builder builder) {
    this.authority = builder.authority;
    this.privateKey = builder.privateKey;
    this.backend = builder.backend;
    this.name = builder.name;
    this.version = builder.version;
    this.circuitId = builder.circuitId;
    this.publicInputsSchemaHashHex = builder.publicInputsSchemaHashHex;
    this.gasScheduleId = builder.gasScheduleId;
    this.curve = builder.curve;
    this.maxProofBytes = builder.maxProofBytes;
    this.metadataUriCid = builder.metadataUriCid;
    this.verifyingKeyBytesCid = builder.verifyingKeyBytesCid;
    this.activationHeight = builder.activationHeight;
    this.withdrawHeight = builder.withdrawHeight;
    this.commitmentHex = builder.commitmentHex;
    this.verifyingKeyBytes =
        builder.verifyingKeyBytes == null ? null : builder.verifyingKeyBytes.clone();
    this.verifyingKeyLength = builder.verifyingKeyLength;
    this.status = builder.status;
  }

  public String authority() { return authority; }
  public String privateKey() { return privateKey; }
  public String backend() { return backend; }
  public String name() { return name; }
  public long version() { return version; }
  public String circuitId() { return circuitId; }
  public String publicInputsSchemaHashHex() { return publicInputsSchemaHashHex; }
  public String gasScheduleId() { return gasScheduleId; }
  public String curve() { return curve; }
  public Long maxProofBytes() { return maxProofBytes; }
  public String metadataUriCid() { return metadataUriCid; }
  public String verifyingKeyBytesCid() { return verifyingKeyBytesCid; }
  public Long activationHeight() { return activationHeight; }
  public Long withdrawHeight() { return withdrawHeight; }
  public String commitmentHex() { return commitmentHex; }
  public byte[] verifyingKeyBytes() {
    return verifyingKeyBytes == null ? null : verifyingKeyBytes.clone();
  }
  public Long verifyingKeyLength() { return verifyingKeyLength; }
  public String status() { return status; }

  public static Builder builder() { return new Builder(); }

  public static final class Builder {
    private String authority;
    private String privateKey;
    private String backend;
    private String name;
    private long version;
    private String circuitId;
    private String publicInputsSchemaHashHex;
    private String gasScheduleId;
    private String curve;
    private Long maxProofBytes;
    private String metadataUriCid;
    private String verifyingKeyBytesCid;
    private Long activationHeight;
    private Long withdrawHeight;
    private String commitmentHex;
    private byte[] verifyingKeyBytes;
    private Long verifyingKeyLength;
    private String status;

    private Builder() {}

    public Builder authority(final String value) { authority = value; return this; }
    public Builder privateKey(final String value) { privateKey = value; return this; }
    public Builder backend(final String value) { backend = value; return this; }
    public Builder name(final String value) { name = value; return this; }
    public Builder version(final long value) { version = value; return this; }
    public Builder circuitId(final String value) { circuitId = value; return this; }
    public Builder publicInputsSchemaHashHex(final String value) {
      publicInputsSchemaHashHex = value;
      return this;
    }
    public Builder gasScheduleId(final String value) { gasScheduleId = value; return this; }
    public Builder curve(final String value) { curve = value; return this; }
    public Builder maxProofBytes(final Long value) { maxProofBytes = value; return this; }
    public Builder metadataUriCid(final String value) { metadataUriCid = value; return this; }
    public Builder verifyingKeyBytesCid(final String value) {
      verifyingKeyBytesCid = value;
      return this;
    }
    public Builder activationHeight(final Long value) { activationHeight = value; return this; }
    public Builder withdrawHeight(final Long value) { withdrawHeight = value; return this; }
    public Builder commitmentHex(final String value) { commitmentHex = value; return this; }
    public Builder verifyingKeyBytes(final byte[] value) {
      verifyingKeyBytes = value == null ? null : value.clone();
      return this;
    }
    public Builder verifyingKeyLength(final Long value) { verifyingKeyLength = value; return this; }
    public Builder status(final String value) { status = value; return this; }
    public VerifyingKeyRegisterRequest build() { return new VerifyingKeyRegisterRequest(this); }
  }
}
