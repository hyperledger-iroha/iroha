import { Kagemusha } from "../../kagemusha.js";

declare const request: Kagemusha.PaymentRequest;
declare const payment: Kagemusha.Payment;
declare const acknowledgement: Kagemusha.Acknowledgement;
declare const bytes: Uint8Array;

const recipientKey: Uint8Array = request.recipientEncryptionKey;
const proof: Kagemusha.PaymentProof = Kagemusha.decodePaymentProof(Kagemusha.encodePaymentProof(payment.proof));
const bodyDigest: Uint8Array = Kagemusha.paymentBodyDigest(payment.output, payment.encryptedCredit);
Kagemusha.paymentRequestTranscript(request);
Kagemusha.paymentOutputTranscript(payment.output);
Kagemusha.assetIdentityDigest(request.asset);
Kagemusha.accountIdentityDigest(request.recipient);
Kagemusha.preparedTransferDigest(request, bytes, bytes, bytes, bytes);
Kagemusha.creditId(bytes, Kagemusha.paymentRequestDigest(request));
Kagemusha.commitCertificateDigest(payment.commitCertificate);
Kagemusha.encodePayment(new Kagemusha.Payment({ version: 1, output: payment.output,
  encryptedCredit: payment.encryptedCredit, commitCertificate: payment.commitCertificate, proof }), request);
Kagemusha.encodeAcknowledgement(acknowledgement, request, payment);
const kind: Kagemusha.Ipm1PayloadKind = "payment";
const payload: Kagemusha.PayloadKind = "payment";

const stageCommand = new Kagemusha.DeviceMintStageCommand({
  version: 1, canonicalAuthorization: bytes, canonicalMintCredit: bytes,
});
const decodedStageCommand: Kagemusha.DeviceMintStageCommand = Kagemusha.decodeDeviceMintStageCommandShapeExact(
  Kagemusha.encodeDeviceMintStageCommandShape(stageCommand),
);
Kagemusha.encodeDeviceMintStageCommandShape(bytes, bytes);
const stageResult = new Kagemusha.DeviceMintStageResult({
  version: 1, disposition: Kagemusha.deviceMintStageDispositions.exactDuplicate, creditId: bytes,
});
const decodedStageResult: Kagemusha.DeviceMintStageResult = Kagemusha.decodeDeviceMintStageResultShapeExact(
  Kagemusha.encodeDeviceMintStageResultShape(stageResult, decodedStageCommand), decodedStageCommand,
);
Kagemusha.validateDeviceMintStageResultAgainstCommand(decodedStageResult, decodedStageCommand);
// @ts-expect-error operation-16 dispositions are closed
new Kagemusha.DeviceMintStageResult({ version: 1, disposition: 2, creditId: bytes });
// @ts-expect-error operation-16 has no public private-opening field
stageCommand.privateOpening;

// @ts-expect-error request modes were removed from the first-release protocol
request.requestMode;
// @ts-expect-error acceptance-intent transport does not exist
Kagemusha.decodeAcceptanceIntent(bytes, request);
// @ts-expect-error no signature-only payment or cancellation authority is exposed
Kagemusha.paymentTerminalSigningBytes(payment, request);
// @ts-expect-error the credit identity takes only nullifier and request digest
Kagemusha.creditId(bytes, bytes, bytes);
// @ts-expect-error prepared transfer binds both sender commitments
Kagemusha.preparedTransferDigest(request, bytes, bytes, bytes);
// @ts-expect-error old payment signatures are not part of the wire schema
payment.terminalSignature;

void recipientKey; void bodyDigest; void kind; void payload;
