package org.hyperledger.iroha.sdk.offline.wallet;

import android.content.Context;
import android.content.SharedPreferences;
import android.util.Base64;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2;
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2WalletNote;
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2WalletNoteState;
import org.junit.Test;
import org.junit.runner.RunWith;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

@RunWith(AndroidJUnit4.class)
public final class AndroidOfflineNoteV2SecureStoreInstrumentedTest {
  private static final String PREFS = "iroha_offline_note_v2_instrumented";
  private static final String KEY_ALIAS = "iroha_offline_note_v2_instrumented_key";

  @Test
  public void testRevisionKeyRotationRejectsRolledBackSnapshot() throws Exception {
    final Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
    final AndroidOfflineNoteV2SecureStore store =
        new AndroidOfflineNoteV2SecureStore(context, PREFS, KEY_ALIAS);
    store.clear();

    final OfflineNoteV2WalletNote note = sourceWalletNote(loadFixture());
    store.upsert(note);
    assertEquals(1, store.listNotes().size());

    final SharedPreferences preferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    final Map<String, ?> revisionOneSnapshot = snapshot(preferences);

    store.upsert(note.withState(OfflineNoteV2WalletNoteState.SPENT, note.getUpdatedAtMs() + 1));
    assertEquals(OfflineNoteV2WalletNoteState.SPENT, store.listNotes().get(0).getState());

    restore(preferences, revisionOneSnapshot);
    final AndroidOfflineNoteV2SecureStore reloaded =
        new AndroidOfflineNoteV2SecureStore(context, PREFS, KEY_ALIAS);
    try {
      reloaded.listNotes();
      fail("rolled-back Offline Note V2 store snapshot should not decrypt");
    } catch (final Exception expected) {
      assertTrue(
          "revision-one ciphertext should fail because its rotated key is missing",
          expected instanceof IllegalArgumentException);
      assertTrue(expected.getMessage().contains("missing Offline Note V2 store key"));
    } finally {
      reloaded.clear();
    }
  }

  private static Map<String, ?> snapshot(final SharedPreferences preferences) {
    return new HashMap<>(preferences.getAll());
  }

  @SuppressWarnings("unchecked")
  private static void restore(
      final SharedPreferences preferences, final Map<String, ?> snapshot) {
    final SharedPreferences.Editor editor = preferences.edit().clear();
    for (final Map.Entry<String, ?> entry : snapshot.entrySet()) {
      final Object value = entry.getValue();
      if (value instanceof String) {
        editor.putString(entry.getKey(), (String) value);
      } else if (value instanceof Long) {
        editor.putLong(entry.getKey(), (Long) value);
      } else if (value instanceof Set<?>) {
        editor.putStringSet(entry.getKey(), new HashSet<>((Set<String>) value));
      } else {
        throw new IllegalArgumentException("unsupported preference value " + entry.getKey());
      }
    }
    if (!editor.commit()) {
      throw new IllegalStateException("failed to restore rolled-back preferences");
    }
  }

  private Map<String, Object> loadFixture() throws Exception {
    final InputStream stream =
        InstrumentationRegistry.getInstrumentation()
            .getContext()
            .getAssets()
            .open("interop_contract_v2.json");
    try {
      final ByteArrayOutputStream out = new ByteArrayOutputStream();
      final byte[] buffer = new byte[8192];
      int read;
      while ((read = stream.read(buffer)) != -1) {
        out.write(buffer, 0, read);
      }
      @SuppressWarnings("unchecked")
      final Map<String, Object> parsed =
          (Map<String, Object>) JsonParser.parse(out.toString("UTF-8"));
      return parsed;
    } finally {
      stream.close();
    }
  }

  private static OfflineNoteV2WalletNote sourceWalletNote(final Map<String, Object> fixture) {
    final Map<String, Object> chain = obj(fixture, "chain_vectors");
    final Map<String, Object> derivation = obj(chain, "derivation");
    final Map<String, Object> issue = obj(chain, "issue");
    return new OfflineNoteV2WalletNote(
        string(derivation, "chain_id"),
        accountFromAssetId(string(issue, "asset_id")),
        string(issue, "asset_id"),
        string(issue, "amount"),
        certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
        hexBytes(string(derivation, "source_note_commitment")),
        hexBytes(string(derivation, "source_note_secret_hex")),
        new OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
            string(derivation, "issuer_load_operation_id"),
            string(derivation, "issuer_load_lineage_id"),
            longValue(derivation, "issuer_load_local_revision")),
        OfflineNoteV2WalletNoteState.SPENDABLE,
        1_700_000_000_000L,
        1_700_000_000_000L);
  }

  private static OfflineNoteV2.KeyCertificateV2 certificate(final Map<String, Object> json) {
    return new OfflineNoteV2.KeyCertificateV2(
        intValue(json, "version"),
        string(json, "platform"),
        string(json, "key_id"),
        string(json, "device_id"),
        string(json, "account_id"),
        base64Bytes(string(json, "public_key")),
        string(json, "assertion_scheme"),
        string(json, "assertion_key_algorithm"),
        base64Bytes(string(json, "assertion_public_key")),
        nullableInt(json, "assertion_usage_count_limit"),
        bool(json, "one_use"),
        base64Bytes(string(json, "issuer_signature_base64")));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> obj(final Map<String, Object> map, final String key) {
    return (Map<String, Object>) map.get(key);
  }

  private static String string(final Map<String, Object> map, final String key) {
    return (String) map.get(key);
  }

  private static boolean bool(final Map<String, Object> map, final String key) {
    return (Boolean) map.get(key);
  }

  private static int intValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).intValue();
  }

  private static long longValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).longValue();
  }

  private static Integer nullableInt(final Map<String, Object> map, final String key) {
    final Number value = (Number) map.get(key);
    return value == null ? null : value.intValue();
  }

  private static byte[] base64Bytes(final String value) {
    return Base64.decode(value, Base64.DEFAULT);
  }

  private static String accountFromAssetId(final String assetId) {
    return assetId.split("#", 2)[1].split("#dataspace:", 2)[0];
  }

  private static byte[] hexBytes(final String value) {
    final String normalized =
        value.startsWith("0x") || value.startsWith("0X") ? value.substring(2) : value;
    final byte[] out = new byte[normalized.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int hi = Character.digit(normalized.charAt(index * 2), 16);
      final int lo = Character.digit(normalized.charAt(index * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException("invalid hex");
      }
      out[index] = (byte) ((hi << 4) | lo);
    }
    return out;
  }
}
