package org.hyperledger.iroha.android.client;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Successful response emitted by `POST /v1/ram-lfe/programs/{program_id}/execute`. */
public final class RamLfeExecuteResponse {
  private final String programId;
  private final String opaqueHash;
  private final String receiptHash;
  private final String outputHex;
  private final String outputHash;
  private final String associatedDataHash;
  private final long executedAtMs;
  private final Long expiresAtMs;
  private final String backend;
  private final String verificationMode;
  private final Map<String, Object> receipt;

  public RamLfeExecuteResponse(
      final String programId,
      final String opaqueHash,
      final String receiptHash,
      final String outputHex,
      final String outputHash,
      final String associatedDataHash,
      final long executedAtMs,
      final Long expiresAtMs,
      final String backend,
      final String verificationMode,
      final Map<String, Object> receipt) {
    this.programId = Objects.requireNonNull(programId, "programId");
    this.opaqueHash = Objects.requireNonNull(opaqueHash, "opaqueHash");
    this.receiptHash = Objects.requireNonNull(receiptHash, "receiptHash");
    this.outputHex = Objects.requireNonNull(outputHex, "outputHex");
    this.outputHash = Objects.requireNonNull(outputHash, "outputHash");
    this.associatedDataHash = Objects.requireNonNull(associatedDataHash, "associatedDataHash");
    this.executedAtMs = executedAtMs;
    this.expiresAtMs = expiresAtMs;
    this.backend = Objects.requireNonNull(backend, "backend");
    this.verificationMode = Objects.requireNonNull(verificationMode, "verificationMode");
    this.receipt =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(receipt, "receipt")));
  }

  public String programId() {
    return programId;
  }

  public String opaqueHash() {
    return opaqueHash;
  }

  public String receiptHash() {
    return receiptHash;
  }

  public String outputHex() {
    return outputHex;
  }

  public String outputHash() {
    return outputHash;
  }

  public String associatedDataHash() {
    return associatedDataHash;
  }

  public long executedAtMs() {
    return executedAtMs;
  }

  public Long expiresAtMs() {
    return expiresAtMs;
  }

  public String backend() {
    return backend;
  }

  public String verificationMode() {
    return verificationMode;
  }

  public Map<String, Object> receipt() {
    return receipt;
  }
}
