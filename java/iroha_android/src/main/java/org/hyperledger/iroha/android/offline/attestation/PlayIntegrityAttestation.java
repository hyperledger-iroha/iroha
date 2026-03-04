package org.hyperledger.iroha.android.offline.attestation;

import java.util.Objects;

/** Immutable attestation payload returned by Play Integrity requests. */
public final class PlayIntegrityAttestation {

  private final String token;
  private final long fetchedAtMs;

  public PlayIntegrityAttestation(final String token, final long fetchedAtMs) {
    this.token = Objects.requireNonNull(token, "token");
    this.fetchedAtMs = fetchedAtMs;
  }

  /** Returns the raw Play Integrity token payload. */
  public String token() {
    return token;
  }

  /** Unix timestamp (ms) when the attestation was fetched. */
  public long fetchedAtMs() {
    return fetchedAtMs;
  }
}
