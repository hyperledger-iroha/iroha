// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import org.hyperledger.iroha.sdk.offline.probe.AndroidKeyMintOneUseSelectionCandidateV1
import org.hyperledger.iroha.sdk.offline.probe.KeyMintOneUsePreparationResultV1
import org.hyperledger.iroha.sdk.offline.probe.KeyMintOneUseSelectionResultV1

/**
 * Ordinary-app KeyMint evidence ingress for the KAGEMUSHA wallet.
 *
 * [prepare] supplies an attested SEC1 key for governance/Core to commit before a transition is
 * constructed. [collect] consumes the exact canonical selection frame produced by Core and the
 * public key already committed to that predecessor. Its result is raw evidence only. The existing
 * monetary coordinator has no KeyMint ratchet-proof method; this adapter must not feed its DER
 * signature into `acceptAuthenticatedDeviceReply` or open a hardware wallet from these bytes.
 */
class KagemushaAndroidKeyMintEvidenceAdapterV1 internal constructor(
    private val collector: Collector,
) {
    /** Use the application's private no-backup journal and AndroidKeyStore. */
    constructor(context: Context) : this(SystemCollector(context.applicationContext ?: context))

    /** Prepare and durably retain one-use key evidence before Core commits its public key. */
    fun prepare(
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ): KeyMintOneUsePreparationResultV1 = collector.prepare(
        laneCommitment.copyOf(),
        secureIndexBeforeLittleEndian.copyOf(),
        secureIndexAfterLittleEndian.copyOf(),
    )

    /** Sign only a Core selection using the previously governed, exact committed SEC1 key. */
    fun collect(
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
        expectedCommittedPublicKey: ByteArray,
    ): KeyMintOneUseSelectionResultV1 = collector.collect(
        canonicalSelectionFrame.copyOf(),
        laneCommitment.copyOf(),
        secureIndexBeforeLittleEndian.copyOf(),
        secureIndexAfterLittleEndian.copyOf(),
        expectedCommittedPublicKey.copyOf(),
    )

    internal interface Collector {
        fun prepare(
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
        ): KeyMintOneUsePreparationResultV1

        fun collect(
            canonicalSelectionFrame: ByteArray,
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
            expectedCommittedPublicKey: ByteArray,
        ): KeyMintOneUseSelectionResultV1
    }

    private class SystemCollector(private val context: Context) : Collector {
        override fun prepare(
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
        ): KeyMintOneUsePreparationResultV1 = AndroidKeyMintOneUseSelectionCandidateV1.prepare(
            context, laneCommitment, secureIndexBeforeLittleEndian, secureIndexAfterLittleEndian,
        )

        override fun collect(
            canonicalSelectionFrame: ByteArray,
            laneCommitment: ByteArray,
            secureIndexBeforeLittleEndian: ByteArray,
            secureIndexAfterLittleEndian: ByteArray,
            expectedCommittedPublicKey: ByteArray,
        ): KeyMintOneUseSelectionResultV1 = AndroidKeyMintOneUseSelectionCandidateV1.collect(
            context, canonicalSelectionFrame, laneCommitment, secureIndexBeforeLittleEndian,
            secureIndexAfterLittleEndian, expectedCommittedPublicKey,
        )
    }
}
