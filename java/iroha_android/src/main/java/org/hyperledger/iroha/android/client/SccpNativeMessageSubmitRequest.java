package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Exact native-proof request payload for {@code POST /v1/bridge/messages}. */
final class SccpNativeMessageSubmitRequest {
  private final String authority;
  private final FeePaymentIntent feePayment;
  private final String nativeProofB64;
  private final String replayWitnessB64;

  public SccpNativeMessageSubmitRequest(
      final String authority,
      final String nativeProofB64,
      final String replayWitnessB64,
      final FeePaymentIntent feePayment) {
    this.authority = SccpSubmitEncoding.requireCanonicalAuthority(authority, "authority");
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        nativeProofB64,
        "nativeProofB64",
        SccpSubmitEncoding.MAX_NATIVE_PROOF_BYTES,
        SccpSubmitEncoding.NATIVE_INBOUND_PROOF_SCHEMA_NAME);
    this.nativeProofB64 = nativeProofB64;
    SccpSubmitEncoding.validateCanonicalReplayWitnessBase64(
        replayWitnessB64,
        "replayWitnessB64");
    this.replayWitnessB64 = replayWitnessB64;
  }

  public String authority() {
    return authority;
  }

  public String nativeProofB64() {
    return nativeProofB64;
  }

  public String replayWitnessB64() {
    return replayWitnessB64;
  }

  public FeePaymentIntent feePayment() {
    return feePayment;
  }

  /** Return the exact Torii JSON shape; settlement selectors are unrepresentable. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("fee_payment", feePayment.toJsonMap());
    json.put("native_proof_b64", nativeProofB64);
    json.put("replay_witness_b64", replayWitnessB64);
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }
}
