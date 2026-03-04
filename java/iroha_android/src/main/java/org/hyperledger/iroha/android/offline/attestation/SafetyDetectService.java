package org.hyperledger.iroha.android.offline.attestation;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/**
 * Abstraction over the Safety Detect attestation backend. Implementations are responsible for
 * minting HMS tokens and returning the raw JWS payloads that Torii expects when admitting offline
 * bundles.
 */
public interface SafetyDetectService {

  /**
    * Executes a Safety Detect attestation request.
    *
    * @param request Normalised attestation parameters (app id, nonce, package name, etc.).
    */
  CompletableFuture<SafetyDetectAttestation> fetch(SafetyDetectRequest request);

  /**
   * Returns a service that always fails, used when the feature flag has not been enabled in the SDK
   * options.
   */
  static SafetyDetectService disabled() {
    return new DisabledSafetyDetectService();
  }

  final class DisabledSafetyDetectService implements SafetyDetectService {

    @Override
    public CompletableFuture<SafetyDetectAttestation> fetch(
        final SafetyDetectRequest request) {
      Objects.requireNonNull(request, "request");
      final CompletableFuture<SafetyDetectAttestation> future = new CompletableFuture<>();
      future.completeExceptionally(
          new IllegalStateException("Safety Detect attestation is disabled"));
      return future;
    }
  }
}
