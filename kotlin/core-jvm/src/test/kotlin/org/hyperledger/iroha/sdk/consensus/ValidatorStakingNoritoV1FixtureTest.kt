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
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
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
        assertEquals(4, transition.preparation.committee.size)
        val policy = transition.preparation.eligibility
        assertEquals(ValidatorStakingNoritoV1.ElectionPolicy.AssetScope.GLOBAL, policy.assetScope)
        assertEquals(9L, policy.assetScale)
        assertEquals(100L, policy.epochLengthBlocks)
        assertTrue(policy.minSelfBond.mantissa.signum() > 0)
        assertTrue(policy.minNominationBond.mantissa.signum() > 0)
        assertTrue(policy.maxValidators in 4L..31L)
        assertContentEquals(policy.encode(), ValidatorStakingNoritoV1.ElectionPolicy.decode(policy.encode()).encode())
        for (member in transition.preparation.committee) {
            assertEquals(48, member.blsPublicKey.bytes().size)
            assertEquals(96, member.proofOfPossession.bytes().size)
            assertContentEquals(member.encode(), ValidatorStakingNoritoV1.CommitteeMember.decode(member.encode()).encode())
        }
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

    @Test
    fun `election policy rejects synthetic scoped imprecise and out of bounds stake`() {
        val fields = fields(preparationFields()[11])
        val synthetic = "5ecd1e80ac7d4d18b22772091a73fc13"
            .chunked(2).map { byteArrayOf(it.toInt(16).toByte()) }
        val invalid = listOf(
            0 to record(synthetic),
            0 to record(List(16) { byteArrayOf(0) }),
            1 to (uint(1, 32) + record(listOf(uint(7, 64)))),
            1 to (uint(0, 32) + byteArrayOf(0)),
            2 to uint(8, 32), 2 to uint(10, 32),
            3 to quantity(byteArrayOf(), 0), 4 to quantity(byteArrayOf(), 0),
            3 to quantity(byteArrayOf(1), 10), 4 to quantity(byteArrayOf(1), 10),
            5 to uint(3, 32), 5 to uint(5, 32), 5 to uint(34, 32),
            6 to uint(0, 64), 6 to uint(2, 64),
        )
        for ((index, value) in invalid) {
            assertFails("policy field $index accepted invalid eligibility") {
                ValidatorStakingNoritoV1.ElectionPolicy.decode(replacing(fields, index, value))
            }
        }
        for (maximum in listOf(4L, 7L, 31L)) {
            assertEquals(maximum, ValidatorStakingNoritoV1.ElectionPolicy.decode(
                replacing(fields, 5, uint(maximum, 32))).maxValidators)
        }
        assertEquals(-1L, ValidatorStakingNoritoV1.ElectionPolicy.decode(
            replacing(fields, 6, uint(-1L, 64))).epochLengthBlocks)
        for (index in listOf(3, 4)) {
            val exact = ValidatorStakingNoritoV1.ElectionPolicy.decode(
                replacing(fields, index, quantity(byteArrayOf(1), 9)))
            assertEquals(9L, if (index == 3) exact.minSelfBond.scale else exact.minNominationBond.scale)
        }
        assertFails { ValidatorStakingNoritoV1.ElectionPolicy.decode(record(fields.dropLast(1))) }
        assertFails { ValidatorStakingNoritoV1.ElectionPolicy.decode(record(fields + listOf(uint(1, 32)))) }
    }

    @Test
    fun `preparation rejects retired layout malformed members and scheduling overflow`() {
        val preparation = preparationFields()
        val members = vectorFields(preparation[12])
        val retiredRoster = members.map { record(listOf(fields(it)[0], uint(1, 64))) }
        val retiredPops = members.map { fields(it)[1] }
        val retired = preparation.toMutableList().apply {
            this[11] = vector(retiredRoster)
            this[12] = vector(retiredPops)
        }
        assertFails { ValidatorStakingNoritoV1.CommitteePreparation.decode(record(retired)) }

        val invalid = listOf(
            0 to uint(2, 16), 1 to ByteArray(32), 3 to uint(0, 64),
            4 to ByteArray(32), 5 to uint(3, 64), 6 to uint(101, 64),
            7 to uint(200, 64), 7 to uint(301, 64), 8 to uint(0, 64),
            9 to ByteArray(32), 10 to ByteArray(32),
            12 to vector(members.take(3)), 12 to vector(members + members.first()),
            12 to vector(members.reversed()),
            12 to vector(members.toMutableList().apply { this[1] = this[0] }),
            12 to vector(List(32) { members.first() }),
        )
        for ((index, value) in invalid) {
            assertFails("preparation field $index accepted invalid binding") {
                ValidatorStakingNoritoV1.CommitteePreparation.decode(replacing(preparation, index, value))
            }
        }
        val overflow = preparation.toMutableList().apply {
            this[2] = uint(-1L, 64)
            this[5] = uint(1, 64)
        }
        assertFails { ValidatorStakingNoritoV1.CommitteePreparation.decode(record(overflow)) }
        val heightOverflow = preparation.toMutableList().apply {
            this[3] = uint(-1L, 64)
            this[6] = uint(1, 64)
            this[7] = uint(100, 64)
        }
        assertFails { ValidatorStakingNoritoV1.CommitteePreparation.decode(record(heightOverflow)) }

        val member = fields(members.first())
        for (length in listOf(0, 95, 97)) {
            assertFails { ValidatorStakingNoritoV1.CommitteeMember.decode(
                replacing(member, 1, uint(length.toLong(), 64) + ByteArray(length))) }
        }
        val key = vectorFields(fields(member[0]).single()).toMutableList()
        key[0] = byteArrayOf(0)
        assertFails { ValidatorStakingNoritoV1.CommitteeMember.decode(
            replacing(member, 0, record(listOf(vector(key))))) }
        assertFails { ValidatorStakingNoritoV1.CommitteeMember.decode(record(member.dropLast(1))) }
    }

    @Test
    fun `preparation retains unsigned heights and owns immutable original bytes`() {
        val high = preparationFields().toMutableList().apply {
            this[2] = uint(-3L, 64)
            this[5] = uint(-1L, 64)
            this[3] = uint(-110L, 64)
            this[6] = uint(-101L, 64)
            this[7] = uint(-2L, 64)
        }
        val bytes = record(high)
        val original = bytes.copyOf()
        val decoded = ValidatorStakingNoritoV1.CommitteePreparation.decode(bytes)
        assertEquals(-1L, decoded.targetEpoch)
        assertEquals(-2L, decoded.lastHeight)
        bytes.fill(0)
        decoded.committee.first().validator.bytes().fill(0)
        decoded.eligibility.xorAssetDefinitionId.bytes().fill(0)
        (decoded.committee as MutableList<*>).clear()
        assertEquals(4, decoded.committee.size)
        assertContentEquals(original, decoded.encode())
    }

    @Test
    fun `authorization and DKG preserve full unsigned height order`() {
        val rows = fixtureRows()
        val authorization = fields(rows.getValue("epoch_authorization")).toMutableList().apply {
            this[3] = uint(Long.MAX_VALUE, 64)
            this[4] = uint(Long.MIN_VALUE, 64)
        }
        assertEquals(Long.MIN_VALUE, ValidatorStakingNoritoV1.EpochAuthorization.decode(record(authorization)).lastHeight)
        assertFails { ValidatorStakingNoritoV1.EpochAuthorization.decode(
            replacing(authorization, 4, uint(Long.MAX_VALUE - 1, 64))) }
        val session = fields(rows.getValue("dkg_session")).toMutableList().apply {
            this[8] = uint(Long.MAX_VALUE - 1, 64)
            this[9] = uint(Long.MAX_VALUE, 64)
            this[10] = uint(Long.MIN_VALUE, 64)
            this[11] = uint(-1L, 64)
        }
        assertEquals(-1L, ValidatorStakingNoritoV1.DkgSession.decode(record(session)).acceptancesEndHeight)
        assertFails { ValidatorStakingNoritoV1.DkgSession.decode(replacing(session, 8, uint(0, 64))) }
        assertFails { ValidatorStakingNoritoV1.DkgSession.decode(replacing(session, 11, uint(Long.MAX_VALUE, 64))) }
        assertFails { ValidatorStakingNoritoV1.DkgSession.decode(replacing(session, 6, uint(34, 16))) }
    }

    private fun preparationFields(): List<ByteArray> =
        fields(fields(fixtureRows().getValue("committee_transition"))[0])

    private fun fields(payload: ByteArray): List<ByteArray> {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val values = mutableListOf<ByteArray>()
        while (decoder.remaining() > 0) {
            values += decoder.readBytes(decoder.readLength(true).toInt())
        }
        return values
    }

    private fun vectorFields(payload: ByteArray): List<ByteArray> {
        val decoder = NoritoDecoder(payload, NoritoHeader.COMPACT_LEN)
        val count = decoder.readUInt(64).toInt()
        val values = List(count) { decoder.readBytes(decoder.readLength(true).toInt()) }
        assertEquals(0, decoder.remaining())
        return values
    }

    private fun record(fields: List<ByteArray>): ByteArray =
        NoritoEncoder(NoritoHeader.COMPACT_LEN).apply {
            fields.forEach { writeLength(it.size.toLong(), true); writeBytes(it) }
        }.toByteArray()

    private fun replacing(fields: List<ByteArray>, index: Int, value: ByteArray): ByteArray =
        record(fields.toMutableList().apply { this[index] = value })

    private fun vector(fields: List<ByteArray>): ByteArray = uint(fields.size.toLong(), 64) + record(fields)

    private fun uint(value: Long, bits: Int): ByteArray =
        NoritoEncoder(NoritoHeader.COMPACT_LEN).apply { writeUInt(value, bits) }.toByteArray()

    private fun quantity(mantissa: ByteArray, scale: Long): ByteArray =
        record(listOf(uint(mantissa.size.toLong(), 32) + mantissa, uint(scale, 32)))

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
