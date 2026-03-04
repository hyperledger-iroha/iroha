package org.hyperledger.iroha.android.client.mock;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.util.Optional;

/** Lightweight helper for probing loopback ports in test environments. */
final class PortAllocator {
  private PortAllocator() {}

  static Optional<Integer> findAvailablePort(final int desired) {
    if (desired > 0) {
      return isAvailable(desired) ? Optional.of(desired) : Optional.empty();
    }
    for (int port = 49152; port <= 65535; port++) {
      if (isAvailable(port)) {
        return Optional.of(port);
      }
    }
    return Optional.empty();
  }

  private static boolean isAvailable(final int port) {
    try (ServerSocket socket = new ServerSocket()) {
      socket.setReuseAddress(true);
      socket.bind(new InetSocketAddress("127.0.0.1", port));
      return true;
    } catch (IOException ex) {
      return false;
    }
  }
}

