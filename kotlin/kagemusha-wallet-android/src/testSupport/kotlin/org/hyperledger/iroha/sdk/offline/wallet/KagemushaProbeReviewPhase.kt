// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Test-driver bookkeeping only. Native capability/source checks remain mandatory. */
internal class KagemushaProbeReviewPhase {
    enum class State { AWAITING_CONFIRMATION, CONFIRMING, EXECUTING, APPROVED_LIVE_RETAINED, FINISHED }
    var state = State.AWAITING_CONFIRMATION
        private set
    fun beginConfirmation() { check(state == State.AWAITING_CONFIRMATION); state = State.CONFIRMING }
    fun confirmationFailed() { check(state == State.CONFIRMING); state = State.AWAITING_CONFIRMATION }
    /** A lost publication acknowledgment never grants an in-memory retry. */
    fun publishFence(publish: () -> Unit) {
        check(state == State.CONFIRMING)
        state = State.FINISHED
        publish()
        state = State.EXECUTING
    }
    /** Call only after the exact live review is restored by typed65. */
    fun retained() { check(state == State.EXECUTING); state = State.APPROVED_LIVE_RETAINED }
    fun retry(requireExactAcknowledgedFence: () -> Unit) {
        check(state == State.APPROVED_LIVE_RETAINED)
        state = State.FINISHED
        requireExactAcknowledgedFence()
        state = State.EXECUTING
    }
    fun finish() { state = State.FINISHED }
    fun endRequested() { if (state == State.APPROVED_LIVE_RETAINED) state = State.FINISHED }
}
