package org.hyperledger.iroha.sdk.examples

import java.math.BigInteger
import java.security.SecureRandom
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.privacy.*

/** Runnable disposable local wallet example; it never creates or submits a transaction. */
object ConfidentialRedemptionExample {
    @JvmStatic fun main(arguments: Array<String>) {
        require(arguments.isEmpty()) { "this local example accepts no private command-line arguments" }
        val proof = generateLocalProof()
        println("Locally verified ${proof.relation}: ${proof.proof.size} bytes, ${proof.nullifiers.size} input note.")
    }

    /** Applications use authenticated network, asset, and root identifiers instead of local fixtures. */
    fun generateLocalProof(): ConfidentialProof {
        val random = SecureRandom()
        val networkBytes = ByteArray(32).also(random::nextBytes)
        networkBytes[31] = (networkBytes[31].toInt() or 1).toByte()
        val network = NetworkId.fromBytes(networkBytes)
        val assetBytes = ByteArray(16).also(random::nextBytes)
        assetBytes[6] = ((assetBytes[6].toInt() and 15) or 0x40).toByte()
        assetBytes[8] = ((assetBytes[8].toInt() and 63) or 0x80).toByte()
        val asset = AssetDefinitionIdEncoder.encodeFromBytes(assetBytes)
        val diversifier = ConfidentialOwnerTag.defaultDiversifier()
        val key = ByteArray(32)
        val rho = ByteArray(32)
        try {
            // Install cleanup before a random provider can partially fill either secret.
            random.nextBytes(key)
            random.nextBytes(rho)
            val owner = ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(key, diversifier)
            val commitment = ConfidentialNoteCommitment.derive(asset, "42", rho, owner)
            val path = LocalZkAssetMerklePathProvider(emptyList(), listOf(commitment))
                .getMerklePathForCommitment(asset, commitment).join()
            ConfidentialProver.create(network, asset, key).use { prover ->
                key.fill(0)
                val input = ConfidentialInputNote(BigInteger.valueOf(42), rho, diversifier, 0)
                val tree = ConfidentialTreeEvidence.Paths(path.rootAtHeight, listOf(path))
                return prover.proveUnshield(tree, listOf(input), BigInteger.valueOf(42))
            }
        } finally { key.fill(0); rho.fill(0); diversifier.fill(0) }
    }
}
