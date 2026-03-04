package org.hyperledger.iroha.android.telemetry;

import android.content.Context;
import android.net.ConnectivityManager;
import android.net.NetworkInfo;

public final class AndroidNetworkContextProviderTests {

  private AndroidNetworkContextProviderTests() {}

  public static void main(final String[] args) {
    emitsSnapshotWhenConnected();
    returnsEmptyWhenDisconnected();
  }

  private static void emitsSnapshotWhenConnected() {
    final NetworkInfo networkInfo = new NetworkInfo(true, true, "MOBILE");
    final ConnectivityManager manager = new ConnectivityManager(networkInfo);
    final Context context = new Context(manager);

    final NetworkContextProvider provider = AndroidNetworkContextProvider.fromContext(context);
    final var snapshot = provider.snapshot();
    assert snapshot.isPresent() : "Expected a snapshot when network is connected";
    assert "cellular".equals(snapshot.get().networkType())
        : "Network type should canonicalise MOBILE to cellular";
    assert snapshot.get().roaming() : "Roaming flag should propagate from NetworkInfo";
  }

  private static void returnsEmptyWhenDisconnected() {
    final NetworkInfo networkInfo = new NetworkInfo(false, false, "WIFI");
    final ConnectivityManager manager = new ConnectivityManager(networkInfo);
    final Context context = new Context(manager);

    final NetworkContextProvider provider = AndroidNetworkContextProvider.fromContext(context);
    assert provider.snapshot().isEmpty()
        : "Disconnected networks should not emit telemetry snapshots";
  }
}
