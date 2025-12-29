package org.hyperledger.iroha.android.offline.attestation;

import java.util.Objects;

/**
 * Immutable attestation payload returned by Huawei Safety Detect requests.
 *
 * <p>The token mirrors the JWS string required by Torii when submitting {@code OfflinePlatformProof}
 * envelopes.
 */
public final class SafetyDetectAttestation {

  private final String token;
  private final long fetchedAtMs;

  public SafetyDetectAttestation(final String token, final long fetchedAtMs) {
    this.token = Objects.requireNonNull(token, "token");
    this.fetchedAtMs = fetchedAtMs;
  }

  /** Returns the raw JWS token emitted by Safety Detect. */
  public String token() {
    return token;
  }

  /** Unix timestamp (ms) when the attestation was fetched. */
  public long fetchedAtMs() {
    return fetchedAtMs;
  }
}
