package org.hyperledger.iroha.android.nexus;

/** Wallet approval result for an app-role Connect session. */
public final class NexusApprovedAccount {

  private final String accountId;
  private final byte[] signingPublicKey;
  private final NexusConnectSession session;

  public NexusApprovedAccount(final String accountId, final byte[] signingPublicKey) {
    this(accountId, signingPublicKey, null);
  }

  public NexusApprovedAccount(
      final String accountId,
      final byte[] signingPublicKey,
      final NexusConnectSession session) {
    this.accountId = accountId == null ? "" : accountId;
    this.signingPublicKey = NexusModelUtils.copy(signingPublicKey);
    this.session = session;
  }

  public String accountId() {
    return accountId;
  }

  public byte[] signingPublicKey() {
    return NexusModelUtils.copy(signingPublicKey);
  }

  public NexusConnectSession session() {
    return session;
  }

  NexusApprovedAccount withSessionAndKey(
      final NexusConnectSession session, final byte[] signingPublicKey) {
    return new NexusApprovedAccount(accountId, signingPublicKey, session);
  }
}
