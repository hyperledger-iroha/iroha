using System.Buffers.Binary;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

public sealed class SignedTransactionEnvelope
{
    private const int Ed25519SignatureLength = 64;
    private const string SignedTransactionBytesParameterName = "signedTransactionBytes";

    private readonly byte[] noritoBytes;
    private readonly byte[] signedTransactionBytes;
    private readonly byte[] payloadBytes;
    private readonly byte[] transactionHash;

    public SignedTransactionEnvelope(
        byte[] noritoBytes,
        byte[] signedTransactionBytes,
        byte[] payloadBytes,
        byte[] transactionHash)
    {
        ArgumentNullException.ThrowIfNull(noritoBytes);
        ArgumentNullException.ThrowIfNull(signedTransactionBytes);
        ArgumentNullException.ThrowIfNull(payloadBytes);
        ArgumentNullException.ThrowIfNull(transactionHash);

        this.noritoBytes = CopyNonEmpty(noritoBytes, nameof(noritoBytes));
        this.signedTransactionBytes = CopyNonEmpty(signedTransactionBytes, nameof(signedTransactionBytes));
        this.payloadBytes = CopyNonEmpty(payloadBytes, nameof(payloadBytes));
        this.transactionHash = transactionHash.ToArray();

        if (this.transactionHash.Length != IrohaHash.Length)
        {
            throw new ArgumentException($"Transaction hash must be {IrohaHash.Length} bytes.", nameof(transactionHash));
        }

        if (!this.noritoBytes.AsSpan().SequenceEqual(this.signedTransactionBytes))
        {
            throw new ArgumentException(
                "Norito bytes must match the signed transaction bytes for the current transaction wire format.",
                nameof(noritoBytes));
        }

        var expectedTransactionHash = ComputeTransactionHash(this.signedTransactionBytes);
        if (!this.transactionHash.AsSpan().SequenceEqual(expectedTransactionHash))
        {
            throw new ArgumentException(
                "Transaction hash must match the signed transaction bytes.",
                nameof(transactionHash));
        }

        ValidateSignedTransactionFields(this.signedTransactionBytes, this.payloadBytes);
    }

    public byte[] NoritoBytes => noritoBytes.ToArray();

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

    private static byte[] ComputeTransactionHash(ReadOnlySpan<byte> signedTransactionBytes)
    {
        var entrypoint = new OfflineNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(signedTransactionBytes);
        return IrohaHash.Hash(entrypoint.ToArray());
    }

    private static void ValidateSignedTransactionFields(byte[] signedTransactionBytes, byte[] payloadBytes)
    {
        var offset = 0;
        var signatureField = ReadField(signedTransactionBytes, ref offset);
        var payloadField = ReadField(signedTransactionBytes, ref offset);
        var attachmentsField = ReadField(signedTransactionBytes, ref offset);
        var multisigField = ReadField(signedTransactionBytes, ref offset);
        if (offset != signedTransactionBytes.Length)
        {
            throw new ArgumentException(
                "Signed transaction bytes must contain exactly signature, payload, attachments, and multisig fields.",
                SignedTransactionBytesParameterName);
        }

        ValidateSignatureConstVec(signatureField);
        if (!payloadField.SequenceEqual(payloadBytes))
        {
            throw new ArgumentException("Payload bytes must match the signed transaction body.", nameof(payloadBytes));
        }

        if (!attachmentsField.SequenceEqual(new byte[] { 0 }) || !multisigField.SequenceEqual(new byte[] { 0 }))
        {
            throw new ArgumentException(
                "Signed transaction bytes must use empty attachments and multisig fields in the current wire format.",
                SignedTransactionBytesParameterName);
        }
    }

    private static ReadOnlySpan<byte> ReadField(ReadOnlySpan<byte> bytes, ref int offset)
    {
        if (bytes.Length - offset < sizeof(ulong))
        {
            throw new ArgumentException(
                "Signed transaction bytes contain a truncated field header.",
                SignedTransactionBytesParameterName);
        }

        var length = BinaryPrimitives.ReadUInt64LittleEndian(bytes.Slice(offset, sizeof(ulong)));
        offset += sizeof(ulong);
        if (length > int.MaxValue || length > (ulong)(bytes.Length - offset))
        {
            throw new ArgumentException(
                "Signed transaction bytes contain a truncated field payload.",
                SignedTransactionBytesParameterName);
        }

        var payload = bytes.Slice(offset, (int)length);
        offset += (int)length;
        return payload;
    }

    private static void ValidateSignatureConstVec(ReadOnlySpan<byte> bytes)
    {
        var offset = 0;
        if (bytes.Length < sizeof(ulong))
        {
            throw new ArgumentException(
                "Signed transaction signature field is truncated.",
                SignedTransactionBytesParameterName);
        }

        var count = BinaryPrimitives.ReadUInt64LittleEndian(bytes[..sizeof(ulong)]);
        offset += sizeof(ulong);
        if (count != Ed25519SignatureLength)
        {
            throw new ArgumentException(
                $"Signed transaction signature field must encode {Ed25519SignatureLength} bytes.",
                SignedTransactionBytesParameterName);
        }

        for (var index = 0; index < Ed25519SignatureLength; index++)
        {
            if (bytes.Length - offset < sizeof(ulong) + 1)
            {
                throw new ArgumentException(
                    "Signed transaction signature field is truncated.",
                    SignedTransactionBytesParameterName);
            }

            var fieldLength = BinaryPrimitives.ReadUInt64LittleEndian(bytes.Slice(offset, sizeof(ulong)));
            offset += sizeof(ulong);
            if (fieldLength != 1)
            {
                throw new ArgumentException(
                    "Signed transaction signature bytes must be encoded as one-byte fields.",
                    SignedTransactionBytesParameterName);
            }

            offset++;
        }

        if (offset != bytes.Length)
        {
            throw new ArgumentException(
                "Signed transaction signature field has trailing bytes.",
                SignedTransactionBytesParameterName);
        }
    }
}
