// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/**
 * Exact public installation originals, retained as bounded DATA until Native authenticates them.
 * The independent application trust key is compiled into Native and cannot be supplied here.
 *
 * [originalsRoot] is the exact UTF-8 absolute path to the retained private financial directory:
 * verifier-pack.norito, producer-inventory.norito, transport.json, wallet-originals/ and
 * finality-originals/. Native opens the original files without following links and verifies the
 * complete authenticated catalog. This constructor neither reads nor creates that directory.
 *
 * All seven originals are required for every installation. Native authenticates the signed
 * application/runtime/genesis and complete financial sources before returning runtime ownership.
 */
class KagemushaWalletInstallationOriginalsV1(
    appManifest: ByteArray,
    signatureEnvelope: ByteArray,
    walletRuntime: ByteArray,
    verifierPack: ByteArray,
    producerInventory: ByteArray,
    signedGenesis: ByteArray,
    originalsRoot: ByteArray,
) {
    private val originals: List<ByteArray>

    init {
        val values = listOf(appManifest, signatureEnvelope, walletRuntime, verifierPack,
            producerInventory, signedGenesis, originalsRoot)
        val bounds = intArrayOf(8 * 1024 * 1024, 2048, 128 * 1024,
            16 * 1024 * 1024 + 65536, 16 * 1024 * 1024, 64 * 1024 * 1024, 4096)
        require(values.indices.all { values[it].size <= bounds[it] }) {
            "installation original exceeds its native input bound"
        }
        require(values.all { it.isNotEmpty() }) {
            "all seven installation originals are required"
        }
        originals = values.map { it.copyOf() }
    }

    internal fun frames(): List<ByteArray> = originals.map { it.copyOf() }
    override fun toString(): String = "KagemushaWalletInstallationOriginalsV1(originals=[REDACTED])"
}

/** Installs the actual authenticated Native proof runtime, before enrolled account admission. */
object KagemushaWalletInstalledRuntimeV1 {
    /**
     * Run on a worker: Native authenticates every installation original and qualifies its proof
     * sources before retaining the platform and custody provider. Missing inputs are invalid;
     * unavailable or stale JNI returns [KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE].
     *
     * The returned owner still requires [KagemushaWalletRuntimeV1.begin] and the existing account's
     * exact Ed25519 challenge signature. Installation generates no keys and admits no wallet.
     */
    @JvmStatic
    fun install(platform: KagemushaWalletAndroidPlatformV1,
        originals: KagemushaWalletInstallationOriginalsV1): KagemushaWalletRuntimeV1 {
        if (!PrivacyNativeBridge.isNativeAvailable()) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
        }
        val frames = originals.frames()
        val result = try {
            if (KagemushaWalletNativeV1.revision() != 1) {
                throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
            }
            KagemushaWalletInstalledRuntimeNativeV1.installRuntime(platform,
                frames[0], frames[1], frames[2], frames[3], frames[4], frames[5], frames[6])
        } catch (_: LinkageError) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
        }
        return KagemushaWalletRuntimeV1(installationRuntimeHandle(result))
    }
}

/** JNI returns one positive registry owner or a negative i32 Native failure, never a verdict. */
internal fun installationRuntimeHandle(result: Long): Long {
    if (result in Int.MIN_VALUE.toLong()..-1L) throw KagemushaWalletExceptionV1(result.toInt())
    if (result <= 0) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    return result
}

/** Bound to the original-only loader in connect_norito_bridge, with no caller-selected trust. */
internal object KagemushaWalletInstalledRuntimeNativeV1 {
    @JvmStatic external fun installRuntime(platform: KagemushaWalletAndroidPlatformV1,
        appManifest: ByteArray, signatureEnvelope: ByteArray, walletRuntime: ByteArray,
        verifierPack: ByteArray, producerInventory: ByteArray, signedGenesis: ByteArray,
        originalsRoot: ByteArray): Long
}
