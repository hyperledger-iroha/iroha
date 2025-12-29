package org.hyperledger.iroha.android.telemetry;

import java.util.Optional;

/** Provider that supplies the device-profile bucket for telemetry emissions. */
public interface DeviceProfileProvider {

  /** Returns the current device profile when available. */
  Optional<DeviceProfile> snapshot();

  /** Returns a no-op provider. */
  static DeviceProfileProvider disabled() {
    return Optional::empty;
  }
}
