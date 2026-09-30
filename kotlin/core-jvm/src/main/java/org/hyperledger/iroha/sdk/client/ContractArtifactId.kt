package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** Exact full-width dataspace and canonical complete-artifact hash within one network. */
class ContractArtifactId(dataspaceId: BigInteger, @JvmField val codeHash: String) {
    /** Unsigned 64-bit dataspace, defensively copied from the supplied numeric value. */
    @JvmField val dataspaceId: BigInteger = BigInteger(dataspaceId.toByteArray())
    /** Canonical lowercase complete-artifact hash used in Torii paths. */
    @JvmField val codeHashHex: String

    init {
        require(this.dataspaceId.signum() >= 0 && this.dataspaceId.bitLength() <= 64) {
            "dataspaceId must be an unsigned 64-bit integer"
        }
        val bytes = HashLiteral.decode(codeHash)
        require(bytes.size == 32 && (bytes.last().toInt() and 1) == 1 && HashLiteral.canonicalize(bytes) == codeHash) {
            "codeHash must be an exact canonical marked hash literal"
        }
        codeHashHex = bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }

    override fun equals(other: Any?): Boolean = this === other ||
        other is ContractArtifactId && dataspaceId == other.dataspaceId && codeHash == other.codeHash

    override fun hashCode(): Int = 31 * dataspaceId.hashCode() + codeHash.hashCode()
}
