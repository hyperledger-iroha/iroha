using System.Buffers.Binary;
using System.Numerics;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Transactions;

/// <summary>
/// The reader and writer of the signed-transaction envelope in one explicit Norito layout.
/// </summary>
/// <remarks>
/// <para>
/// <see cref="TransactionEncodingContext"/> and <see cref="TransactionBuilder"/> convert their
/// validated inputs to the wire model and encode through this class, so the builder and the
/// decoder share one definition of every envelope shape in both the compact and the dense layout
/// (<c>norito.md</c>, "Dense layout").
/// </para>
/// <para>
/// Decoding is strict: each value has exactly one encoding, so a decoded transaction must
/// re-encode to the input bytes. Instruction payloads stay uninterpreted and keep the layout they
/// were read in. Multisig signature bundles, signed attachments and <c>IvmProved</c> executables
/// are not modeled and are refused.
/// </para>
/// </remarks>
internal static class TransactionWireCodec
{
    internal const int MaximumTransactionBytes = 10 * 1024 * 1024;
    internal const int MaximumInstructionWireIdBytes = 192;
    internal const int NetworkIdLength = 32;
    internal const int HashLength = 32;
    internal const int AssetDefinitionIdLength = 16;

    private const uint DomainNetworkTag = 0;
    private const uint DomainGenesisTag = 1;
    private const uint ControllerSingleTag = 0;
    private const uint ControllerMultisigTag = 1;
    private const uint ExecutableInstructionsTag = 0;
    private const uint ExecutableContractCallTag = 1;
    private const uint ExecutableIvmTag = 2;
    private const uint ExecutableIvmProvedTag = 3;
    private const uint ExecutableBatchTag = 4;
    private const uint BatchInstructionTag = 0;
    private const uint BatchContractCallTag = 1;
    private const uint FeeAuthorityTag = 0;
    private const uint FeeSponsorTag = 1;
    private const uint MaximumFeeChargeKindTag = 1;

    private static readonly UTF8Encoding StrictUtf8 = new(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true);

    // ---- Encoding ----

    internal static byte[] EncodeSignedTransaction(WireSignedTransaction transaction)
    {
        ArgumentNullException.ThrowIfNull(transaction);
        return EncodeSignedTransaction(
            transaction.Payload.Layout,
            transaction.Signature,
            EncodePayload(transaction.Payload));
    }

    /// <summary>
    /// The single-signature signed transaction: the signature, the already encoded payload and an
    /// absent multisig bundle.
    /// </summary>
    internal static byte[] EncodeSignedTransaction(
        NoritoWireLayout layout,
        ReadOnlySpan<byte> signature,
        ReadOnlySpan<byte> payload)
    {
        var transactionSignature = new CanonicalNoritoWriter(layout);
        transactionSignature.WriteField(EncodeConstVec(layout, signature));

        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(transactionSignature.ToArray());
        writer.WriteField(payload);
        writer.WriteField(EncodeNone(layout));
        return writer.ToArray();
    }

    internal static byte[] EncodePayload(WireTransactionPayload payload)
    {
        ArgumentNullException.ThrowIfNull(payload);
        var layout = payload.Layout;
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(EncodeDomain(layout, payload.NetworkId, genesis: payload.IsGenesisDomain));
        writer.WriteField(EncodeAccountController(layout, payload.Authority));
        writer.WriteField(EncodeUInt64(layout, payload.CreationTimeMilliseconds));
        writer.WriteField(EncodeExecutable(layout, payload.Executable));
        writer.WriteField(EncodeOptionUInt64(layout, payload.TimeToLiveMilliseconds));
        writer.WriteField(EncodeOptionUInt32(layout, payload.Nonce));
        writer.WriteField(EncodeFeePayment(layout, payload.FeePayment));
        writer.WriteField(EncodeMetadata(layout, payload.Metadata));
        // attachments: None.
        writer.WriteField(EncodeNone(layout));
        return writer.ToArray();
    }

    /// <summary>
    /// The transaction domain: the 32-byte network id, or the genesis-only marker when
    /// <paramref name="genesis"/> is set.
    /// </summary>
    internal static byte[] EncodeDomain(NoritoWireLayout layout, ReadOnlySpan<byte> networkId, bool genesis = false)
    {
        var writer = new CanonicalNoritoWriter(layout);
        if (genesis)
        {
            writer.WriteEnumTag(DomainGenesisTag);
            return writer.ToArray();
        }

        if (networkId.Length != NetworkIdLength)
        {
            throw new ArgumentException($"Network id must be {NetworkIdLength} bytes.", nameof(networkId));
        }

        writer.WriteEnumTag(DomainNetworkTag);
        writer.WriteField(networkId);
        return writer.ToArray();
    }

    internal static byte[] EncodeAccountController(NoritoWireLayout layout, WireAccountController controller)
    {
        ArgumentNullException.ThrowIfNull(controller);
        var writer = new CanonicalNoritoWriter(layout);
        switch (controller)
        {
            case WireSingleKeyController single:
                writer.WriteEnumTag(ControllerSingleTag);
                writer.WriteField(EncodeConstVec(layout, single.TaggedPublicKey));
                break;
            case WireMultisigController multisig:
                var members = new CanonicalNoritoWriter(layout);
                members.WriteSequenceLength(checked((ulong)multisig.Members.Count));
                foreach (var member in multisig.Members)
                {
                    var memberWriter = new CanonicalNoritoWriter(layout);
                    memberWriter.WriteField(EncodeConstVec(layout, member.TaggedPublicKey));
                    memberWriter.WriteField(EncodeUInt16(layout, member.Weight));
                    members.WriteField(memberWriter.ToArray());
                }

                var policy = new CanonicalNoritoWriter(layout);
                policy.WriteField([multisig.Version]);
                policy.WriteField(EncodeUInt16(layout, multisig.Threshold));
                policy.WriteField(members.ToArray());
                writer.WriteEnumTag(ControllerMultisigTag);
                writer.WriteField(policy.ToArray());
                break;
            default:
                throw new ArgumentException("Unknown account controller.", nameof(controller));
        }

        return writer.ToArray();
    }

    /// <summary>
    /// An element sequence of <c>u8</c> (<c>ConstVec&lt;u8&gt;</c>): the count, then one
    /// <c>[len = 1][byte]</c> pair per byte in the compact layout or the raw bytes in the dense
    /// layout.
    /// </summary>
    internal static byte[] EncodeConstVec(NoritoWireLayout layout, ReadOnlySpan<byte> bytes)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength((ulong)bytes.Length);
        writer.WriteByteElements(bytes);
        return writer.ToArray();
    }

    /// <summary>A byte vector (<c>Vec&lt;u8&gt;</c>): the count, then the raw bytes.</summary>
    internal static byte[] EncodeBytesVec(NoritoWireLayout layout, ReadOnlySpan<byte> bytes)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength((ulong)bytes.Length);
        writer.WriteBytes(bytes);
        return writer.ToArray();
    }

    internal static byte[] EncodeString(NoritoWireLayout layout, string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(Encoding.UTF8.GetBytes(value));
        return writer.ToArray();
    }

    internal static byte[] EncodeUInt16(NoritoWireLayout layout, ushort value)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteUInt16LittleEndian(value);
        return writer.ToArray();
    }

    internal static byte[] EncodeUInt32(NoritoWireLayout layout, uint value)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteUInt32LittleEndian(value);
        return writer.ToArray();
    }

    internal static byte[] EncodeUInt64(NoritoWireLayout layout, ulong value)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteUInt64LittleEndian(value);
        return writer.ToArray();
    }

    /// <summary>An absent <c>Option</c>: the one-byte tag <c>0</c> in every layout.</summary>
    internal static byte[] EncodeNone(NoritoWireLayout layout)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteByte(0);
        return writer.ToArray();
    }

    /// <summary>A present <c>Option</c>: the one-byte tag <c>1</c>, then the length-prefixed value.</summary>
    internal static byte[] EncodeSome(NoritoWireLayout layout, ReadOnlySpan<byte> value)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteByte(1);
        writer.WriteField(value);
        return writer.ToArray();
    }

    internal static byte[] EncodeOptionUInt64(NoritoWireLayout layout, ulong? value) =>
        value is { } present ? EncodeSome(layout, EncodeUInt64(layout, present)) : EncodeNone(layout);

    internal static byte[] EncodeOptionUInt32(NoritoWireLayout layout, uint? value) =>
        value is { } present ? EncodeSome(layout, EncodeUInt32(layout, present)) : EncodeNone(layout);

    internal static byte[] EncodeExecutable(NoritoWireLayout layout, WireExecutable executable)
    {
        ArgumentNullException.ThrowIfNull(executable);
        var writer = new CanonicalNoritoWriter(layout);
        switch (executable)
        {
            case WireInstructionsExecutable instructions:
                var sequence = new CanonicalNoritoWriter(layout);
                sequence.WriteSequenceLength(checked((ulong)instructions.Instructions.Count));
                foreach (var instruction in instructions.Instructions)
                {
                    sequence.WriteField(EncodeInstruction(layout, instruction));
                }

                writer.WriteEnumTag(ExecutableInstructionsTag);
                writer.WriteField(sequence.ToArray());
                break;
            case WireContractCallExecutable call:
                writer.WriteEnumTag(ExecutableContractCallTag);
                writer.WriteField(EncodeContractInvocation(layout, call.Invocation));
                break;
            case WireIvmExecutable ivm:
                var bytecode = new CanonicalNoritoWriter(layout);
                bytecode.WriteField(EncodeBytesVec(layout, ivm.Bytecode));
                writer.WriteEnumTag(ExecutableIvmTag);
                writer.WriteField(bytecode.ToArray());
                break;
            case WireBatchExecutable batch:
                if (batch.Items.Count == 0)
                {
                    throw new ArgumentException("Executable batches must contain at least one item.", nameof(executable));
                }

                var items = new CanonicalNoritoWriter(layout);
                items.WriteSequenceLength(checked((ulong)batch.Items.Count));
                foreach (var entry in batch.Items)
                {
                    var item = new CanonicalNoritoWriter(layout);
                    switch (entry)
                    {
                        case WireBatchInstruction instruction:
                            item.WriteEnumTag(BatchInstructionTag);
                            item.WriteField(EncodeInstruction(layout, instruction.Instruction));
                            break;
                        case WireBatchContractCall contractCall:
                            item.WriteEnumTag(BatchContractCallTag);
                            item.WriteField(EncodeContractInvocation(layout, contractCall.Invocation));
                            break;
                        default:
                            throw new ArgumentException("Unknown executable batch entry.", nameof(executable));
                    }

                    items.WriteField(item.ToArray());
                }

                writer.WriteEnumTag(ExecutableBatchTag);
                writer.WriteField(items.ToArray());
                break;
            default:
                throw new ArgumentException("Unknown executable.", nameof(executable));
        }

        return writer.ToArray();
    }

    /// <summary>
    /// The instruction envelope: the wire id and a byte vector holding the payload. The dense
    /// envelope carries the bare dense payload, so no nested Norito header appears in it.
    /// </summary>
    internal static byte[] EncodeInstruction(NoritoWireLayout layout, WireInstruction instruction)
    {
        ArgumentNullException.ThrowIfNull(instruction);
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(EncodeString(layout, instruction.WireId));
        writer.WriteField(EncodeBytesVec(layout, instruction.Payload));
        return writer.ToArray();
    }

    internal static byte[] EncodeContractInvocation(NoritoWireLayout layout, WireContractInvocation invocation)
    {
        ArgumentNullException.ThrowIfNull(invocation);
        if (invocation.ExpectedCodeHash.Length != HashLength)
        {
            throw new ArgumentException($"Expected code hash must be {HashLength} bytes.", nameof(invocation));
        }

        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(EncodeString(layout, invocation.ContractAddress));
        writer.WriteField(invocation.ExpectedCodeHash);
        writer.WriteField(EncodeString(layout, invocation.Entrypoint));
        writer.WriteField(invocation.Arguments is { } arguments
            ? EncodeSome(layout, EncodeBytesVec(layout, arguments))
            : EncodeNone(layout));
        return writer.ToArray();
    }

    internal static byte[] EncodeFeePayment(NoritoWireLayout layout, WireFeePayment feePayment)
    {
        ArgumentNullException.ThrowIfNull(feePayment);
        var body = new CanonicalNoritoWriter(layout);
        var writer = new CanonicalNoritoWriter(layout);
        switch (feePayment)
        {
            case WireAuthorityFeePayment:
                writer.WriteEnumTag(FeeAuthorityTag);
                break;
            case WireSponsorFeePayment sponsor:
                var programId = new CanonicalNoritoWriter(layout);
                programId.WriteField(EncodeAccountController(layout, sponsor.Sponsor));
                programId.WriteField(EncodeString(layout, sponsor.ProgramName));
                body.WriteField(programId.ToArray());
                body.WriteField(EncodeUInt64(layout, sponsor.ProgramRevision));
                writer.WriteEnumTag(FeeSponsorTag);
                break;
            default:
                throw new ArgumentException("Unknown fee payment intent.", nameof(feePayment));
        }

        var limits = new CanonicalNoritoWriter(layout);
        limits.WriteSequenceLength(checked((ulong)feePayment.ChargeLimits.Count));
        foreach (var limit in feePayment.ChargeLimits)
        {
            limits.WriteField(EncodeFeeChargeLimit(layout, limit));
        }

        body.WriteField(limits.ToArray());
        body.WriteField(EncodeOptionUInt64(layout, feePayment.GasLimit));
        writer.WriteField(body.ToArray());
        return writer.ToArray();
    }

    /// <summary>
    /// A numeric value: the mantissa as a <c>u32</c> byte count and its minimal little-endian
    /// two's-complement bytes (none for zero), then the <c>u32</c> scale. The <c>u32</c> count is
    /// fixed by <c>norito.md</c> in every layout.
    /// </summary>
    internal static byte[] EncodeNumeric(NoritoWireLayout layout, WireNumeric value)
    {
        ArgumentNullException.ThrowIfNull(value);
        var mantissaBytes = value.Mantissa.IsZero
            ? Array.Empty<byte>()
            : value.Mantissa.ToByteArray(isUnsigned: false, isBigEndian: false);

        var mantissa = new CanonicalNoritoWriter(layout);
        mantissa.WriteUInt32LittleEndian((uint)mantissaBytes.Length);
        mantissa.WriteBytes(mantissaBytes);

        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(mantissa.ToArray());
        writer.WriteField(EncodeUInt32(layout, value.Scale));
        return writer.ToArray();
    }

    /// <summary>The metadata map as a sequence of key and JSON-text entries, in the given order.</summary>
    internal static byte[] EncodeMetadata(NoritoWireLayout layout, IReadOnlyList<WireMetadataEntry> entries)
    {
        ArgumentNullException.ThrowIfNull(entries);
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteSequenceLength(checked((ulong)entries.Count));
        foreach (var entry in entries)
        {
            var entryWriter = new CanonicalNoritoWriter(layout);
            entryWriter.WriteField(EncodeString(layout, entry.Key));
            entryWriter.WriteField(EncodeJsonText(layout, entry.JsonText));
            writer.WriteField(entryWriter.ToArray());
        }

        return writer.ToArray();
    }

    /// <summary>A JSON value: its text as one length-prefixed string field.</summary>
    internal static byte[] EncodeJsonText(NoritoWireLayout layout, string jsonText)
    {
        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(EncodeString(layout, jsonText));
        return writer.ToArray();
    }

    private static byte[] EncodeFeeChargeLimit(NoritoWireLayout layout, WireFeeChargeLimit limit)
    {
        if (limit.Kind > MaximumFeeChargeKindTag)
        {
            throw new ArgumentOutOfRangeException(nameof(limit), "Unknown fee charge kind.");
        }

        if (limit.AssetDefinitionId.Length != AssetDefinitionIdLength)
        {
            throw new ArgumentException(
                $"Asset definition id must be {AssetDefinitionIdLength} bytes.",
                nameof(limit));
        }

        var kind = new CanonicalNoritoWriter(layout);
        kind.WriteEnumTag(limit.Kind);

        var assetDefinition = new CanonicalNoritoWriter(layout);
        assetDefinition.WriteByteElements(limit.AssetDefinitionId);

        var writer = new CanonicalNoritoWriter(layout);
        writer.WriteField(kind.ToArray());
        writer.WriteField(assetDefinition.ToArray());
        writer.WriteField(EncodeNumeric(layout, limit.MaxAmount));
        return writer.ToArray();
    }

    // ---- Decoding ----

    /// <summary>
    /// Strictly decodes one bare signed transaction in <paramref name="layout"/>. Truncated input,
    /// trailing bytes, another layout's bytes and every non-canonical encoding are refused.
    /// </summary>
    /// <remarks>
    /// Input is bounded by <see cref="MaximumTransactionBytes"/>. A declared count is checked
    /// against the remaining input and elements are collected as they parse, so a hostile count
    /// reserves nothing.
    /// </remarks>
    internal static WireSignedTransaction DecodeSignedTransaction(
        ReadOnlySpan<byte> bytes,
        NoritoWireLayout layout,
        string parameterName)
    {
        RequireBoundedInput(bytes, layout, "Signed transaction", parameterName);

        var reader = new CanonicalNoritoReader(bytes, "SignedTransaction", parameterName, layout);
        var signatureField = reader.ReadField("signature");
        var payloadField = reader.ReadField("payload");
        var multisigField = reader.ReadField("multisig_signatures");
        reader.RequireEnd();

        var wrapper = new CanonicalNoritoReader(signatureField, "TransactionSignature", parameterName, layout);
        var signatureOf = wrapper.ReadField("signature");
        wrapper.RequireEnd();
        var signature = DecodeConstVec(signatureOf, "SignatureOf<TransactionPayload>", parameterName, layout);

        RequireAbsent(
            multisigField,
            "SignedTransaction.multisig_signatures",
            "Multisig signature bundles are not modeled by the managed transaction decoder.",
            parameterName,
            layout);

        var decoded = new WireSignedTransaction(signature, ReadPayload(payloadField, layout, parameterName));
        RequireCanonical(EncodeSignedTransaction(decoded), bytes, "Signed transaction", parameterName);
        return decoded;
    }

    /// <summary>
    /// Strictly decodes one bare transaction payload in <paramref name="layout"/>.
    /// </summary>
    internal static WireTransactionPayload DecodePayload(
        ReadOnlySpan<byte> bytes,
        NoritoWireLayout layout,
        string parameterName)
    {
        RequireBoundedInput(bytes, layout, "Transaction payload", parameterName);

        var decoded = ReadPayload(bytes, layout, parameterName);
        RequireCanonical(EncodePayload(decoded), bytes, "Transaction payload", parameterName);
        return decoded;
    }

    /// <summary>
    /// Whether <paramref name="wireId"/> is a well-formed instruction wire id: 1 to 192 bytes of
    /// ASCII letters, digits and <c>. _ : - @ /</c>.
    /// </summary>
    internal static bool IsWellFormedWireId(string wireId)
    {
        if (string.IsNullOrEmpty(wireId) || wireId.Length > MaximumInstructionWireIdBytes)
        {
            return false;
        }

        foreach (var character in wireId)
        {
            if (!char.IsAsciiLetterOrDigit(character) && character is not ('.' or '_' or ':' or '-' or '@' or '/'))
            {
                return false;
            }
        }

        return true;
    }

    private static WireTransactionPayload ReadPayload(
        ReadOnlySpan<byte> bytes,
        NoritoWireLayout layout,
        string parameterName)
    {
        var reader = new CanonicalNoritoReader(bytes, "TransactionPayload", parameterName, layout);
        var domain = reader.ReadField("domain");
        var authority = reader.ReadField("authority");
        var creationTime = reader.ReadField("creation_time_ms");
        var executable = reader.ReadField("instructions");
        var timeToLive = reader.ReadField("time_to_live_ms");
        var nonce = reader.ReadField("nonce");
        var feePayment = reader.ReadField("fee_payment");
        var metadata = reader.ReadField("metadata");
        var attachments = reader.ReadField("attachments");
        reader.RequireEnd();

        RequireAbsent(
            attachments,
            "TransactionPayload.attachments",
            "Signed attachments are not modeled by the managed transaction decoder.",
            parameterName,
            layout);

        var timeToLiveMilliseconds = DecodeOptionUInt64(
            timeToLive, "TransactionPayload.time_to_live_ms", parameterName, layout);
        if (timeToLiveMilliseconds == 0)
        {
            throw new ArgumentException("TransactionPayload.time_to_live_ms must be non-zero.", parameterName);
        }

        var nonceValue = DecodeOptionUInt32(nonce, "TransactionPayload.nonce", parameterName, layout);
        if (nonceValue == 0)
        {
            throw new ArgumentException("TransactionPayload.nonce must be non-zero.", parameterName);
        }

        return new WireTransactionPayload(
            layout,
            DecodeDomain(domain, parameterName, layout),
            DecodeAccountController(authority, "TransactionPayload.authority", parameterName, layout),
            DecodeUInt64(creationTime, "TransactionPayload.creation_time_ms", parameterName),
            DecodeExecutable(executable, parameterName, layout),
            timeToLiveMilliseconds,
            nonceValue,
            DecodeFeePayment(feePayment, parameterName, layout),
            DecodeMetadata(metadata, parameterName, layout));
    }

    private static byte[]? DecodeDomain(ReadOnlySpan<byte> bytes, string parameterName, NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "TransactionDomain", parameterName, layout);
        var tag = reader.ReadEnumTag("discriminant");
        switch (tag)
        {
            case DomainNetworkTag:
                var networkId = reader.ReadField("network_id");
                reader.RequireEnd();
                if (networkId.Length != NetworkIdLength)
                {
                    throw new ArgumentException(
                        $"TransactionDomain.network_id must be {NetworkIdLength} bytes.",
                        parameterName);
                }

                return networkId.ToArray();
            case DomainGenesisTag:
                reader.RequireEnd();
                return null;
            default:
                throw new ArgumentException($"TransactionDomain has unknown discriminant {tag}.", parameterName);
        }
    }

    private static WireAccountController DecodeAccountController(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var tag = reader.ReadEnumTag("discriminant");
        var body = reader.ReadField("controller");
        reader.RequireEnd();
        switch (tag)
        {
            case ControllerSingleTag:
                return new WireSingleKeyController(
                    DecodeConstVec(body, $"{context}.public_key", parameterName, layout));
            case ControllerMultisigTag:
                var policy = new CanonicalNoritoReader(body, $"{context}.policy", parameterName, layout);
                var version = policy.ReadField("version");
                var threshold = policy.ReadField("threshold");
                var membersField = policy.ReadField("members");
                policy.RequireEnd();
                if (version.Length != 1)
                {
                    throw new ArgumentException($"{context}.policy.version must be one byte.", parameterName);
                }

                var members = new CanonicalNoritoReader(membersField, $"{context}.policy.members", parameterName, layout);
                var count = members.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
                var decoded = new List<WireMultisigMember>();
                for (var index = 0; index < count; index++)
                {
                    var member = new CanonicalNoritoReader(
                        members.ReadField($"[{index}]"),
                        $"{context}.policy.members[{index}]",
                        parameterName,
                        layout);
                    var publicKey = member.ReadField("public_key");
                    var weight = member.ReadField("weight");
                    member.RequireEnd();
                    decoded.Add(new WireMultisigMember(
                        DecodeConstVec(publicKey, $"{context}.policy.members[{index}].public_key", parameterName, layout),
                        DecodeUInt16(weight, $"{context}.policy.members[{index}].weight", parameterName)));
                }

                members.RequireEnd();
                return new WireMultisigController(
                    version[0],
                    DecodeUInt16(threshold, $"{context}.policy.threshold", parameterName),
                    decoded.AsReadOnly());
            default:
                throw new ArgumentException($"{context} has unknown discriminant {tag}.", parameterName);
        }
    }

    private static WireExecutable DecodeExecutable(ReadOnlySpan<byte> bytes, string parameterName, NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "Executable", parameterName, layout);
        var tag = reader.ReadEnumTag("discriminant");
        var body = reader.ReadField("payload");
        reader.RequireEnd();
        switch (tag)
        {
            case ExecutableInstructionsTag:
                var instructions = new CanonicalNoritoReader(body, "Executable.Instructions", parameterName, layout);
                var instructionCount = instructions.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
                var decodedInstructions = new List<WireInstruction>();
                for (var index = 0; index < instructionCount; index++)
                {
                    decodedInstructions.Add(DecodeInstruction(
                        instructions.ReadField($"[{index}]"),
                        $"Executable.Instructions[{index}]",
                        parameterName,
                        layout));
                }

                instructions.RequireEnd();
                return new WireInstructionsExecutable(decodedInstructions.AsReadOnly());
            case ExecutableContractCallTag:
                return new WireContractCallExecutable(
                    DecodeContractInvocation(body, "Executable.ContractCall", parameterName, layout));
            case ExecutableIvmTag:
                var bytecode = new CanonicalNoritoReader(body, "Executable.Ivm", parameterName, layout);
                var bytecodeField = bytecode.ReadField("bytecode");
                bytecode.RequireEnd();
                return new WireIvmExecutable(
                    DecodeBytesVec(bytecodeField, "Executable.Ivm.bytecode", parameterName, layout));
            case ExecutableIvmProvedTag:
                // TODO: model IvmProved (bytecode, overlay and the two commitments) once a shared
                // fixture pins its dense bytes.
                throw new ArgumentException(
                    "IvmProved executables are not modeled by the managed transaction decoder.",
                    parameterName);
            case ExecutableBatchTag:
                var items = new CanonicalNoritoReader(body, "Executable.Batch", parameterName, layout);
                var itemCount = items.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
                if (itemCount == 0)
                {
                    throw new ArgumentException("Executable.Batch must contain at least one item.", parameterName);
                }

                var decodedItems = new List<WireBatchItem>();
                for (var index = 0; index < itemCount; index++)
                {
                    var itemContext = $"Executable.Batch[{index}]";
                    var item = new CanonicalNoritoReader(items.ReadField($"[{index}]"), itemContext, parameterName, layout);
                    var itemTag = item.ReadEnumTag("discriminant");
                    var itemBody = item.ReadField("payload");
                    item.RequireEnd();
                    decodedItems.Add(itemTag switch
                    {
                        BatchInstructionTag => new WireBatchInstruction(
                            DecodeInstruction(itemBody, $"{itemContext}.Instruction", parameterName, layout)),
                        BatchContractCallTag => new WireBatchContractCall(
                            DecodeContractInvocation(itemBody, $"{itemContext}.ContractCall", parameterName, layout)),
                        _ => throw new ArgumentException(
                            $"{itemContext} has unknown discriminant {itemTag}.",
                            parameterName),
                    });
                }

                items.RequireEnd();
                return new WireBatchExecutable(decodedItems.AsReadOnly());
            default:
                throw new ArgumentException($"Executable has unknown discriminant {tag}.", parameterName);
        }
    }

    private static WireInstruction DecodeInstruction(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var wireIdField = reader.ReadField("wire_id");
        var payloadField = reader.ReadField("payload");
        reader.RequireEnd();

        var wireId = DecodeString(wireIdField, $"{context}.wire_id", parameterName, layout);
        if (!IsWellFormedWireId(wireId))
        {
            throw new ArgumentException($"{context}.wire_id is not a well-formed instruction wire id.", parameterName);
        }

        var payload = DecodeBytesVec(payloadField, $"{context}.payload", parameterName, layout);
        if (layout == NoritoWireLayout.Dense)
        {
            // A dense body has no self-describing header: a registered wire ID's payload must be
            // exactly that instruction's canonical dense payload, which the native bridge decodes
            // as the typed instruction. An unregistered wire ID stays an opaque envelope.
            NoritoInstructionLayoutNative.Transcode(
                wireId,
                payload,
                NoritoWireLayout.Dense,
                NoritoWireLayout.Dense,
                parameterName);
        }

        return new WireInstruction(wireId, payload);
    }

    private static WireContractInvocation DecodeContractInvocation(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var contractAddress = reader.ReadField("contract_address");
        var expectedCodeHash = reader.ReadField("expected_code_hash");
        var entrypoint = reader.ReadField("entrypoint");
        var argumentsField = reader.ReadField("arguments");
        reader.RequireEnd();
        if (expectedCodeHash.Length != HashLength)
        {
            throw new ArgumentException($"{context}.expected_code_hash must be {HashLength} bytes.", parameterName);
        }

        byte[]? arguments = null;
        var option = new CanonicalNoritoReader(argumentsField, $"{context}.arguments", parameterName, layout);
        if (ReadOptionTag(ref option, $"{context}.arguments", parameterName))
        {
            arguments = DecodeBytesVec(option.ReadField("value"), $"{context}.arguments", parameterName, layout);
        }

        option.RequireEnd();
        return new WireContractInvocation(
            DecodeString(contractAddress, $"{context}.contract_address", parameterName, layout),
            expectedCodeHash.ToArray(),
            DecodeString(entrypoint, $"{context}.entrypoint", parameterName, layout),
            arguments);
    }

    private static WireFeePayment DecodeFeePayment(ReadOnlySpan<byte> bytes, string parameterName, NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "FeePaymentIntent", parameterName, layout);
        var tag = reader.ReadEnumTag("discriminant");
        var bodyField = reader.ReadField("payload");
        reader.RequireEnd();

        var body = new CanonicalNoritoReader(bodyField, "FeePaymentIntent.payload", parameterName, layout);
        switch (tag)
        {
            case FeeAuthorityTag:
            {
                var limits = body.ReadField("charge_limits");
                var gasLimit = body.ReadField("gas_limit");
                body.RequireEnd();
                return new WireAuthorityFeePayment(
                    DecodeFeeChargeLimits(limits, parameterName, layout),
                    DecodeOptionUInt64(gasLimit, "FeePaymentIntent.gas_limit", parameterName, layout));
            }
            case FeeSponsorTag:
            {
                var programIdField = body.ReadField("program_id");
                var revision = body.ReadField("program_revision");
                var limits = body.ReadField("charge_limits");
                var gasLimit = body.ReadField("gas_limit");
                body.RequireEnd();

                var programId = new CanonicalNoritoReader(
                    programIdField, "FeePaymentIntent.program_id", parameterName, layout);
                var sponsor = programId.ReadField("sponsor");
                var name = programId.ReadField("name");
                programId.RequireEnd();
                return new WireSponsorFeePayment(
                    DecodeAccountController(sponsor, "FeePaymentIntent.program_id.sponsor", parameterName, layout),
                    DecodeString(name, "FeePaymentIntent.program_id.name", parameterName, layout),
                    DecodeUInt64(revision, "FeePaymentIntent.program_revision", parameterName),
                    DecodeFeeChargeLimits(limits, parameterName, layout),
                    DecodeOptionUInt64(gasLimit, "FeePaymentIntent.gas_limit", parameterName, layout));
            }
            default:
                throw new ArgumentException($"FeePaymentIntent has unknown discriminant {tag}.", parameterName);
        }
    }

    private static IReadOnlyList<WireFeeChargeLimit> DecodeFeeChargeLimits(
        ReadOnlySpan<byte> bytes,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "FeePaymentIntent.charge_limits", parameterName, layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
        var limits = new List<WireFeeChargeLimit>();
        for (var index = 0; index < count; index++)
        {
            var context = $"FeePaymentIntent.charge_limits[{index}]";
            var limit = new CanonicalNoritoReader(reader.ReadField($"[{index}]"), context, parameterName, layout);
            var kindField = limit.ReadField("kind");
            var assetDefinitionField = limit.ReadField("asset_definition_id");
            var maxAmount = limit.ReadField("max_amount");
            limit.RequireEnd();

            var kind = new CanonicalNoritoReader(kindField, $"{context}.kind", parameterName, layout);
            var kindTag = kind.ReadEnumTag("discriminant");
            kind.RequireEnd();
            if (kindTag > MaximumFeeChargeKindTag)
            {
                throw new ArgumentException($"{context}.kind has unknown discriminant {kindTag}.", parameterName);
            }

            var assetDefinition = new CanonicalNoritoReader(
                assetDefinitionField, $"{context}.asset_definition_id", parameterName, layout);
            var assetDefinitionId = assetDefinition.ReadByteElements(AssetDefinitionIdLength, "bytes");
            assetDefinition.RequireEnd();

            limits.Add(new WireFeeChargeLimit(
                kindTag,
                assetDefinitionId,
                DecodeNumeric(maxAmount, $"{context}.max_amount", parameterName, layout)));
        }

        reader.RequireEnd();
        return limits.AsReadOnly();
    }

    private static WireNumeric DecodeNumeric(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var mantissaField = reader.ReadField("mantissa");
        var scale = reader.ReadField("scale");
        reader.RequireEnd();
        if (mantissaField.Length < sizeof(uint)
            || BinaryPrimitives.ReadUInt32LittleEndian(mantissaField) != (uint)(mantissaField.Length - sizeof(uint)))
        {
            throw new ArgumentException($"{context}.mantissa length is invalid.", parameterName);
        }

        return new WireNumeric(
            new BigInteger(mantissaField[sizeof(uint)..], isUnsigned: false, isBigEndian: false),
            DecodeUInt32(scale, $"{context}.scale", parameterName));
    }

    private static IReadOnlyList<WireMetadataEntry> DecodeMetadata(
        ReadOnlySpan<byte> bytes,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, "Metadata", parameterName, layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
        var entries = new List<WireMetadataEntry>();
        byte[]? previousKey = null;
        for (var index = 0; index < count; index++)
        {
            var context = $"Metadata[{index}]";
            var entry = new CanonicalNoritoReader(reader.ReadField($"[{index}]"), context, parameterName, layout);
            var keyField = entry.ReadField("key");
            var valueField = entry.ReadField("value");
            entry.RequireEnd();

            var key = DecodeString(keyField, $"{context}.key", parameterName, layout);
            // The map is ordered by key: strictly ascending UTF-8 bytes, so no key repeats.
            var keyBytes = Encoding.UTF8.GetBytes(key);
            if (previousKey is not null && previousKey.AsSpan().SequenceCompareTo(keyBytes) >= 0)
            {
                throw new ArgumentException("Metadata keys must be unique and in ascending order.", parameterName);
            }

            previousKey = keyBytes;

            var json = new CanonicalNoritoReader(valueField, $"{context}.value", parameterName, layout);
            var jsonText = json.ReadField("json");
            json.RequireEnd();
            entries.Add(new WireMetadataEntry(
                key,
                DecodeString(jsonText, $"{context}.value", parameterName, layout)));
        }

        reader.RequireEnd();
        return entries.AsReadOnly();
    }

    /// <summary>
    /// Strictly decodes an element sequence of <c>u8</c> (<c>ConstVec&lt;u8&gt;</c>): the count,
    /// then one <c>[len = 1][byte]</c> pair per byte (compact) or the raw bytes (dense).
    /// </summary>
    internal static byte[] DecodeConstVec(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var count = reader.ReadBoundedSequenceLength(
            "count",
            minimumElementBytes: layout == NoritoWireLayout.Dense ? 1 : 2);
        var values = reader.ReadByteElements(count, "bytes");
        reader.RequireEnd();
        return values;
    }

    /// <summary>Strictly decodes a byte vector (<c>Vec&lt;u8&gt;</c>): the count, then the raw bytes.</summary>
    internal static byte[] DecodeBytesVec(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var count = reader.ReadBoundedSequenceLength("count", minimumElementBytes: 1);
        var values = reader.ReadExact(count, "bytes").ToArray();
        reader.RequireEnd();
        return values;
    }

    /// <summary>Strictly decodes a string: its length-prefixed, well-formed UTF-8 bytes.</summary>
    internal static string DecodeString(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        var text = reader.ReadField("utf8");
        reader.RequireEnd();
        try
        {
            return StrictUtf8.GetString(text);
        }
        catch (DecoderFallbackException error)
        {
            throw new ArgumentException($"{context} is not valid UTF-8.", parameterName, error);
        }
    }

    private static ushort DecodeUInt16(ReadOnlySpan<byte> bytes, string context, string parameterName) =>
        bytes.Length == sizeof(ushort)
            ? BinaryPrimitives.ReadUInt16LittleEndian(bytes)
            : throw new ArgumentException($"{context} must be {sizeof(ushort)} bytes.", parameterName);

    private static uint DecodeUInt32(ReadOnlySpan<byte> bytes, string context, string parameterName) =>
        bytes.Length == sizeof(uint)
            ? BinaryPrimitives.ReadUInt32LittleEndian(bytes)
            : throw new ArgumentException($"{context} must be {sizeof(uint)} bytes.", parameterName);

    private static ulong DecodeUInt64(ReadOnlySpan<byte> bytes, string context, string parameterName) =>
        bytes.Length == sizeof(ulong)
            ? BinaryPrimitives.ReadUInt64LittleEndian(bytes)
            : throw new ArgumentException($"{context} must be {sizeof(ulong)} bytes.", parameterName);

    private static ulong? DecodeOptionUInt64(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        ulong? value = null;
        if (ReadOptionTag(ref reader, context, parameterName))
        {
            value = DecodeUInt64(reader.ReadField("value"), context, parameterName);
        }

        reader.RequireEnd();
        return value;
    }

    private static uint? DecodeOptionUInt32(
        ReadOnlySpan<byte> bytes,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        uint? value = null;
        if (ReadOptionTag(ref reader, context, parameterName))
        {
            value = DecodeUInt32(reader.ReadField("value"), context, parameterName);
        }

        reader.RequireEnd();
        return value;
    }

    // An Option tag is one byte in every layout: 0 for None, 1 for Some.
    private static bool ReadOptionTag(ref CanonicalNoritoReader reader, string context, string parameterName) =>
        reader.ReadByte("option") switch
        {
            0 => false,
            1 => true,
            var tag => throw new ArgumentException($"{context} has invalid Option tag {tag}.", parameterName),
        };

    private static void RequireAbsent(
        ReadOnlySpan<byte> bytes,
        string context,
        string unmodeledMessage,
        string parameterName,
        NoritoWireLayout layout)
    {
        var reader = new CanonicalNoritoReader(bytes, context, parameterName, layout);
        if (ReadOptionTag(ref reader, context, parameterName))
        {
            // TODO: model this value once a shared fixture pins its bytes in both layouts.
            throw new ArgumentException(unmodeledMessage, parameterName);
        }

        reader.RequireEnd();
    }

    private static void RequireBoundedInput(
        ReadOnlySpan<byte> bytes,
        NoritoWireLayout layout,
        string subject,
        string parameterName)
    {
        _ = NoritoWireLayoutRules.RequireTyped(layout, nameof(layout));
        if (bytes.IsEmpty || bytes.Length > MaximumTransactionBytes)
        {
            throw new ArgumentException(
                $"{subject} must contain 1..{MaximumTransactionBytes} bytes.",
                parameterName);
        }
    }

    private static void RequireCanonical(
        ReadOnlySpan<byte> encoded,
        ReadOnlySpan<byte> input,
        string subject,
        string parameterName)
    {
        if (!encoded.SequenceEqual(input))
        {
            throw new ArgumentException($"{subject} is not canonically encoded.", parameterName);
        }
    }
}
