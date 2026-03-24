package org.hyperledger.iroha.sdk.connect

import java.util.Locale

/** Connect transport direction (app -> wallet or wallet -> app). */
enum class ConnectDirection(
    @JvmField val tag: Byte,
    @JvmField val label: String,
) {
    APP_TO_WALLET(0.toByte(), "app_to_wallet"),
    WALLET_TO_APP(1.toByte(), "wallet_to_app");

    companion object {
        @JvmStatic
        fun fromLabel(value: String?): ConnectDirection {
            require(!value.isNullOrEmpty()) { "direction label must not be null or empty" }
            val normalized = value.trim().lowercase(Locale.ROOT)
            for (direction in entries) {
                if (direction.label == normalized) return direction
            }
            throw IllegalArgumentException("unknown Connect direction label: $value")
        }

        @JvmStatic
        fun fromTag(tag: Int): ConnectDirection {
            for (direction in entries) {
                if (direction.tag == tag.toByte()) return direction
            }
            throw IllegalArgumentException("unknown Connect direction tag: $tag")
        }
    }
}
