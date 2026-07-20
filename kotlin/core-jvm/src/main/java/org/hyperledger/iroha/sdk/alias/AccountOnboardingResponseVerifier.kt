package org.hyperledger.iroha.sdk.alias

/** Receipt and HTTP binding for sponsored-onboarding apply responses. */
object AccountOnboardingResponseVerifier {
    /**
     * Requires an internally consistent response for the exact submitted receipt and HTTP status.
     *
     * Live classification may only move toward idempotent completion between planning and apply.
     */
    @JvmStatic
    fun requireValidForReceipt(
        receipt: AccountOnboardingPlanReceiptV1,
        response: AccountOnboardingResponseV1,
        httpStatus: Int,
    ): AccountOnboardingResponseV1 {
        require(
            response.accountId == receipt.body.request.accountId &&
                response.alias == receipt.body.request.alias,
        ) {
            "account onboarding response account or alias differs from the receipt"
        }
        require(
            dispositionTransitionAllowed(
                receipt.body.resource.disposition,
                response.disposition,
            ),
        ) {
            "account onboarding response disposition is not an allowed transition from the receipt"
        }
        when (response.status) {
            AccountOnboardingStatusV1.UNCHANGED -> require(httpStatus == HTTP_OK) {
                "Unchanged account onboarding response requires HTTP 200"
            }
            AccountOnboardingStatusV1.QUEUED,
            AccountOnboardingStatusV1.REPAIRED -> require(httpStatus == HTTP_ACCEPTED) {
                "Queued or Repaired account onboarding response requires HTTP 202"
            }
        }
        return response
    }

    private fun dispositionTransitionAllowed(
        planned: AliasPlanDispositionV1,
        live: AliasPlanDispositionV1,
    ): Boolean = when (planned) {
        AliasPlanDispositionV1.CREATE -> live == AliasPlanDispositionV1.CREATE ||
            live == AliasPlanDispositionV1.REPAIR || live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.REPAIR -> live == AliasPlanDispositionV1.REPAIR ||
            live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.NO_OP -> live == AliasPlanDispositionV1.NO_OP
        AliasPlanDispositionV1.CONFLICT -> false
    }

    private const val HTTP_OK = 200
    private const val HTTP_ACCEPTED = 202
}
