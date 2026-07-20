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
    final String canonicalAlias = AccountAliasName.parse(alias).canonicalText();
    if (!canonicalAlias.equals(alias)) {
      throw new IllegalArgumentException("alias must be canonical");
    }
    this.alias = canonicalAlias;
    this.transactionHashHex = transactionHashHex;
    this.status = status;
    this.disposition = disposition;
    switch (status) {
      case UNCHANGED:
        if (transactionHashHex != null
            || disposition != AliasSetupModels.AliasPlanDispositionV1.NO_OP) {
          throw new IllegalArgumentException(
              "Unchanged onboarding must omit transactionHashHex and report no-op");
        }
        break;
      case QUEUED:
        if (transactionHashHex == null
            || disposition != AliasSetupModels.AliasPlanDispositionV1.CREATE) {
          throw new IllegalArgumentException(
              "Queued onboarding must carry transactionHashHex and report create");
        }
        break;
      case REPAIRED:
        if (transactionHashHex == null
            || (disposition != AliasSetupModels.AliasPlanDispositionV1.REPAIR
                && disposition != AliasSetupModels.AliasPlanDispositionV1.NO_OP)) {
          throw new IllegalArgumentException(
              "Repaired onboarding must carry transactionHashHex and report repair or no-op");
        }
        break;
      default:
        throw new IllegalArgumentException("unsupported account onboarding status");
    }
  }

  public String accountId() { return accountId; }
  public String alias() { return alias; }
  public String transactionHashHex() { return transactionHashHex; }
  public AccountOnboardingStatusV1 status() { return status; }
  public AliasSetupModels.AliasPlanDispositionV1 disposition() { return disposition; }
}
