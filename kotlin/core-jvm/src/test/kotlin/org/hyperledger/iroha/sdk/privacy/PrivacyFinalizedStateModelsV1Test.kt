package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.util.HashLiteral

class PrivacyFinalizedStateModelsV1Test {
    @Test
    fun closedSelectorsUseExactNativeIdsAndWidths() {
        val requests = listOf(
            PrivacyZkAceReplayNullifierRequestV1(fixed32(1), fixed32(2)) to Triple(97, 0, 64),
            PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
                fixed32(3),
            ) to Triple(98, 0, 32),
            PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
                fixed32(4),
            ) to Triple(98, 1, 32),
            PrivacyProofManagedPoolStateRequestV1(
                PrivacyProtocolIdV1.PQ_MASP_STARK_V0,
                fixed32(5),
            ) to Triple(98, 2, 32),
            PrivacyOrchardPoolStateRequestV1(fixed32(6)) to Triple(99, 0, 32),
            PrivacyOrchardNullifierRequestV1(fixed32(7), fixed32(8)) to Triple(100, 0, 64),
            PrivacyAnonymousPgcPoolStateRequestV1(fixed32(9)) to Triple(101, 0, 32),
            PrivacyZkAmsAdmissionRequestV1(
                fixed32(10), fixed32(11), fixed32(12), fixed32(13),
            ) to Triple(102, 0, 128),
            PrivacyZkAmsProvisionRequestV1(
                fixed32(14), fixed32(15), fixed32(16), fixed32(17),
            ) to Triple(103, 0, 128),
            PrivacyZkX509CertificateNullifierRequestV1(
                fixed32(18), fixed32(19), fixed32(20),
            ) to Triple(104, 0, 96),
        )
        requests.forEach { (request, expected) ->
            assertEquals(expected.first, request.queryId)
            assertEquals(expected.second, request.protocolIndex)
            assertEquals(expected.third, request.requestBinding().size)
            val first = request.requestBinding()
            first[0] = (first[0].toInt() xor 0xff).toByte()
            assertNotEquals(first.toList(), request.requestBinding().toList())
        }
    }

    @Test
    fun x509SelectorIsExactlyTrustAnchorPolicyAndNullifier() {
        val trustAnchor = fixed32(0x31)
        val policy = fixed32(0x32)
        val nullifier = fixed32(0x33)
        val binding = PrivacyZkX509CertificateNullifierRequestV1(
            trustAnchor,
            policy,
            nullifier,
        ).requestBinding()
        assertEquals(96, binding.size)
        assertContentEquals(trustAnchor, binding.copyOfRange(0, 32))
        assertContentEquals(policy, binding.copyOfRange(32, 64))
        assertContentEquals(nullifier, binding.copyOfRange(64, 96))
    }

    @Test
    fun projectionBindsNetworkSelectorAndImmutableFinality() {
        val networkId = NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        val policy = fixed32(0x21)
        val replay = fixed32(0x22)
        val request = PrivacyZkAceReplayNullifierRequestV1(policy, replay)
        val blockHash = HashLiteral.canonicalize(fixed32(0x31))
        val projection = """
            {
              "network_id":"$networkId",
              "policy_id":${bytesJson(policy)},
              "replay_nullifier":${bytesJson(replay)},
              "policy_record_digest":${bytesJson(fixed32(0x23))},
              "statement_digest":${bytesJson(fixed32(0x24))},
              "admitted_at_height":"4",
              "action_index":"0",
              "finalized_height":"7",
              "finalized_block_hash":"$blockHash"
            }
        """.trimIndent().toByteArray()
        val view = assertIs<PrivacyZkAceReplayNullifierProvenanceV1>(
            PrivacyFinalizedStateProjectionV1.parse(projection, request, networkId),
        )
        assertEquals(BigInteger.valueOf(7), view.finalizedHeight)
        assertContentEquals(policy, view.policyId)
        assertContentEquals(replay, view.replayNullifier)
        assertContentEquals(HashLiteral.decode(blockHash), view.finalizedBlockHash)
        val returned = view.policyId
        returned[0] = 0
        assertContentEquals(policy, view.policyId)

        assertFailsWith<IllegalArgumentException> {
            PrivacyFinalizedStateProjectionV1.parse(
                projection.toString(Charsets.UTF_8)
                    .replace(bytesJson(replay), bytesJson(fixed32(0x25)))
                    .toByteArray(),
                request,
                networkId,
            )
        }
    }

    private fun fixed32(value: Int): ByteArray = ByteArray(32) { value.toByte() }

    private fun bytesJson(value: ByteArray): String =
        value.joinToString(prefix = "[", postfix = "]") { (it.toInt() and 0xff).toString() }
}
