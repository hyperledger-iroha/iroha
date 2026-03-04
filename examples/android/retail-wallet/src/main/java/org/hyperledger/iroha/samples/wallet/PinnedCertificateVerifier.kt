package org.hyperledger.iroha.samples.wallet

import android.content.Context
import android.util.Base64
import java.io.InputStream
import java.security.MessageDigest
import java.util.Locale

data class PinnedRootStatus(
    val configuredAliases: List<String>,
    val configuredDigests: List<String>,
    val matchedAlias: String?,
    val expectedSha256: String?,
    val actualSha256: String,
    val matched: Boolean
)

object PinnedCertificateVerifier {

    private const val PINNED_ROOT_ASSET = "pinned_root.pem"

    fun verifyPinnedRoot(
        context: Context,
        policy: SecurityPolicy,
        auditLogger: PosAuditLogger
    ): PinnedRootStatus {
        val input = context.assets.open(PINNED_ROOT_ASSET)
        val actualFingerprint = input.use { fingerprint(it) }
        val matchingRoot = policy.pinnedRoots.firstOrNull { root ->
            root.sha256.equals(actualFingerprint, ignoreCase = true)
        }
        val status = PinnedRootStatus(
            configuredAliases = policy.pinnedRoots.map { it.alias },
            configuredDigests = policy.pinnedRoots.map { it.sha256 },
            matchedAlias = matchingRoot?.alias,
            expectedSha256 = matchingRoot?.sha256,
            actualSha256 = actualFingerprint,
            matched = matchingRoot != null
        )
        auditLogger.logPinVerification(status, policy.version)
        return status
    }

    fun fingerprint(input: InputStream): String {
        val raw = decodePem(input.readBytes())
        val digest = MessageDigest.getInstance("SHA-256")
        val hash = digest.digest(raw)
        val builder = StringBuilder(hash.size * 2)
        for (byte in hash) {
            builder.append(String.format(Locale.US, "%02x", byte))
        }
        return builder.toString()
    }

    private fun decodePem(data: ByteArray): ByteArray {
        val pem = String(data)
        val stripped = pem
            .replace("-----BEGIN CERTIFICATE-----", "")
            .replace("-----END CERTIFICATE-----", "")
            .replace("\\s".toRegex(), "")
        return Base64.decode(stripped, Base64.DEFAULT)
    }

}
