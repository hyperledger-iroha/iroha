package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import kotlin.test.*
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

class SumeragiStatusModelsTest {
    private fun root(): MutableMap<String, Any?> =
        SumeragiJsonPrimitives.parseObject(NativeStatusFixtures.json(), "fixture").toMutableMap()
    private fun changed(change: (MutableMap<String, Any?>) -> Unit): String =
        JsonEncoder.encode(root().also(change))
    @Test fun `Rust native corpus preserves all unsigned ranges and every halt variant`() {
        assertEquals(8, NativeStatusFixtures.rows().size)
        for ((name, row) in NativeStatusFixtures.rows()) {
            val status = SumeragiStatus.parseJson(row.first)
            assertEquals(8, status.protocolVersion)
            assertEquals(BigInteger("18446744073709551615"), status.view)
            assertEquals(BigInteger("4294967295"), status.level)
            assertEquals(BigInteger("18446744073709551615"), status.footprint.votes)
            assertEquals(BigInteger.ONE, status.applyLag())
            assertEquals(status, SumeragiStatus.parseJson(row.first.toByteArray()))
            assertEquals(status.hashCode(), SumeragiStatus.parseJson(row.first).hashCode())
            if (name == "validator") {
                assertTrue(status.isSigning()); assertFalse(status.isHalted())
                assertEquals("AB".repeat(32), status.beaconHorizon?.activeSessionId)
                assertTrue(requireNotNull(status.beaconHorizon).localProviderReady)
            } else {
                assertFalse(status.isSigning()); assertNull(status.beaconHorizon)
                assertEquals(name != "observer", status.isHalted())
            }
        }
    }
    @Test fun `every top level and nested field is required and unknown fields fail closed`() {
        for (field in root().keys) assertFails(field) { SumeragiStatus.parseJson(changed { it.remove(field) }) }
        for (owner in listOf("footprint", "beacon_horizon")) {
            val original = SumeragiJsonPrimitives.objectValue(root()[owner], owner)
            for (field in original.keys) assertFails("$owner.$field") {
                SumeragiStatus.parseJson(changed { it[owner] = original.toMutableMap().also { nested -> nested.remove(field) } })
            }
            assertFails { SumeragiStatus.parseJson(changed { it[owner] = original + ("legacy" to 0L) }) }
        }
        for (retired in listOf("height_context", "height_context_id", "phase", "locked_prepare_qc",
            "highest_prepare_qc", "last_timeout_certificate", "last_commit_qc", "last_committed_subject",
            "body_state", "liveness", "restart_required", "node_fingerprint", "build_fingerprint",
            "execution_commitment", "transaction_input_commitment", "merge_carrier", "rbc_status")) {
            assertFails(retired) { SumeragiStatus.parseJson(changed { it[retired] = null }) }
        }
    }
    @Test fun `malformed scalars duplicate keys and retired versions never alias canonical values`() {
        val payload = NativeStatusFixtures.json()
        for (version in listOf(0L, 4L, 6L, 7L, 9L)) assertFails { SumeragiStatus.parseJson(changed { it["protocol_version"] = version }) }
        for (bad in listOf<Any?>(-1L, "15", 1.5, BigInteger.ONE.shiftLeft(64), null, true)) {
            assertFails { SumeragiStatus.parseJson(changed { it["height"] = bad }) }
        }
        assertFails { SumeragiStatus.parseJson(changed { it["level"] = BigInteger.ONE.shiftLeft(32) }) }
        assertFails { SumeragiStatus.parseJson(changed { it["stage"] = 3L }) }
        for (field in listOf("awaiting", "unanchored", "abstaining")) assertFails { SumeragiStatus.parseJson(changed { it[field] = 1L }) }
        assertFails { SumeragiStatus.parseJson("{\"view\":0," + payload.substring(1)) }
        assertFails { SumeragiStatus.parseJson(changed { it["height"] = 15L }.replace("\"height\":15", "\"height\":-0")) }
        assertFails { SumeragiStatus.parseJson(byteArrayOf(0x7b, 0xc3.toByte(), 0x28)) }
        assertFails { SumeragiStatus.parseJson(ByteArray(1_048_577)) }
        assertFails { SumeragiStatus.parseJson("") }
        for (field in listOf("config_fingerprint", "instance")) {
            assertFails { SumeragiStatus.parseJson(changed { it[field] = "00".repeat(32) + "00" }) }
        }
        assertFails { SumeragiStatus.parseJson(changed { it["instance"] = "CD".repeat(32) }) }
        val key = root()["leader"] as String
        for (bad in listOf("bls_normal:$key", key.lowercase(), " $key", "ea0130" + "00".repeat(48))) {
            assertFails { SumeragiStatus.parseJson(changed { it["leader"] = bad }) }
        }
    }
    @Test fun `horizon and halt shape retain exact optional semantics`() {
        val h = SumeragiJsonPrimitives.objectValue(root()["beacon_horizon"], "horizon")
        for ((field, value) in listOf("active_session_id" to null, "next_required_pulse_height" to null,
            "active_session_id" to "ab".repeat(32), "local_provider_ready" to 1L)) {
            assertFails { SumeragiStatus.parseJson(changed { it["beacon_horizon"] = h + (field to value) }) }
        }
        for (bad in listOf(mapOf("reason" to "unknown", "details" to null),
            mapOf("reason" to "driver_anomaly", "details" to 1L),
            mapOf("reason" to "apply_diverged", "details" to null),
            mapOf("reason" to "safety_record_corrupt"))) {
            assertFails { SumeragiStatus.parseJson(changed { it["halted"] = bad }) }
        }
    }
    @Test fun `available horizon with no demand retains explicit nullable fields`() {
        val value = SumeragiStatus.parseJson(changed {
            it["beacon_horizon"] = mapOf("epoch_length_blocks" to 0L,
                "next_required_pulse_height" to null, "active_session_id" to null,
                "session_covers_next_pulse" to false, "local_provider_ready" to false)
        })
        val horizon = requireNotNull(value.beaconHorizon)
        assertEquals(BigInteger.ZERO, horizon.epochLengthBlocks)
        assertNull(horizon.activeSessionId)
        assertNull(horizon.nextRequiredPulseHeight)
        assertFalse(horizon.localProviderReady)
        assertEquals(value, SumeragiStatusWire.decodeCanonical(SumeragiStatusWire.encode(value)))
    }

}
