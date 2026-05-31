package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.sccp.EvmSccpProver;
import org.hyperledger.iroha.android.sccp.SourceSccpProofs;
import org.hyperledger.iroha.android.sccp.TronSccpProver;

/** Request payload for {@code POST /v1/bridge/proofs/submit}. */
public final class BridgeProofSubmitRequest {

  private final String authority;
  private final Object privateKey;
  private final String publicKeyHex;
  private final String signatureB64;
  private final Map<String, Object> burnBundle;
  private final Map<String, Object> messageBundle;
  private final String networkIdHex;
  private final String verifierAddressHex;
  private final String bridgeAddressHex;
  private final String verifierCodeHashHex;
  private final String verifierKeyHashHex;
  private final String expectedDestinationBindingHashHex;
  private final String tronVerifierAddress;
  private final String proofBytesHex;
  private final Object creationTimeMs;

  private BridgeProofSubmitRequest(final Builder builder) {
    this.authority = requireNonBlank(builder.authority, "authority");
    this.privateKey = builder.privateKey;
    this.publicKeyHex = normalizeOptional(builder.publicKeyHex);
    this.signatureB64 = normalizeOptional(builder.signatureB64);
    this.burnBundle = immutableCopy(builder.burnBundle);
    this.messageBundle = immutableCopy(builder.messageBundle);
    this.networkIdHex = normalizeOptional(builder.networkIdHex);
    this.verifierAddressHex = normalizeOptional(builder.verifierAddressHex);
    this.bridgeAddressHex = normalizeOptional(builder.bridgeAddressHex);
    this.verifierCodeHashHex = normalizeOptional(builder.verifierCodeHashHex);
    this.verifierKeyHashHex = normalizeOptional(builder.verifierKeyHashHex);
    this.expectedDestinationBindingHashHex =
        normalizeOptional(builder.expectedDestinationBindingHashHex);
    this.tronVerifierAddress = normalizeOptional(builder.tronVerifierAddress);
    this.proofBytesHex = normalizeOptional(builder.proofBytesHex);
    this.creationTimeMs = builder.creationTimeMs;

    final int bundleCount = (burnBundle == null ? 0 : 1) + (messageBundle == null ? 0 : 1);
    if (bundleCount != 1) {
      throw new IllegalArgumentException(
          "bridge proof submit must provide exactly one of burnBundle or messageBundle");
    }
    if (burnBundle != null && hasSccpDestinationMaterial()) {
      throw new IllegalArgumentException(
          "SCCP destination fields and proofBytesHex are only valid for messageBundle submissions");
    }
  }

  public String authority() {
    return authority;
  }

  public Object privateKey() {
    return privateKey;
  }

  public String publicKeyHex() {
    return publicKeyHex;
  }

  public String signatureB64() {
    return signatureB64;
  }

  public Map<String, Object> burnBundle() {
    return burnBundle;
  }

  public Map<String, Object> messageBundle() {
    return messageBundle;
  }

  public String networkIdHex() {
    return networkIdHex;
  }

  public String verifierAddressHex() {
    return verifierAddressHex;
  }

  public String bridgeAddressHex() {
    return bridgeAddressHex;
  }

  public String verifierCodeHashHex() {
    return verifierCodeHashHex;
  }

  public String verifierKeyHashHex() {
    return verifierKeyHashHex;
  }

  public String expectedDestinationBindingHashHex() {
    return expectedDestinationBindingHashHex;
  }

  public String tronVerifierAddress() {
    return tronVerifierAddress;
  }

  public String proofBytesHex() {
    return proofBytesHex;
  }

  public Object creationTimeMs() {
    return creationTimeMs;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    if (privateKey != null) {
      json.put("private_key", privateKey);
    }
    if (publicKeyHex != null) {
      json.put("public_key_hex", publicKeyHex);
    }
    if (signatureB64 != null) {
      json.put("signature_b64", signatureB64);
    }
    if (burnBundle != null) {
      json.put("burn_bundle", burnBundle);
    }
    if (messageBundle != null) {
      json.put("message_bundle", messageBundle);
    }
    if (networkIdHex != null) {
      json.put("network_id_hex", networkIdHex);
    }
    if (verifierAddressHex != null) {
      json.put("verifier_address_hex", verifierAddressHex);
    }
    if (bridgeAddressHex != null) {
      json.put("bridge_address_hex", bridgeAddressHex);
    }
    if (verifierCodeHashHex != null) {
      json.put("verifier_code_hash_hex", verifierCodeHashHex);
    }
    if (verifierKeyHashHex != null) {
      json.put("verifier_key_hash_hex", verifierKeyHashHex);
    }
    if (expectedDestinationBindingHashHex != null) {
      json.put("expected_destination_binding_hash_hex", expectedDestinationBindingHashHex);
    }
    if (tronVerifierAddress != null) {
      json.put("tron_verifier_address", tronVerifierAddress);
    }
    if (proofBytesHex != null) {
      json.put("proof_bytes_hex", proofBytesHex);
    }
    if (creationTimeMs != null) {
      json.put("creation_time_ms", creationTimeMs);
    }
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Build an on-chain bridge-proof submit request from an EVM-family SCCP proof submission. */
  public static BridgeProofSubmitRequest fromEvmSccpSubmission(
      final String authority,
      final Map<String, Object> messageBundle,
      final EvmSccpProver.Submission submission,
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    return evmSccpMessageBuilder(authority, messageBundle, submission, destinationBinding).build();
  }

  /** Build an on-chain bridge-proof submit request from a TRON SCCP proof submission. */
  public static BridgeProofSubmitRequest fromTronSccpSubmission(
      final String authority,
      final Map<String, Object> messageBundle,
      final TronSccpProver.Submission submission,
      final SourceSccpProofs.TronDestinationBinding destinationBinding) {
    return tronSccpMessageBuilder(authority, messageBundle, submission, destinationBinding).build();
  }

  /** Prepopulate a bridge-proof submit builder from an EVM-family SCCP proof submission. */
  public static Builder evmSccpMessageBuilder(
      final String authority,
      final Map<String, Object> messageBundle,
      final EvmSccpProver.Submission submission,
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    requireEvmSubmissionMatchesDestination(submission, destinationBinding);
    requireSccpProofMatchesMessageBundle(submission.proofBytes(), messageBundle);
    return builder()
        .authority(authority)
        .messageBundle(Objects.requireNonNull(messageBundle, "messageBundle"))
        .networkIdHex(destinationBinding.networkId)
        .verifierAddressHex(destinationBinding.verifierAddress)
        .bridgeAddressHex(destinationBinding.bridgeAddress)
        .verifierCodeHashHex(destinationBinding.verifierCodeHash)
        .verifierKeyHashHex(destinationBinding.verifierKeyHash)
        .expectedDestinationBindingHashHex(destinationBinding.hash)
        .proofBytesHex("0x" + hexLower(submission.proofBytes()));
  }

  /** Prepopulate a bridge-proof submit builder from a TRON SCCP proof submission. */
  public static Builder tronSccpMessageBuilder(
      final String authority,
      final Map<String, Object> messageBundle,
      final TronSccpProver.Submission submission,
      final SourceSccpProofs.TronDestinationBinding destinationBinding) {
    requireTronSubmissionMatchesDestination(submission, destinationBinding);
    requireSccpProofMatchesMessageBundle(submission.proofBytes(), messageBundle);
    return builder()
        .authority(authority)
        .messageBundle(Objects.requireNonNull(messageBundle, "messageBundle"))
        .networkIdHex(destinationBinding.networkId)
        .verifierCodeHashHex(destinationBinding.verifierCodeHash)
        .verifierKeyHashHex(destinationBinding.verifierKeyHash)
        .expectedDestinationBindingHashHex(destinationBinding.hash)
        .tronVerifierAddress(destinationBinding.verifierAddress)
        .proofBytesHex("0x" + hexLower(submission.proofBytes()));
  }

  private boolean hasSccpDestinationMaterial() {
    return networkIdHex != null
        || verifierAddressHex != null
        || bridgeAddressHex != null
        || verifierCodeHashHex != null
        || verifierKeyHashHex != null
        || expectedDestinationBindingHashHex != null
        || tronVerifierAddress != null
        || proofBytesHex != null;
  }

  private static Map<String, Object> immutableCopy(final Map<String, Object> value) {
    if (value == null) {
      return null;
    }
    return Collections.unmodifiableMap(new LinkedHashMap<>(value));
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null) {
      throw new IllegalArgumentException(field + " is required");
    }
    final String trimmed = value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " is required");
    }
    return trimmed;
  }

  private static String normalizeOptional(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static void requireEvmSubmissionMatchesDestination(
      final EvmSccpProver.Submission submission,
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    Objects.requireNonNull(submission, "submission");
    Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (submission.version() != 1) {
      throw new IllegalArgumentException("EVM SCCP submission version must be 1");
    }
    if (!"contract_call".equals(submission.submissionKind())) {
      throw new IllegalArgumentException("EVM SCCP submission must be a contract_call");
    }
    if (submission.sourceDomain() != destinationBinding.sourceDomain) {
      throw new IllegalArgumentException(
          "EVM SCCP submission sourceDomain must match destination binding");
    }
    if (submission.targetDomain() != destinationBinding.targetDomain) {
      throw new IllegalArgumentException(
          "EVM SCCP submission targetDomain must match destination binding");
    }
    if (!Objects.equals(submission.verifierBackend(), destinationBinding.verifierBackend)) {
      throw new IllegalArgumentException(
          "EVM SCCP submission verifierBackend must match destination binding");
    }
    if (!Objects.equals(submission.proofFamily(), destinationBinding.proofFamily)) {
      throw new IllegalArgumentException(
          "EVM SCCP submission proofFamily must match destination binding");
    }
    if (!Objects.equals(submission.destinationBindingHash(), destinationBinding.hash)) {
      throw new IllegalArgumentException(
          "EVM SCCP submission destinationBindingHash must match destination binding");
    }
  }

  private static void requireTronSubmissionMatchesDestination(
      final TronSccpProver.Submission submission,
      final SourceSccpProofs.TronDestinationBinding destinationBinding) {
    Objects.requireNonNull(submission, "submission");
    Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (submission.version() != 1) {
      throw new IllegalArgumentException("TRON SCCP submission version must be 1");
    }
    if (!"contract_call".equals(submission.submissionKind())) {
      throw new IllegalArgumentException("TRON SCCP submission must be a contract_call");
    }
    if (submission.sourceDomain() != destinationBinding.sourceDomain) {
      throw new IllegalArgumentException(
          "TRON SCCP submission sourceDomain must match destination binding");
    }
    if (submission.targetDomain() != destinationBinding.targetDomain) {
      throw new IllegalArgumentException(
          "TRON SCCP submission targetDomain must match destination binding");
    }
    if (!Objects.equals(submission.verifierBackend(), destinationBinding.verifierBackend)) {
      throw new IllegalArgumentException(
          "TRON SCCP submission verifierBackend must match destination binding");
    }
    if (!Objects.equals(submission.proofFamily(), destinationBinding.proofFamily)) {
      throw new IllegalArgumentException(
          "TRON SCCP submission proofFamily must match destination binding");
    }
    if (!Objects.equals(submission.destinationBindingHash(), destinationBinding.hash)) {
      throw new IllegalArgumentException(
          "TRON SCCP submission destinationBindingHash must match destination binding");
    }
  }

  private static void requireSccpProofMatchesMessageBundle(
      final byte[] proofBytes, final Map<String, Object> messageBundle) {
    final byte[] bytes = Objects.requireNonNull(proofBytes, "proofBytes");
    if (bytes.length != SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
      throw new IllegalArgumentException("SCCP proof bytes must be a 384-byte Groth16 ABI tuple");
    }
    boolean nonZero = false;
    for (final byte value : bytes) {
      if (value != 0) {
        nonZero = true;
        break;
      }
    }
    if (!nonZero) {
      throw new IllegalArgumentException("SCCP proof bytes must not be all zero");
    }
    HttpClientTransport.validateSccpGroth16ProofHex(hexLower(bytes));
    final Map<String, Object> bundle = Objects.requireNonNull(messageBundle, "messageBundle");
    final Object commitmentValue = bundle.get("commitment");
    if (!(commitmentValue instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("message_bundle.commitment.message_id is required");
    }
    final Map<?, ?> commitment = (Map<?, ?>) commitmentValue;
    final Object messageIdValue =
        commitment.containsKey("message_id") ? commitment.get("message_id") : commitment.get("messageId");
    final Object commitmentRootValue =
        bundle.containsKey("commitment_root") ? bundle.get("commitment_root") : bundle.get("commitmentRoot");
    if (messageIdValue == null || commitmentRootValue == null) {
      throw new IllegalArgumentException(
          "message_bundle.commitment.message_id and message_bundle.commitment_root are required");
    }
    if (!(messageIdValue instanceof String)) {
      throw new IllegalArgumentException(
          "message_bundle.commitment.message_id must contain 64 hex characters");
    }
    if (!(commitmentRootValue instanceof String)) {
      throw new IllegalArgumentException(
          "message_bundle.commitment_root must contain 64 hex characters");
    }
    final String messageId =
        normalizeHex32((String) messageIdValue, "message_bundle.commitment.message_id");
    final String commitmentRoot =
        normalizeHex32((String) commitmentRootValue, "message_bundle.commitment_root");
    if (!proofWordHex(bytes, 0).equals(abiWordU32Hex(1))) {
      throw new IllegalArgumentException("proof_bytes_hex.version must be 1");
    }
    if (!proofWordHex(bytes, 1).equals(messageId)) {
      throw new IllegalArgumentException(
          "proof_bytes_hex.message_id must match message_bundle.commitment.message_id");
    }
    if (!proofWordHex(bytes, 2).equals(abiWordU32Hex(SourceSccpProofs.DOMAIN_SORA))) {
      throw new IllegalArgumentException("proof_bytes_hex.source_domain must be SORA");
    }
    if (!proofWordHex(bytes, 3).equals(commitmentRoot)) {
      throw new IllegalArgumentException(
          "proof_bytes_hex.commitment_root must match message_bundle.commitment_root");
    }
  }

  private static String normalizeHex32(final String value, final String field) {
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must be a canonical hex string");
    }
    final String normalized;
    if (value.startsWith("0x") || value.startsWith("0X")) {
      normalized = value.substring(2);
    } else {
      normalized = value;
    }
    if (normalized.length() != 64) {
      throw new IllegalArgumentException(field + " must contain 64 hex characters");
    }
    for (int index = 0; index < normalized.length(); index++) {
      final char c = normalized.charAt(index);
      if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'))) {
        throw new IllegalArgumentException(field + " must contain 64 hex characters");
      }
    }
    return normalized.toLowerCase(java.util.Locale.ROOT);
  }

  private static String abiWordU32Hex(final int value) {
    final String hex = Integer.toHexString(value);
    final StringBuilder out = new StringBuilder(64);
    for (int i = hex.length(); i < 64; i++) {
      out.append('0');
    }
    out.append(hex);
    return out.toString();
  }

  private static String proofWordHex(final byte[] proofBytes, final int index) {
    final byte[] word = new byte[32];
    System.arraycopy(proofBytes, index * 32, word, 0, word.length);
    return hexLower(word);
  }

  private static final int SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 = 384;

  private static String hexLower(final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final byte[] source = Objects.requireNonNull(bytes, "bytes");
    final char[] out = new char[source.length * 2];
    for (int index = 0; index < source.length; index++) {
      final int value = source[index] & 0xff;
      out[index * 2] = digits[value >>> 4];
      out[index * 2 + 1] = digits[value & 0x0f];
    }
    return new String(out);
  }

  public static final class Builder {
    private String authority;
    private Object privateKey;
    private String publicKeyHex;
    private String signatureB64;
    private Map<String, Object> burnBundle;
    private Map<String, Object> messageBundle;
    private String networkIdHex;
    private String verifierAddressHex;
    private String bridgeAddressHex;
    private String verifierCodeHashHex;
    private String verifierKeyHashHex;
    private String expectedDestinationBindingHashHex;
    private String tronVerifierAddress;
    private String proofBytesHex;
    private Object creationTimeMs;

    private Builder() {}

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder privateKey(final Object privateKey) {
      this.privateKey = privateKey;
      return this;
    }

    public Builder publicKeyHex(final String publicKeyHex) {
      this.publicKeyHex = publicKeyHex;
      return this;
    }

    public Builder signatureB64(final String signatureB64) {
      this.signatureB64 = signatureB64;
      return this;
    }

    public Builder burnBundle(final Map<String, Object> burnBundle) {
      this.burnBundle = burnBundle;
      return this;
    }

    public Builder messageBundle(final Map<String, Object> messageBundle) {
      this.messageBundle = messageBundle;
      return this;
    }

    public Builder networkIdHex(final String networkIdHex) {
      this.networkIdHex = networkIdHex;
      return this;
    }

    public Builder verifierAddressHex(final String verifierAddressHex) {
      this.verifierAddressHex = verifierAddressHex;
      return this;
    }

    public Builder bridgeAddressHex(final String bridgeAddressHex) {
      this.bridgeAddressHex = bridgeAddressHex;
      return this;
    }

    public Builder verifierCodeHashHex(final String verifierCodeHashHex) {
      this.verifierCodeHashHex = verifierCodeHashHex;
      return this;
    }

    public Builder verifierKeyHashHex(final String verifierKeyHashHex) {
      this.verifierKeyHashHex = verifierKeyHashHex;
      return this;
    }

    public Builder expectedDestinationBindingHashHex(
        final String expectedDestinationBindingHashHex) {
      this.expectedDestinationBindingHashHex = expectedDestinationBindingHashHex;
      return this;
    }

    public Builder tronVerifierAddress(final String tronVerifierAddress) {
      this.tronVerifierAddress = tronVerifierAddress;
      return this;
    }

    public Builder proofBytesHex(final String proofBytesHex) {
      this.proofBytesHex = proofBytesHex;
      return this;
    }

    public Builder creationTimeMs(final Object creationTimeMs) {
      this.creationTimeMs = creationTimeMs;
      return this;
    }

    public BridgeProofSubmitRequest build() {
      return new BridgeProofSubmitRequest(this);
    }
  }
}
