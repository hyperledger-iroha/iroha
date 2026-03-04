package org.hyperledger.iroha.android.offline.attestation;

/** Hook invoked whenever a Safety Detect attestation attempt succeeds or fails. */
public interface SafetyDetectTelemetry {

  SafetyDetectTelemetry NO_OP =
      new SafetyDetectTelemetry() {
        @Override
        public void onAttempt(final SafetyDetectRequest request) {}

        @Override
        public void onSuccess(
            final SafetyDetectRequest request, final SafetyDetectAttestation attestation) {}

        @Override
        public void onFailure(final SafetyDetectRequest request, final Throwable error) {}
      };

  default void onAttempt(final SafetyDetectRequest request) {}

  void onSuccess(SafetyDetectRequest request, SafetyDetectAttestation attestation);

  void onFailure(SafetyDetectRequest request, Throwable error);
}
