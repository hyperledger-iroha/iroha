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
  private final String destinationProofB64;

  public SccpDestinationProofSubmitRequest(
      final String authority,
      final String destinationProofB64,
      final FeePaymentIntent feePayment) {
    this.authority = SccpSubmitEncoding.requireCanonicalAuthority(authority, "authority");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        destinationProofB64,
        "destinationProofB64",
        SccpSubmitEncoding.MAX_DESTINATION_ARTIFACT_BYTES,
        SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME);
    this.destinationProofB64 = destinationProofB64;
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

  /** Return the exact Torii JSON shape; route overrides are unrepresentable. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("fee_payment", feePayment.toJsonMap());
    json.put("destination_proof_b64", destinationProofB64);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }
}
