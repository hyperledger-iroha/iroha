package org.hyperledger.iroha.sdk.tx.norito

import java.io.File
import org.hyperledger.iroha.sdk.client.JsonParser

/** Shared Rust-produced bytes for Kotlin and Java consumer assertions. */
object InstructionBatchHashFixture {
    private val values: Map<*, *> by lazy {
        val path = "fixtures/multisig/instruction_batch_hash_v1.json"
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, path) }.first(File::isFile)
        (JsonParser.parse(file.readText(Charsets.UTF_8)) as Map<*, *>).also {
            check(it["schema"] == "iroha.multisig.instruction-batch-hash.v1")
            check((it["layout_flags"] as Number).toInt() == 2)
        }
    }

    @JvmStatic
    fun bytes(key: String): ByteArray {
        val value = values[key] as String
        check(value.length % 2 == 0)
        return value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}
