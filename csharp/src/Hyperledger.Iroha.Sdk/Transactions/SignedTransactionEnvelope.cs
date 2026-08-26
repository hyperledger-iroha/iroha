using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

public sealed class SignedTransactionEnvelope
{
    private const byte SupportedSignedTransactionVersion = 1;
    private const int Ed25519SignatureLength = 64;
    private const string SignedTransactionBytesParameterName = "signedTransactionBytes";

    private readonly byte[] versionedNoritoBytes;
    private readonly byte[] signedTransactionBytes;
    private readonly byte[] payloadBytes;
    private readonly byte[] transactionHash;

    public SignedTransactionEnvelope(
        byte[] versionedNoritoBytes,
        byte[] signedTransactionBytes,
        byte[] payloadBytes,
        byte[] transactionHash)
    {
        ArgumentNullException.ThrowIfNull(versionedNoritoBytes);
        ArgumentNullException.ThrowIfNull(signedTransactionBytes);
        ArgumentNullException.ThrowIfNull(payloadBytes);
        ArgumentNullException.ThrowIfNull(transactionHash);

        this.versionedNoritoBytes = CopyNonEmpty(
            versionedNoritoBytes,
            nameof(versionedNoritoBytes));
        this.signedTransactionBytes = CopyNonEmpty(signedTransactionBytes, nameof(signedTransactionBytes));
        this.payloadBytes = CopyNonEmpty(payloadBytes, nameof(payloadBytes));
        this.transactionHash = transactionHash.ToArray();

        if (this.transactionHash.Length != IrohaHash.Length)
        {
            throw new ArgumentException($"Transaction hash must be {IrohaHash.Length} bytes.", nameof(transactionHash));
        }

        if (this.versionedNoritoBytes[0] != SupportedSignedTransactionVersion)
        {
            throw new ArgumentException(
                $"Signed transaction version must be {SupportedSignedTransactionVersion}.",
                nameof(versionedNoritoBytes));
        }

        if (this.versionedNoritoBytes.Length != this.signedTransactionBytes.Length + 1
            || !this.versionedNoritoBytes.AsSpan(1).SequenceEqual(this.signedTransactionBytes))
        {
            throw new ArgumentException(
                "Versioned Norito bytes must contain the signed transaction bytes after the version byte.",
                nameof(versionedNoritoBytes));
        }

        ValidateSignedTransactionFields(this.signedTransactionBytes, this.payloadBytes);

        var expectedTransactionHash = ComputeTransactionHash(this.payloadBytes);
        if (!this.transactionHash.AsSpan().SequenceEqual(expectedTransactionHash))
        {
            throw new ArgumentException(
                "Transaction hash must match the signed transaction intent.",
                nameof(transactionHash));
        }
    }

    public byte[] VersionedNoritoBytes => versionedNoritoBytes.ToArray();

    public byte[] SignedTransactionBytes => signedTransactionBytes.ToArray();

    public byte[] PayloadBytes => payloadBytes.ToArray();

    public byte[] TransactionHash => transactionHash.ToArray();

    public string TransactionHashHex => Convert.ToHexString(transactionHash).ToLowerInvariant();

    private static byte[] CopyNonEmpty(byte[] value, string paramName)
    {
        if (value.Length == 0)
        {
            throw new ArgumentException("Value must not be empty.", paramName);
        }

        return value.ToArray();
    }

    private static byte[] ComputeTransactionHash(ReadOnlySpan<byte> payloadBytes)
    {
        var entrypoint = new CanonicalNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(payloadBytes);
        return IrohaHash.Hash(entrypoint.ToArray());
    }

    private static void ValidateSignedTransactionFields(byte[] signedTransactionBytes, byte[] payloadBytes)
    {
        var transaction = new CanonicalNoritoReader(
            signedTransactionBytes,
            "Signed transaction",
            SignedTransactionBytesParameterName);
        var transactionSignature = transaction.ReadField("signature");
        var payloadField = transaction.ReadField("payload");
        var multisigField = transaction.ReadField("multisig_signatures");
        transaction.RequireEnd();

        var signatureWrapper = new CanonicalNoritoReader(
            transactionSignature,
            "TransactionSignature",
            SignedTransactionBytesParameterName);
        var signatureOf = signatureWrapper.ReadField("signature");
        signatureWrapper.RequireEnd();
        ValidateSignatureConstVec(signatureOf);
        if (!payloadField.SequenceEqual(payloadBytes))
        {
            throw new ArgumentException("Payload bytes must match the signed transaction body.", nameof(payloadBytes));
        }

        if (!multisigField.SequenceEqual(new byte[] { 0 }))
        {
            throw new ArgumentException(
                "Signed transaction bytes must use an empty multisig field in the single-signature wire format.",
                SignedTransactionBytesParameterName);
        }
    }

    private static void ValidateSignatureConstVec(ReadOnlySpan<byte> bytes)
    {
        var signature = new CanonicalNoritoReader(
            bytes,
            "SignatureOf<TransactionPayload>",
            SignedTransactionBytesParameterName);
        var count = signature.ReadSequenceLength("bytes.count");
        if (count != Ed25519SignatureLength)
        {
            throw new ArgumentException(
                $"Signed transaction signature field must encode {Ed25519SignatureLength} bytes.",
                SignedTransactionBytesParameterName);
        }

        for (var index = 0; index < Ed25519SignatureLength; index++)
        {
            if (signature.ReadField($"bytes[{index}]").Length != 1)
            {
                throw new ArgumentException(
                    "Signed transaction signature bytes must be encoded as one-byte fields.",
                    SignedTransactionBytesParameterName);
            }
        }
        signature.RequireEnd();
    }
}
