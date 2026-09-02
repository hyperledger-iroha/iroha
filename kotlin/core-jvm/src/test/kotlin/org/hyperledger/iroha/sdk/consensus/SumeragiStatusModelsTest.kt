// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.util.HashLiteral

class SumeragiStatusModelsTest {
    @Test
    fun `authoritative parser preserves the complete typed v4 snapshot and exact u64 range`() {
        val maximum = "18446744073709551615"
        val status = SumeragiV2Status.parseJson(
            statusJson(rootView = maximum, executedWireLen = maximum, noProgressAge = maximum),
        )

        assertEquals(SUMERAGI_STATUS_PROTOCOL_VERSION, status.protocolVersion)
        assertEquals(BigInteger(maximum), status.view)
        assertEquals(BigInteger(maximum), status.liveness.noProgressAgeMs)
        assertEquals(
            BigInteger(maximum),
            status.lastCommitQc?.certificate?.executionCommitment?.executedBlockWireLen,
        )
        assertEquals(SumeragiStatusPhase.PREPARE, status.phase)
        assertEquals(SumeragiStatusConsensusMode.PERMISSIONED, status.heightContext.mode)
        assertEquals(SumeragiStatusWorkStage.COMPLETE, status.liveness.work.validation)
        assertEquals(SumeragiStatusQueueKind.NETWORK_INGRESS, status.liveness.queues.single().queue)
        assertEquals(
            SumeragiStatusProgressTransition.PREPARE_VOTE_ADMITTED,
            status.liveness.lastProgress?.transition,
        )
        assertNull(status.lockedPrepareQc)
        assertFalse(status.restartRequired)
        assertTrue(status.lastCommitQc != null)
    }

    @Test
    fun `execution commitment accepts one thousand Offline Cash top-ups and rejects legacy names`() {
        val offlineCashTopUpRoot = hash(0x38)
        val canonicalPayload = statusJson().replace(
            "\"offline_cash_top_up_count\": 0,",
            "\"offline_cash_top_up_root\": \"$offlineCashTopUpRoot\", " +
                "\"offline_cash_top_up_count\": 1000,",
        )
        val commitment = SumeragiV2Status.parseJson(canonicalPayload)
            .lastCommitQc?.certificate?.executionCommitment
        assertEquals(BigInteger.valueOf(1_000), commitment?.offlineCashTopUpCount)
        assertEquals(offlineCashTopUpRoot, commitment?.offlineCashTopUpRoot)

        assertFails {
            SumeragiV2Status.parseJson(
                canonicalPayload.replace(
                    "offline_cash_top_up_count",
                    "topup_anchor_count",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                canonicalPayload.replace(
                    "offline_cash_top_up_root",
                    "topup_anchor_root",
                ),
            )
        }
    }

    @Test
    fun `authoritative parser rejects unknown missing duplicate and noncanonical scalar fields`() {
        val payload = statusJson()
        assertFails {
            SumeragiV2Status.parseJson(payload.replaceFirst("{", "{\"mode_tag\":\"legacy\","))
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replace("\"restart_required\": false,", ""))
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replaceFirst("{", "{\"protocol_version\":4,"))
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replaceFirst("\"height\": 10", "\"height\": \"10\""))
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replaceFirst("\"view\": 2", "\"view\": -0"))
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"view\": 2", "\"view\": 18446744073709551616"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(byteArrayOf(0x7b, 0x22, 0xc3.toByte(), 0x28, 0x22, 0x7d))
        }
    }

    @Test
    fun `authoritative parser enforces exact tags phase body and commit frontier geometry`() {
        val payload = statusJson()
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"phase\": \"prepare\", \"details\": null", "\"phase\": \"prepare\""),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"state\": \"validated\"", "\"state\": \"missing\""),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"pending_persistence_id\": null", "\"pending_persistence_id\": 0"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"phase\": \"prepare\"", "\"phase\": \"commit\""),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"last_committed_height\": 9", "\"last_committed_height\": 10"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"last_committed_subject\": {", "\"last_committed_subject\": null, \"retired\": {"),
            )
        }
    }

    @Test
    fun `authoritative parser enforces execution manifest carrier and commit QC invariants`() {
        val payload = statusJson()
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replace(
                    "\"native_amx_application_manifest_version\": 1",
                    "\"native_amx_application_manifest_version\": 2",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replace(
                    "\"native_amx_application_manifest_count\": 0",
                    "\"native_amx_application_manifest_count\": 1",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replace("\"lane_finality_manifest\": null,", ""))
        }
        val laneRoot = hash(0x38)
        val withLane = SumeragiV2Status.parseJson(
            payload.replace(
                "\"lane_finality_manifest\": null",
                "\"lane_finality_manifest\": {\"root\": \"$laneRoot\", \"leaf_count\": 1}",
            ),
        )
        assertEquals(
            laneRoot,
            withLane.lastCommitQc?.certificate?.executionCommitment?.laneFinalityManifest?.root,
        )
        assertEquals(
            BigInteger.ONE,
            withLane.lastCommitQc?.certificate?.executionCommitment?.laneFinalityManifest?.leafCount,
        )
        listOf(0, 1_025).forEach { count ->
            assertFails {
                SumeragiV2Status.parseJson(
                    payload.replace(
                        "\"lane_finality_manifest\": null",
                        "\"lane_finality_manifest\": {\"root\": \"$laneRoot\", \"leaf_count\": $count}",
                    ),
                )
            }
        }
        assertFails {
            SumeragiV2Status.parseJson(payload.replace("\"merge_carrier\": null,", ""))
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replace("\"executed_block_wire_len\": 123", "\"executed_block_wire_len\": 0"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst(
                    "\"signed_power\": 3",
                    "\"signed_power\": 2",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"signer_count\": 3", "\"signer_count\": 4")
                    .replaceFirst("\"signed_power\": 3", "\"signed_power\": 4"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst(
                    "\"proposal_round\": {\"context_id\": [\"${hash(0x41)}\"], \"height\": 9, \"view\": 1}",
                    "\"proposal_round\": {\"context_id\": [\"${hash(0x41)}\"], \"height\": 9, \"view\": 2}",
                ),
            )
        }
    }

    @Test
    fun `authoritative liveness parser rejects future split duplicate and malformed records`() {
        val payload = statusJson()
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst(
                    "\"round\": {\"context_id\": [\"${hash(0x14)}\"], \"height\": 10, \"view\": 1}",
                    "\"round\": {\"context_id\": [\"${hash(0x14)}\"], \"height\": 10, \"view\": 3}",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst(
                    "\"proposal_round\": {\"context_id\": [\"${hash(0x14)}\"], \"height\": 10, \"view\": 1}",
                    "\"proposal_round\": {\"context_id\": [\"${hash(0x14)}\"], \"height\": 10, \"view\": 0}",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replace(
                    "\"queues\": [$QUEUE],",
                    "\"queues\": [$QUEUE,$QUEUE],",
                ),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"depth\": 1, \"capacity\": 4", "\"depth\": 5, \"capacity\": 4"),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("\"kind\": \"proposal\"", "\"kind\": \"timeout_vote\""),
            )
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replace(
                    "\"ignore_counts\": [$IGNORE_COUNT]",
                    "\"ignore_counts\": [$IGNORE_COUNT,$IGNORE_COUNT]",
                ),
            )
        }
    }

    @Suppress("UNCHECKED_CAST")
    @Test
    fun `authoritative collections are defensive and operational diagnostics are rejected`() {
        val payload = statusJson()
        val status = SumeragiV2Status.parseJson(payload)

        assertFails {
            (status.liveness.queues as MutableList<SumeragiStatusQueue>).clear()
        }
        assertFails {
            SumeragiV2Status.parseJson(
                payload.replaceFirst("{", "{\"lane_settlement_commitments\":[],"),
            )
        }
    }

    private fun statusJson(
        rootView: String = "2",
        executedWireLen: String = "123",
        noProgressAge: String = "19",
    ): String {
        val subject = SUBJECT
        val commitment = executionCommitment(executedWireLen)
        return """
            {
              "protocol_version": 4,
              "node_fingerprint": "${hash(0x11)}",
              "build_fingerprint": "${hash(0x12)}",
              "config_fingerprint": "${hash(0x13)}",
              "restart_required": false,
              "height_context_id": ["${hash(0x14)}"],
              "height": 10,
              "view": $rootView,
              "phase": {"phase": "prepare", "details": null},
              "leader": 1,
              "locked_prepare_qc": null,
              "highest_prepare_qc": null,
              "last_timeout_certificate": null,
              "body_state": {"state": "validated", "details": null},
              "pending_persistence_id": null,
              "last_committed_height": 9,
              "last_committed_subject": $subject,
              "height_context": {
                "epoch": 1,
                "epoch_end_height": 20,
                "mode": {"mode": "permissioned", "details": null},
                "epoch_seed": "${(0..31).joinToString("") { "%02X".format(it) }}",
                "validator_count": 4,
                "quorum": {"min_signers": 3, "total_power": 4}
              },
              "last_commit_qc": {
                "certificate": {
                  "round": {"context_id": ["${hash(0x41)}"], "height": 9, "view": 1},
                  "proposal_round": {"context_id": ["${hash(0x41)}"], "height": 9, "view": 1},
                  "phase": {"phase": "commit", "details": null},
                  "subject": $subject,
                  "execution_commitment": $commitment
                },
                "validator_count": 4,
                "signer_count": 3,
                "min_signers": 3,
                "signed_power": 3,
                "total_power": 4
              },
              "liveness": {
                "generation": 2,
                "prepare_quorums": [{
                  "round": {"context_id": ["${hash(0x14)}"], "height": 10, "view": 1},
                  "proposal_round": {"context_id": ["${hash(0x14)}"], "height": 10, "view": 1},
                  "subject": $subject,
                  "execution_commitment": $commitment,
                  "signer_count": 2,
                  "signed_power": 2,
                  "min_signers": 3,
                  "total_power": 4
                }],
                "commit_quorums": [],
                "timeout_quorums": [],
                "outbound_intents": [{
                  "kind": {"kind": "proposal", "details": null},
                  "round": {"context_id": ["${hash(0x14)}"], "height": 10, "view": 1},
                  "proposal_round": {"context_id": ["${hash(0x14)}"], "height": 10, "view": 1},
                  "subject": $subject,
                  "stage": {"stage": "sent", "details": null}
                }],
                "work": {
                  "candidate": {"stage": "idle", "details": null},
                  "body_recovery": {"stage": "idle", "details": null},
                  "body_store": {"stage": "idle", "details": null},
                  "validation": {"stage": "complete", "details": null},
                  "application": {"stage": "idle", "details": null},
                  "successor_height": {"stage": "idle", "details": null}
                },
                "queues": [$QUEUE],
                "last_progress": {
                  "generation": 2,
                  "round": {"context_id": ["${hash(0x14)}"], "height": 10, "view": 1},
                  "transition": {"transition": "prepare_vote_admitted", "details": null},
                  "age_ms": 19
                },
                "no_progress_age_ms": $noProgressAge,
                "blocker": {"blocker": "prepare_quorum_missing", "details": null},
                "ignore_counts": [$IGNORE_COUNT]
              }
            }
        """.trimIndent()
    }

    private fun executionCommitment(executedWireLen: String): String = """
        {
          "parent_state_root": "${hash(0x34)}",
          "post_state_root": "${hash(0x35)}",
          "ordinary_writes_root": "${hash(0x36)}",
          "offline_cash_top_up_count": 0,
          "native_amx_application_manifest_version": 1,
          "native_amx_application_manifest_root": "$EMPTY_MANIFEST_ROOT",
          "native_amx_application_manifest_count": 0,
          "lane_finality_manifest": null,
          "merge_carrier": null,
          "executed_block_wire_len": $executedWireLen,
          "executed_block_wire_hash": "${hash(0x37)}"
        }
    """.trimIndent()

    companion object {
        private fun hash(seed: Int): String =
            HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })

        private const val EMPTY_MANIFEST_ROOT =
            "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
        private val SUBJECT = """
            {
              "parent_block_hash": "${hash(0x31)}",
              "block_hash": "${hash(0x32)}",
              "payload_hash": "${hash(0x33)}"
            }
        """.trimIndent()
        private val QUEUE = """
            {
              "queue": {"queue": "network_ingress", "details": null},
              "depth": 1, "capacity": 4, "oldest_age_ms": 17, "service_debt": 2
            }
        """.trimIndent()
        private val IGNORE_COUNT = """
            {"reason": {"reason": "duplicate", "details": null}, "count": 2}
        """.trimIndent()
    }
}
