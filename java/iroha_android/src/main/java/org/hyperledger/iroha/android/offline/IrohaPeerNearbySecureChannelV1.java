package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyAuthenticationV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyEncryptedRecordV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyHelloV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyP256V1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySignatureVerifierV1;

/**
 * Java facade over the audited IPN1 P-256/HKDF/AES-GCM core.
 *
 * Only verified IPM1 bytes may cross the encrypted application boundary. This
 * prevents a Java radio adapter from accidentally treating raw Nearby BYTES as
 * an authenticated payment.
 */
public final class IrohaPeerNearbySecureChannelV1 {
  public static final String SERVICE_ID =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.SERVICE_ID;

  @FunctionalInterface
  public interface SignatureVerifier {
    boolean verify(
        IrohaPeerNearbyRoleV1 role,
        byte[] certificate,
        byte[] signedBytes,
        byte[] signature);
  }

  private final IrohaPeerNearbySessionV1 session;

  public IrohaPeerNearbySecureChannelV1(
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerNearbyRoleV1 role,
      final byte[] sessionId,
      final byte[] requestCanonicalHash,
      final byte[] deviceCertificate) {
    this(
        profile,
        role,
        sessionId,
        requestCanonicalHash,
        deviceCertificate,
        null,
        null);
  }

  /** Deterministic package-private entry point used only by cross-SDK golden tests. */
  IrohaPeerNearbySecureChannelV1(
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerNearbyRoleV1 role,
      final byte[] sessionId,
      final byte[] requestCanonicalHash,
      final byte[] deviceCertificate,
      final byte[] nonce,
      final byte[] ephemeralPrivateKey) {
    final org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile sharedProfile =
        IrohaPeerNfcV1.sharedProfile(Objects.requireNonNull(profile, "profile"));
    final org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1 sharedRole =
        Objects.requireNonNull(role, "role").toShared();
    if (nonce == null || ephemeralPrivateKey == null) {
      if (nonce != null || ephemeralPrivateKey != null) {
        throw new IllegalArgumentException("Nearby test entropy must be supplied together");
      }
      session =
          new IrohaPeerNearbySessionV1(
              sharedProfile,
              sharedRole,
              Objects.requireNonNull(sessionId, "sessionId").clone(),
              Objects.requireNonNull(requestCanonicalHash, "requestCanonicalHash").clone(),
              Objects.requireNonNull(deviceCertificate, "deviceCertificate").clone());
      return;
    }
    session =
        new IrohaPeerNearbySessionV1(
            sharedProfile,
            sharedRole,
            Objects.requireNonNull(sessionId, "sessionId").clone(),
            Objects.requireNonNull(requestCanonicalHash, "requestCanonicalHash").clone(),
            Objects.requireNonNull(deviceCertificate, "deviceCertificate").clone(),
            nonce.clone(),
            IrohaPeerNearbyP256V1.fromPrivateBytes(ephemeralPrivateKey.clone()));
  }

  public byte[] localHello() {
    return session.getLocalHello().encode();
  }

  public void acceptPeerHello(final byte[] encoded) {
    session.acceptPeerHello(IrohaPeerNearbyHelloV1.decode(encoded.clone()));
  }

  public byte[] authenticationPreimage() {
    return session.authenticationPreimage();
  }

  public byte[] makeAuthentication(final byte[] signature) {
    return session.makeAuthentication(signature.clone()).encode();
  }

  public void acceptPeerAuthentication(
      final byte[] encoded, final SignatureVerifier verifier) {
    Objects.requireNonNull(verifier, "verifier");
    final IrohaPeerNearbySignatureVerifierV1 bridge =
        (role, certificate, signedBytes, signature) ->
            verifier.verify(
                IrohaPeerNearbyRoleV1.fromShared(role),
                certificate.clone(),
                signedBytes.clone(),
                signature.clone());
    session.acceptPeerAuthentication(
        IrohaPeerNearbyAuthenticationV1.decode(encoded.clone()), bridge);
  }

  public boolean isAuthenticated() {
    return session.isAuthenticated();
  }

  public byte[] sealIpm1(final byte[] encodedMessage) {
    final byte[] message = Objects.requireNonNull(encodedMessage, "encodedMessage").clone();
    try {
      IrohaPeerWireMessageV1.decode(message);
      return session.seal(message).encode();
    } finally {
      Arrays.fill(message, (byte) 0);
    }
  }

  public byte[] openIpm1(final byte[] encodedRecord) {
    final IrohaPeerNearbyEncryptedRecordV1 record =
        IrohaPeerNearbyEncryptedRecordV1.decode(encodedRecord.clone());
    final byte[] message = session.open(record);
    try {
      IrohaPeerWireMessageV1.decode(message);
      return message.clone();
    } finally {
      Arrays.fill(message, (byte) 0);
    }
  }
}
