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
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.assertIs
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader

/** Rust-authored canonical DTO bytes, decoded by the Kotlin implementation. */
class ValidatorStakingNoritoV1FixtureTest {
    private val expectedKinds = setOf(
        "authority_generation", "epoch_authorization", "dkg_session", "dkg_transcript",
        "committee_transition", "monetary_plan", "monetary_bond_plan", "monetary_unbond_plan",
        "monetary_slash_plan", "reward_claim_plan", "fee_reward_claim_plan", "rebind_peer",
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

        for (kind in listOf("reward_claim_plan", "fee_reward_claim_plan")) {
            val claim = ValidatorStakingNoritoV1.RewardClaimPlan.decode(rows.getValue(kind))
            assertNotNull(claim.networkScope)
            assertEquals(210L, claim.validUntilHeight)
            assertEquals(200L, claim.expectedState?.throughEpoch)
            assertEquals(1, claim.records.size)
            assertEquals(201L, claim.records.single().epoch)
            assertEquals(32, claim.records.single().recordHash.bytes().size)
            assertEquals(1, claim.sources.size)
            assertEquals(BigInteger.valueOf(5), claim.sources.single().expectedAccrued?.mantissa)
            assertEquals(BigInteger.valueOf(15), claim.sources.single().payout.mantissa)
            assertContentEquals(plan.destinationAsset.encode(), claim.sources.single().sourceAsset.encode())
            assertContentEquals(plan.sourceAsset.encode(), claim.sources.single().destinationAsset.encode())
            assertContentEquals(rows.getValue(kind), claim.encode())
            if (kind == "reward_claim_plan") {
                assertNull(claim.feeClaim)
            } else {
                val fee = assertNotNull(claim.feeClaim)
                assertContentEquals(ByteArray(32) { 0x77 }, fee.lifecycleSeal.bytes())
                assertEquals(plan.sourceAsset.account, fee.beneficiaryId)
                assertEquals(4L, fee.beneficiaryRevision)
                assertContentEquals(plan.destinationAsset.encode(), fee.sourceAsset.encode())
                assertContentEquals(plan.sourceAsset.encode(), fee.destinationAsset.encode())
                assertEquals(BigInteger.valueOf(7), fee.amount.mantissa)
                assertEquals(5L, fee.expectedClaimSequence)
                assertContentEquals(fee.encode(), ValidatorStakingNoritoV1.FeeRewardClaim.decode(fee.encode()).encode())
            }
        }

        val rebind = ValidatorStakingNoritoV1.RebindPeer.decode(rows.getValue("rebind_peer"))
        assertEquals(0L, rebind.laneId)
        assertContentEquals(rows.getValue("rebind_peer"), rebind.encode())
    }

    @Test
    fun monetaryOperationFixturesBindExactTypedFieldsAndNetworkXor() {
        val rows = fixtureRows()
        val registration = ValidatorStakingNoritoV1.MonetaryPlan.decode(rows.getValue("monetary_plan"))
        val transition = ValidatorStakingNoritoV1.CommitteeTransition.decode(rows.getValue("committee_transition"))
        val bond = ValidatorStakingNoritoV1.MonetaryPlan.decode(rows.getValue("monetary_bond_plan"))
        val unbond = ValidatorStakingNoritoV1.MonetaryPlan.decode(rows.getValue("monetary_unbond_plan"))
        val slash = ValidatorStakingNoritoV1.MonetaryPlan.decode(rows.getValue("monetary_slash_plan"))
        val bondBinding = assertIs<ValidatorStakingNoritoV1.MonetaryPrecondition.Bond>(bond.precondition).bond
        val unbondBinding = assertIs<ValidatorStakingNoritoV1.MonetaryPrecondition.Unbond>(unbond.precondition).unbond
        val slashBinding = assertIs<ValidatorStakingNoritoV1.MonetaryPrecondition.Slash>(slash.precondition).slash
        assertEquals(SigningAlgorithm.BLS_NORMAL, bondBinding.peerId.algorithm)
        assertContentEquals(transition.preparation.committee[0].validator.bytes(), bondBinding.peerId.encode())
        assertEquals(transition.preparation.committee[0].blsPublicKey, bondBinding.peerId.publicKey)
        assertContentEquals(ByteArray(32) { 0x75 }, unbondBinding.requestHash.bytes())
        assertEquals(BigInteger.valueOf(1500), slashBinding.slashableExposure.mantissa)
        assertEquals(0L, slashBinding.slashableExposure.scale)
        for ((name, plan) in listOf("monetary_bond_plan" to bond, "monetary_unbond_plan" to unbond, "monetary_slash_plan" to slash)) {
            assertEquals(transition.preparation.networkId, plan.networkScope)
            assertEquals(210L, plan.validUntilHeight)
            assertEquals(registration.amount.mantissa, plan.amount.mantissa)
            assertEquals(0L, plan.amount.scale)
            assertEquals(201L, plan.precondition.activationHeight)
            for (asset in listOf(plan.sourceAsset, plan.destinationAsset)) {
                assertEquals(transition.preparation.eligibility.xorAssetDefinitionId, asset.definition)
                assertNull(asset.scopeDataspace)
            }
            val deposits = plan.precondition.kind == ValidatorStakingNoritoV1.MonetaryPrecondition.Kind.BOND
            assertContentEquals((if (deposits) registration.sourceAsset else registration.destinationAsset).encode(), plan.sourceAsset.encode())
            assertContentEquals((if (deposits) registration.destinationAsset else registration.sourceAsset).encode(), plan.destinationAsset.encode())
            assertContentEquals(rows.getValue(name), plan.encode())
        }
    }

    @Test
    fun monetaryBindingsRejectMalformedPeerHashAndExposure() {
        val rows = fixtureRows()
        fun assertRejected(kind: String, tag: Long, body: List<ByteArray>) {
            val original = fields(rows.getValue(kind))
            val variant = uint(tag, 32) + record(listOf(record(body)))
            assertFails { ValidatorStakingNoritoV1.MonetaryPrecondition.decode(variant) }
            assertFails { ValidatorStakingNoritoV1.MonetaryPlan.decode(replacing(original, 5, variant)) }
        }
        val variant = fields(rows.getValue("monetary_bond_plan"))[5]
        val peer = fields(fields(variant.copyOfRange(4, variant.size))[0])[1]
        val key = vectorFields(fields(peer)[0])
        val badPeers = listOf(
            byteArrayOf(), record(emptyList()), record(listOf(byteArrayOf())), record(listOf(vector(emptyList()))),
            record(listOf(vector(listOf(byteArrayOf(0xff.toByte())) + key.drop(1)))),
            record(listOf(vector(key.dropLast(1)))), record(listOf(vector(key + listOf(byteArrayOf(1))))),
            record(listOf(vector(listOf(byteArrayOf(2)) + List(48) { byteArrayOf(0) }))),
            record(listOf(vector(listOf(byteArrayOf(2, 0)) + key.drop(1)))),
            record(listOf(vector(key), byteArrayOf(0))),
        )
        for (malformed in badPeers) assertRejected("monetary_bond_plan", 1, listOf(uint(201, 64), malformed))
        for (width in listOf(0, 1, 31, 33)) {
            assertRejected("monetary_unbond_plan", 2, listOf(uint(201, 64), ByteArray(width) { 0x75 }))
        }
        val unmarked = ByteArray(32) { 0x75 }.apply { this[31] = 0x74 }
        assertRejected("monetary_unbond_plan", 2, listOf(uint(201, 64), unmarked))
        for (malformed in listOf(quantity(byteArrayOf(0xff.toByte()), 0), quantity(byteArrayOf(10), 1), quantity(byteArrayOf(1), 29), byteArrayOf())) {
            assertRejected("monetary_slash_plan", 3, listOf(uint(201, 64), malformed))
        }
        for ((kind, tag, binding) in listOf(
            Triple("monetary_bond_plan", 1L, peer),
            Triple("monetary_unbond_plan", 2L, ByteArray(32) { 0x75 }),
            Triple("monetary_slash_plan", 3L, quantity(byteArrayOf(0xdc.toByte(), 5), 0)),
        )) {
            assertRejected(kind, tag, listOf(uint(201, 64)))
            assertRejected(kind, tag, listOf(uint(201, 64), binding, byteArrayOf(0)))
            for (width in listOf(0, 7, 9)) assertRejected(kind, tag, listOf(ByteArray(width), binding))
            assertRejected(kind, 4, listOf(uint(201, 64), binding))
            assertRejected(kind, 0xffff_ffffL, listOf(uint(201, 64), binding))
        }
        val body = record(listOf(uint(201, 64), ByteArray(32) { 0x75 }))
        val nonminimal = uint(2, 32) + byteArrayOf((body.size or 0x80).toByte(), 0) + body
        assertFails { ValidatorStakingNoritoV1.MonetaryPrecondition.decode(nonminimal) }
    }

    @Test
    fun monetaryBindingsPreserveUnsignedHeightAndOwnedValues() {
        val rows = fixtureRows()
        for ((kind, tag) in listOf("monetary_bond_plan" to 1L, "monetary_unbond_plan" to 2L, "monetary_slash_plan" to 3L)) {
            val plan = fields(rows.getValue(kind)).toMutableList()
            val variant = plan[5]
            val binding = fields(fields(variant.copyOfRange(4, variant.size))[0]).toMutableList()
            for (height in listOf(0L, -1L)) {
                binding[0] = uint(height, 64)
                plan[5] = uint(tag, 32) + record(listOf(record(binding)))
                val bytes = record(plan)
                val original = bytes.copyOf()
                val decoded = ValidatorStakingNoritoV1.MonetaryPlan.decode(bytes)
                bytes.fill(0)
                assertEquals(height, decoded.precondition.activationHeight)
                assertContentEquals(original, decoded.encode())
                when (val value = decoded.precondition) {
                    is ValidatorStakingNoritoV1.MonetaryPrecondition.Bond -> {
                        value.bond.peerId.publicKey.bytes().fill(0)
                        assertTrue(value.bond.peerId.publicKey.bytes().any { it.toInt() != 0 })
                    }
                    is ValidatorStakingNoritoV1.MonetaryPrecondition.Unbond -> {
                        value.unbond.requestHash.bytes().fill(0)
                        assertContentEquals(ByteArray(32) { 0x75 }, value.unbond.requestHash.bytes())
                    }
                    is ValidatorStakingNoritoV1.MonetaryPrecondition.Slash -> assertEquals(BigInteger.valueOf(1500), value.slash.slashableExposure.mantissa)
                    is ValidatorStakingNoritoV1.MonetaryPrecondition.Registration -> error("wrong monetary variant")
                }
            }
        }
        // PeerId is a generic Rust key identity, not a BLS-only wire alias.
        val replacementPeer = fields(rows.getValue("rebind_peer"))[2]
        val peer = ValidatorStakingNoritoV1.PeerId.decode(replacementPeer)
        assertEquals(SigningAlgorithm.ED25519, peer.algorithm)
        assertContentEquals(replacementPeer, peer.encode())
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
            "monetary_bond_plan" to ValidatorStakingNoritoV1.MonetaryPlan::decode,
            "monetary_unbond_plan" to ValidatorStakingNoritoV1.MonetaryPlan::decode,
            "monetary_slash_plan" to ValidatorStakingNoritoV1.MonetaryPlan::decode,
            "reward_claim_plan" to ValidatorStakingNoritoV1.RewardClaimPlan::decode,
            "fee_reward_claim_plan" to ValidatorStakingNoritoV1.RewardClaimPlan::decode,
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

    @Test
    fun `reward plans require explicit fee option and bounded ordered records`() {
        val plan = fields(fixtureRows().getValue("reward_claim_plan"))
        val records = vectorFields(plan[3])
        val sources = vectorFields(plan[4])
        val invalid = listOf(
            record(plan.dropLast(1)), record(plan + listOf(byteArrayOf(0))),
            replacing(plan, 5, byteArrayOf(2)), replacing(plan, 5, byteArrayOf(0, 0)),
            replacing(plan, 1, uint(0, 64)),
            replacing(plan, 3, vector(List(65) { records[0] })),
            replacing(plan, 4, vector(List(65) { sources[0] })),
            replacing(plan, 3, vector(listOf(records[0], records[0]))),
            replacing(plan, 4, vector(listOf(sources[0], sources[0]))),
            replacing(plan, 2, option(record(listOf(option(uint(201, 64)))))),
        )
        for (bytes in invalid) assertFails { ValidatorStakingNoritoV1.RewardClaimPlan.decode(bytes) }
        val reference = fields(records[0])
        assertFails { ValidatorStakingNoritoV1.RewardRecordRef.decode(replacing(reference, 1, ByteArray(31) { 1 })) }
        assertFails { ValidatorStakingNoritoV1.RewardRecordRef.decode(record(reference.dropLast(1))) }
        assertFails { ValidatorStakingNoritoV1.RewardClaimState.decode(byteArrayOf()) }
        assertFails { ValidatorStakingNoritoV1.RewardClaimState.decode(record(listOf(byteArrayOf(0), byteArrayOf(0)))) }
        assertNull(ValidatorStakingNoritoV1.RewardClaimState.decode(record(listOf(byteArrayOf(0)))).throughEpoch)
        assertEquals(0L, ValidatorStakingNoritoV1.RewardClaimState.decode(record(listOf(option(uint(0, 64))))).throughEpoch)
        val high = plan.toMutableList().apply {
            this[1] = uint(-1L, 64)
            this[2] = option(record(listOf(option(uint(-2L, 64)))))
            this[3] = vector(listOf(replacing(reference, 0, uint(-1L, 64))))
        }
        val decoded = ValidatorStakingNoritoV1.RewardClaimPlan.decode(record(high))
        assertEquals(-1L, decoded.validUntilHeight)
        assertEquals(-1L, decoded.records.single().epoch)
        assertContentEquals(record(high), decoded.encode())
    }

    @Test
    fun `reward plan retains all sixty four sources in their signed order`() {
        val plan = fields(fixtureRows().getValue("reward_claim_plan")).toMutableList()
        val reference = fields(vectorFields(plan[3])[0])
        val source = fields(vectorFields(plan[4])[0])
        val sourceAsset = fields(source[0])
        val destinationAsset = fields(source[1])
        val sources = List(64) { index ->
            val definition = record(listOf(byteArrayOf(index.toByte())) + List(15) { byteArrayOf(9) })
            record(source.toMutableList().apply {
                this[0] = replacing(sourceAsset, 1, definition)
                this[1] = replacing(destinationAsset, 1, definition)
            })
        }
        plan[2] = byteArrayOf(0)
        plan[3] = vector(List(64) { replacing(reference, 0, uint(it.toLong(), 64)) })
        plan[4] = vector(sources)
        val payload = record(plan)
        val decoded = ValidatorStakingNoritoV1.RewardClaimPlan.decode(payload)
        assertNull(decoded.expectedState)
        assertEquals(64, decoded.records.size)
        assertEquals(64, decoded.sources.size)
        sources.zip(decoded.sources).forEach { (bytes, value) -> assertContentEquals(bytes, value.encode()) }
        assertContentEquals(payload, decoded.encode())
    }

    @Test
    fun `reward sources require semantic account definition and scope order`() {
        val plan = fields(fixtureRows().getValue("reward_claim_plan"))
        val source = fields(vectorFields(plan[4])[0])
        val asset = fields(source[0])
        val destination = fields(source[1])
        fun claim(owner: ByteArray, scope: ByteArray): ByteArray = record(listOf(
            record(listOf(owner, asset[1], scope)), replacing(destination, 2, scope), source[2], source[3],
        ))
        fun assertOrdered(sources: List<ByteArray>) {
            val payload = replacing(plan, 4, vector(sources))
            assertContentEquals(payload, ValidatorStakingNoritoV1.RewardClaimPlan.decode(payload).encode())
            assertFails {
                ValidatorStakingNoritoV1.RewardClaimPlan.decode(replacing(plan, 4, vector(sources.reversed())))
            }
        }
        val scopes = listOf(uint(0, 32)) + listOf(255L, 256L, -1L).map {
            uint(1, 32) + record(listOf(uint(it, 64)))
        }
        assertOrdered(scopes.map { claim(asset[0], it) })
        // Rust compares the u16 threshold numerically; its little-endian frame reverses 255/256.
        val key = fields(asset[0].copyOfRange(4, asset[0].size))[0]
        fun multisig(threshold: Long): ByteArray = uint(1, 32) + record(listOf(record(listOf(
            byteArrayOf(1), uint(threshold, 16), vector(listOf(record(listOf(key, uint(256, 16))))),
        ))))
        assertOrdered(listOf(asset[0], multisig(255), multisig(256)).map { claim(it, uint(0, 32)) })
        val laterDefinition = record(List(16) { byteArrayOf(0xff.toByte()) })
        val later = record(listOf(replacing(asset, 1, laterDefinition),
            replacing(destination, 1, laterDefinition), source[2], source[3]))
        assertOrdered(listOf(record(source), later))
    }

    @Test
    fun `reward sources bind assets and retain zero payout dust`() {
        val plan = fields(fixtureRows().getValue("reward_claim_plan"))
        val source = fields(vectorFields(plan[4])[0])
        val destination = fields(source[1])
        val invalid = listOf(
            record(source.dropLast(1)), record(source + listOf(byteArrayOf(0))),
            replacing(source, 2, option(quantity(byteArrayOf(), 0))),
            replacing(source, 1, replacing(destination, 1, record(List(16) { byteArrayOf(9) }))),
            replacing(source, 1, replacing(destination, 2, uint(1, 32) + record(listOf(uint(7, 64))))),
        )
        for (bytes in invalid) assertFails { ValidatorStakingNoritoV1.RewardClaimSource.decode(bytes) }
        val dust = ValidatorStakingNoritoV1.RewardClaimSource.decode(replacing(source, 3, quantity(byteArrayOf(), 0)))
        assertEquals(BigInteger.ZERO, dust.payout.mantissa)
        assertEquals(BigInteger.valueOf(5), dust.expectedAccrued?.mantissa)
        assertNull(ValidatorStakingNoritoV1.RewardClaimSource.decode(replacing(source, 2, byteArrayOf(0))).expectedAccrued)
    }

    @Test
    fun `fee reward claim rejects changed custody and zero amount`() {
        val plan = fields(fixtureRows().getValue("fee_reward_claim_plan"))
        val decoded = ValidatorStakingNoritoV1.RewardClaimPlan.decode(record(plan))
        val fee = assertNotNull(decoded.feeClaim)
        val claim = fields(fee.encode())
        val source = fields(claim[3])
        val destination = fields(claim[4])
        val invalid = listOf(
            record(claim.dropLast(1)), record(claim + listOf(byteArrayOf(0))),
            replacing(claim, 0, ByteArray(32)),
            replacing(claim, 0, ByteArray(31) { 0x77 }),
            replacing(claim, 0, ByteArray(33) { 0x77 }),
            replacing(claim, 0, record(List(32) { byteArrayOf(0x77) })),
            replacing(claim, 5, quantity(byteArrayOf(), 0)),
            replacing(claim, 3, replacing(source, 2, uint(1, 32) + record(listOf(uint(7, 64))))),
            replacing(claim, 4, replacing(destination, 1, record(List(16) { byteArrayOf(9) }))),
            replacing(claim, 4, replacing(destination, 2, uint(1, 32) + record(listOf(uint(7, 64))))),
        )
        for (bytes in invalid) {
            assertFails { ValidatorStakingNoritoV1.FeeRewardClaim.decode(bytes) }
            assertFails { ValidatorStakingNoritoV1.RewardClaimPlan.decode(replacing(plan, 5, option(bytes))) }
        }
        val high = claim.toMutableList().apply {
            this[2] = uint(-1L, 64)
            this[6] = uint(-1L, 64)
        }
        val max = ValidatorStakingNoritoV1.FeeRewardClaim.decode(record(high))
        assertEquals(-1L, max.beneficiaryRevision)
        assertEquals(-1L, max.expectedClaimSequence)
    }

    @Test
    fun `fee reward claim retains exact self custody payment`() {
        val plan = fields(fixtureRows().getValue("fee_reward_claim_plan"))
        val original = ValidatorStakingNoritoV1.RewardClaimPlan.decode(record(plan))
        val claim = fields(assertNotNull(original.feeClaim).encode())
        val selfCustody = replacing(claim, 3, claim[4])
        val payload = replacing(plan, 5, option(selfCustody))
        val decoded = ValidatorStakingNoritoV1.RewardClaimPlan.decode(payload)
        val fee = assertNotNull(decoded.feeClaim)
        assertContentEquals(fee.sourceAsset.encode(), fee.destinationAsset.encode())
        assertEquals(BigInteger.valueOf(7), fee.amount.mantissa)
        assertContentEquals(selfCustody, fee.encode())
        assertContentEquals(payload, decoded.encode())
    }

    @Test
    fun `reward plans own original bytes and defensive collections`() {
        val bytes = fixtureRows().getValue("fee_reward_claim_plan")
        val original = bytes.copyOf()
        val decoded = ValidatorStakingNoritoV1.RewardClaimPlan.decode(bytes)
        bytes.fill(0)
        decoded.records.single().recordHash.bytes().fill(0)
        decoded.sources.single().sourceAsset.account.bytes().fill(0)
        assertNotNull(decoded.feeClaim).lifecycleSeal.bytes().fill(0)
        val records = decoded.records as MutableList<ValidatorStakingNoritoV1.RewardRecordRef>
        records[0] = ValidatorStakingNoritoV1.RewardRecordRef.decode(
            replacing(fields(records[0].encode()), 0, uint(202, 64)))
        val sources = decoded.sources as MutableList<ValidatorStakingNoritoV1.RewardClaimSource>
        sources[0] = ValidatorStakingNoritoV1.RewardClaimSource.decode(
            replacing(fields(sources[0].encode()), 3, quantity(byteArrayOf(), 0)))
        assertEquals(201L, decoded.records.single().epoch)
        assertEquals(BigInteger.valueOf(15), decoded.sources.single().payout.mantissa)
        assertContentEquals(original, decoded.encode())
    }

    private fun option(value: ByteArray): ByteArray = byteArrayOf(1) + record(listOf(value))

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

    @Test
    fun monetaryPlansRetainFullUnsignedExpiry() {
        val plan = fields(fixtureRows().getValue("monetary_plan"))
        for (expiry in listOf(1L, Long.MAX_VALUE, Long.MIN_VALUE, -1L)) {
            val payload = replacing(plan, 1, uint(expiry, 64))
            val decoded = ValidatorStakingNoritoV1.MonetaryPlan.decode(payload)
            assertEquals(expiry, decoded.validUntilHeight)
            assertContentEquals(payload, decoded.encode())
        }
        assertFails { ValidatorStakingNoritoV1.MonetaryPlan.decode(replacing(plan, 1, uint(0, 64))) }
    }

    @Test
    fun decodedAuthorityAndDkgCollectionsRetainOriginalValues() {
        val rows = fixtureRows()
        val authority = ValidatorStakingNoritoV1.AuthorityGeneration.decode(rows.getValue("authority_generation"))
        (authority.validators as MutableList<*>).clear()
        assertEquals(4, authority.validators.size)
        assertContentEquals(rows.getValue("authority_generation"), authority.encode())

        val transcript = ValidatorStakingNoritoV1.DkgTranscript.decode(rows.getValue("dkg_transcript"))
        val dealer = transcript.dealerCommitments.first()
        val dealerBytes = dealer.encode()
        val coefficients = dealer.coefficientCommitments.size
        assertEquals(2, coefficients)
        (dealer.coefficientCommitments as MutableList<*>).clear()
        assertEquals(coefficients, dealer.coefficientCommitments.size)
        assertContentEquals(dealerBytes, dealer.encode())
        (transcript.dealerCommitments as MutableList<*>).clear()
        (transcript.recipientKeys as MutableList<*>).clear()
        (transcript.encryptedShares as MutableList<*>).clear()
        (transcript.shareAcceptances as MutableList<*>).clear()
        (transcript.qualifiedDealers as MutableList<*>).clear()
        assertEquals(4, transcript.dealerCommitments.size)
        assertEquals(4, transcript.recipientKeys.size)
        assertEquals(16, transcript.encryptedShares.size)
        assertEquals(16, transcript.shareAcceptances.size)
        assertEquals(4, transcript.qualifiedDealers.size)
        assertContentEquals(rows.getValue("dkg_transcript"), transcript.encode())
    }

    @Test
    fun transitionReadinessRetainsItsDecodedRecords() {
        val transition = fields(fixtureRows().getValue("committee_transition"))
        // These opaque proof fields exercise collection ownership only; decoding
        // does not authenticate possession or authorize a committee transition.
        val readiness = record(listOf(uint(0, 32), byteArrayOf(1), byteArrayOf(2)))
        val payload = replacing(transition, 2, vector(listOf(readiness, readiness)))
        val decoded = ValidatorStakingNoritoV1.CommitteeTransition.decode(payload)
        (decoded.readiness as MutableList<*>).clear()
        assertEquals(2, decoded.readiness.size)
        assertContentEquals(readiness, decoded.readiness.first().encode())
        assertContentEquals(payload, decoded.encode())
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
