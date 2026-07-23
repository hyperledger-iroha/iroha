package org.hyperledger.iroha.android.client;

import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Public normalized evidence returned for a contract operation. */
public final class ContractOperationReceipt {
  private final String operationKind;
  private final String status;
  private final String transport;
  private final String dataspace;
  private final String contractAlias;
  private final String contractAddress;
  private final String codeHashHex;
  private final String abiHashHex;
  private final String txHashHex;
  private final String entrypoint;
  private final String entrypointHashHex;
  private final Long gasLimit;
  private final Long gasUsed;
  private final FeePaymentIntent feePayment;
  private final String payloadDigestHex;

  public ContractOperationReceipt(
      final String operationKind,
      final String status,
      final String transport,
      final String dataspace,
      final String contractAlias,
      final String contractAddress,
      final String codeHashHex,
      final String abiHashHex,
      final String txHashHex,
      final String entrypoint,
      final String entrypointHashHex,
      final Long gasLimit,
      final Long gasUsed,
      final FeePaymentIntent feePayment,
      final String payloadDigestHex) {
    this.operationKind = operationKind;
    this.status = status;
    this.transport = transport;
    this.dataspace = dataspace;
    this.contractAlias = contractAlias;
    this.contractAddress = contractAddress;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
    this.txHashHex = txHashHex;
    this.entrypoint = entrypoint;
    this.entrypointHashHex = entrypointHashHex;
    this.gasLimit = gasLimit;
    this.gasUsed = gasUsed;
    this.feePayment = feePayment;
    this.payloadDigestHex = payloadDigestHex;
  }

  public String operationKind() { return operationKind; }
  public String status() { return status; }
  public String transport() { return transport; }
  public String dataspace() { return dataspace; }
  public String contractAlias() { return contractAlias; }
  public String contractAddress() { return contractAddress; }
  public String codeHashHex() { return codeHashHex; }
  public String abiHashHex() { return abiHashHex; }
  public String txHashHex() { return txHashHex; }
  public String entrypoint() { return entrypoint; }
  public String entrypointHashHex() { return entrypointHashHex; }
  public Long gasLimit() { return gasLimit; }
  public Long gasUsed() { return gasUsed; }
  public FeePaymentIntent feePayment() { return feePayment; }
  public String payloadDigestHex() { return payloadDigestHex; }
}
