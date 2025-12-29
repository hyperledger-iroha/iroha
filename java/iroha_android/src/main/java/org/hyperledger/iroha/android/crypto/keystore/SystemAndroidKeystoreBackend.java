package org.hyperledger.iroha.android.crypto.keystore;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.KeyStore.PrivateKeyEntry;
import java.security.PrivateKey;
import java.security.ProviderException;
import java.security.PublicKey;
import java.security.spec.AlgorithmParameterSpec;
import java.time.Duration;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;

/**
 * Reflection-driven {@link AndroidKeystoreBackend} implementation that bridges to the platform
 * Android Keystore runtime when the SDK is running on Android hardware.
 *
 * <p>The implementation avoids compile-time dependencies on {@code android.*} packages so the
 * desktop JVM build remains compilable. When the runtime does not expose the Android Keystore
 * classes (for example, on desktop tests), the factory returns {@link Optional#empty()} and callers
 * fall back to the stub/software providers.
 */
final class SystemAndroidKeystoreBackend implements AndroidKeystoreBackend {

  private static final String ANDROID_KEYSTORE = "AndroidKeyStore";
  private static final String STRONGBOX_UNAVAILABLE_CLASS =
      "android.security.keystore.StrongBoxUnavailableException";
  private static final String KEY_GEN_SPEC_BUILDER_CLASS =
      "android.security.keystore.KeyGenParameterSpec$Builder";
  private static final String KEY_PROPERTIES_CLASS =
      "android.security.keystore.KeyProperties";

  private final KeyProviderMetadata metadata;

  private SystemAndroidKeystoreBackend(final KeyProviderMetadata metadata) {
    this.metadata = metadata;
  }

  static Optional<AndroidKeystoreBackend> create() {
    if (!isAndroidRuntime()) {
      return Optional.empty();
    }
    final KeyStore keyStore;
    try {
      keyStore = KeyStore.getInstance(ANDROID_KEYSTORE);
      keyStore.load(null);
    } catch (final GeneralSecurityException | IOException ex) {
      return Optional.empty();
    }

    final boolean supportsStrongBox = detectStrongBoxSupport(keyStore);

    final KeyProviderMetadata.Builder metadataBuilder =
        KeyProviderMetadata.builder("android-keystore")
            .setSupportsAttestationCertificates(true);
    if (supportsStrongBox) {
      metadataBuilder.setStrongBoxBacked(true);
    } else {
      metadataBuilder
          .setHardwareBacked(true)
          .setSecurityLevel(KeyProviderMetadata.HardwareSecurityLevel.TRUSTED_ENVIRONMENT);
    }
    final KeyProviderMetadata metadata = metadataBuilder.build();

    return Optional.of(new SystemAndroidKeystoreBackend(metadata));
  }

  private static boolean isAndroidRuntime() {
    try {
      Class.forName("android.os.Build");
      return true;
    } catch (final ClassNotFoundException ignored) {
      return false;
    }
  }

  @Override
  public Optional<KeyPair> load(final String alias) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    if (alias.isBlank()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    try {
      final KeyStore keyStore = loadKeyStore();
      if (!keyStore.containsAlias(alias)) {
        return Optional.empty();
      }
      final KeyStore.Entry entry = keyStore.getEntry(alias, null);
      if (!(entry instanceof PrivateKeyEntry privateKeyEntry)) {
        return Optional.empty();
      }
      final PublicKey publicKey = privateKeyEntry.getCertificate().getPublicKey();
      final PrivateKey privateKey = privateKeyEntry.getPrivateKey();
      return Optional.of(new KeyPair(publicKey, privateKey));
    } catch (final GeneralSecurityException | IOException ex) {
      throw new KeyManagementException("Failed to load key from Android Keystore", ex);
    }
  }

  @Override
  public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
      throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    Objects.requireNonNull(parameters, "parameters");
    if (alias.isBlank()) {
      throw new IllegalArgumentException("alias must not be blank");
    }

    final KeyPairGenerator generator = createKeyPairGenerator(parameters.algorithm());

    final boolean strongBoxRequested =
        parameters.requireStrongBox() || parameters.preferStrongBox();
    final KeyGenerationResult generated =
        generateInternal(
            generator,
            alias,
            parameters,
            /*strongBoxRequested=*/ strongBoxRequested);

    return generated;
  }

  private KeyGenerationResult generateInternal(
      final KeyPairGenerator generator,
      final String alias,
      final KeyGenParameters parameters,
      final boolean strongBoxRequested)
      throws KeyManagementException {
    AlgorithmParameterSpec spec;
    try {
      spec = buildKeyGenParameterSpec(alias, parameters, strongBoxRequested);
    } catch (final GeneralSecurityException ex) {
      if (strongBoxRequested
          && parameters.allowStrongBoxFallback()
          && isStrongBoxUnavailable(ex)) {
        return generateInternal(generator, alias, parameters, /*strongBoxRequested=*/ false);
      }
      throw new KeyManagementException("Failed to prepare Android Keystore parameters", ex);
    }

    try {
      generator.initialize(spec);
      final KeyPair pair = generator.generateKeyPair();
      return new KeyGenerationResult(pair, strongBoxRequested);
    } catch (final ProviderException ex) {
      if (strongBoxRequested && parameters.allowStrongBoxFallback() && isStrongBoxUnavailable(ex)) {
        // Retry without StrongBox backing.
        return generateInternal(generator, alias, parameters, /*strongBoxRequested=*/ false);
      }
      throw new KeyManagementException("Android Keystore generation failed", ex);
    } catch (final GeneralSecurityException ex) {
      if (strongBoxRequested
          && parameters.allowStrongBoxFallback()
          && isStrongBoxUnavailable(ex)) {
        return generateInternal(generator, alias, parameters, /*strongBoxRequested=*/ false);
      }
      throw new KeyManagementException("Android Keystore generation failed", ex);
    }
  }

  @Override
  public KeyPair generateEphemeral(final KeyGenParameters parameters) throws KeyManagementException {
    // Android Keystore persists generated keys; generating/discarding aliases would leave state
    // behind. Defer to software providers for ephemeral operations.
    throw new KeyManagementException("Android Keystore does not support unmanaged ephemeral keys");
  }

  @Override
  public KeyProviderMetadata metadata() {
    return metadata;
  }

  @Override
  public String name() {
    return metadata.name();
  }

  @Override
  public Optional<KeyAttestation> attestation(final String alias) {
    Objects.requireNonNull(alias, "alias");
    try {
      return loadAttestationBundle(alias);
    } catch (final GeneralSecurityException | IOException ex) {
      return Optional.empty();
    }
  }

  @Override
  public Optional<KeyAttestation> generateAttestation(
      final String alias, final byte[] challenge) throws KeyManagementException {
    Objects.requireNonNull(alias, "alias");
    if (alias.isBlank()) {
      throw new IllegalArgumentException("alias must not be blank");
    }
    final byte[] challengeCopy = challenge == null ? new byte[0] : challenge.clone();
    try {
      final KeyStore keyStore = loadKeyStore();
      if (!keyStore.containsAlias(alias)) {
        return Optional.empty();
      }
      if (challengeCopy.length > 0) {
        final Optional<KeyAttestation> fresh =
            generateAttestationWithChallenge(keyStore, alias, challengeCopy);
        if (fresh.isPresent()) {
          return fresh;
        }
        throw new KeyManagementException(
            "Android Keystore attestation challenge unsupported on this device/API level");
      }
      return loadAttestationBundle(alias, keyStore);
    } catch (final GeneralSecurityException | IOException ex) {
      throw new KeyManagementException("Failed to read Android Keystore attestation", ex);
    }
  }

  private static KeyStore loadKeyStore() throws GeneralSecurityException, IOException {
    final KeyStore keyStore = KeyStore.getInstance(ANDROID_KEYSTORE);
    keyStore.load(null);
    return keyStore;
  }

  private static KeyPairGenerator createKeyPairGenerator(final String algorithm)
      throws KeyManagementException {
    final String resolvedAlgorithm = algorithm == null ? "Ed25519" : algorithm;
    try {
      return KeyPairGenerator.getInstance(resolvedAlgorithm, ANDROID_KEYSTORE);
    } catch (final GeneralSecurityException ex) {
      throw new KeyManagementException(
          "Android Keystore does not support algorithm " + resolvedAlgorithm, ex);
    }
  }

  private static AlgorithmParameterSpec buildKeyGenParameterSpec(
      final String alias, final KeyGenParameters parameters, final boolean strongBox)
      throws GeneralSecurityException {
    try {
      final Class<?> builderClass = Class.forName(KEY_GEN_SPEC_BUILDER_CLASS);
      final Class<?> keyPropertiesClass = Class.forName(KEY_PROPERTIES_CLASS);

      final int purposeSign = keyPropertiesClass.getField("PURPOSE_SIGN").getInt(null);
      final int purposeVerify = keyPropertiesClass.getField("PURPOSE_VERIFY").getInt(null);

      final Object builder =
          builderClass
              .getConstructor(String.class, int.class)
              .newInstance(alias, purposeSign | purposeVerify);

      invoke(builder, "setDigests", new Class<?>[] {String[].class}, new Object[] {new String[] {
        keyPropertiesClass.getField("DIGEST_NONE").get(null).toString()
      }});

      if (parameters.userAuthenticationRequired()) {
        invoke(builder, "setUserAuthenticationRequired", new Class<?>[] {boolean.class}, true);
        final Duration timeout = parameters.userAuthenticationTimeout();
        long seconds = timeout == null ? 0L : timeout.getSeconds();
        if (seconds < 0L) {
          seconds = 0L;
        }
        final int safeSeconds =
            (int) Math.max(0L, Math.min(Integer.MAX_VALUE, seconds));
        invokeIfPresent(
            builder,
            "setUserAuthenticationValidityDurationSeconds",
            new Class<?>[] {int.class},
            safeSeconds);
      }

      if (strongBox) {
        // This throws when StrongBox is unsupported; propagate so the caller can decide on fallback.
        invoke(builder, "setIsStrongBoxBacked", new Class<?>[] {boolean.class}, true);
      } else {
        invokeIfPresent(builder, "setIsStrongBoxBacked", new Class<?>[] {boolean.class}, false);
      }

      final byte[] challenge = parameters.attestationChallenge();
      if (challenge != null && challenge.length > 0) {
        try {
          invoke(
              builder, "setAttestationChallenge", new Class<?>[] {byte[].class}, challenge.clone());
        } catch (final NoSuchMethodException ex) {
          throw new GeneralSecurityException(
              "Attestation challenges are not supported on this Android API level", ex);
        }
      }

      final Object spec = invoke(builder, "build", new Class<?>[0]);
      if (!(spec instanceof AlgorithmParameterSpec algorithmParameterSpec)) {
        throw new GeneralSecurityException("Android KeyGenParameterSpec build returned unexpected type");
      }
      return algorithmParameterSpec;
    } catch (final ReflectiveOperationException ex) {
      throw new GeneralSecurityException("Failed to construct Android KeyGenParameterSpec", ex);
    }
  }

  private static Object invoke(
      final Object target,
      final String name,
      final Class<?>[] parameterTypes,
      final Object... args)
      throws ReflectiveOperationException {
    final Class<?> clazz = target.getClass();
    final java.lang.reflect.Method method = clazz.getMethod(name, parameterTypes);
    return method.invoke(target, args);
  }

  private static void invokeIfPresent(
      final Object target,
      final String name,
      final Class<?>[] parameterTypes,
      final Object... args)
      throws ReflectiveOperationException {
    try {
      invoke(target, name, parameterTypes, args);
    } catch (final NoSuchMethodException ignored) {
      // Method not available on this API level; ignore when optional.
    }
  }

  private static boolean isStrongBoxUnavailable(final Throwable throwable) {
    Throwable current = throwable;
    while (current != null) {
      final String className = current.getClass().getName();
      if (STRONGBOX_UNAVAILABLE_CLASS.equals(className)
          || current instanceof NoSuchMethodException
          || current instanceof ClassNotFoundException) {
        return true;
      }
      current = current.getCause();
    }
    return false;
  }

  private static boolean detectStrongBoxSupport(final KeyStore keyStore) {
    try {
      final KeyPairGenerator generator = KeyPairGenerator.getInstance("Ed25519", ANDROID_KEYSTORE);
      final KeyGenParameters parameters =
          KeyGenParameters.builder().setRequireStrongBox(true).build();
      final AlgorithmParameterSpec spec =
          buildKeyGenParameterSpec("__iroha_strongbox_probe__", parameters, true);
      generator.initialize(spec);
      cleanupProbeAlias(keyStore);
      return true;
    } catch (final ProviderException ex) {
      return false;
    } catch (final GeneralSecurityException ex) {
      if (isStrongBoxUnavailable(ex)) {
        return false;
      }
      return false;
    }
  }

  private static void cleanupProbeAlias(final KeyStore keyStore) {
    try {
      if (keyStore.containsAlias("__iroha_strongbox_probe__")) {
        keyStore.deleteEntry("__iroha_strongbox_probe__");
      }
    } catch (final GeneralSecurityException ignored) {
      // Probe cleanup is best-effort; failures are logged by callers if needed.
    }
  }

  private Optional<KeyAttestation> loadAttestationBundle(final String alias)
      throws GeneralSecurityException, IOException {
    final KeyStore keyStore = loadKeyStore();
    return loadAttestationBundle(alias, keyStore);
  }

  private Optional<KeyAttestation> loadAttestationBundle(final String alias, final KeyStore keyStore)
      throws GeneralSecurityException {
    final java.security.cert.Certificate[] chain = keyStore.getCertificateChain(alias);
    return buildAttestation(alias, chain);
  }

  private Optional<KeyAttestation> generateAttestationWithChallenge(
      final KeyStore keyStore, final String alias, final byte[] challenge)
      throws GeneralSecurityException {
    try {
      final java.lang.reflect.Method attestKey =
          keyStore.getClass().getMethod("getCertificateChain", String.class, byte[].class);
      final Object certificateResult = attestKey.invoke(keyStore, alias, challenge.clone());
      if (!(certificateResult instanceof java.security.cert.Certificate[] chain)) {
        return Optional.empty();
      }
      return buildAttestation(alias, chain);
    } catch (final NoSuchMethodException ignored) {
      return Optional.empty();
    } catch (final ReflectiveOperationException ex) {
      throw new GeneralSecurityException("Android Keystore attestation invocation failed", ex);
    }
  }

  private Optional<KeyAttestation> buildAttestation(
      final String alias, final java.security.cert.Certificate[] chain) {
    if (chain == null || chain.length == 0) {
      return Optional.empty();
    }

    final KeyAttestation.Builder builder = KeyAttestation.builder().setAlias(alias);
    for (final java.security.cert.Certificate certificate : chain) {
      if (certificate instanceof java.security.cert.X509Certificate x509Certificate) {
        builder.addCertificate(x509Certificate);
      }
    }
    return Optional.of(builder.build());
  }
}
