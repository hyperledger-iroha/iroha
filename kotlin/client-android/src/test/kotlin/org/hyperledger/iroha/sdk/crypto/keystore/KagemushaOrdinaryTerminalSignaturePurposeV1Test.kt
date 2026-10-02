// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import java.nio.file.Files
import java.nio.file.Paths
import org.junit.jupiter.api.Test
import kotlin.test.assertFailsWith

/** Actual private production purpose validator, TESTDATA only; no OS/Native/proof grant. */
class KagemushaOrdinaryTerminalSignaturePurposeV1Test {
    @Test fun realTerminalPurposeIsOneAndRefusesPreparationTwo() {
        val w=vector("w_send_split_9")
        requireAppPlatformSigningMessageV1(w,KagemushaAndroidAppSignaturePurposeV1.ORDINARY_CASH_TERMINAL_APPROVAL)
        assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(w,
            KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL) }
        val changed=w.copyOf().also { it[52]=2 }
        assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(changed,
            KagemushaAndroidAppSignaturePurposeV1.ORDINARY_CASH_TERMINAL_APPROVAL) }
    }
    @Test fun terminalOriginalCannotUseFinancialSelectionBytesOrTrailingApproval() {
        for (original in listOf(vector("s_send_split_9"),vector("w_send_split_9")+byteArrayOf(0))) {
            assertFailsWith<IllegalArgumentException> { requireAppPlatformSigningMessageV1(original,
                KagemushaAndroidAppSignaturePurposeV1.ORDINARY_CASH_TERMINAL_APPROVAL) }
        }
    }
    private fun vector(name: String): ByteArray {
        val path=sequenceOf("fixtures","../fixtures","../../fixtures").map {
            Paths.get(it,"offline/kagemusha_app_platform_messages_v1.tsv") }.first { Files.isRegularFile(it) }
        return Files.readAllLines(path).first { it.startsWith("$name\t") }.substringAfter('\t').chunked(2)
            .map { it.toInt(16).toByte() }.toByteArray()
    }
}
