package org.hyperledger.iroha.android.telemetry;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Locale;
import java.util.Objects;
import java.util.Optional;

/**
 * {@link NetworkContextProvider} implementation that reflects into the Android connectivity APIs
 * at runtime. The provider works on the plain JDK because it only references the Android types via
 * {@link Class#forName(String)} and {@link Method} lookups; applications should supply their
 * {@code android.content.Context} instance when registering the provider.
 */
public final class AndroidNetworkContextProvider implements NetworkContextProvider {

  private final Object androidContext;

  private AndroidNetworkContextProvider(final Object androidContext) {
    this.androidContext = Objects.requireNonNull(androidContext, "androidContext");
  }

  /**
   * Creates a provider backed by the supplied Android {@code Context}.
   *
   * @param androidContext instance of {@code android.content.Context}
   */
  public static NetworkContextProvider fromContext(final Object androidContext) {
    return new AndroidNetworkContextProvider(androidContext);
  }

  @Override
  public Optional<NetworkContext> snapshot() {
    try {
      final Class<?> contextClass = Class.forName("android.content.Context");
      if (!contextClass.isInstance(androidContext)) {
        return Optional.empty();
      }
      final Field serviceField = contextClass.getField("CONNECTIVITY_SERVICE");
      final String serviceName = (String) serviceField.get(null);
      final Method getSystemService = contextClass.getMethod("getSystemService", String.class);
      final Object connectivityManager = getSystemService.invoke(androidContext, serviceName);
      if (connectivityManager == null) {
        return Optional.empty();
      }
      final Class<?> managerClass = Class.forName("android.net.ConnectivityManager");
      final Method getActiveNetworkInfo = managerClass.getMethod("getActiveNetworkInfo");
      final Object networkInfo = getActiveNetworkInfo.invoke(connectivityManager);
      if (networkInfo == null) {
        return Optional.empty();
      }
      final Class<?> infoClass = Class.forName("android.net.NetworkInfo");
      final Method isConnected = infoClass.getMethod("isConnected");
      final boolean connected = Boolean.TRUE.equals(isConnected.invoke(networkInfo));
      if (!connected) {
        return Optional.empty();
      }
      final Method getTypeName = infoClass.getMethod("getTypeName");
      final String rawType = (String) getTypeName.invoke(networkInfo);
      final Method isRoaming = infoClass.getMethod("isRoaming");
      final boolean roaming = Boolean.TRUE.equals(isRoaming.invoke(networkInfo));
      return Optional.of(NetworkContext.of(normalizeNetworkType(rawType), roaming));
    } catch (ReflectiveOperationException | SecurityException ignored) {
      return Optional.empty();
    }
  }

  private static String normalizeNetworkType(final String value) {
    if (value == null) {
      return "unknown";
    }
    final String normalized = value.trim().toLowerCase(Locale.ROOT);
    return switch (normalized) {
      case "wifi", "wi-fi" -> "wifi";
      case "mobile", "cellular" -> "cellular";
      case "ethernet" -> "ethernet";
      case "bluetooth" -> "bluetooth";
      case "vpn" -> "vpn";
      default -> "other";
    };
  }
}
