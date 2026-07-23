package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Exact request payload for {@code POST /v1/bridge/proofs/submit}. */
public final class SccpDestinationProofSubmitRequest {
  private final String authority;
  private final FeePaymentIntent feePayment;
  private final String signatureB64;
  private final String transactionPayloadB64;
  private final String destinationProofB64;
  private final Long creationTimeMs;

  public SccpDestinationProofSubmitRequest(
      final String authority,
      final String destinationProofB64,
      final FeePaymentIntent feePayment,
      final String signatureB64,
      final String transactionPayloadB64,
      final Long creationTimeMs) {
    this.authority = SccpSubmitEncoding.requireCanonicalAuthority(authority, "authority");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    this.signatureB64 = SccpSubmitEncoding.normalizeOptionalSignature(signatureB64);
    this.transactionPayloadB64 =
        SccpSubmitEncoding.normalizeOptionalTransactionPayload(
            transactionPayloadB64, creationTimeMs, this.authority, this.feePayment);
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        destinationProofB64,
        "destinationProofB64",
        SccpSubmitEncoding.MAX_DESTINATION_ARTIFACT_BYTES,
        SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME);
    this.destinationProofB64 = destinationProofB64;
    this.creationTimeMs = SccpSubmitEncoding.normalizeOptionalCreationTimeMs(creationTimeMs);
    SccpSubmitEncoding.validateDetachedSigningState(
        this.signatureB64, this.transactionPayloadB64, this.creationTimeMs);
  }

  public SccpDestinationProofSubmitRequest(
      final String authority,
      final String destinationProofB64,
      final FeePaymentIntent feePayment) {
    this(authority, destinationProofB64, feePayment, null, null, null);
  }

  public String authority() {
    return authority;
  }

  public String destinationProofB64() {
    return destinationProofB64;
  }

  public FeePaymentIntent feePayment() {
    return feePayment;
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

  /** Return the exact Torii JSON shape; route overrides are unrepresentable. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("fee_payment", feePayment.toJsonMap());
    json.put("destination_proof_b64", destinationProofB64);
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
