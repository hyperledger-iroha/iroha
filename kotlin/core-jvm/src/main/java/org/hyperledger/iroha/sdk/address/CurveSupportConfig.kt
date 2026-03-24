package org.hyperledger.iroha.sdk.address

class CurveSupportConfig private constructor(
    @JvmField val allowMlDsa: Boolean,
    @JvmField val allowGost: Boolean,
    @JvmField val allowSm2: Boolean,
) {
    companion object {
        @JvmStatic
        fun ed25519Only(): CurveSupportConfig = CurveSupportConfig(
            allowMlDsa = false,
            allowGost = false,
            allowSm2 = false,
        )

        @JvmStatic
        fun builder(): Builder = Builder()
    }

    class Builder {
        private var allowMlDsa = false
        private var allowGost = false
        private var allowSm2 = false

        fun allowMlDsa(value: Boolean): Builder = apply { allowMlDsa = value }
        fun allowGost(value: Boolean): Builder = apply { allowGost = value }
        fun allowSm2(value: Boolean): Builder = apply { allowSm2 = value }

        fun build(): CurveSupportConfig = CurveSupportConfig(allowMlDsa, allowGost, allowSm2)
    }
}
