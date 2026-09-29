package org.hyperledger.iroha.sdk.consensus

import java.io.File

/** Exact Rust-generated native status corpus; missing capture is a test failure. */
object NativeStatusFixtures {
    @JvmStatic fun rows(): Map<String, Pair<String, String>> {
        val relative = "fixtures/sumeragi/native_status_v1.tsv"
        val file = generateSequence(File(System.getProperty("user.dir"))) { it.parentFile }
            .map { File(it, relative) }.firstOrNull { it.isFile }
            ?: error("missing shared native status fixture: $relative")
        val rows = file.readLines(Charsets.UTF_8).filter { it.isNotBlank() && !it.startsWith("#") }.map {
            val fields = it.split('\t')
            check(fields.size == 3) { "invalid native status fixture row" }
            check(fields[2].isNotEmpty() && fields[2].length % 2 == 0 && fields[2].all { it in "0123456789abcdef" })
            fields[0] to (fields[1] to fields[2])
        }
        check(rows.map { it.first }.toSet().size == rows.size) { "duplicate native status fixture case" }
        val byName = rows.toMap()
        check(byName.keys == setOf("validator", "observer", "safety_record_corrupt", "safety_record_inconsistent",
            "safety_violation", "apply_diverged", "publication_recovery_required", "driver_anomaly")) {
            "native status fixture must contain exactly every canonical case"
        }
        return byName
    }
    @JvmStatic fun json(name: String = "validator"): String = requireNotNull(rows()[name]).first
}
