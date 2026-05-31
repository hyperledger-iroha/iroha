package org.hyperledger.iroha.android.nexus;

/** App-role Connect dependency used by {@link NexusAppClient}. */
public interface NexusConnectTransport {

  NexusConnectSession startConnect(NexusConnectOptions options, NexusAppConfig config);

  NexusApprovedAccount awaitApproval(NexusConnectSession session, NexusAppConfig config);

  NexusWalletSignature requestSignature(
      NexusConnectSession session, NexusSignableTransaction signable, NexusAppConfig config);
}
