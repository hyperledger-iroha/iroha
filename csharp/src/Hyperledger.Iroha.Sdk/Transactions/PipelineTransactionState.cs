namespace Hyperledger.Iroha.Transactions;

public enum PipelineTransactionState
{
    Queued = 1,
    Approved,
    Committed,
    Applied,
    Rejected,
    Expired,
}
