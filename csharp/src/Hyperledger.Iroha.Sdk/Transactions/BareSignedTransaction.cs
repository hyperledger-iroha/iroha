using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// A strictly decoded single-signature transaction and its canonical headerless Norito bytes in
/// one explicit layout.
/// </summary>
/// <remarks>
/// <para>
/// The layout is named by the caller, or declared by the header of a frame
/// (<see cref="DecodeFrame"/>); it is never inferred from payload bytes. A transaction signed in
/// one layout verifies only against that layout's payload bytes, so <see cref="SigningHash"/>
/// differs between <see cref="NoritoWireLayout.Compact"/> and <see cref="NoritoWireLayout.Dense"/>.
/// </para>
/// <para>
/// Decoding checks structure and canonical encoding only. It neither verifies the signature nor
/// admits the account, name and domain identities the transaction carries. Instruction payloads
/// are kept uninterpreted, so a decoded transaction cannot be transcoded into the other layout.
/// </para>
/// <para>
/// No ledger selects the dense layout yet and no transaction wire version names it, so a dense
/// value has no versioned submission bytes and no entrypoint hash. Submit compact transactions
/// through <see cref="SignedTransactionEnvelope"/>.
/// </para>
/// </remarks>
public sealed class BareSignedTransaction
{
    internal const string SignedTransactionSchemaName =
        "iroha_data_model::transaction::signed::model::SignedTransaction";

    private readonly byte[] signedTransactionBytes;
    private readonly byte[] payloadBytes;
    private readonly byte[] signingHash;

    private BareSignedTransaction(WireSignedTransaction wire, byte[] signedTransactionBytes)
    {
        Wire = wire;
        this.signedTransactionBytes = signedTransactionBytes;
        payloadBytes = TransactionWireCodec.EncodePayload(wire.Payload);
        signingHash = IrohaHash.Hash(payloadBytes);
    }

    /// <summary>The layout of every byte this value exposes.</summary>
    public NoritoWireLayout Layout => Wire.Payload.Layout;

    /// <summary>The bare signed transaction: signature, payload and absent multisig bundle.</summary>
    public byte[] SignedTransactionBytes => signedTransactionBytes.ToArray();

    /// <summary>The bare transaction payload the signature is bound to.</summary>
    public byte[] PayloadBytes => payloadBytes.ToArray();

    /// <summary>
    /// The Iroha hash of <see cref="PayloadBytes"/>: the message the transaction signature covers
    /// in this layout.
    /// </summary>
    public byte[] SigningHash => signingHash.ToArray();

    /// <summary>The raw signature bytes carried by the transaction.</summary>
    public byte[] Signature => Wire.Signature.ToArray();

    /// <summary>The decoded wire-level values.</summary>
    internal WireSignedTransaction Wire { get; }

    /// <summary>
    /// Strictly decodes a bare signed transaction in <paramref name="layout"/>. Truncated input,
    /// trailing bytes, bytes of the other layout and non-canonical encodings are refused.
    /// </summary>
    public static BareSignedTransaction Decode(
        ReadOnlySpan<byte> signedTransactionBytes,
        NoritoWireLayout layout)
    {
        var wire = TransactionWireCodec.DecodeSignedTransaction(
            signedTransactionBytes,
            layout,
            nameof(signedTransactionBytes));
        return new BareSignedTransaction(wire, signedTransactionBytes.ToArray());
    }

    /// <summary>
    /// Strictly decodes a framed signed transaction. The frame's header minor version and flag
    /// byte select the layout: minor <c>0</c> with <c>COMPACT_LEN</c> is the compact layout and
    /// minor <c>1</c> is the dense layout. Fixed-width (flags <c>0x00</c>) frames are refused.
    /// </summary>
    public static BareSignedTransaction DecodeFrame(ReadOnlySpan<byte> frame)
    {
        var layout = NoritoWireLayoutRules.RequireTyped(NoritoCodec.ReadLayout(frame), nameof(frame));
        return Decode(NoritoCodec.DecodeWithLayout(SignedTransactionSchemaName, frame, layout), layout);
    }

    /// <summary>
    /// Frames <see cref="SignedTransactionBytes"/> with a header that declares
    /// <see cref="Layout"/>.
    /// </summary>
    public byte[] EncodeFrame() =>
        NoritoCodec.EncodeWithLayout(SignedTransactionSchemaName, signedTransactionBytes, Layout);
}
