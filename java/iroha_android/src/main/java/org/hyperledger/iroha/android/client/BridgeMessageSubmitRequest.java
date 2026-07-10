package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Native-proof-only request for {@code POST /v1/bridge/messages}. */
public final class BridgeMessageSubmitRequest {
  private final String authority;
  private final String publicKeyHex;
  private final String signatureB64;
  private final String nativeProofB64;
  private final Long creationTimeMs;

  public BridgeMessageSubmitRequest(
      final String authority,
      final String publicKeyHex,
      final String signatureB64,
      final String nativeProofB64,
      final Long creationTimeMs) {
    this.authority = SccpSubmitEncoding.requireCanonicalNonBlank(authority, "authority");
    this.publicKeyHex = SccpSubmitEncoding.normalizeOptionalPublicKeyHex(publicKeyHex);
    this.signatureB64 =
        SccpSubmitEncoding.normalizeOptionalExactBase64(signatureB64, "signatureB64");
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        nativeProofB64, "nativeProofB64", SccpSubmitEncoding.MAX_ARTIFACT_BYTES);
    this.nativeProofB64 = nativeProofB64;
    this.creationTimeMs = SccpSubmitEncoding.normalizeOptionalCreationTimeMs(creationTimeMs);
    if ((this.publicKeyHex == null) != (this.signatureB64 == null)) {
      throw new IllegalArgumentException("publicKeyHex and signatureB64 must be supplied together");
    }
  }

  public BridgeMessageSubmitRequest(final String authority, final String nativeProofB64) {
    this(authority, null, null, nativeProofB64, null);
  }

  public String nativeProofB64() {
    return nativeProofB64;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    if (publicKeyHex != null) json.put("public_key_hex", publicKeyHex);
    if (signatureB64 != null) json.put("signature_b64", signatureB64);
    json.put("native_proof_b64", nativeProofB64);
    if (creationTimeMs != null) json.put("creation_time_ms", creationTimeMs);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }
}
