package org.hyperledger.iroha.sdk.offline

/** Describes an impending or elapsed verdict deadline for UI consumption. */
class OfflineVerdictWarning(
    val certificateIdHex: String,
    val controllerId: String,
    val controllerDisplay: String,
    val verdictIdHex: String?,
    val deadlineKind: DeadlineKind,
    val deadlineMs: Long,
    val millisecondsRemaining: Long,
    val state: State,
    val headline: String,
    val details: String,
) {
    enum class DeadlineKind {
        REFRESH,
        POLICY,
        CERTIFICATE,
    }

    enum class State {
        WARNING,
        EXPIRED,
    }
}
