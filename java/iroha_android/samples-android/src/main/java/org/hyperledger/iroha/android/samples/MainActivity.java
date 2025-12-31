package org.hyperledger.iroha.android.samples;

import android.app.Activity;
import android.os.Bundle;
import android.widget.TextView;
import org.hyperledger.iroha.android.address.AccountAddress;

public class MainActivity extends Activity {
  @Override
  protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);

    byte[] dummyKey = new byte[32];
    String message;
    try {
      AccountAddress address = AccountAddress.fromAccount("wonderland", dummyKey, "ed25519");
      message =
          "Iroha sample app linked against the AAR.\n\n"
              + "Address (hex):\n"
              + address.canonicalHex();
    } catch (AccountAddress.AccountAddressException e) {
      message =
          "Iroha sample app linked against the AAR.\n\n"
              + "Address error:\n"
              + e.getMessage();
    }

    TextView view = new TextView(this);
    view.setText(message);
    view.setPadding(48, 64, 48, 64);

    setContentView(view);
  }
}
