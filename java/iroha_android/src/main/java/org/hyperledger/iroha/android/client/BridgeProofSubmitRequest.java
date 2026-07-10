package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Exact first-release request payload for {@code POST /v1/bridge/proofs/submit}. */
public final class BridgeProofSubmitRequest {
  private final String authority;
  private final String publicKeyHex;
  private final String signatureB64;
  private final String messageBundleB64;
  private final String networkIdHex;
  private final String verifierAddressHex;
  private final String bridgeAddressHex;
  private final String verifierCodeHashHex;
  private final String verifierKeyHashHex;
  private final String tronVerifierAddress;
  private final String proofBytesHex;
  private final Long creationTimeMs;

  private BridgeProofSubmitRequest(final Builder builder) {
    authority = SccpSubmitEncoding.requireCanonicalNonBlank(builder.authority, "authority");
    publicKeyHex = SccpSubmitEncoding.normalizeOptionalPublicKeyHex(builder.publicKeyHex);
    signatureB64 =
        SccpSubmitEncoding.normalizeOptionalExactBase64(builder.signatureB64, "signatureB64");
    messageBundleB64 = builder.messageBundleB64;
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        messageBundleB64, "messageBundleB64", SccpSubmitEncoding.MAX_ARTIFACT_BYTES);
    networkIdHex = SccpSubmitEncoding.normalizeOptionalHex(builder.networkIdHex, 32, "networkIdHex");
    verifierAddressHex =
        SccpSubmitEncoding.normalizeOptionalHex(
            builder.verifierAddressHex, 20, "verifierAddressHex");
    bridgeAddressHex =
        SccpSubmitEncoding.normalizeOptionalHex(builder.bridgeAddressHex, 20, "bridgeAddressHex");
    verifierCodeHashHex =
        SccpSubmitEncoding.normalizeOptionalHex(
            builder.verifierCodeHashHex, 32, "verifierCodeHashHex");
    verifierKeyHashHex =
        SccpSubmitEncoding.normalizeOptionalHex(
            builder.verifierKeyHashHex, 32, "verifierKeyHashHex");
    tronVerifierAddress = SccpSubmitEncoding.normalizeOptional(builder.tronVerifierAddress);
    proofBytesHex = SccpSubmitEncoding.normalizeOptional(builder.proofBytesHex);
    creationTimeMs = SccpSubmitEncoding.normalizeOptionalCreationTimeMs(builder.creationTimeMs);

    if ((publicKeyHex == null) != (signatureB64 == null)) {
      throw new IllegalArgumentException("publicKeyHex and signatureB64 must be supplied together");
    }

    final boolean destinationPresent = hasDestinationMaterial();
    if ((proofBytesHex == null) != !destinationPresent) {
      throw new IllegalArgumentException(
          "proofBytesHex and complete destination material must be supplied together");
    }
    if (destinationPresent) {
      requireCompleteDestinationTuple();
    }
  }

  public String authority() {
    return authority;
  }

  public String messageBundleB64() {
    return messageBundleB64;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    putIfPresent(json, "public_key_hex", publicKeyHex);
    putIfPresent(json, "signature_b64", signatureB64);
    putIfPresent(json, "message_bundle_b64", messageBundleB64);
    putIfPresent(json, "network_id_hex", networkIdHex);
    putIfPresent(json, "verifier_address_hex", verifierAddressHex);
    putIfPresent(json, "bridge_address_hex", bridgeAddressHex);
    putIfPresent(json, "verifier_code_hash_hex", verifierCodeHashHex);
    putIfPresent(json, "verifier_key_hash_hex", verifierKeyHashHex);
    putIfPresent(json, "tron_verifier_address", tronVerifierAddress);
    putIfPresent(json, "proof_bytes_hex", proofBytesHex);
    putIfPresent(json, "creation_time_ms", creationTimeMs);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }

  public static Builder builder() {
    return new Builder();
  }

  private boolean hasDestinationMaterial() {
    return networkIdHex != null
        || verifierAddressHex != null
        || bridgeAddressHex != null
        || verifierCodeHashHex != null
        || verifierKeyHashHex != null
        || tronVerifierAddress != null;
  }

  private void requireCompleteDestinationTuple() {
    final boolean evm = verifierAddressHex != null || bridgeAddressHex != null;
    final boolean tron = tronVerifierAddress != null;
    if (evm == tron) {
      throw new IllegalArgumentException(
          "destination material must select exactly one EVM or TRON family");
    }
    if (networkIdHex == null || verifierCodeHashHex == null || verifierKeyHashHex == null) {
      throw new IllegalArgumentException("complete SCCP destination material is required");
    }
    if (evm && (verifierAddressHex == null || bridgeAddressHex == null)) {
      throw new IllegalArgumentException("complete EVM SCCP destination material is required");
    }
    if (tron && tronVerifierAddress.isEmpty()) {
      throw new IllegalArgumentException("complete TRON SCCP destination material is required");
    }
  }

  private static void putIfPresent(
      final Map<String, Object> target, final String key, final Object value) {
    if (value != null) {
      target.put(key, value);
    }
  }

  /** Builder for the exact request shape. */
  public static final class Builder {
    private String authority;
    private String publicKeyHex;
    private String signatureB64;
    private String messageBundleB64;
    private String networkIdHex;
    private String verifierAddressHex;
    private String bridgeAddressHex;
    private String verifierCodeHashHex;
    private String verifierKeyHashHex;
    private String tronVerifierAddress;
    private String proofBytesHex;
    private Long creationTimeMs;

    public Builder authority(final String value) {
      authority = value;
      return this;
    }

    public Builder publicKeyHex(final String value) {
      publicKeyHex = value;
      return this;
    }

    public Builder signatureB64(final String value) {
      signatureB64 = value;
      return this;
    }

    public Builder messageBundleB64(final String value) {
      messageBundleB64 = value;
      return this;
    }

    public Builder networkIdHex(final String value) {
      networkIdHex = value;
      return this;
    }

    public Builder verifierAddressHex(final String value) {
      verifierAddressHex = value;
      return this;
    }

    public Builder bridgeAddressHex(final String value) {
      bridgeAddressHex = value;
      return this;
    }

    public Builder verifierCodeHashHex(final String value) {
      verifierCodeHashHex = value;
      return this;
    }

    public Builder verifierKeyHashHex(final String value) {
      verifierKeyHashHex = value;
      return this;
    }

    public Builder tronVerifierAddress(final String value) {
      tronVerifierAddress = value;
      return this;
    }

    public Builder proofBytesHex(final String value) {
      proofBytesHex = value;
      return this;
    }

    public Builder creationTimeMs(final Long value) {
      creationTimeMs = value;
      return this;
    }

    public BridgeProofSubmitRequest build() {
      return new BridgeProofSubmitRequest(this);
    }
  }
}
