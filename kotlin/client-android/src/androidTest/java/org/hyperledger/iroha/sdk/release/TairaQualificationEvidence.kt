// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.release

import android.system.Os
import android.system.OsConstants
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileOutputStream
import java.nio.ByteBuffer
import java.nio.CharBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Originals exporter, not an authority, release receipt, or controller execution report. */
internal class TairaQualificationEvidence(val arguments: TairaQualificationArguments) {
    private val root: File
    private val activeTests = mutableSetOf<String>()
    private val completedCases = linkedMapOf<String, Map<String, Any?>>()

    init {
        val files = InstrumentationRegistry.getInstrumentation().context.filesDir.canonicalFile
        val parent = File(files, "taira-original-evidence")
        if (!parent.exists()) Os.mkdir(parent.path, PRIVATE_DIRECTORY_MODE)
        requirePrivateDirectory(parent)
        root = File(parent, arguments.runId)
        // mkdir is exclusive: an existing run is never reused, deleted or overwritten.
        Os.mkdir(root.path, PRIVATE_DIRECTORY_MODE)
        requirePrivateDirectory(root)
        if (arguments.kind == "native-ledger-wire") {
            Os.mkdir(File(root, "cases").path, PRIVATE_DIRECTORY_MODE)
        }
    }

    @Synchronized fun testStarted(selector: String) {
        check(selector in arguments.selectedTests && activeTests.add(selector)) {
            "Unexpected or duplicate active qualification test"
        }
    }

    @Synchronized fun testFinished(selector: String) {
        check(activeTests.remove(selector)) { "Qualification test was not active" }
    }

    @Synchronized fun requireActive(selector: String) {
        check(selector in activeTests) { "Original listener must start the exact test" }
    }

    @Synchronized fun caseDirectory(selector: String): String {
        requireActive(selector)
        check(arguments.kind == "native-ledger-wire") { "Wire evidence on a managed run" }
        val method = selector.substringAfter('/')
        check(Regex("[A-Za-z][A-Za-z0-9]+").matches(method))
        val relative = "cases/$method"
        Os.mkdir(File(root, relative).path, PRIVATE_DIRECTORY_MODE)
        return relative
    }

    @Synchronized fun retainCase(selector: String, value: Map<String, Any?>) {
        requireActive(selector)
        check(!completedCases.containsKey(selector)) { "Wire case is already retained" }
        completedCases[selector] = value
    }

    @Synchronized fun retainCasesManifest() {
        if (arguments.kind == "native-ledger-wire") {
            writeJson("cases.json", mapOf("cases" to arguments.selectedTests.mapNotNull {
                completedCases[it]
            }))
        }
    }

    @Synchronized fun writeJson(relative: String, value: Any?): Map<String, Any?> =
        write(relative, (JsonEncoder.encode(value) + "\n").toByteArray(StandardCharsets.UTF_8))

    @Synchronized fun write(relative: String, bytes: ByteArray): Map<String, Any?> {
        check(Regex("[A-Za-z0-9._/-]+").matches(relative)) { "Unsafe original path" }
        check(relative.split('/').all { it.isNotEmpty() && it != "." && it != ".." })
        val file = File(root, relative)
        val parentPath = file.parentFile!!.canonicalPath
        check(parentPath == root.canonicalPath || parentPath.startsWith(root.canonicalPath + "/"))
        requirePrivateDirectory(file.parentFile!!)
        val descriptor = Os.open(
            file.path,
            OsConstants.O_WRONLY or OsConstants.O_CREAT or OsConstants.O_EXCL or
                OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            PRIVATE_FILE_MODE,
        )
        FileOutputStream(descriptor).use { output ->
            output.write(bytes)
            output.flush()
            descriptor.sync()
        }
        return mapOf(
            "path" to "android-sdk-qualification/${arguments.runId}/$relative",
            "sizeBytes" to bytes.size,
            "sha256" to MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") {
                String.format(Locale.ROOT, "%02x", it.toInt() and 0xff)
            },
        )
    }

    companion object {
        private const val PRIVATE_DIRECTORY_MODE = 448 // 0700
        private const val PRIVATE_FILE_MODE = 384 // 0600
        private var active: TairaQualificationEvidence? = null

        @Synchronized fun start(arguments: TairaQualificationArguments): TairaQualificationEvidence {
            check(active == null) { "One original-evidence run per instrumentation process" }
            return TairaQualificationEvidence(arguments).also { active = it }
        }

        @Synchronized fun current(selector: String): TairaQualificationEvidence =
            checkNotNull(active) { "Original listener was not initialized" }.also {
                it.requireActive(selector)
            }

        fun utf8(bytes: ByteArray): String = StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(bytes)).toString()

        fun utf8(text: String): ByteArray {
            val encoded = StandardCharsets.UTF_8.newEncoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .encode(CharBuffer.wrap(text))
            return ByteArray(encoded.remaining()).also { encoded.get(it) }
        }

        fun timestamp(): String = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.ROOT)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }.format(Date())

        private fun requirePrivateDirectory(directory: File) {
            val stat = Os.lstat(directory.path)
            check(OsConstants.S_ISDIR(stat.st_mode) && (stat.st_mode and 63) == 0) {
                "Evidence directory must be private and must not be a symlink"
            }
        }
    }
}
