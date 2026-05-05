package org.hyperledger.iroha.sdk.tx

class MultisigSignatures private constructor(signatures: List<MultisigSignature>) {

    private val _signatures: List<MultisigSignature>

    init {
        for (signature in signatures) {
            requireNotNull(signature) { "signatures must not contain null entries" }
        }
        _signatures = signatures.toList()
    }

    val signatures: List<MultisigSignature> get() = _signatures

    companion object {
        @JvmStatic
        fun of(signatures: List<MultisigSignature>): MultisigSignatures =
            MultisigSignatures(signatures)

        @JvmStatic
        fun of(vararg signatures: MultisigSignature): MultisigSignatures =
            MultisigSignatures(signatures.toList())
    }
}
