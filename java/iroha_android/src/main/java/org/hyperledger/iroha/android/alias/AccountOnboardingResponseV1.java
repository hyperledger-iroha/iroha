package org.hyperledger.iroha.android.alias;

import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed result returned by sponsored onboarding apply. */
public final class AccountOnboardingResponseV1 {
  private final String accountId;
  private final String alias;
  private final String transactionHashHex;
  private final AccountOnboardingStatusV1 status;
  private final AliasSetupModels.AliasPlanDispositionV1 disposition;

  /** Constructs one exact apply response. */
  public AccountOnboardingResponseV1(
      final String accountId,
      final String alias,
      final String transactionHashHex,
      final AccountOnboardingStatusV1 status,
      final AliasSetupModels.AliasPlanDispositionV1 disposition) {
    if (status == null || disposition == null) {
      throw new IllegalArgumentException("status and disposition must not be null");
    }
    if (transactionHashHex != null
        && (!transactionHashHex.matches("[0-9a-f]{64}"))) {
      throw new IllegalArgumentException(
          "transactionHashHex must contain 64 lowercase hex characters");
    }
    this.accountId = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    this.alias = AccountAliasName.parse(alias).canonicalText();
    this.transactionHashHex = transactionHashHex;
    this.status = status;
    this.disposition = disposition;
  }

  public String accountId() { return accountId; }
  public String alias() { return alias; }
  public String transactionHashHex() { return transactionHashHex; }
  public AccountOnboardingStatusV1 status() { return status; }
  public AliasSetupModels.AliasPlanDispositionV1 disposition() { return disposition; }
}
