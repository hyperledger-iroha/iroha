package org.hyperledger.iroha.qualification;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import org.junit.jupiter.engine.JupiterTestEngine;
import org.junit.platform.engine.TestExecutionResult;
import org.junit.platform.engine.discovery.DiscoverySelectors;
import org.junit.platform.engine.support.descriptor.MethodSource;
import org.junit.platform.launcher.EngineFilter;
import org.junit.platform.launcher.TestExecutionListener;
import org.junit.platform.launcher.TestIdentifier;
import org.junit.platform.launcher.core.LauncherConfig;
import org.junit.platform.launcher.core.LauncherDiscoveryRequestBuilder;
import org.junit.platform.launcher.core.LauncherFactory;

/** Fixed isolated runner for the actual packaged Kotlin SoraFS Java assertion owner. */
public final class SorafsJavaConsumerQualificationRunner {
  private static final String SUITE =
      "org.hyperledger.iroha.sdk.sorafs.SorafsReferenceValidatorsJavaConsumerTest";

  private SorafsJavaConsumerQualificationRunner() {}

  private static final class Results implements TestExecutionListener {
    private final Map<String, Long> started = new LinkedHashMap<String, Long>();
    private final Map<String, Double> elapsed = new LinkedHashMap<String, Double>();
    private boolean failed;

    @Override
    public void executionStarted(final TestIdentifier test) {
      if (test.isTest()) {
        if (started.size() >= 25 || test.getUniqueId().length() > 4096) {
          failed = true;
          return;
        }
        if (started.put(test.getUniqueId(), System.nanoTime()) != null) failed = true;
      }
    }

    @Override
    public void executionSkipped(final TestIdentifier test, final String reason) {
      failed = true;
    }

    @Override
    public void executionFinished(final TestIdentifier test, final TestExecutionResult result) {
      if (result.getStatus() != TestExecutionResult.Status.SUCCESSFUL) failed = true;
      if (!test.isTest()) return;
      if (!test.getSource().isPresent() || !(test.getSource().get() instanceof MethodSource)) {
        failed = true;
        return;
      }
      final MethodSource source = (MethodSource) test.getSource().get();
      final Long begin = started.remove(test.getUniqueId());
      if (!SUITE.equals(source.getClassName()) || begin == null
          || elapsed.size() >= 25
          || !source.getMethodName().matches("[A-Za-z_][A-Za-z0-9_]{0,255}")) {
        failed = true;
        return;
      }
      final double seconds = (System.nanoTime() - begin.longValue()) / 1_000_000_000.0;
      if (elapsed.put(source.getMethodName(), seconds) != null) failed = true;
    }

    private byte[] report() {
      final StringBuilder xml = new StringBuilder();
      xml.append("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"")
          .append(SUITE).append("\" tests=\"").append(elapsed.size())
          .append("\" failures=\"").append(failed || !started.isEmpty() ? 1 : 0)
          .append("\" errors=\"0\" skipped=\"0\">\n");
      for (final Map.Entry<String, Double> entry : elapsed.entrySet()) {
        xml.append("  <testcase classname=\"").append(SUITE).append("\" name=\"")
            .append(entry.getKey()).append("\" time=\"").append(entry.getValue())
            .append("\"/>\n");
      }
      return xml.append("</testsuite>\n").toString().getBytes(StandardCharsets.UTF_8);
    }
  }

  /** Execute the fixed suite with no discovered third-party engine/listener or classpath scan. */
  public static void main(final String[] args) throws Exception {
    if (args.length != 0) throw new IllegalArgumentException("runner accepts no output paths");
    final Results results = new Results();
    final LauncherConfig config = LauncherConfig.builder()
        .enableTestEngineAutoRegistration(false)
        .enableTestExecutionListenerAutoRegistration(false)
        .enableLauncherDiscoveryListenerAutoRegistration(false)
        .enableLauncherSessionListenerAutoRegistration(false)
        .addTestEngines(new JupiterTestEngine())
        .build();
    LauncherFactory.create(config).execute(
        LauncherDiscoveryRequestBuilder.request()
            .selectors(DiscoverySelectors.selectClass(SUITE))
            .filters(EngineFilter.includeEngines("junit-jupiter"))
            .configurationParameter("junit.jupiter.execution.parallel.enabled", "false")
            .configurationParameter("junit.jupiter.extensions.autodetection.enabled", "false")
            .build(), results);
    final byte[] report = results.report();
    if (report.length > 16 * 1024) throw new AssertionError("bounded report exceeded its limit");
    System.out.println("SORAFS_JAVA_REPORT_V1=" + Base64.getEncoder().encodeToString(report));
    System.out.flush();
    if (System.out.checkError()) throw new AssertionError("bounded report output failed");
    if (results.failed || !results.started.isEmpty() || results.elapsed.size() != 25) {
      throw new AssertionError("the complete fixed Java consumer suite did not succeed");
    }
  }
}
