package org.hyperledger.iroha.sdk.offline

/**
 * Mints owner-self-signed Offline Note key certificates for P2P output claims.
 *
 * Output certificates must be signed by the owner account named in the certificate accountId, not
 * by the issuer/operator key used for load and topup paths. The implementation lives in the
 * application because only it holds the owner account signing key.
 */
interface OfflineNoteOwnerCertificateSigner {
    /**
     * Returns a fresh certificate for [accountId].
     *
     * The account must encode a single Ed25519 signatory, and the certificate issuerSignature must
     * be a raw Ed25519 signature over [OfflineNote.KeyCertificate.signingBytes]. The certificate
     * payload must be fresh for every output because its payload hash is a one-use replay anchor.
     */
    fun freshOwnerCertificate(accountId: String): OfflineNote.KeyCertificate
}
