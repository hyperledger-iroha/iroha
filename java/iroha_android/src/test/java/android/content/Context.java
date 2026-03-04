package android.content;

import android.net.ConnectivityManager;

/** Minimal stub used by reflection-based tests. */
public class Context {
  public static final String CONNECTIVITY_SERVICE = "connectivity";

  private final ConnectivityManager connectivityManager;

  public Context(final ConnectivityManager connectivityManager) {
    this.connectivityManager = connectivityManager;
  }

  public Object getSystemService(final String name) {
    if (CONNECTIVITY_SERVICE.equals(name)) {
      return connectivityManager;
    }
    return null;
  }
}
