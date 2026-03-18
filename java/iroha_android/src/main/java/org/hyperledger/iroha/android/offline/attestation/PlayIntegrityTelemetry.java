package org.hyperledger.iroha.android.offline.attestation;

/** Hook invoked whenever a Play Integrity attestation attempt succeeds or fails. */
public interface PlayIntegrityTelemetry {

  PlayIntegrityTelemetry NO_OP =
      new PlayIntegrityTelemetry() {
        @Override
        public void onAttempt(final PlayIntegrityRequest request) {}

        @Override
        public void onSuccess(
            final PlayIntegrityRequest request, final PlayIntegrityAttestation attestation) {}

        @Override
        public void onFailure(final PlayIntegrityRequest request, final Throwable error) {}
      };

  default void onAttempt(final PlayIntegrityRequest request) {}

  void onSuccess(PlayIntegrityRequest request, PlayIntegrityAttestation attestation);

  void onFailure(PlayIntegrityRequest request, Throwable error);
}
