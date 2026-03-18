package org.hyperledger.iroha.android.telemetry;

import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.crypto.Blake2b;

/**
 * Telemetry configuration derived from {@code iroha_config}.
 *
 * <p>The options capture runtime policy (enable/disable) and the redaction parameters used by
 * telemetry observers when emitting signals required by the AND7 roadmap deliverables.
 */
public final class TelemetryOptions {

  private final boolean enabled;
  private final Redaction redaction;

  private TelemetryOptions(final boolean enabled, final Redaction redaction) {
    this.enabled = enabled;
    this.redaction = redaction;
  }

  private TelemetryOptions(final Builder builder) {
    this(
        builder.enabled,
        builder.redaction != null ? builder.redaction : Redaction.disabled());
  }

  /** Returns {@code true} when telemetry observers should emit events. */
  public boolean enabled() {
    return enabled;
  }

  /** Returns the configured redaction policy. */
  public Redaction redaction() {
    return redaction;
  }

  public Builder toBuilder() {
    return new Builder().setEnabled(enabled).setTelemetryRedaction(redaction);
  }

  /** Convenience helper that returns a disabled telemetry configuration. */
  public static TelemetryOptions disabled() {
    return builder().setEnabled(false).setTelemetryRedaction(Redaction.disabled()).build();
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private boolean enabled = true;
    private Redaction redaction = Redaction.disabled();

    public Builder setEnabled(final boolean enabled) {
      this.enabled = enabled;
      return this;
    }

    public Builder setTelemetryRedaction(final Redaction redaction) {
      this.redaction = Objects.requireNonNull(redaction, "redaction");
      return this;
    }

    public TelemetryOptions build() {
      if (!enabled) {
        return new TelemetryOptions(false, Redaction.disabled());
      }
      return new TelemetryOptions(this);
    }
  }

  /**
   * Redaction policy shared between Android telemetry emitters and the CLI tooling that validates
   * parity with the Rust baseline.
   */
  public static final class Redaction {
    private static final String DEFAULT_ALGORITHM = "blake2b-256";

    private final boolean enabled;
    private final byte[] salt;
    private final String saltVersion;
    private final String rotationId;
    private final String algorithm;

    private Redaction(final Builder builder) {
      this.enabled = builder.enabled;
      this.salt = builder.salt == null ? new byte[0] : builder.salt.clone();
      this.saltVersion = builder.saltVersion;
      this.rotationId =
          builder.rotationId == null || builder.rotationId.isEmpty()
              ? builder.saltVersion
              : builder.rotationId;
      this.algorithm = builder.algorithm;
      if (enabled) {
        if (salt.length == 0) {
          throw new IllegalStateException("Telemetry redaction requires a non-empty salt");
        }
        if (saltVersion == null || saltVersion.isEmpty()) {
          throw new IllegalStateException("Telemetry redaction requires a salt version");
        }
        if (rotationId == null || rotationId.isEmpty()) {
          throw new IllegalStateException("Telemetry redaction requires a rotation id");
        }
        if (!DEFAULT_ALGORITHM.equalsIgnoreCase(algorithm)) {
          throw new IllegalStateException("Unsupported telemetry hash algorithm: " + algorithm);
        }
      }
    }

    private Redaction(
        final boolean enabled,
        final byte[] salt,
        final String saltVersion,
        final String algorithm,
        final String rotationId) {
      this.enabled = enabled;
      this.salt = salt.clone();
      this.saltVersion = saltVersion;
      this.rotationId = rotationId;
      this.algorithm = algorithm;
    }

    /** Returns a disabled redaction policy. */
    public static Redaction disabled() {
      return new Redaction(false, new byte[0], "none", DEFAULT_ALGORITHM, "none");
    }

    public static Builder builder() {
      return new Builder();
    }

    /** True when hashing/masking should be applied to outbound telemetry signals. */
    public boolean enabled() {
      return enabled;
    }

    /** Returns a copy of the salt bytes. */
    public byte[] salt() {
      return salt.clone();
    }

    /** Version/epoch identifier used when exporting {@code android.telemetry.redaction.salt_version}. */
    public String saltVersion() {
      return saltVersion;
    }

    /** Identifier describing the salt rotation entry in the manifest. */
    public String rotationId() {
      return rotationId;
    }

    /** Hash algorithm identifier (currently only {@code blake2b-256}). */
    public String algorithm() {
      return algorithm;
    }

    /**
     * Hashes {@code authority} using the configured salt and algorithm.
     *
     * @return Empty when disabled or when {@code authority} is blank.
     */
    public Optional<String> hashAuthority(final String authority) {
      if (!enabled) {
        return Optional.empty();
      }
      if (authority == null) {
        return Optional.empty();
      }
      final String normalised = authority.trim();
      if (normalised.isEmpty()) {
        return Optional.empty();
      }
      final byte[] authorityBytes =
          normalised.toLowerCase(Locale.ROOT).getBytes(StandardCharsets.UTF_8);
      return Optional.of(bytesToHex(hashWithSalt(authorityBytes)));
    }

    /**
     * Hashes {@code identifier} using the configured salt and algorithm. Unlike {@link
     * #hashAuthority(String)} the input casing is preserved to allow stack trace/crash identifiers
     * that already ship in canonical form.
     *
     * @return Empty when disabled or when {@code identifier} is blank.
     */
    public Optional<String> hashIdentifier(final String identifier) {
      if (!enabled) {
        return Optional.empty();
      }
      if (identifier == null) {
        return Optional.empty();
      }
      final String normalised = identifier.trim();
      if (normalised.isEmpty()) {
        return Optional.empty();
      }
      final byte[] identifierBytes = normalised.getBytes(StandardCharsets.UTF_8);
      return Optional.of(bytesToHex(hashWithSalt(identifierBytes)));
    }

    private byte[] hashWithSalt(final byte[] payload) {
      final byte[] input = new byte[salt.length + payload.length];
      System.arraycopy(salt, 0, input, 0, salt.length);
      System.arraycopy(payload, 0, input, salt.length, payload.length);
      return hash(input);
    }

    private byte[] hash(final byte[] input) {
      if (!DEFAULT_ALGORITHM.equalsIgnoreCase(algorithm)) {
        throw new IllegalStateException("Unsupported telemetry hash algorithm: " + algorithm);
      }
      return Blake2b.digest256(input);
    }

    private static String bytesToHex(final byte[] data) {
      final StringBuilder builder = new StringBuilder(data.length * 2);
      for (final byte value : data) {
        builder.append(String.format(Locale.ROOT, "%02x", value));
      }
      return builder.toString();
    }

    public static final class Builder {
      private boolean enabled = true;
      private byte[] salt = new byte[0];
      private String saltVersion = "";
      private String rotationId = "";
      private String algorithm = DEFAULT_ALGORITHM;

      public Builder setEnabled(final boolean enabled) {
        this.enabled = enabled;
        return this;
      }

      public Builder setSaltHex(final String hex) {
        this.salt = decodeHex(Objects.requireNonNull(hex, "hex"));
        return this;
      }

      public Builder setSalt(final byte[] salt) {
        this.salt = Objects.requireNonNull(salt, "salt").clone();
        return this;
      }

      public Builder setSaltVersion(final String saltVersion) {
        this.saltVersion = Objects.requireNonNull(saltVersion, "saltVersion");
        return this;
      }

      public Builder setRotationId(final String rotationId) {
        this.rotationId = Objects.requireNonNull(rotationId, "rotationId");
        return this;
      }

      public Builder setAlgorithm(final String algorithm) {
        this.algorithm = Objects.requireNonNull(algorithm, "algorithm");
        return this;
      }

      public Redaction build() {
        if (!enabled) {
          return Redaction.disabled();
        }
        return new Redaction(this);
      }

      private static byte[] decodeHex(final String hex) {
        final String trimmed = hex.trim();
        if ((trimmed.length() & 1) == 1) {
          throw new IllegalArgumentException("Hex salt must contain an even number of characters");
        }
        final byte[] out = new byte[trimmed.length() / 2];
        for (int i = 0; i < trimmed.length(); i += 2) {
          final int hi = Character.digit(trimmed.charAt(i), 16);
          final int lo = Character.digit(trimmed.charAt(i + 1), 16);
          if (hi < 0 || lo < 0) {
            throw new IllegalArgumentException("Salt contains non-hexadecimal characters");
          }
          out[i / 2] = (byte) ((hi << 4) + lo);
        }
        return out;
      }
    }
  }
}
