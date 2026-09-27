// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertNotNull
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader

/** Rust-authored canonical DTO bytes, decoded by the Kotlin implementation. */
class ValidatorStakingNoritoV1FixtureTest {
    private val expectedKinds = setOf(
        "authority_generation", "epoch_authorization", "dkg_session", "dkg_transcript",
        "committee_transition", "monetary_plan", "rebind_peer",
    )

    @Test
    fun `first release validator and staking records match Rust bytes`() {
        val rows = fixtureRows()
        assertEquals(expectedKinds, rows.keys)

        val authority = ValidatorStakingNoritoV1.AuthorityGeneration.decode(rows.getValue("authority_generation"))
        assertEquals(0L, authority.generation)
        assertEquals(4, authority.validators.size)
        assertContentEquals(rows.getValue("authority_generation"), authority.encode())

        val epoch = ValidatorStakingNoritoV1.EpochAuthorization.decode(rows.getValue("epoch_authorization"))
        assertEquals(ValidatorStakingNoritoV1.EpochAuthorization.Decision.GENESIS, epoch.decision)
        assertEquals(0L, epoch.authorityGeneration)
        assertContentEquals(rows.getValue("epoch_authorization"), epoch.encode())

        val session = ValidatorStakingNoritoV1.DkgSession.decode(rows.getValue("dkg_session"))
        assertEquals(4, session.committeeSize)
        assertEquals(2, session.threshold)
        assertEquals(1L, session.authorityGeneration)
        assertContentEquals(rows.getValue("dkg_session"), session.encode())

        val transcript = ValidatorStakingNoritoV1.DkgTranscript.decode(rows.getValue("dkg_transcript"))
        assertEquals(4, transcript.recipientKeys.size)
        assertEquals(4, transcript.dealerCommitments.size)
        assertEquals(16, transcript.encryptedShares.size)
        assertEquals(16, transcript.shareAcceptances.size)
        assertEquals(131L, transcript.finalizedAtHeight)
        assertContentEquals(rows.getValue("dkg_transcript"), transcript.encode())

        val transition = ValidatorStakingNoritoV1.CommitteeTransition.decode(rows.getValue("committee_transition"))
        assertEquals(2L, transition.preparation.targetEpoch)
        assertEquals(4, transition.preparation.roster.size)
        assertEquals(1L, assertNotNull(transition.credentials).authority.generation)
        assertEquals(ValidatorStakingNoritoV1.EpochAuthorization.Decision.ACTIVATE, assertNotNull(transition.outcome).decision)
        assertContentEquals(rows.getValue("committee_transition"), transition.encode())

        val plan = ValidatorStakingNoritoV1.MonetaryPlan.decode(rows.getValue("monetary_plan"))
        assertNotNull(plan.networkScope)
        assertEquals(BigInteger.valueOf(1000L), plan.amount.mantissa)
        assertEquals(ValidatorStakingNoritoV1.MonetaryPrecondition.Kind.REGISTRATION, plan.precondition.kind)
        assertEquals(201L, plan.precondition.activationHeight)
        assertEquals(plan.sourceAsset.definition, plan.destinationAsset.definition)
        assertContentEquals(rows.getValue("monetary_plan"), plan.encode())

        val rebind = ValidatorStakingNoritoV1.RebindPeer.decode(rows.getValue("rebind_peer"))
        assertEquals(0L, rebind.laneId)
        assertContentEquals(rows.getValue("rebind_peer"), rebind.encode())
    }

    @Test
    fun `truncated canonical records fail closed`() {
        val rows = fixtureRows()
        val decoders: Map<String, (ByteArray) -> Any> = mapOf(
            "authority_generation" to ValidatorStakingNoritoV1.AuthorityGeneration::decode,
            "epoch_authorization" to ValidatorStakingNoritoV1.EpochAuthorization::decode,
            "dkg_session" to ValidatorStakingNoritoV1.DkgSession::decode,
            "dkg_transcript" to ValidatorStakingNoritoV1.DkgTranscript::decode,
            "committee_transition" to ValidatorStakingNoritoV1.CommitteeTransition::decode,
            "monetary_plan" to ValidatorStakingNoritoV1.MonetaryPlan::decode,
            "rebind_peer" to ValidatorStakingNoritoV1.RebindPeer::decode,
        )
        for ((kind, bytes) in rows) {
            assertFails("$kind accepted a truncated record") {
                decoders.getValue(kind)(bytes.copyOf(bytes.size - 1))
            }
        }
    }

    @Test
    fun `quantity rejects noncanonical decimals and retains unsigned positive mantissa`() {
        fun quantity(mantissa: ByteArray, scale: Long): ByteArray {
            val mantissaField = NoritoEncoder(NoritoHeader.COMPACT_LEN).apply {
                writeUInt(mantissa.size.toLong(), 32)
                writeBytes(mantissa)
            }.toByteArray()
            val scaleField = NoritoEncoder(NoritoHeader.COMPACT_LEN).apply {
                writeUInt(scale, 32)
            }.toByteArray()
            return NoritoEncoder(NoritoHeader.COMPACT_LEN).apply {
                writeLength(mantissaField.size.toLong(), true)
                writeBytes(mantissaField)
                writeLength(scaleField.size.toLong(), true)
                writeBytes(scaleField)
            }.toByteArray()
        }

        assertEquals(BigInteger.valueOf(200),
            ValidatorStakingNoritoV1.Quantity.decode(quantity(byteArrayOf(0xC8.toByte(), 0), 0)).mantissa)
        assertFails { ValidatorStakingNoritoV1.Quantity.decode(quantity(byteArrayOf(10), 1)) }
        assertFails { ValidatorStakingNoritoV1.Quantity.decode(quantity(byteArrayOf(), 1)) }
        assertFails { ValidatorStakingNoritoV1.Quantity.decode(quantity(byteArrayOf(1), 29)) }
    }

    private fun fixtureRows(): Map<String, ByteArray> {
        val rows = linkedMapOf<String, ByteArray>()
        for (line in Files.readAllLines(fixturePath())) {
            if (line.isBlank() || line.startsWith("#")) continue
            val fields = line.split('\t')
            require(fields.size == 2 && fields[0] in expectedKinds && fields[0] !in rows) {
                "invalid validator-staking fixture row: $line"
            }
            rows[fields[0]] = fields[1].chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        }
        return rows
    }

    private fun fixturePath(): Path {
        var directory = Paths.get("").toAbsolutePath()
        while (directory.parent != null) {
            val candidate = directory.resolve("fixtures/validator_staking/norito_v1.tsv")
            if (Files.exists(candidate)) return candidate
            directory = directory.parent
        }
        error("fixtures/validator_staking/norito_v1.tsv was not found")
    }
}
