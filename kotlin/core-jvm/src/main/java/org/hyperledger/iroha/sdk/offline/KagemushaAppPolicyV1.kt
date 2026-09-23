// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest

private val APP_AUTHORITY_POLICY_DOMAIN_V1 =
    "iroha:kagemusha:v1:app-attestation-authority-policy\u0000".toByteArray(StandardCharsets.US_ASCII)
private val APP_STATIC_BINDING_DOMAIN_V1 =
    "iroha:kagemusha:v1:app-device-static-binding\u0000".toByteArray(StandardCharsets.US_ASCII)

/**
 * Shape-only mirror of the deployment-owned app-attestation policy digest.
 *
 * [authorityKeyCanonicalNorito] must come from the governed Ed25519 authority selection as the
 * exact canonical `iroha_crypto::PublicKey` archive. This helper does not authorize that key,
 * app, profile, or any monetary transition; native Core validates the governed policy.
 */
class KagemushaAppAttestationAuthorityPolicyV1(
    authorityKeyCanonicalNorito: ByteArray,
    val platformClass: KagemushaHardwarePlatformClassV1,
    appSigningIdentityDigest: ByteArray,
    appReleaseDigest: ByteArray,
    val maximumLifetimeMs: Long,
) {
    private val authorityKey = authorityKeyCanonicalNorito.copyOf()
    private val signingIdentity = fixed32(appSigningIdentityDigest, "appSigningIdentityDigest")
    private val appRelease = fixed32(appReleaseDigest, "appReleaseDigest")

    init {
        require(authorityKey.isNotEmpty() && authorityKey.size <= 512)
        require(signingIdentity.any { it != 0.toByte() } && appRelease.any { it != 0.toByte() })
        require(maximumLifetimeMs != 0L)
    }

    fun authorityKeyCanonicalNorito(): ByteArray = authorityKey.copyOf()
    fun appSigningIdentityDigest(): ByteArray = signingIdentity.copyOf()
    fun appReleaseDigest(): ByteArray = appRelease.copyOf()

    /** Exact Rust V1 policy hash, including one platform-class byte after key length and bytes. */
    fun canonicalDigestShape(): ByteArray = MessageDigest.getInstance("SHA-256").digest(
        APP_AUTHORITY_POLICY_DOMAIN_V1 + u64LeAppPolicyV1(authorityKey.size.toLong()) +
            authorityKey + byteArrayOf(platformClass.ordinal.toByte()) + signingIdentity +
            appRelease + u64LeAppPolicyV1(maximumLifetimeMs),
    )
}

/** Stable enrollment policy binding approved before challenge nonces. */
class KagemushaAppDevicePolicyBindingV1(
    appSigningIdentityDigest: ByteArray,
    appReleaseDigest: ByteArray,
    releaseId: ByteArray,
    hardwareProfileId: ByteArray,
    deviceKeyReference: ByteArray,
    laneId: ByteArray,
) {
    private val fields = listOf(
        fixed32(appSigningIdentityDigest, "appSigningIdentityDigest"),
        fixed32(appReleaseDigest, "appReleaseDigest"),
        fixed32(releaseId, "releaseId"),
        fixed32(hardwareProfileId, "hardwareProfileId"),
        fixed32(deviceKeyReference, "deviceKeyReference"),
        fixed32(laneId, "laneId"),
    )

    init { require(fields.all { it.any { byte -> byte != 0.toByte() } }) }

    /** Exact Rust V1 static app/device binding hash, without an authority claim. */
    fun canonicalDigestShape(): ByteArray = MessageDigest.getInstance("SHA-256").run {
        update(APP_STATIC_BINDING_DOMAIN_V1)
        fields.forEach(::update)
        digest()
    }
}

private fun u64LeAppPolicyV1(value: Long): ByteArray =
    ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
