package org.hyperledger.iroha.android.tx;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.offline.OfflineEnvelopeOptions;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.SigningException;

public final class TransactionBuilderOfflineEnvelopeTests {

  private TransactionBuilderOfflineEnvelopeTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAndSignEnvelopeIncludesMetadata();
    encodeAndSignEnvelopeWithExportedKey();
    System.out.println("[IrohaAndroid] Transaction builder offline envelope tests passed.");
  }

  private static void encodeAndSignEnvelopeIncludesMetadata()
      throws NoritoException, KeyManagementException, SigningException {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final NoritoCodecAdapter codecAdapter = new NoritoJavaCodecAdapter();
    final TransactionBuilder builder = new TransactionBuilder(codecAdapter, keyManager);

    final TransactionPayload payload = TransactionPayload.builder().build();
    final OfflineEnvelopeOptions options =
        OfflineEnvelopeOptions.builder()
            .setIssuedAtMs(1234L)
            .putMetadata("context", "offline")
            .build();

    final OfflineSigningEnvelope envelope =
        builder.encodeAndSignEnvelope(payload, "alias", KeySecurityPreference.SOFTWARE_ONLY, options);

    assert envelope.keyAlias().equals("alias") : "Key alias should match";
    assert envelope.metadata().get("context").equals("offline") : "Metadata should round trip";
    assert envelope.issuedAtMs() == 1234L : "Issued timestamp should match options";

    final OfflineSigningEnvelopeCodec codec = new OfflineSigningEnvelopeCodec();
    final OfflineSigningEnvelope decoded = codec.decode(codec.encode(envelope));
    assert decoded.equals(envelope) : "Encoding/decoding should preserve envelope";
  }

  private static void encodeAndSignEnvelopeWithExportedKey()
      throws Exception {
    final SoftwareKeyProvider provider = new SoftwareKeyProvider();
    final IrohaKeyManager keyManager = IrohaKeyManager.fromProviders(List.of(provider));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);

    final TransactionPayload payload = TransactionPayload.builder().build();
    final char[] passphrase = "envelope-passphrase".toCharArray();
    keyManager.generateOrLoad("alias", KeySecurityPreference.SOFTWARE_ONLY);
    final KeyExportBundle bundle = provider.exportDeterministic("alias", passphrase);

    final OfflineEnvelopeOptions options =
        OfflineEnvelopeOptions.builder()
            .setExportedKeyBundle(bundle.encode())
            .build();

    final OfflineSigningEnvelope envelope =
        builder.encodeAndSignEnvelope(payload, "alias", KeySecurityPreference.SOFTWARE_ONLY, options);

    assert envelope.exportedKeyBundle().isPresent() : "Envelope should include exported key";
    assert envelope.exportedKeyBundle().get().length == bundle.encode().length
        : "Exported key bytes should be preserved";
    Arrays.fill(passphrase, '\0');
  }
}
