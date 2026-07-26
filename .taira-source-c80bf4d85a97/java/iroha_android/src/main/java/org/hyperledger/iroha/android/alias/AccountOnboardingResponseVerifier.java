package org.hyperledger.iroha.android.alias;

import java.util.Objects;

/** Receipt and HTTP binding for sponsored-onboarding apply responses. */
public final class AccountOnboardingResponseVerifier {
  private static final int HTTP_OK = 200;
  private static final int HTTP_ACCEPTED = 202;

  private AccountOnboardingResponseVerifier() {}

  /**
   * Requires an internally consistent response for the exact submitted receipt and HTTP status.
   *
   * <p>Live classification may only move toward idempotent completion between planning and apply.
   */
  public static AccountOnboardingResponseV1 requireValidForReceipt(
      final AccountOnboardingPlanReceiptV1 receipt,
      final AccountOnboardingResponseV1 response,
      final int httpStatus) {
    final AccountOnboardingPlanReceiptV1 exactReceipt =
        Objects.requireNonNull(receipt, "receipt");
    final AccountOnboardingResponseV1 exactResponse =
        Objects.requireNonNull(response, "response");
    if (!exactResponse.accountId().equals(exactReceipt.body().request().accountId())
        || !exactResponse.alias().equals(exactReceipt.body().request().alias())) {
      throw new IllegalArgumentException(
          "account onboarding response account or alias differs from the receipt");
    }
    if (!dispositionTransitionAllowed(
        exactReceipt.body().resource().disposition(), exactResponse.disposition())) {
      throw new IllegalArgumentException(
          "account onboarding response disposition is not an allowed transition from the receipt");
    }
    switch (exactResponse.status()) {
      case UNCHANGED:
        if (httpStatus != HTTP_OK) {
          throw new IllegalArgumentException(
              "Unchanged account onboarding response requires HTTP 200");
        }
        break;
      case QUEUED:
      case REPAIRED:
        if (httpStatus != HTTP_ACCEPTED) {
          throw new IllegalArgumentException(
              "Queued or Repaired account onboarding response requires HTTP 202");
        }
        break;
      default:
        throw new IllegalArgumentException("unsupported account onboarding status");
    }
    return exactResponse;
  }

  private static boolean dispositionTransitionAllowed(
      final AliasSetupModels.AliasPlanDispositionV1 planned,
      final AliasSetupModels.AliasPlanDispositionV1 live) {
    switch (planned) {
      case CREATE:
        return live == AliasSetupModels.AliasPlanDispositionV1.CREATE
            || live == AliasSetupModels.AliasPlanDispositionV1.REPAIR
            || live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case REPAIR:
        return live == AliasSetupModels.AliasPlanDispositionV1.REPAIR
            || live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case NO_OP:
        return live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case CONFLICT:
      default:
        return false;
    }
  }
}
