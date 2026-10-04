// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Original committed execution coordinates; construction does not authenticate finality. */
class MusubiPinOutboxCheckFloorV1(
    @JvmField val height: BigInteger,
    blockHash: ByteArray,
    contextId: ByteArray,
) {
    private val blockHash = checkedPinOutboxBytes(blockHash, "floor block hash")
    private val contextId = checkedPinOutboxBytes(contextId, "floor context ID")

    init {
        MusubiValidationV1.requireU64(height, "pinOutbox.floor.height")
        require(height > BigInteger.ZERO) { "Musubi pin-outbox floor height must be positive" }
        require((this.contextId[31].toInt() and 1) == 1 &&
            !(this.contextId.take(31).all { it.toInt() == 0 } && this.contextId[31].toInt() == 1)) {
            "Musubi pin-outbox context ID must be a nonzero canonical hash"
        }
    }

    /** Exact original block hash, defensively copied. */
    fun blockHash(): ByteArray = blockHash.copyOf()

    /** Canonical native height context hash, defensively copied. */
    fun contextId(): ByteArray = contextId.copyOf()
}

/** Complete current row asserted by a native Check; this value alone grants no authority. */
class MusubiPinOutboxHighWaterV1(
    @JvmField val version: Int,
    @JvmField val networkId: NetworkId,
    @JvmField val pinAuthority: String,
    sessionId: ByteArray,
    @JvmField val revision: BigInteger,
    inventoryDigest: ByteArray,
    @JvmField val recordedAtHeight: BigInteger,
    transactionHash: ByteArray,
) {
    private val sessionId = checkedPinOutboxBytes(sessionId, "session ID")
    private val inventoryDigest = checkedPinOutboxBytes(inventoryDigest, "inventory digest")
    private val transactionHash = checkedPinOutboxBytes(transactionHash, "transaction hash")

    init {
        require(version == 1) { "Musubi pin-outbox high-water version must be one" }
        requireCanonicalI105Address(pinAuthority, "pinOutbox.pinAuthority")
        MusubiValidationV1.requireU64(revision, "pinOutbox.revision")
        MusubiValidationV1.requireU64(recordedAtHeight, "pinOutbox.recordedAtHeight")
        require(revision > BigInteger.ZERO && recordedAtHeight > BigInteger.ZERO) {
            "Musubi pin-outbox revision and execution height must be positive"
        }
    }

    /** Exact retained session, defensively copied. */
    fun sessionId(): ByteArray = sessionId.copyOf()

    /** Complete inventory commitment, defensively copied. */
    fun inventoryDigest(): ByteArray = inventoryDigest.copyOf()

    /** Exact original signed Advance identity, defensively copied. */
    fun transactionHash(): ByteArray = transactionHash.copyOf()
}

/** Closed authority-wide absence or exact current-row assertion. */
sealed class MusubiPinOutboxCheckExpectationV1 {
    /** No row may exist for the authority, even under another local session or inventory. */
    object Absent : MusubiPinOutboxCheckExpectationV1()

    /** Every field of the authority's current row must equal [row]. */
    class Present(@JvmField val row: MusubiPinOutboxHighWaterV1) : MusubiPinOutboxCheckExpectationV1()
}

internal fun checkedPinOutboxBytes(bytes: ByteArray, label: String): ByteArray {
    require(bytes.size == 32 && bytes.any { it.toInt() != 0 }) {
        "Musubi pin-outbox $label must be nonzero 32 bytes"
    }
    return bytes.copyOf()
}
