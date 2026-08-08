package org.hyperledger.iroha.android.client;

import java.util.Map;

/** Successful response emitted by `POST /v1/contracts/call`. */
public final class ContractCallResponse {
  private final boolean ok;
  private final boolean submitted;
  private final String dataspace;
  private final String codeHashHex;
  private final String abiHashHex;
  private final long creationTimeMs;
  private final String contractAddress;
  private final String txHashHex;
  private final Map<String, Object> pipelineStatus;
  private final String entrypoint;
  private final Long transactionTtlMs;
  private final String entrypointHashHex;
  private final String transactionPayloadB64;
  private final String signingMessageB64;
  private final ContractOperationReceipt operationReceipt;

  public ContractCallResponse(
      final boolean ok,
      final boolean submitted,
      final String dataspace,
      final String codeHashHex,
      final String abiHashHex,
      final long creationTimeMs,
      final String contractAddress,
      final String txHashHex,
      final Map<String, Object> pipelineStatus,
      final String entrypoint,
      final Long transactionTtlMs,
      final String entrypointHashHex,
      final String transactionPayloadB64,
      final String signingMessageB64,
      final ContractOperationReceipt operationReceipt) {
    this.ok = ok;
    this.submitted = submitted;
    this.dataspace = dataspace;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
    this.creationTimeMs = creationTimeMs;
    this.contractAddress = contractAddress;
    this.txHashHex = txHashHex;
    this.pipelineStatus = pipelineStatus;
    this.entrypoint = entrypoint;
    this.transactionTtlMs = transactionTtlMs;
    this.entrypointHashHex = entrypointHashHex;
    this.transactionPayloadB64 = transactionPayloadB64;
    this.signingMessageB64 = signingMessageB64;
    this.operationReceipt = operationReceipt;
  }

  public boolean ok() { return ok; }
  public boolean submitted() { return submitted; }
  public String dataspace() { return dataspace; }
  public String codeHashHex() { return codeHashHex; }
  public String abiHashHex() { return abiHashHex; }
  public long creationTimeMs() { return creationTimeMs; }
  public String contractAddress() { return contractAddress; }
  public String txHashHex() { return txHashHex; }
  public Map<String, Object> pipelineStatus() { return pipelineStatus; }
  public String entrypoint() { return entrypoint; }
  public Long transactionTtlMs() { return transactionTtlMs; }
  public String entrypointHashHex() { return entrypointHashHex; }
  public String transactionPayloadB64() { return transactionPayloadB64; }
  public String signingMessageB64() { return signingMessageB64; }
  public ContractOperationReceipt operationReceipt() { return operationReceipt; }
}
