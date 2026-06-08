package org.hyperledger.iroha.sdk.offline

/** Offline Note proof verifier that verifies against a chain-supplied `VerifyingKeyBox`. */
class ChainVkOfflineNoteProofVerifier(vkBoxNorito: ByteArray) : OfflineNoteProofVerifier {
    private val vkBoxNorito = vkBoxNorito.copyOf()

    init {
        require(this.vkBoxNorito.isNotEmpty()) { "vkBoxNorito must not be empty" }
    }

    override fun verifyAudit(audit: OfflineNote.AuditBundle): Boolean =
        NativeOfflineNoteProver.verifyAudit(OfflineNote.encodeAudit(audit), vkBoxNorito)

    override fun verifyRedeem(redemption: OfflineNote.Redeem): Boolean =
        NativeOfflineNoteProver.verifyRedeem(OfflineNote.encodeRedeem(redemption), vkBoxNorito)
}
