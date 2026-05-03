package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

public final class MultisigSeedHelperTests {

  @Test
  public void derivedControllerIdIsRejected() throws Exception {
    final String signer = accountIdForSeed((byte) 7);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final Optional<byte[]> derivedKey = MultisigSeedHelper.deriveDeterministicPublicKey(spec);
    assert derivedKey.isPresent() : "derived key must be computed";

    final String multisigAccount = accountIdForPublicKey(derivedKey.get());
    assert MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "derived controller must be rejected";

    try {
      MultisigRegisterInstruction.builder()
          .setAccountId(multisigAccount)
          .setSpec(spec)
          .build();
      throw new AssertionError("expected derived controller rejection");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void nonDerivedControllerIdIsAllowed() throws Exception {
    final String signer = accountIdForSeed((byte) 8);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final String multisigAccount = accountIdForSeed((byte) 9);
    assert !MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "non-derived controller must not be rejected";

    MultisigRegisterInstruction.builder()
        .setAccountId(multisigAccount)
        .setSpec(spec)
        .build();
  }

  @Test
  public void deterministicSeedIgnoresInsertionOrder() throws Exception {
    final String signerA = accountIdForSeed((byte) 0x21);
    final String signerB = accountIdForSeed((byte) 0x22);
    final MultisigSpec left =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(1)
            .addSignatory(signerA, 1)
            .addSignatory(signerB, 1)
            .build();
    final MultisigSpec right =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(1)
            .addSignatory(signerB, 1)
            .addSignatory(signerA, 1)
            .build();
    final Optional<byte[]> leftKey = MultisigSeedHelper.deriveDeterministicPublicKey(left);
    final Optional<byte[]> rightKey = MultisigSeedHelper.deriveDeterministicPublicKey(right);
    assert leftKey.isPresent() : "left derived key must be computed";
    assert rightKey.isPresent() : "right derived key must be computed";
    assert Arrays.equals(leftKey.get(), rightKey.get())
        : "deterministic controller seed must depend only on the canonical signatory set";
  }

  @Test
  public void deterministicSeedAccountIdMaterialUsesTransparentControllerPayload()
      throws Exception {
    final Object parts = accountIdPartsForSeed((byte) 0x31);
    final TypeAdapter<Object> adapter = accountIdAdapter();
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.MINOR_VERSION);
    adapter.encode(encoder, parts);

    final NoritoDecoder decoder =
        new NoritoDecoder(encoder.toByteArray(), NoritoHeader.MINOR_VERSION);
    final long controllerTag = NoritoAdapters.uint(32).decode(decoder);
    assert controllerTag == 0L
        : "AccountId payload must start with AccountController::Single tag";
    final long publicKeyLength = decoder.readLength(decoder.compactLenActive());
    final byte[] publicKeyPayload = decoder.readBytes(Math.toIntExact(publicKeyLength));
    assert decoder.remaining() == 0 : "AccountId payload must not wrap AccountController";

    final NoritoDecoder publicKeyDecoder =
        new NoritoDecoder(publicKeyPayload, NoritoHeader.MINOR_VERSION);
    final String literal = NoritoAdapters.stringAdapter().decode(publicKeyDecoder);
    assert literal.startsWith("ed0120") : "public key literal must be an Ed25519 multihash";
    assert publicKeyDecoder.remaining() == 0 : "public key payload must be a single string";
  }

  private static String accountIdForSeed(final byte seed) throws Exception {
    final byte[] seedBytes = new byte[32];
    Arrays.fill(seedBytes, seed);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seedBytes, 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    return accountIdForPublicKey(publicKey);
  }

  private static String accountIdForPublicKey(final byte[] publicKey) throws Exception {
    final AccountAddress address = AccountAddress.fromAccount(publicKey, "ed25519");
    return address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static Object accountIdPartsForSeed(final byte seed) throws Exception {
    final java.lang.reflect.Method method =
        MultisigSeedHelper.class.getDeclaredMethod("parseAccountIdParts", String.class);
    method.setAccessible(true);
    final Optional<?> parsed = (Optional<?>) method.invoke(null, accountIdForSeed(seed));
    return parsed.orElseThrow(() -> new AssertionError("expected account id parts"));
  }

  @SuppressWarnings("unchecked")
  private static TypeAdapter<Object> accountIdAdapter() throws Exception {
    final java.lang.reflect.Field field =
        MultisigSeedHelper.class.getDeclaredField("ACCOUNT_ID_ADAPTER");
    field.setAccessible(true);
    return (TypeAdapter<Object>) field.get(null);
  }
}
