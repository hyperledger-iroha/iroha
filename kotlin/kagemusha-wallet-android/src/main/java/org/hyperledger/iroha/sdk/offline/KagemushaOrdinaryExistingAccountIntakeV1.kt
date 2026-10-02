// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Native creates this exact final continuation privately and lends it only to the measured
 * manifest Application's fixed protected-storage method. Its identity and single-use invocation
 * are checked in the active Native thread scope; constructing a lookalike supplies no authority.
 * No constructor, key getter, runtime setter or capability decoder is exposed.
 */
class KagemushaOrdinaryExistingAccountIntakeV1 private constructor() {
    /** Called only while the product's actual activation ownership gate holds its original.
     * Native drains the array immediately; this finalizer also drains every exceptional path.
     */
    fun consumeOriginal(signatoryAccountId: String, borrowedSeed: ByteArray) =
        consumeExistingAndroidAccountLoan(signatoryAccountId, borrowedSeed) { seed ->
            KagemushaOrdinaryRuntimeJniV1.consumeExistingAndroidAccount(this, signatoryAccountId, seed)
        }
}

/** Validation and wiping only; the Native-created continuation supplies the origin gate. */
internal fun consumeExistingAndroidAccountLoan(signatory: String, borrowedSeed: ByteArray,
    consume: (ByteArray) -> Boolean) {
    try {
        require(borrowedSeed.size == 32 && signatory.isNotEmpty() && signatory.length <= 512 &&
            signatory == signatory.trim() && signatory.none { it.isWhitespace() || it.isISOControl() })
        check(consume(borrowedSeed)) { "The actual protected-storage Native loan was refused" }
    } finally { borrowedSeed.fill(0) }
}
