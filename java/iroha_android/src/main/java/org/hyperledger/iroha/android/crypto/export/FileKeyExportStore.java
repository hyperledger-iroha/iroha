package org.hyperledger.iroha.android.crypto.export;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.util.Objects;
import java.util.Optional;
import java.util.Properties;

/** File-backed key export store using a {@link Properties} map. */
public final class FileKeyExportStore implements KeyExportStore {
  private final File file;
  private final Object lock = new Object();

  public FileKeyExportStore(final File file) {
    this.file = Objects.requireNonNull(file, "file");
  }

  @Override
  public Optional<String> load(final String alias) throws KeyExportException {
    if (alias == null || alias.isBlank()) {
      return Optional.empty();
    }
    synchronized (lock) {
      final Properties properties = readProperties();
      return Optional.ofNullable(properties.getProperty(alias));
    }
  }

  @Override
  public void store(final String alias, final String bundleBase64) throws KeyExportException {
    if (alias == null || alias.isBlank()) {
      throw new KeyExportException("alias must be provided");
    }
    if (bundleBase64 == null || bundleBase64.isBlank()) {
      throw new KeyExportException("bundleBase64 must be provided");
    }
    synchronized (lock) {
      final Properties properties = readProperties();
      properties.setProperty(alias, bundleBase64);
      writeProperties(properties);
    }
  }

  @Override
  public void delete(final String alias) throws KeyExportException {
    if (alias == null || alias.isBlank()) {
      return;
    }
    synchronized (lock) {
      final Properties properties = readProperties();
      if (properties.remove(alias) != null) {
        writeProperties(properties);
      }
    }
  }

  private Properties readProperties() throws KeyExportException {
    final Properties properties = new Properties();
    if (!file.exists()) {
      return properties;
    }
    try (FileInputStream input = new FileInputStream(file)) {
      properties.load(input);
      return properties;
    } catch (final IOException ex) {
      throw new KeyExportException("Failed to read key export store", ex);
    }
  }

  private void writeProperties(final Properties properties) throws KeyExportException {
    final File parent = file.getParentFile();
    if (parent != null && !parent.exists() && !parent.mkdirs()) {
      throw new KeyExportException("Failed to create key export store directory");
    }
    try (FileOutputStream output = new FileOutputStream(file)) {
      properties.store(output, "Iroha Android key exports");
    } catch (final IOException ex) {
      throw new KeyExportException("Failed to write key export store", ex);
    }
  }
}
