using Hyperledger.Iroha.Kagemusha;
using KagemushaCodec = Hyperledger.Iroha.Kagemusha.Kagemusha;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// Atomically debits the payer named by the transaction authority and creates one
/// receiver-bound KAGEMUSHA mint credit in the pooled reserve.
/// </summary>
public sealed record class TopUpKagemushaV1Instruction : TransactionInstruction
{
    public TopUpKagemushaV1Instruction(KagemushaTopUpRequestV1 request)
    {
        ArgumentNullException.ThrowIfNull(request);
        _ = KagemushaCodec.EncodeTopUpRequest(request);
        Request = request;
    }

    /// <summary>The complete proof-bearing deterministic top-up intent.</summary>
    public KagemushaTopUpRequestV1 Request { get; }

    internal override string WireId => "iroha.kagemusha.v1.top_up";

    internal override string TypeName =>
        "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1";

    internal override byte[] EncodePayload(TransactionEncodingContext context)
    {
        _ = context;
        return KagemushaCodec.EncodeTopUpInstructionPayload(Request);
    }

    internal override byte[] EncodeFramedPayload(TransactionEncodingContext context)
    {
        _ = context;
        return KagemushaCodec.EncodeTopUpInstructionFrame(Request);
    }
}
