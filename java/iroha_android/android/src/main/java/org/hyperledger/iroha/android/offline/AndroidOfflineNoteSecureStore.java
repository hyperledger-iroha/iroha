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

/** Android Keystore-backed encrypted store for Offline Note wallet notes. */
public final class AndroidOfflineNoteSecureStore implements OfflineNoteStore {
  public static final String DEFAULT_PREFERENCES_NAME = "org.hyperledger.iroha.offline_note";
  public static final String DEFAULT_KEY_ALIAS = "org.hyperledger.iroha.offline_note.store";

  private static final String KEYSTORE_PROVIDER = "AndroidKeyStore";
  private static final String AES_GCM = "AES/GCM/NoPadding";
  private static final int GCM_TAG_BITS = 128;
  private static final int AES_KEY_BITS = 256;
  private static final String INDEX_KEY = "note_index";
  private static final String STORE_REVISION_KEY = "store_revision";
  private static final String NOTE_PREFIX = "note.";
  private static final String VALUE_PREFIX = "enc:";

  private final SharedPreferences preferences;
  private final String keyAlias;

  public AndroidOfflineNoteSecureStore(final Context context) {
    this(context, DEFAULT_PREFERENCES_NAME, DEFAULT_KEY_ALIAS);
  }

  public AndroidOfflineNoteSecureStore(
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
    final Map<String, OfflineNoteWalletNote> notes = loadNotes();
    final T result = mutation.apply(notes);
    saveNotes(notes);
    return result;
  }

  @Override
  public synchronized List<OfflineNoteWalletNote> listNotes() {
    final List<OfflineNoteWalletNote> notes = new ArrayList<>(loadNotes().values());
    notes.sort(
        Comparator.comparingLong(OfflineNoteWalletNote::createdAtMs)
            .thenComparing(OfflineNoteWalletNote::noteCommitmentHex));
    return notes;
  }

  @Override
  public synchronized OfflineNoteWalletNote findNote(final byte[] noteCommitment) {
    return loadNotes().get(OfflineNoteWallet.hexLower(noteCommitment));
  }

  @Override
  public synchronized void upsert(final OfflineNoteWalletNote note) {
    Objects.requireNonNull(note, "note");
    mutateNotes(notes -> {
      notes.put(note.noteCommitmentHex(), note);
      return null;
    });
  }

  public synchronized void delete(final byte[] noteCommitment) {
    final Map<String, OfflineNoteWalletNote> notes = loadNotes();
    notes.remove(OfflineNoteWallet.hexLower(noteCommitment));
    saveNotes(notes);
  }

  public synchronized void clear() {
    final long revision = currentRevision();
    final boolean cleared = preferences.edit().clear().commit();
    if (!cleared) {
      throw new IllegalStateException("failed to clear Offline Note wallet notes");
    }
    deleteKeyAlias(keyAlias);
    if (revision > 0L) {
      deleteKeyAlias(storeKeyAlias(revision));
    }
  }

  private Set<String> indexSnapshot() {
    return new HashSet<>(preferences.getStringSet(INDEX_KEY, Collections.<String>emptySet()));
  }

  private Map<String, OfflineNoteWalletNote> loadNotes() {
    final Map<String, OfflineNoteWalletNote> notes = new LinkedHashMap<>();
    for (final String commitmentHex : indexSnapshot()) {
      final String encrypted = preferences.getString(noteKey(commitmentHex), null);
      if (encrypted == null) {
        throw new IllegalStateException("missing Offline Note wallet note ciphertext");
      }
      final OfflineNoteWalletNote note = OfflineNoteWalletNoteJsonCodec.decode(decrypt(encrypted));
      if (!note.noteCommitmentHex().equals(commitmentHex)) {
        throw new IllegalStateException("Offline Note wallet note commitment mismatch");
      }
      notes.put(commitmentHex, note);
    }
    return notes;
  }

  private void saveNotes(final Map<String, OfflineNoteWalletNote> notes) {
    final SharedPreferences.Editor editor = preferences.edit();
    final Set<String> oldIndex = indexSnapshot();
    final Set<String> newIndex = new HashSet<>(notes.keySet());
    final long oldRevision = currentRevision();
    final long revision = oldRevision + 1L;
    for (final String oldCommitment : oldIndex) {
      if (!newIndex.contains(oldCommitment)) {
        editor.remove(noteKey(oldCommitment));
      }
    }
    for (final Map.Entry<String, OfflineNoteWalletNote> entry : notes.entrySet()) {
      editor.putString(
          noteKey(entry.getKey()),
          encrypt(OfflineNoteWalletNoteJsonCodec.encode(entry.getValue()), revision));
    }
    editor.putStringSet(INDEX_KEY, newIndex);
    editor.putLong(STORE_REVISION_KEY, revision);
    if (!editor.commit()) {
      throw new IllegalStateException("failed to persist Offline Note wallet notes");
    }
    if (oldRevision == 0L) {
      deleteKeyAlias(keyAlias);
    } else {
      deleteKeyAlias(storeKeyAlias(oldRevision));
    }
  }

  private long currentRevision() {
    return preferences.getLong(STORE_REVISION_KEY, 0L);
  }

  private String encrypt(final byte[] plaintext, final long revision) {
    try {
      final Cipher cipher = Cipher.getInstance(AES_GCM);
      cipher.init(Cipher.ENCRYPT_MODE, secretKey(storeKeyAlias(revision), true));
      return VALUE_PREFIX
          + revision
          + ":"
          + Base64.getEncoder().encodeToString(cipher.getIV())
          + ":"
          + Base64.getEncoder().encodeToString(cipher.doFinal(plaintext));
    } catch (final GeneralSecurityException e) {
      throw new IllegalStateException("failed to encrypt Offline Note wallet note", e);
    }
  }

  private byte[] decrypt(final String envelope) {
    if (!envelope.startsWith(VALUE_PREFIX)) {
      throw new IllegalStateException("unknown Offline Note wallet note envelope");
    }
    final String[] parts = envelope.substring(VALUE_PREFIX.length()).split(":", -1);
    if (parts.length == 2) {
      if (parseRevision(parts[0]) != null) {
        throw new IllegalStateException("invalid Offline Note wallet note envelope");
      }
      try {
        final Cipher cipher = Cipher.getInstance(AES_GCM);
        cipher.init(
            Cipher.DECRYPT_MODE,
            secretKey(keyAlias, false),
            new GCMParameterSpec(GCM_TAG_BITS, Base64.getDecoder().decode(parts[0])));
        return cipher.doFinal(Base64.getDecoder().decode(parts[1]));
      } catch (final GeneralSecurityException e) {
        throw new IllegalStateException("failed to decrypt Offline Note wallet note", e);
      }
    }
    if (parts.length != 3) {
      throw new IllegalStateException("invalid Offline Note wallet note envelope");
    }
    final Long revision = parseRevision(parts[0]);
    if (revision == null) {
      throw new IllegalStateException("invalid Offline Note wallet note revision");
    }
    try {
      final Cipher cipher = Cipher.getInstance(AES_GCM);
      cipher.init(
          Cipher.DECRYPT_MODE,
          secretKey(storeKeyAlias(revision.longValue()), false),
          new GCMParameterSpec(GCM_TAG_BITS, Base64.getDecoder().decode(parts[1])));
      return cipher.doFinal(Base64.getDecoder().decode(parts[2]));
    } catch (final GeneralSecurityException e) {
      throw new IllegalStateException("failed to decrypt Offline Note wallet note", e);
    }
  }

  private static Long parseRevision(final String value) {
    try {
      return Long.valueOf(value);
    } catch (final NumberFormatException e) {
      return null;
    }
  }

  private SecretKey secretKey(final String alias, final boolean createIfMissing) throws GeneralSecurityException {
    final KeyStore keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER);
    try {
      keyStore.load(null);
    } catch (final java.io.IOException e) {
      throw new GeneralSecurityException("failed to load Android Keystore", e);
    }
    if (keyStore.containsAlias(alias)) {
      final SecretKey key = (SecretKey) keyStore.getKey(alias, null);
      if (key != null) {
        return key;
      }
    }
    if (!createIfMissing) {
      throw new GeneralSecurityException("missing Offline Note store key for " + alias);
    }
    return generateSecretKey(alias);
  }

  private SecretKey generateSecretKey(final String alias) throws GeneralSecurityException {
    final KeyGenerator generator =
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER);
    final KeyGenParameterSpec.Builder builder =
        new KeyGenParameterSpec.Builder(
                alias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(AES_KEY_BITS)
            .setRandomizedEncryptionRequired(true);
    generator.init(builder.build());
    return generator.generateKey();
  }

  private void deleteKeyAlias(final String alias) {
    try {
      final KeyStore keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER);
      keyStore.load(null);
      if (keyStore.containsAlias(alias)) {
        keyStore.deleteEntry(alias);
      }
    } catch (final GeneralSecurityException | java.io.IOException e) {
      throw new IllegalStateException("failed to delete Offline Note store key", e);
    }
  }

  private String storeKeyAlias(final long revision) {
    return keyAlias + ".rev." + revision;
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
