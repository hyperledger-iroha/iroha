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
 *
 * <p>{@link #close()} (or {@link #destroy()}) is idempotent and destroys the underlying session's
 * owned AES-key buffers and P-256 scalar reference. JVM providers and {@code BigInteger} may retain
 * opaque internal copies that cannot be physically overwritten; explicit closure shortens their
 * reachable lifetime.
 */
public final class IrohaPeerNearbySecureChannelV1 implements AutoCloseable {
  public static final String SERVICE_ID =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.SERVICE_ID;
  private static final int MAXIMUM_HELLO_RECORD_BYTES =
      10 + 16 + 32 + 32 + 2 + 65 + 4
          + org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES;
  private static final int MAXIMUM_AUTHENTICATION_RECORD_BYTES =
      60
          + org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1
              .MAXIMUM_AUTHENTICATION_SIGNATURE_BYTES;
  private static final int MAXIMUM_ENCRYPTED_RECORD_BYTES =
      38 + org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES + 16;
  private static final int MAXIMUM_IPM1_MESSAGE_BYTES =
      IrohaPeerWireMessageV1.HEADER_LENGTH
          + IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES;

  @FunctionalInterface
  public interface SignatureVerifier {
    boolean verify(
        IrohaPeerNearbyRoleV1 role,
        byte[] certificate,
        byte[] signedBytes,
        byte[] signature);
  }

  private final IrohaPeerNearbySessionV1 session;
  private volatile boolean closed;

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
      final byte[] requiredSession = requireBound(sessionId, 16, 16, "sessionId");
      final byte[] requiredRequest =
          requireBound(requestCanonicalHash, 32, 32, "requestCanonicalHash");
      final byte[] requiredCertificate =
          requireBound(
              deviceCertificate,
              1,
              org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES,
              "deviceCertificate");
      final byte[] ownedSession = requiredSession.clone();
      final byte[] ownedRequest = requiredRequest.clone();
      final byte[] ownedCertificate = requiredCertificate.clone();
      final IrohaPeerNearbySessionV1 constructed;
      try {
        constructed = new IrohaPeerNearbySessionV1(
            sharedProfile, sharedRole, ownedSession, ownedRequest, ownedCertificate);
      } finally {
        wipe(ownedCertificate, ownedRequest, ownedSession);
      }
      session = constructed;
      return;
    }
    final byte[] requiredSession = requireBound(sessionId, 16, 16, "sessionId");
    final byte[] requiredRequest =
        requireBound(requestCanonicalHash, 32, 32, "requestCanonicalHash");
    final byte[] requiredCertificate =
        requireBound(
            deviceCertificate,
            1,
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES,
            "deviceCertificate");
    final byte[] requiredNonce = requireBound(nonce, 32, 32, "nonce");
    final byte[] requiredPrivateKey =
        requireBound(ephemeralPrivateKey, 32, 32, "ephemeralPrivateKey");
    final byte[] ownedSession = requiredSession.clone();
    final byte[] ownedRequest = requiredRequest.clone();
    final byte[] ownedCertificate = requiredCertificate.clone();
    final byte[] ownedNonce = requiredNonce.clone();
    final byte[] ownedPrivateKey = requiredPrivateKey.clone();
    final IrohaPeerNearbySessionV1 constructed;
    IrohaPeerNearbyP256V1 keyAgreement = null;
    try {
      keyAgreement = IrohaPeerNearbyP256V1.fromPrivateBytes(ownedPrivateKey);
      constructed = new IrohaPeerNearbySessionV1(
          sharedProfile,
          sharedRole,
          ownedSession,
          ownedRequest,
          ownedCertificate,
          ownedNonce,
          keyAgreement);
    } catch (RuntimeException | Error failure) {
      if (keyAgreement != null) keyAgreement.close();
      throw failure;
    } finally {
      wipe(ownedPrivateKey, ownedNonce, ownedCertificate, ownedRequest, ownedSession);
    }
    session = constructed;
  }

  public synchronized byte[] localHello() {
    requireOpen();
    return session.getLocalHello().encode();
  }

  public synchronized void acceptPeerHello(final byte[] encoded) {
    requireOpen();
    final byte[] owned = boundedClone(encoded, 1, MAXIMUM_HELLO_RECORD_BYTES, "encoded");
    try {
      session.acceptPeerHello(IrohaPeerNearbyHelloV1.decode(owned));
    } finally {
      wipe(owned);
    }
  }

  public synchronized byte[] authenticationPreimage() {
    requireOpen();
    return session.authenticationPreimage();
  }

  public synchronized byte[] makeAuthentication(final byte[] signature) {
    requireOpen();
    final byte[] owned = boundedClone(
        signature,
        1,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1
            .MAXIMUM_AUTHENTICATION_SIGNATURE_BYTES,
        "signature");
    try {
      return session.makeAuthentication(owned).encode();
    } finally {
      wipe(owned);
    }
  }

  public void acceptPeerAuthentication(
      final byte[] encoded, final SignatureVerifier verifier) {
    requireOpen();
    Objects.requireNonNull(verifier, "verifier");
    final IrohaPeerNearbySignatureVerifierV1 bridge =
        (role, certificate, signedBytes, signature) ->
            verifier.verify(
                IrohaPeerNearbyRoleV1.fromShared(role),
                certificate.clone(),
                signedBytes.clone(),
                signature.clone());
    final byte[] owned =
        boundedClone(encoded, 1, MAXIMUM_AUTHENTICATION_RECORD_BYTES, "encoded");
    try {
      session.acceptPeerAuthentication(IrohaPeerNearbyAuthenticationV1.decode(owned), bridge);
    } finally {
      wipe(owned);
    }
  }

  public synchronized boolean isAuthenticated() {
    return !closed && session.isAuthenticated();
  }

  /** Returns whether this facade has been explicitly destroyed. */
  public synchronized boolean isDestroyed() {
    return closed;
  }

  public synchronized byte[] sealIpm1(final byte[] encodedMessage) {
    requireOpen();
    final byte[] message =
        boundedClone(encodedMessage, 1, MAXIMUM_IPM1_MESSAGE_BYTES, "encodedMessage");
    try {
      IrohaPeerWireMessageV1.decode(message);
      return session.seal(message).encode();
    } finally {
      Arrays.fill(message, (byte) 0);
    }
  }

  public synchronized byte[] openIpm1(final byte[] encodedRecord) {
    requireOpen();
    final byte[] owned =
        boundedClone(encodedRecord, 1, MAXIMUM_ENCRYPTED_RECORD_BYTES, "encodedRecord");
    final IrohaPeerNearbyEncryptedRecordV1 record;
    try {
      record = IrohaPeerNearbyEncryptedRecordV1.decode(owned);
    } finally {
      wipe(owned);
    }
    final byte[] message = session.open(record);
    try {
      IrohaPeerWireMessageV1.decode(message);
      return message.clone();
    } finally {
      Arrays.fill(message, (byte) 0);
    }
  }

  /** Idempotently destroys the secure session and its owned key material. */
  public synchronized void destroy() {
    close();
  }

  /** Idempotently destroys the secure session and its owned key material. */
  @Override
  public synchronized void close() {
    if (closed) return;
    closed = true;
    session.close();
  }

  private void requireOpen() {
    if (closed) throw new IllegalStateException("Nearby secure channel has been destroyed");
  }

  private static byte[] boundedClone(
      final byte[] value, final int minimum, final int maximum, final String name) {
    return requireBound(value, minimum, maximum, name).clone();
  }

  private static byte[] requireBound(
      final byte[] value, final int minimum, final int maximum, final String name) {
    final byte[] required = Objects.requireNonNull(value, name);
    if (required.length < minimum || required.length > maximum) {
      throw new IllegalArgumentException(name + " length is outside its Nearby V1 bound");
    }
    return required;
  }

  private static void wipe(final byte[]... values) {
    for (final byte[] value : values) {
      if (value != null) Arrays.fill(value, (byte) 0);
    }
  }
}
