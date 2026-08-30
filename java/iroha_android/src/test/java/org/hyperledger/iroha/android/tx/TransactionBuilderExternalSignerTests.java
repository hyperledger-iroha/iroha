// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.tx;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;

import java.util.Collections;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestNetworkIds;
import org.junit.Test;

public final class TransactionBuilderExternalSignerTests {
  @Test
  public void externalSignerOnlyBuilderNeverResolvesOrCreatesAnAlias() throws Exception {
    final TransactionBuilder builder =
        new TransactionBuilder(
            new NoritoJavaCodecAdapter(
                org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT));
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(TestNetworkIds.fromSeed(23L))
            .setAuthority(TestAccountIds.ed25519Authority(0x53))
            .setExecutable(Executable.ivm(new byte[] {1, 2, 3}))
            .build();
    final Signer signer =
        new Signer() {
          @Override
          public byte[] sign(final byte[] message) throws SigningException {
            return new byte[] {4, 5, 6};
          }

          @Override
          public byte[] publicKey() {
            return new byte[] {7, 8, 9};
          }

          @Override
          public String algorithm() {
            return "Ed25519";
          }
        };

    final SignedTransaction signed = builder.encodeAndSign(payload, signer);

    assertArrayEquals(new byte[] {4, 5, 6}, signed.signature());
    assertFalse(signed.keyAlias().isPresent());
    assertThrows(
        org.hyperledger.iroha.android.KeyManagementException.class,
        () ->
            builder.encodeAndSign(
                payload,
                "must-not-exist",
                IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY));
  }
}
