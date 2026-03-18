package android.net;

/** Minimal stub used by reflection-based tests. */
public class NetworkInfo {

  private final boolean connected;
  private final boolean roaming;
  private final String typeName;

  public NetworkInfo(final boolean connected, final boolean roaming, final String typeName) {
    this.connected = connected;
    this.roaming = roaming;
    this.typeName = typeName;
  }

  public boolean isConnected() {
    return connected;
  }

  public boolean isRoaming() {
    return roaming;
  }

  public String getTypeName() {
    return typeName;
  }
}
