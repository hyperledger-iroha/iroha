using System.Buffers.Binary;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Offline;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Transactions;

public enum KagemushaInstructionType
{
    Transfer,
    RedeemRecursive,
}

public static class KagemushaInstructionTypeExtensions
{
    public static string ArchiveTypeName(this KagemushaInstructionType instructionType)
    {
        return instructionType switch
        {
            KagemushaInstructionType.Transfer => "KagemushaTransfer",
            KagemushaInstructionType.RedeemRecursive => "RedeemKagemushaRecursive",
            _ => throw new ArgumentOutOfRangeException(nameof(instructionType), instructionType, "Unknown Kagemusha instruction type."),
        };
    }

    public static string WireName(this KagemushaInstructionType instructionType)
    {
        return instructionType switch
        {
            KagemushaInstructionType.Transfer => "iroha_data_model::isi::offline::KagemushaTransfer",
            KagemushaInstructionType.RedeemRecursive => "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
            _ => throw new ArgumentOutOfRangeException(nameof(instructionType), instructionType, "Unknown Kagemusha instruction type."),
        };
    }
}

public sealed record class KagemushaInstructionArchiveInstruction : TransactionInstruction
{
    private readonly byte[] instructionArchive;

    public KagemushaInstructionArchiveInstruction(
        KagemushaInstructionType instructionType,
        byte[] instructionArchive)
    {
        InstructionType = instructionType;
        this.instructionArchive = CopyAndValidateArchive(instructionType, instructionArchive, nameof(instructionArchive));
    }

    public KagemushaInstructionType InstructionType { get; }

    public byte[] InstructionArchive => (byte[])instructionArchive.Clone();

    internal override string WireId => InstructionType.WireName();

    internal override string TypeName => InstructionType.ArchiveTypeName();

    public static KagemushaInstructionArchiveInstruction RedeemRecursive(
        KagemushaRecursiveSpendRedeemInstructionArchive instructionArchive)
    {
        ArgumentNullException.ThrowIfNull(instructionArchive);
        return new KagemushaInstructionArchiveInstruction(
            KagemushaInstructionType.RedeemRecursive,
            instructionArchive.NoritoBytes);
    }

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        return ExtractPayload(instructionArchive);
    }

    internal override byte[] EncodeFramedPayload(TransactionEncodingContext context)
    {
        return InstructionArchive;
    }

    private static byte[] CopyAndValidateArchive(
        KagemushaInstructionType instructionType,
        byte[] archive,
        string parameterName)
    {
        if (archive is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (archive.Length == 0)
        {
            throw new ArgumentException("Kagemusha instruction archive must not be empty.", parameterName);
        }

        if (archive.Length > KagemushaRecursiveSpendNative.NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Kagemusha instruction archive must not exceed {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
                parameterName);
        }

        var copy = (byte[])archive.Clone();
        if (!PrivacyNative.IsNoritoV1Archive(copy))
        {
            throw new ArgumentException(
                "Kagemusha instruction archive must be a valid Norito instruction archive.",
                parameterName);
        }

        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(copy))
        {
            throw new ArgumentException(
                "Kagemusha instruction archive must contain a non-empty Norito payload.",
                parameterName);
        }

        var expectedSchema = NoritoCodec.SchemaHash(instructionType.ArchiveTypeName());
        if (!copy.AsSpan(6, expectedSchema.Length).SequenceEqual(expectedSchema))
        {
            throw new ArgumentException(
                $"Kagemusha instruction archive schema must match {instructionType.ArchiveTypeName()}.",
                parameterName);
        }

        return copy;
    }

    private static byte[] ExtractPayload(byte[] archive)
    {
        var payloadLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, 8)));
        var payloadOffset = archive.Length - payloadLength;
        var payload = new byte[payloadLength];
        Array.Copy(archive, payloadOffset, payload, 0, payloadLength);
        return payload;
    }
}
