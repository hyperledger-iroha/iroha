package org.hyperledger.iroha.android;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.Set;
import java.util.stream.Collectors;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.Parameterized;

/**
 * Runs the Android-only main-based tests under Gradle for CI coverage.
 */
@RunWith(Parameterized.class)
public final class GradleAndroidHarnessTests {
  private static final String[] MAIN_CLASSES =
      new String[] {
        "org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactoryTests",
        "org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorTests",
        "org.hyperledger.iroha.android.client.transport.OkHttpTransportExecutorTests",
        "org.hyperledger.iroha.android.telemetry.OkHttpTelemetryIntegrationTests",
        "org.hyperledger.iroha.android.offline.HttpSafetyDetectOkHttpTests",
      };

  private final String className;

  public GradleAndroidHarnessTests(final String className) {
    this.className = className;
  }

  @Parameterized.Parameters(name = "{0}")
  public static Collection<Object[]> parameters() {
    return Arrays.stream(selectedMains()).map(name -> new Object[] {name}).toList();
  }

  @Test
  public void runHarnessMain() throws Exception {
    invokeMain(className);
  }

  private static String[] selectedMains() {
    final String overrides = System.getProperty("android.test.mains", "").trim();
    if (overrides.isEmpty()) {
      return MAIN_CLASSES;
    }
    final Set<String> allowList =
        Arrays.stream(overrides.split(","))
            .map(String::trim)
            .filter(s -> !s.isEmpty())
            .collect(Collectors.toCollection(LinkedHashSet::new));
    final String[] filtered =
        Arrays.stream(MAIN_CLASSES).filter(allowList::contains).toArray(String[]::new);
    if (filtered.length == 0) {
      throw new IllegalArgumentException(
          "android.test.mains filter produced no matches: " + overrides);
    }
    return filtered;
  }

  private static void invokeMain(final String target) throws Exception {
    final Class<?> clazz = Class.forName(target);
    final Method main = clazz.getMethod("main", String[].class);
    try {
      main.invoke(null, (Object) new String[0]);
    } catch (final InvocationTargetException ex) {
      final Throwable cause = ex.getCause();
      if (cause instanceof Exception exception) {
        throw exception;
      }
      if (cause instanceof Error error) {
        throw error;
      }
      throw new RuntimeException(cause);
    }
  }
}
