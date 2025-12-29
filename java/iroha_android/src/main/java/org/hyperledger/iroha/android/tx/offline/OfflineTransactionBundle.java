package org.hyperledger.iroha.android.tx.offline;

import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;

/**
 * Convenience container that bundles an {@link OfflineSigningEnvelope} together with an optional
 * hardware attestation. SDK helpers return this when callers request an offline transaction that
 * should be paired with StrongBox/TEE attestation material.
 */
public final class OfflineTransactionBundle {
  private final OfflineSigningEnvelope envelope;
  private final Optional<KeyAttestation> attestation;

  public OfflineTransactionBundle(
      final OfflineSigningEnvelope envelope, final Optional<KeyAttestation> attestation) {
    this.envelope = Objects.requireNonNull(envelope, "envelope");
    this.attestation = attestation == null ? Optional.empty() : attestation;
  }

  /** Returns the offline signing envelope containing the encoded payload and signature. */
  public OfflineSigningEnvelope envelope() {
    return envelope;
  }

  /**
   * Returns the attestation bundle when a hardware-backed provider (StrongBox/TEE) supplied one.
   * Callers should forward the contents to remote verifiers alongside the envelope.
   */
  public Optional<KeyAttestation> attestation() {
    return attestation;
  }
}
