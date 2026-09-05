// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;

import java.lang.reflect.Method;
import java.math.BigInteger;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;
import java.util.stream.Collectors;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAccountIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaReceiveFoldResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaStagedPaymentV1;
import org.junit.Test;

/** Java facade checks for the sole request, payment, acknowledgement lifecycle. */
public final class KagemushaWalletV1Tests {
  @Test
  public void facadePublishesOnlyTheDirectThreeMessageLifecycle() {
    final Set<String> methods =
        Arrays.stream(KagemushaWalletV1.class.getDeclaredMethods())
            .map(Method::getName)
            .collect(Collectors.toSet());

    assertEquals(
        new HashSet<>(
            Arrays.asList(
                "open",
                "recover",
                "hardwareCredential",
                "aggregateState",
                "journalRevision",
                "createPaymentRequest",
                "send",
                "stagePayment",
                "stageMintCredit",
                "foldReceiveCredit",
                "drainPendingCredits",
                "recoverPayment",
                "recordAcknowledgement",
                "redeem",
                "recoverRedemption",
                "rotateHardwareEpoch")),
        methods);
  }

  @Test
  public void facadeSignaturesBindRequestPaymentAndAcknowledgementDirectly() throws Exception {
    assertEquals(
        KagemushaPaymentRequestV1.class,
        KagemushaWalletV1.class
            .getMethod(
                "createPaymentRequest", KagemushaAccountIdV1.class, BigInteger.class, long.class)
            .getReturnType());
    assertEquals(
        KagemushaPaymentV1.class,
        KagemushaWalletV1.class
            .getMethod("send", KagemushaPaymentRequestV1.class)
            .getReturnType());
    assertEquals(
        KagemushaStagedPaymentV1.class,
        KagemushaWalletV1.class
            .getMethod(
                "stagePayment", KagemushaPaymentRequestV1.class, KagemushaPaymentV1.class)
            .getReturnType());
    assertEquals(
        KagemushaReceiveFoldResultV1.class,
        KagemushaWalletV1.class.getMethod("foldReceiveCredit", byte[].class).getReturnType());
    assertEquals(
        KagemushaPaymentV1.class,
        KagemushaWalletV1.class
            .getMethod("recoverPayment", KagemushaPaymentRequestV1.class, byte[].class)
            .getReturnType());
    assertEquals(
        void.class,
        KagemushaWalletV1.class
            .getMethod(
                "recordAcknowledgement",
                KagemushaPaymentRequestV1.class,
                KagemushaPaymentV1.class,
                KagemushaAcknowledgementV1.class)
            .getReturnType());
  }
}
