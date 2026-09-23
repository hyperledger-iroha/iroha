// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertIs
import org.hyperledger.iroha.sdk.offline.probe.KeyMintOneUsePreparationResultV1
import org.hyperledger.iroha.sdk.offline.probe.KeyMintOneUseSelectionResultV1
import org.junit.jupiter.api.Test

class KagemushaAndroidKeyMintEvidenceAdapterV1Test {
    private class CapturingCollector : KagemushaAndroidKeyMintEvidenceAdapterV1.Collector {
        var preparations = 0
        var collections = 0
        var preparedLane = byteArrayOf()
        var selectedFrame = byteArrayOf()
        var selectedKey = byteArrayOf()

        override fun prepare(
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
        ): KeyMintOneUsePreparationResultV1 {
            preparations += 1
            preparedLane = laneCommitment.copyOf()
            laneCommitment.fill(0)
            return KeyMintOneUsePreparationResultV1.Unavailable("test platform unavailable")
        }

        override fun collect(
            canonicalSelectionFrame: ByteArray,
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
            expectedCommittedPublicKey: ByteArray,
        ): KeyMintOneUseSelectionResultV1 {
            collections += 1
            selectedFrame = canonicalSelectionFrame.copyOf()
            selectedKey = expectedCommittedPublicKey.copyOf()
            canonicalSelectionFrame.fill(0)
            expectedCommittedPublicKey.fill(0)
            return KeyMintOneUseSelectionResultV1.Frozen("proof", "native ratchet proof not admitted")
        }
    }

    @Test fun preparationForwardsRawEvidenceRequestWithoutOpeningAMonetaryProvider() {
        val collector = CapturingCollector()
        val adapter = KagemushaAndroidKeyMintEvidenceAdapterV1(collector)
        val lane = ByteArray(32) { 4 }
        val result = adapter.prepare(lane, ByteArray(16), byteArrayOf(1) + ByteArray(15))
        assertIs<KeyMintOneUsePreparationResultV1.Unavailable>(result)
        assertEquals(1, collector.preparations)
        assertEquals(0, collector.collections)
        assertEquals(4.toByte(), lane[0])
        assertEquals(4.toByte(), collector.preparedLane[0])
    }

    @Test fun selectionForwardsExactCoreBytesAndCommittedKeyWithoutPromotingEvidence() {
        val collector = CapturingCollector()
        val adapter = KagemushaAndroidKeyMintEvidenceAdapterV1(collector)
        val frame = byteArrayOf(3, 5, 7)
        val expectedKey = byteArrayOf(0x04) + ByteArray(64) { 9 }
        val result = adapter.collect(frame, ByteArray(32) { 4 }, ByteArray(16),
            byteArrayOf(1) + ByteArray(15), expectedKey)
        assertIs<KeyMintOneUseSelectionResultV1.Frozen>(result)
        assertEquals(1, collector.collections)
        assertEquals(0, collector.preparations)
        assertEquals(3.toByte(), collector.selectedFrame[0])
        assertEquals(9.toByte(), collector.selectedKey[1])
        assertEquals(3.toByte(), frame[0])
        assertEquals(9.toByte(), expectedKey[1])
    }
}
