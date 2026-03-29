using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Transactions;

public sealed class LedgerClient
{
    private readonly ToriiClient torii;

    internal LedgerClient(ToriiClient torii)
    {
        this.torii = torii;
    }

    public TransactionBuilder BuildTransaction(string chainId, string authorityAccountId)
    {
        return new TransactionBuilder(chainId, authorityAccountId);
    }

    public Task SubmitAsync(SignedTransactionEnvelope transaction, CancellationToken cancellationToken = default)
    {
        return torii.SubmitTransactionAsync(transaction, cancellationToken);
    }

    public async Task<PipelineTransactionStatus> SubmitAndWaitAsync(
        SignedTransactionEnvelope transaction,
        PipelineSubmitOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(transaction);

        await SubmitAsync(transaction, cancellationToken);
        return await WaitForAsync(transaction.TransactionHashHex, options, cancellationToken);
    }

    public async Task<PipelineTransactionStatus> WaitForAsync(
        string transactionHashHex,
        PipelineSubmitOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(transactionHashHex);

        var effective = options ?? PipelineSubmitOptions.Default;
        if (effective.Timeout <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Pipeline wait timeout must be positive.");
        }

        if (effective.PollInterval <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Pipeline poll interval must be positive.");
        }

        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(effective.Timeout);

        while (true)
        {
            timeout.Token.ThrowIfCancellationRequested();
            var status = await torii.GetPipelineTransactionStatusAsync(transactionHashHex, effective.Scope, timeout.Token);
            if (status is not null && status.IsTerminal)
            {
                return status;
            }

            await Task.Delay(effective.PollInterval, timeout.Token);
        }
    }
}
