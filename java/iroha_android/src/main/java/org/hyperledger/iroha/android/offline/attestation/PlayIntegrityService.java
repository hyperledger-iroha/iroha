package org.hyperledger.iroha.android.offline.attestation;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/**
 * Abstraction over the Play Integrity backend. Implementations are responsible for minting tokens
 * and returning the raw payload required by Torii when admitting offline bundles.
 */
public interface PlayIntegrityService {

  /**
   * Executes a Play Integrity attestation request.
   *
   * @param request Normalised attestation parameters (cloud project, nonce, package name, etc.).
   */
  CompletableFuture<PlayIntegrityAttestation> fetch(PlayIntegrityRequest request);

  /** Returns a service that always fails, used when the feature is disabled. */
  static PlayIntegrityService disabled() {
    return new DisabledPlayIntegrityService();
  }

  final class DisabledPlayIntegrityService implements PlayIntegrityService {
    @Override
    public CompletableFuture<PlayIntegrityAttestation> fetch(
        final PlayIntegrityRequest request) {
      Objects.requireNonNull(request, "request");
      final CompletableFuture<PlayIntegrityAttestation> future = new CompletableFuture<>();
      future.completeExceptionally(
          new IllegalStateException("Play Integrity attestation is disabled"));
      return future;
    }
  }
}
