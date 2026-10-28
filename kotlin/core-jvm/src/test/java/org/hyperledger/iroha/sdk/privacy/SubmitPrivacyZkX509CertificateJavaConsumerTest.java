package org.hyperledger.iroha.sdk.privacy;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.sdk.core.model.*;
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.sdk.tx.norito.NoritoException;
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.sdk.tx.SignedTransaction;
import org.junit.jupiter.api.Test;

/** Java consumers exercise the canonical Kotlin request and attached action factory directly. */
public final class SubmitPrivacyZkX509CertificateJavaConsumerTest {
  @Test
  public void canonicalAttachedRequestMaterializesWithoutInlineProofOrMutableAliases() throws Exception {
    final PrivacyExact12TypedFixtureRowV1 row = X509AttachedRequestTestInputs.row();
    final byte[] encoded = row.getSubmitProofInstructionNorito();
    final SubmitPrivacyZkX509CertificateV1 request =
        SubmitPrivacyZkX509CertificateV1.fromCanonicalRequestV1(encoded);
    Arrays.fill(encoded, (byte) 0);
    assertArrayEquals(row.getSubmitProofInstructionNorito(), request.canonicalRequestBytes());
    final NoritoJavaCodecAdapter codec = X509AttachedRequestTestInputs.codec();
    final TransactionPayload payload = codec.decodeTransaction(row.getUnsignedTransactionPayloadNorito());
    assertArrayEquals(row.getStatementNorito(),
        request.materializeStatementV1(payload, codec.getChainDiscriminant()));
    assertArrayEquals(row.getTransactionIntentProjectionNorito(),
        codec.actionStatementProjectionV1(payload, request.getProofActionIndex()));
    final InstructionBox unadmitted = InstructionBox.fromWirePayload(
        SubmitPrivacyZkX509CertificateV1.WIRE_ID, request.canonicalRequestBytes());
    assertThrows(IllegalArgumentException.class, unadmitted::requirePrivacyExact12ConstructionAdmission);
    assertThrows(IllegalArgumentException.class, () ->
        SubmitPrivacyZkX509CertificateV1.fromStatementV1(row.getStatementNorito(), 0L));
  }

  @Test
  public void inspectingAuthenticSignedWireDoesNotAdmitDecodedInstructions() throws Exception {
    final PrivacyExact12TypedFixtureRowV1 row = X509AttachedRequestTestInputs.row();
    final byte[] exact = row.getSignedTransactionVersionedNorito();
    final SignedTransaction signed = SignedTransactionEncoder.decodeVersioned(exact);
    assertArrayEquals(exact, SignedTransactionEncoder.encodeVersioned(signed));
    final NoritoJavaCodecAdapter codec = X509AttachedRequestTestInputs.codec();
    final TransactionPayload decoded = codec.decodeTransaction(signed.encodedPayload());
    assertThrows(NoritoException.class, () -> codec.encodeTransaction(decoded));
    final InstructionBox box = ((Executable.Instructions) decoded.getExecutable()).getInstructions().get(0);
    assertThrows(IllegalArgumentException.class, box::requirePrivacyExact12ConstructionAdmission);
  }

  @Test
  public void separatePrivacyAndAttachmentOrdinalsSurvivePlanningAndSignedProofReplacement() {
    final PrivacyExact12TypedFixtureRowV1 row = X509AttachedRequestTestInputs.row();
    final byte[] independent = X509AttachedRequestTestInputs.independentStatement(row.getStatementNorito());
    final SubmitPrivacyZkX509CertificateV1 request =
        SubmitPrivacyZkX509CertificateV1.fromStatementV1(independent, 1L);
    assertEquals(0L, X509AttachedRequestTestInputs.privacyOrdinal(independent));
    assertEquals(1L, request.getProofActionIndex());
    assertEquals(0L, request.getProofSlotIndex());
    final ActionIntentV1 action = request.actionIntentV1(
        ActionAuthorityBindingV1.TransactionAuthority.INSTANCE,
        new ActionInvocationBindingV1.Direct(4L), 0L);
    assertEquals(1L, action.getActionIndex());
    assertEquals(ActionRoleV1.ProofEffect, action.getRole());
    assertEquals(new ActionInvocationBindingV1.Direct(4L), action.getInvocation());
    final SignedAttachmentOccurrenceV1 planned =
        SubmitPrivacyZkX509CertificateV1.plannedProofOccurrenceV1(0L);
    assertEquals(0L, planned.getLength());
    assertEquals(AttachmentKindV1.Proof, planned.getKind());
    assertEquals(0, planned.getBytes().length);
    assertThrows(IllegalArgumentException.class, () ->
        new SignedAttachmentsV1(Collections.singletonList(action), Collections.singletonList(planned))
            .validateStructure());
    final byte[] proofBytes = new byte[] {1, 2, 3};
    final SignedAttachmentOccurrenceV1 signedProof =
        SignedAttachmentOccurrenceV1.of(0L, AttachmentKindV1.Proof, proofBytes);
    final ActionIntentV1 completed = request.withSignedProofV1(action, signedProof);
    Arrays.fill(proofBytes, (byte) 0);
    assertEquals(action.getAuthority(), completed.getAuthority());
    assertEquals(action.getInvocation(), completed.getInvocation());
    assertEquals(action.getPublicInputsDigest(), completed.getPublicInputsDigest());
    assertEquals(AttachmentRoleV1.ActionProof, completed.getAttachments().get(0).getRole());
    assertArrayEquals(new byte[] {1, 2, 3}, signedProof.getBytes());
    assertThrows(IllegalArgumentException.class, () -> request.withSignedProofV1(action,
        SignedAttachmentOccurrenceV1.of(1L, AttachmentKindV1.Proof, new byte[] {1})));
    assertThrows(IllegalArgumentException.class, () -> request.withSignedProofV1(action,
        SignedAttachmentOccurrenceV1.of(0L, AttachmentKindV1.KeyMaterial, new byte[] {1})));
    assertThrows(IllegalArgumentException.class, () -> request.actionIntentV1(
        ActionAuthorityBindingV1.TransactionAuthority.INSTANCE, ActionInvocationBindingV1.Transaction.INSTANCE, 0L));
  }
}
