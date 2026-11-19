using System.Numerics;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

// Wire-level values of a signed transaction, as TransactionWireCodec reads and writes them in
// one explicit Norito layout. Identities are raw bytes and text: nothing here has passed the
// native account, name or domain validators, so a decoded value is never an admitted identity.

/// <summary>A signed single-signature transaction in the layout of its payload.</summary>
internal sealed record WireSignedTransaction(byte[] Signature, WireTransactionPayload Payload);

/// <summary>
/// The signature-bound transaction payload. <see cref="Layout"/> belongs to the value because
/// instruction payloads are carried uninterpreted and cannot be transcoded.
/// </summary>
internal sealed record WireTransactionPayload(
    NoritoWireLayout Layout,
    byte[]? NetworkId,
    WireAccountController Authority,
    ulong CreationTimeMilliseconds,
    WireExecutable Executable,
    ulong? TimeToLiveMilliseconds,
    uint? Nonce,
    WireFeePayment FeePayment,
    IReadOnlyList<WireMetadataEntry> Metadata)
{
    /// <summary>True for the genesis-only transaction domain, which carries no network id.</summary>
    internal bool IsGenesisDomain => NetworkId is null;
}

/// <summary>An account controller: one tagged public key or a multisig policy.</summary>
internal abstract record WireAccountController;

/// <summary>A single-key controller; the key is the algorithm tag byte followed by the key bytes.</summary>
internal sealed record WireSingleKeyController(byte[] TaggedPublicKey) : WireAccountController;

/// <summary>A multisig controller policy.</summary>
internal sealed record WireMultisigController(
    byte Version,
    ushort Threshold,
    IReadOnlyList<WireMultisigMember> Members) : WireAccountController;

/// <summary>One multisig member: its tagged public key and voting weight.</summary>
internal sealed record WireMultisigMember(byte[] TaggedPublicKey, ushort Weight);

/// <summary>The executable of a transaction.</summary>
internal abstract record WireExecutable;

/// <summary>An ordered list of instruction envelopes (discriminant 0).</summary>
internal sealed record WireInstructionsExecutable(IReadOnlyList<WireInstruction> Instructions) : WireExecutable;

/// <summary>One deployed-contract call (discriminant 1).</summary>
internal sealed record WireContractCallExecutable(WireContractInvocation Invocation) : WireExecutable;

/// <summary>Raw IVM bytecode (discriminant 2).</summary>
internal sealed record WireIvmExecutable(byte[] Bytecode) : WireExecutable;

/// <summary>An ordered atomic mix of instructions and contract calls (discriminant 4).</summary>
internal sealed record WireBatchExecutable(IReadOnlyList<WireBatchItem> Items) : WireExecutable;

/// <summary>
/// An instruction envelope: the wire id and the payload bytes it names. In the compact layout the
/// payload is the instruction's complete Norito frame; in the dense layout it is the bare dense
/// payload with no nested header, which the decoder admits only as the canonical payload of the
/// instruction the native bridge registers under the wire id (an unregistered wire id stays
/// opaque).
/// </summary>
internal sealed record WireInstruction(string WireId, byte[] Payload);

/// <summary>One item of a batch executable.</summary>
internal abstract record WireBatchItem;

/// <summary>A batch item holding an instruction envelope (discriminant 0).</summary>
internal sealed record WireBatchInstruction(WireInstruction Instruction) : WireBatchItem;

/// <summary>A batch item holding a contract call (discriminant 1).</summary>
internal sealed record WireBatchContractCall(WireContractInvocation Invocation) : WireBatchItem;

/// <summary>A signature-bound invocation of one deployed contract revision.</summary>
internal sealed record WireContractInvocation(
    string ContractAddress,
    byte[] ExpectedCodeHash,
    string Entrypoint,
    byte[]? Arguments);

/// <summary>The fee payment intent: who pays, the charge limits and the gas bound.</summary>
internal abstract record WireFeePayment(IReadOnlyList<WireFeeChargeLimit> ChargeLimits, ulong? GasLimit);

/// <summary>The transaction authority pays (discriminant 0).</summary>
internal sealed record WireAuthorityFeePayment(IReadOnlyList<WireFeeChargeLimit> ChargeLimits, ulong? GasLimit)
    : WireFeePayment(ChargeLimits, GasLimit);

/// <summary>A sponsor program pays (discriminant 1).</summary>
internal sealed record WireSponsorFeePayment(
    WireAccountController Sponsor,
    string ProgramName,
    ulong ProgramRevision,
    IReadOnlyList<WireFeeChargeLimit> ChargeLimits,
    ulong? GasLimit)
    : WireFeePayment(ChargeLimits, GasLimit);

/// <summary>One fee charge limit; <see cref="Kind"/> is the charge-kind discriminant.</summary>
internal sealed record WireFeeChargeLimit(uint Kind, byte[] AssetDefinitionId, WireNumeric MaxAmount);

/// <summary>A numeric value exactly as carried: a signed mantissa and a base-10 scale.</summary>
internal sealed record WireNumeric(BigInteger Mantissa, uint Scale);

/// <summary>One metadata entry: its key and the JSON text of its value.</summary>
internal sealed record WireMetadataEntry(string Key, string JsonText);
