package org.hyperledger.iroha.android.client;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Canonical payload signed by an external RAM-LFE output-opening authority. */
public final class RamLfeOutputOpeningPayload {
  private final String programId;
  private final String inputCiphertextHash;
  private final String outputCiphertextHash;
  private final String parameterDigest;
  private final String evaluationKeyDigest;
  private final String openedOutputHash;
  private final long openedAtMs;
  private final Long expiresAtMs;

  public RamLfeOutputOpeningPayload(
      final String programId,
      final String inputCiphertextHash,
      final String outputCiphertextHash,
      final String parameterDigest,
      final String evaluationKeyDigest,
      final String openedOutputHash,
      final long openedAtMs,
      final Long expiresAtMs) {
    this.programId = Objects.requireNonNull(programId, "programId");
    this.inputCiphertextHash = Objects.requireNonNull(inputCiphertextHash, "inputCiphertextHash");
    this.outputCiphertextHash = Objects.requireNonNull(outputCiphertextHash, "outputCiphertextHash");
    this.parameterDigest = Objects.requireNonNull(parameterDigest, "parameterDigest");
    this.evaluationKeyDigest = Objects.requireNonNull(evaluationKeyDigest, "evaluationKeyDigest");
    this.openedOutputHash = Objects.requireNonNull(openedOutputHash, "openedOutputHash");
    this.openedAtMs = openedAtMs;
    this.expiresAtMs = expiresAtMs;
  }

  public String programId() {
    return programId;
  }

  public String inputCiphertextHash() {
    return inputCiphertextHash;
  }

  public String outputCiphertextHash() {
    return outputCiphertextHash;
  }

  public String parameterDigest() {
    return parameterDigest;
  }

  public String evaluationKeyDigest() {
    return evaluationKeyDigest;
  }

  public String openedOutputHash() {
    return openedOutputHash;
  }

  public long openedAtMs() {
    return openedAtMs;
  }

  public Long expiresAtMs() {
    return expiresAtMs;
  }

  Map<String, Object> toJsonMap() {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("program_id", HttpClientTransport.normalizeNonBlank(programId, "opening.payload.programId"));
    payload.put(
        "input_ciphertext_hash",
        HttpClientTransport.normalizeHex32(inputCiphertextHash, "opening.payload.inputCiphertextHash"));
    payload.put(
        "output_ciphertext_hash",
        HttpClientTransport.normalizeHex32(outputCiphertextHash, "opening.payload.outputCiphertextHash"));
    payload.put(
        "parameter_digest",
        HttpClientTransport.normalizeHex32(parameterDigest, "opening.payload.parameterDigest"));
    payload.put(
        "evaluation_key_digest",
        HttpClientTransport.normalizeHex32(evaluationKeyDigest, "opening.payload.evaluationKeyDigest"));
    payload.put(
        "opened_output_hash",
        HttpClientTransport.normalizeHex32(openedOutputHash, "opening.payload.openedOutputHash"));
    payload.put("opened_at_ms", openedAtMs);
    if (expiresAtMs != null) {
      payload.put("expires_at_ms", expiresAtMs);
    }
    return payload;
  }
}
