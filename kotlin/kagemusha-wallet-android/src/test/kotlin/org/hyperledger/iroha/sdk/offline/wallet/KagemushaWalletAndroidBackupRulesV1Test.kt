// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * The library manifest and backup rules keep the custody backup set empty (G2 design rev 2):
 * `allowBackup="false"` and every domain excluded for cloud backup and device transfer, so no
 * restore stream reaches the app and clears `no_backup` and its Keystore namespace.
 */
class KagemushaWalletAndroidBackupRulesV1Test {
    private val main = File("src/main")

    private fun document(path: String): Element {
        val file = File(main, path)
        assertTrue(file.isFile, "missing ${file.absolutePath}")
        val factory = DocumentBuilderFactory.newInstance().apply {
            isNamespaceAware = true
            setFeature("http://apache.org/xml/features/disallow-doctype-decl", true)
        }
        return factory.newDocumentBuilder().parse(file).documentElement
    }

    private fun children(element: Element): List<Element> {
        val nodes = element.childNodes
        return (0 until nodes.length).mapNotNull { nodes.item(it) as? Element }
    }

    private fun assertExcludesEveryDomain(section: Element) {
        val rules = children(section)
        assertTrue(rules.all { it.tagName == "exclude" }, "${section.tagName} may only exclude")
        assertTrue(rules.all { it.getAttribute("path") == "." }, "${section.tagName} must exclude whole domains")
        assertEquals(DOMAINS, rules.map { it.getAttribute("domain") }.toSet(), section.tagName)
        assertEquals(DOMAINS.size, rules.size, section.tagName)
    }

    @Test fun `the library manifest disables backup and names both rule files`() {
        val manifest = document("AndroidManifest.xml")
        val application = children(manifest).single { it.tagName == "application" }
        assertEquals("false", application.getAttributeNS(ANDROID, "allowBackup"))
        assertEquals("@xml/kagemusha_wallet_v1_data_extraction_rules", application.getAttributeNS(ANDROID, "dataExtractionRules"))
        assertEquals("@xml/kagemusha_wallet_v1_full_backup_content", application.getAttributeNS(ANDROID, "fullBackupContent"))
        assertFalse(application.hasAttributeNS(ANDROID, "backupAgent"))
        assertFalse(application.hasAttributeNS(ANDROID, "fullBackupOnly"))
    }

    @Test fun `cloud backup and device transfer exclude every domain`() {
        val rules = document("res/xml/kagemusha_wallet_v1_data_extraction_rules.xml")
        assertEquals("data-extraction-rules", rules.tagName)
        val sections = children(rules)
        assertEquals(listOf("cloud-backup", "device-transfer"), sections.map { it.tagName })
        sections.forEach(::assertExcludesEveryDomain)
    }

    @Test fun `legacy full backup excludes every domain`() {
        val rules = document("res/xml/kagemusha_wallet_v1_full_backup_content.xml")
        assertEquals("full-backup-content", rules.tagName)
        assertExcludesEveryDomain(rules)
    }

    @Test fun `the payment-key adapter never binds the key to user state or reads absence from masking APIs`() {
        val sources = listOf("KagemushaWalletAndroidKeyStoreV1.kt", "KagemushaWalletAndroidPaymentKeyV1.kt")
            .map { File(main, "java/org/hyperledger/iroha/sdk/offline/wallet/$it") }
        for (source in sources) {
            assertTrue(source.isFile, "missing ${source.absolutePath}")
            val text = source.readText()
            for (forbidden in FORBIDDEN_CALLS) {
                assertFalse(text.contains(forbidden), "${source.name} must not call $forbidden")
            }
        }
    }

    private companion object {
        const val ANDROID = "http://schemas.android.com/apk/res/android"

        val DOMAINS = setOf(
            "root", "file", "database", "sharedpref", "external",
            "device_root", "device_file", "device_database", "device_sharedpref",
        )

        /** Key-binding setters spec §2.3 forbids, and AndroidKeyStoreSpi calls that mask errors as absence. */
        val FORBIDDEN_CALLS = listOf(
            "setUserAuthenticationRequired(", "setUserAuthenticationParameters(",
            "setUserAuthenticationValidityDurationSeconds(", "setUserAuthenticationValidWhileOnBody(",
            "setInvalidatedByBiometricEnrollment(", "setUnlockedDeviceRequired(", "setMaxUsageCount(",
            "setUserPresenceRequired(", "setUserConfirmationRequired(",
            "containsAlias(", ".aliases()", ".size()", "isKeyEntry(", "isCertificateEntry(",
        )
    }
}
