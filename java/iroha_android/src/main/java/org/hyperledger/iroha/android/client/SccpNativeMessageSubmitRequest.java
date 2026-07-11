package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Exact native-proof request payload for {@code POST /v1/bridge/messages}. */
public final class SccpNativeMessageSubmitRequest {
  private final String authority;
  private final String signatureB64;
  private final String transactionPayloadB64;
  private final String nativeProofB64;
  private final Long creationTimeMs;

  public SccpNativeMessageSubmitRequest(
      final String authority,
      final String nativeProofB64,
      final String signatureB64,
      final String transactionPayloadB64,
      final Long creationTimeMs) {
    this.authority = SccpSubmitEncoding.requireCanonicalAuthority(authority, "authority");
    this.signatureB64 = SccpSubmitEncoding.normalizeOptionalSignature(signatureB64);
    this.transactionPayloadB64 =
        SccpSubmitEncoding.normalizeOptionalTransactionPayload(
            transactionPayloadB64, creationTimeMs, this.authority);
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        nativeProofB64, "nativeProofB64", SccpSubmitEncoding.MAX_NATIVE_PROOF_BYTES);
    this.nativeProofB64 = nativeProofB64;
    this.creationTimeMs = SccpSubmitEncoding.normalizeOptionalCreationTimeMs(creationTimeMs);
    SccpSubmitEncoding.validateDetachedSigningState(
        this.signatureB64, this.transactionPayloadB64, this.creationTimeMs);
  }

  public SccpNativeMessageSubmitRequest(final String authority, final String nativeProofB64) {
    this(authority, nativeProofB64, null, null, null);
  }

  public String authority() {
    return authority;
  }

  public String nativeProofB64() {
    return nativeProofB64;
  }

  public String signatureB64() {
    return signatureB64;
  }

  public String transactionPayloadB64() {
    return transactionPayloadB64;
  }

  public Long creationTimeMs() {
    return creationTimeMs;
  }

  /** Return the exact Torii JSON shape; settlement selectors are unrepresentable. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("native_proof_b64", nativeProofB64);
    if (signatureB64 != null) json.put("signature_b64", signatureB64);
    if (transactionPayloadB64 != null) {
      json.put("transaction_payload_b64", transactionPayloadB64);
    }
    if (creationTimeMs != null) json.put("creation_time_ms", creationTimeMs);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }
}
