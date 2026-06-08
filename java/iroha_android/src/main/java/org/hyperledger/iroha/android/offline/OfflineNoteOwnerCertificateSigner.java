package org.hyperledger.iroha.android.offline;

/**
 * Mints owner-self-signed Offline Note key certificates for P2P output claims.
 *
 * <p>Output certificates must be signed by the owner account named in the certificate accountId,
 * not by the issuer/operator key used for load and topup paths.
 */
public interface OfflineNoteOwnerCertificateSigner {
  /**
   * Returns a fresh certificate for {@code accountId}.
   *
   * <p>The account must encode a single Ed25519 signatory, and the certificate issuerSignature must
   * be a raw Ed25519 signature over {@link OfflineNote.KeyCertificate#signingBytes()}.
   */
  OfflineNote.KeyCertificate freshOwnerCertificate(String accountId);
}
