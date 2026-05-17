package org.hyperledger.iroha.android.offline;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/** Android Keystore-backed encrypted store for Offline Note V2 wallet notes. */
public final class AndroidOfflineNoteV2SecureStore implements OfflineNoteV2Store {
  public static final String DEFAULT_PREFERENCES_NAME = "org.hyperledger.iroha.offline_note_v2";
  public static final String DEFAULT_KEY_ALIAS = "org.hyperledger.iroha.offline_note_v2.store";

  private static final String KEYSTORE_PROVIDER = "AndroidKeyStore";
  private static final String AES_GCM = "AES/GCM/NoPadding";
  private static final int GCM_TAG_BITS = 128;
  private static final int AES_KEY_BITS = 256;
  private static final String INDEX_KEY = "note_index";
  private static final String NOTE_PREFIX = "note.";
  private static final String VALUE_PREFIX = "v1:";

  private final SharedPreferences preferences;
  private final String keyAlias;

  public AndroidOfflineNoteV2SecureStore(final Context context) {
    this(context, DEFAULT_PREFERENCES_NAME, DEFAULT_KEY_ALIAS);
  }

  public AndroidOfflineNoteV2SecureStore(
      final Context context, final String preferencesName, final String keyAlias) {
    final Context applicationContext =
        Objects.requireNonNull(context, "context").getApplicationContext() == null
            ? context
            : context.getApplicationContext();
    this.preferences =
        applicationContext.getSharedPreferences(
            requireNonBlank(preferencesName, "preferencesName"), Context.MODE_PRIVATE);
    this.keyAlias = requireNonBlank(keyAlias, "keyAlias");
  }

  @Override
  public synchronized <T> T mutateNotes(final Mutation<T> mutation) {
    final Map<String, OfflineNoteV2WalletNote> notes = loadNotes();
    final T result = mutation.apply(notes);
    saveNotes(notes);
    return result;
  }

  @Override
  public synchronized List<OfflineNoteV2WalletNote> listNotes() {
    final List<OfflineNoteV2WalletNote> notes = new ArrayList<>(loadNotes().values());
    notes.sort(
        Comparator.comparingLong(OfflineNoteV2WalletNote::createdAtMs)
            .thenComparing(OfflineNoteV2WalletNote::noteCommitmentHex));
    return notes;
  }

  @Override
  public synchronized OfflineNoteV2WalletNote findNote(final byte[] noteCommitment) {
    return loadNotes().get(OfflineNoteV2Wallet.hexLower(noteCommitment));
  }

  @Override
  public synchronized void upsert(final OfflineNoteV2WalletNote note) {
    Objects.requireNonNull(note, "note");
    mutateNotes(notes -> {
      notes.put(note.noteCommitmentHex(), note);
      return null;
    });
  }

  public synchronized void delete(final byte[] noteCommitment) {
    final String commitmentHex = OfflineNoteV2Wallet.hexLower(noteCommitment);
    final Set<String> index = indexSnapshot();
    index.remove(commitmentHex);
    final boolean deleted =
        preferences
            .edit()
            .remove(noteKey(commitmentHex))
            .putStringSet(INDEX_KEY, index)
            .commit();
    if (!deleted) {
      throw new IllegalStateException("failed to delete Offline Note V2 wallet note");
    }
  }

  public synchronized void clear() {
    final boolean cleared = preferences.edit().clear().commit();
    if (!cleared) {
      throw new IllegalStateException("failed to clear Offline Note V2 wallet notes");
    }
  }

  private Set<String> indexSnapshot() {
    return new HashSet<>(preferences.getStringSet(INDEX_KEY, Collections.<String>emptySet()));
  }

  private Map<String, OfflineNoteV2WalletNote> loadNotes() {
    final Map<String, OfflineNoteV2WalletNote> notes = new LinkedHashMap<>();
    for (final String commitmentHex : indexSnapshot()) {
      final String encrypted = preferences.getString(noteKey(commitmentHex), null);
      if (encrypted != null) {
        notes.put(commitmentHex, OfflineNoteV2WalletNoteJsonCodec.decode(decrypt(encrypted)));
      }
    }
    return notes;
  }

  private void saveNotes(final Map<String, OfflineNoteV2WalletNote> notes) {
    final SharedPreferences.Editor editor = preferences.edit();
    final Set<String> oldIndex = indexSnapshot();
    final Set<String> newIndex = new HashSet<>(notes.keySet());
    for (final String oldCommitment : oldIndex) {
      if (!newIndex.contains(oldCommitment)) {
        editor.remove(noteKey(oldCommitment));
      }
    }
    for (final Map.Entry<String, OfflineNoteV2WalletNote> entry : notes.entrySet()) {
      editor.putString(noteKey(entry.getKey()), encrypt(OfflineNoteV2WalletNoteJsonCodec.encode(entry.getValue())));
    }
    editor.putStringSet(INDEX_KEY, newIndex);
    if (!editor.commit()) {
      throw new IllegalStateException("failed to persist Offline Note V2 wallet notes");
    }
  }

  private String encrypt(final byte[] plaintext) {
    try {
      final Cipher cipher = Cipher.getInstance(AES_GCM);
      cipher.init(Cipher.ENCRYPT_MODE, secretKey());
      return VALUE_PREFIX
          + Base64.getEncoder().encodeToString(cipher.getIV())
          + ":"
          + Base64.getEncoder().encodeToString(cipher.doFinal(plaintext));
    } catch (final GeneralSecurityException e) {
      throw new IllegalStateException("failed to encrypt Offline Note V2 wallet note", e);
    }
  }

  private byte[] decrypt(final String envelope) {
    if (!envelope.startsWith(VALUE_PREFIX)) {
      throw new IllegalStateException("unknown Offline Note V2 wallet note envelope");
    }
    final String[] parts = envelope.substring(VALUE_PREFIX.length()).split(":", -1);
    if (parts.length != 2) {
      throw new IllegalStateException("invalid Offline Note V2 wallet note envelope");
    }
    try {
      final Cipher cipher = Cipher.getInstance(AES_GCM);
      cipher.init(
          Cipher.DECRYPT_MODE,
          secretKey(),
          new GCMParameterSpec(GCM_TAG_BITS, Base64.getDecoder().decode(parts[0])));
      return cipher.doFinal(Base64.getDecoder().decode(parts[1]));
    } catch (final GeneralSecurityException e) {
      throw new IllegalStateException("failed to decrypt Offline Note V2 wallet note", e);
    }
  }

  private SecretKey secretKey() throws GeneralSecurityException {
    final KeyStore keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER);
    try {
      keyStore.load(null);
    } catch (final java.io.IOException e) {
      throw new GeneralSecurityException("failed to load Android Keystore", e);
    }
    if (keyStore.containsAlias(keyAlias)) {
      final SecretKey key = (SecretKey) keyStore.getKey(keyAlias, null);
      if (key != null) {
        return key;
      }
    }
    final KeyGenerator generator =
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER);
    final KeyGenParameterSpec spec =
        new KeyGenParameterSpec.Builder(
                keyAlias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(AES_KEY_BITS)
            .setRandomizedEncryptionRequired(true)
            .build();
    generator.init(spec);
    return generator.generateKey();
  }

  private static String noteKey(final String commitmentHex) {
    return NOTE_PREFIX + commitmentHex;
  }

  private static String requireNonBlank(final String value, final String field) {
    final String checked = Objects.requireNonNull(value, field);
    if (checked.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return checked;
  }
}
