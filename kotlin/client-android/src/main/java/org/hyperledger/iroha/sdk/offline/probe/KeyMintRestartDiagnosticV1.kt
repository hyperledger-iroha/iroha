// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

/** Documented exhausted-key observations after an independent healthy StrongBox operation. */
internal enum class KeyMintRestartObservationV1 {
    ABSENT,
    PERMANENTLY_INVALIDATED,
}

/**
 * Diagnostic sequencing only; these observations do not establish hardware one-use enforcement.
 * The Android caller supplies a direct KeyPermanentlyInvalidatedException type check. Generic
 * provider, lookup, authentication, and signing failures must retain the first-use marker.
 */
internal fun <K> completeKeyMintRestartDiagnosticV1(
    verifyFreshControl: () -> Unit,
    readConsumedKey: () -> K?,
    signConsumedKey: (K) -> Unit,
    isPermanentlyInvalidated: (Exception) -> Boolean,
    removeCompletedMarker: () -> Unit,
): KeyMintRestartObservationV1 {
    // Control-key generation, signing, verification and cleanup must all finish first.
    verifyFreshControl()
    val observation = try {
        val key = readConsumedKey()
        if (key == null) {
            KeyMintRestartObservationV1.ABSENT
        } else {
            signConsumedKey(key)
            error("consumed one-use alias signed after reboot")
        }
    } catch (error: Exception) {
        if (!isPermanentlyInvalidated(error)) throw error
        KeyMintRestartObservationV1.PERMANENTLY_INVALIDATED
    }
    removeCompletedMarker()
    return observation
}
