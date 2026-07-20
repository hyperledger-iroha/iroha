namespace Hyperledger.Iroha.Transactions;

/// <summary>A signature-bound invocation of one deployed contract revision.</summary>
public sealed class TransactionContractInvocation
{
    public const int MaximumArgumentsBytes = 1024 * 1024;

    private readonly byte[] expectedCodeHash;
    private readonly byte[]? arguments;

    public TransactionContractInvocation(
        string contractAddress,
        ReadOnlySpan<byte> expectedCodeHash,
        string entrypoint,
        byte[]? arguments = null)
    {
        if (!ContractAddressV1.IsCanonical(contractAddress))
        {
            throw new ArgumentException(
                "Contract address must be a canonical lowercase V1 Bech32m literal.",
                nameof(contractAddress));
        }
        ContractAddress = contractAddress;
        Entrypoint = RequireCanonicalText(entrypoint, nameof(entrypoint), lowercase: false);
        if (expectedCodeHash.Length != 32)
        {
            throw new ArgumentException("Expected code hash must contain exactly 32 bytes.", nameof(expectedCodeHash));
        }
        if ((expectedCodeHash[^1] & 1) == 0)
        {
            throw new ArgumentException("Expected code hash must use the canonical marked Hash encoding.", nameof(expectedCodeHash));
        }
        if (arguments is { Length: > MaximumArgumentsBytes })
        {
            throw new ArgumentException(
                $"Contract arguments exceed the {MaximumArgumentsBytes}-byte wire limit.",
                nameof(arguments));
        }

        this.expectedCodeHash = expectedCodeHash.ToArray();
        this.arguments = arguments?.ToArray();
    }

    public string ContractAddress { get; }

    public byte[] ExpectedCodeHash => expectedCodeHash.ToArray();

    public string ExpectedCodeHashLiteral
    {
        get
        {
            var body = Convert.ToHexString(expectedCodeHash);
            var checksum = Crc16(System.Text.Encoding.ASCII.GetBytes($"hash:{body}"));
            return $"hash:{body}#{checksum:X4}";
        }
    }

    public string Entrypoint { get; }

    public byte[]? Arguments => arguments?.ToArray();

    internal ReadOnlySpan<byte> ExpectedCodeHashSpan => expectedCodeHash;

    internal ReadOnlySpan<byte> ArgumentsSpan => arguments;

    internal bool HasArguments => arguments is not null;

    private static string RequireCanonicalText(string? value, string paramName, bool lowercase)
    {
        if (string.IsNullOrEmpty(value)
            || !string.Equals(value.Trim(), value, StringComparison.Ordinal)
            || value.Any(char.IsWhiteSpace)
            || value.Any(char.IsControl)
            || (lowercase && !string.Equals(value.ToLowerInvariant(), value, StringComparison.Ordinal)))
        {
            throw new ArgumentException("Value must be exact canonical text.", paramName);
        }
        return value;
    }

    private static ushort Crc16(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xffff;
        foreach (var value in bytes)
        {
            crc ^= value << 8;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 0x8000) != 0
                    ? ((crc << 1) ^ 0x1021) & 0xffff
                    : (crc << 1) & 0xffff;
            }
        }
        return (ushort)crc;
    }
}

/// <summary>One ordered item in an atomic mixed executable batch.</summary>
public abstract record TransactionBatchEntry
{
    private TransactionBatchEntry() { }

    public sealed record InstructionEntry : TransactionBatchEntry
    {
        public InstructionEntry(TransactionInstruction instruction)
        {
            Value = instruction ?? throw new ArgumentNullException(nameof(instruction));
        }

        public TransactionInstruction Value { get; }
    }

    public sealed record ContractCallEntry : TransactionBatchEntry
    {
        public ContractCallEntry(TransactionContractInvocation invocation)
        {
            Invocation = invocation ?? throw new ArgumentNullException(nameof(invocation));
        }

        public TransactionContractInvocation Invocation { get; }
    }

    public static TransactionBatchEntry Instruction(TransactionInstruction instruction) =>
        new InstructionEntry(instruction);

    public static TransactionBatchEntry ContractCall(TransactionContractInvocation invocation) =>
        new ContractCallEntry(invocation);
}
