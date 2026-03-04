package android.net;

/** Minimal stub used by reflection-based tests. */
public class ConnectivityManager {

  private final NetworkInfo activeNetworkInfo;

  public ConnectivityManager(final NetworkInfo activeNetworkInfo) {
    this.activeNetworkInfo = activeNetworkInfo;
  }

  public NetworkInfo getActiveNetworkInfo() {
    return activeNetworkInfo;
  }
}
