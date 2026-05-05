package org.hyperledger.iroha.android.client;

import java.util.List;

/** Successful response emitted by `POST /v1/contracts/deploy`. */
public final class ContractDeployResponse {
  private final boolean ok;
  private final String bundleName;
  private final String bundleDigest;
  private final String chainFingerprint;
  private final boolean dryRun;
  private final List<String> completedStages;
  private final String failurePoint;
  private final List<ContractReceipt> contracts;
  private final List<InitCallReceipt> initCalls;
  private final List<AssertionReceipt> assertions;

  public ContractDeployResponse(
      final boolean ok,
      final String bundleName,
      final String bundleDigest,
      final String chainFingerprint,
      final boolean dryRun,
      final List<String> completedStages,
      final String failurePoint,
      final List<ContractReceipt> contracts,
      final List<InitCallReceipt> initCalls,
      final List<AssertionReceipt> assertions) {
    this.ok = ok;
    this.bundleName = bundleName;
    this.bundleDigest = bundleDigest;
    this.chainFingerprint = chainFingerprint;
    this.dryRun = dryRun;
    this.completedStages = completedStages;
    this.failurePoint = failurePoint;
    this.contracts = contracts;
    this.initCalls = initCalls;
    this.assertions = assertions;
  }

  public boolean ok() { return ok; }
  public String bundleName() { return bundleName; }
  public String bundleDigest() { return bundleDigest; }
  public String chainFingerprint() { return chainFingerprint; }
  public boolean dryRun() { return dryRun; }
  public List<String> completedStages() { return completedStages; }
  public String failurePoint() { return failurePoint; }
  public List<ContractReceipt> contracts() { return contracts; }
  public List<InitCallReceipt> initCalls() { return initCalls; }
  public List<AssertionReceipt> assertions() { return assertions; }

  public static final class ContractReceipt {
    private final String name;
    private final String contractAlias;
    private final String contractAddress;
    private final String previousContractAddress;
    private final boolean upgraded;
    private final String dataspace;
    private final Long deployNonce;
    private final String txHashHex;
    private final String codeHashHex;
    private final String abiHashHex;
    private final String status;

    public ContractReceipt(
        final String name,
        final String contractAlias,
        final String contractAddress,
        final String previousContractAddress,
        final boolean upgraded,
        final String dataspace,
        final Long deployNonce,
        final String txHashHex,
        final String codeHashHex,
        final String abiHashHex,
        final String status) {
      this.name = name;
      this.contractAlias = contractAlias;
      this.contractAddress = contractAddress;
      this.previousContractAddress = previousContractAddress;
      this.upgraded = upgraded;
      this.dataspace = dataspace;
      this.deployNonce = deployNonce;
      this.txHashHex = txHashHex;
      this.codeHashHex = codeHashHex;
      this.abiHashHex = abiHashHex;
      this.status = status;
    }

    public String name() { return name; }
    public String contractAlias() { return contractAlias; }
    public String contractAddress() { return contractAddress; }
    public String previousContractAddress() { return previousContractAddress; }
    public boolean upgraded() { return upgraded; }
    public String dataspace() { return dataspace; }
    public Long deployNonce() { return deployNonce; }
    public String txHashHex() { return txHashHex; }
    public String codeHashHex() { return codeHashHex; }
    public String abiHashHex() { return abiHashHex; }
    public String status() { return status; }
  }

  public static final class InitCallReceipt {
    private final String id;
    private final String contractAlias;
    private final String entrypoint;
    private final String txHashHex;
    private final String status;

    public InitCallReceipt(
        final String id,
        final String contractAlias,
        final String entrypoint,
        final String txHashHex,
        final String status) {
      this.id = id;
      this.contractAlias = contractAlias;
      this.entrypoint = entrypoint;
      this.txHashHex = txHashHex;
      this.status = status;
    }

    public String id() { return id; }
    public String contractAlias() { return contractAlias; }
    public String entrypoint() { return entrypoint; }
    public String txHashHex() { return txHashHex; }
    public String status() { return status; }
  }

  public static final class AssertionReceipt {
    private final String id;
    private final String contractAlias;
    private final String entrypoint;
    private final String status;
    private final Object actualResult;
    private final Object expectedResult;
    private final String error;

    public AssertionReceipt(
        final String id,
        final String contractAlias,
        final String entrypoint,
        final String status,
        final Object actualResult,
        final Object expectedResult,
        final String error) {
      this.id = id;
      this.contractAlias = contractAlias;
      this.entrypoint = entrypoint;
      this.status = status;
      this.actualResult = actualResult;
      this.expectedResult = expectedResult;
      this.error = error;
    }

    public String id() { return id; }
    public String contractAlias() { return contractAlias; }
    public String entrypoint() { return entrypoint; }
    public String status() { return status; }
    public Object actualResult() { return actualResult; }
    public Object expectedResult() { return expectedResult; }
    public String error() { return error; }
  }
}
