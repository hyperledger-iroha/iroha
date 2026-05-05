namespace Hyperledger.Iroha.Transactions;

public sealed record class PipelineTransactionStatus
{
    public string HashHex { get; init; } = string.Empty;

    public PipelineTransactionState State { get; init; }

    public string RawKind { get; init; } = string.Empty;

    public ulong? BlockHeight { get; init; }

    public string Scope { get; init; } = string.Empty;

    public string ResolvedFrom { get; init; } = string.Empty;

    public string? RejectionContentBase64 { get; init; }

    public bool IsTerminal =>
        State is PipelineTransactionState.Applied
            or PipelineTransactionState.Committed
            or PipelineTransactionState.Rejected
            or PipelineTransactionState.Expired;

    public bool IsSuccess =>
        State is PipelineTransactionState.Applied or PipelineTransactionState.Committed;
}
