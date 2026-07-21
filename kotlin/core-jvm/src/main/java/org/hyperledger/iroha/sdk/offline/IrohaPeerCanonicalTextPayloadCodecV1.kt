package org.hyperledger.iroha.sdk.offline

/**
 * Exact Offline Note UTF-8 application-byte boundary shared by QR, NFC, and Nearby.
 *
 * Kagemusha profile 2 uses typed native archives and is deliberately rejected
 * here so callers cannot bypass [KagemushaPeerPayload.decode].
 */
object IrohaPeerCanonicalTextPayloadCodecV1 {
    @JvmStatic
    @JvmOverloads
    fun maximumCanonicalTextBytes(
        profile: IrohaPeerPayloadProfile,
        limits: IrohaPeerWireLimitsV1 = IrohaPeerWireLimitsV1.PEER_V1,
    ): Int {
        requireOfflineNote(profile)
        return minOf(
            limits.maximumCanonicalBytes,
            limits.maximumEncodedBytes(profile),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun canonicalBytes(
        text: String,
        profile: IrohaPeerPayloadProfile,
        limits: IrohaPeerWireLimitsV1 = IrohaPeerWireLimitsV1.PEER_V1,
    ): ByteArray {
        requireOfflineNote(profile)
        require(text.isNotEmpty()) { "Peer canonical text must not be empty" }
        val bytes = text.toByteArray(Charsets.UTF_8)
        require(bytes.toString(Charsets.UTF_8) == text) {
            "Peer canonical text is not exact UTF-8"
        }
        requireWithinBound(bytes.size, profile, limits)
        return bytes
    }

    @JvmStatic
    @JvmOverloads
    fun canonicalText(
        bytes: ByteArray,
        profile: IrohaPeerPayloadProfile,
        limits: IrohaPeerWireLimitsV1 = IrohaPeerWireLimitsV1.PEER_V1,
    ): String {
        requireOfflineNote(profile)
        require(bytes.isNotEmpty()) { "Peer canonical text must not be empty" }
        requireWithinBound(bytes.size, profile, limits)
        val text = bytes.toString(Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) {
            "Peer canonical payload is not exact UTF-8"
        }
        return text
    }

    private fun requireWithinBound(
        actual: Int,
        profile: IrohaPeerPayloadProfile,
        limits: IrohaPeerWireLimitsV1,
    ) {
        val maximum = maximumCanonicalTextBytes(profile, limits)
        require(actual <= maximum) {
            "Peer canonical text for ${profile.name} is $actual bytes; maximum is $maximum"
        }
    }

    private fun requireOfflineNote(profile: IrohaPeerPayloadProfile) {
        require(profile == IrohaPeerPayloadProfile.OFFLINE_NOTE) {
            "Peer generic canonical text is only supported for OFFLINE_NOTE"
        }
    }
}
