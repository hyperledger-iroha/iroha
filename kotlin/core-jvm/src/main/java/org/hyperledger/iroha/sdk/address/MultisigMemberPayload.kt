package org.hyperledger.iroha.sdk.address

class MultisigMemberPayload(
    @JvmField val curveId: Int,
    @JvmField val weight: Int,
    publicKey: ByteArray,
) {
    private val _publicKey: ByteArray = publicKey.copyOf()

    val publicKey: ByteArray get() = _publicKey.copyOf()
}

/** Rust MultisigPolicy orders complete keys by canonical algorithm name, then unsigned payload. */
internal fun compareMultisigMemberKeys(left: MultisigMemberPayload, right: MultisigMemberPayload): Int {
    val leftAlgorithm = requireNotNull(algorithmForCurveId(left.curveId)) { "Unknown multisig member curve" }
    val rightAlgorithm = requireNotNull(algorithmForCurveId(right.curveId)) { "Unknown multisig member curve" }
    val algorithmOrder = leftAlgorithm.compareTo(rightAlgorithm)
    if (algorithmOrder != 0) return algorithmOrder
    val leftKey = left.publicKey
    val rightKey = right.publicKey
    for (index in 0 until minOf(leftKey.size, rightKey.size)) {
        val byteOrder = (leftKey[index].toInt() and 0xFF).compareTo(rightKey[index].toInt() and 0xFF)
        if (byteOrder != 0) return byteOrder
    }
    return leftKey.size.compareTo(rightKey.size)
}
