using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Transactions;

/// <summary>Exact quoted intent and the transaction signed from that payload.</summary>
public sealed record class QuotedSignedTransaction(
    SignedTransactionEnvelope Transaction,
    ToriiFeeQuoteResponse Quote);

/// <summary>Quoted transaction plus its authoritative pipeline result.</summary>
public sealed record class QuotedTransactionSubmission(
    SignedTransactionEnvelope Transaction,
    ToriiFeeQuoteResponse Quote,
    PipelineTransactionStatus Status);
