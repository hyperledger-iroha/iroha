using System.Buffers.Binary;
using System.Text;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Privacy;

/// <summary>Original-byte X509 fixture checks; no proof or State authority is granted.</summary>
public static partial class PrivacyExact12FixtureCodecV1
{
    private const string X509RequestSchema = "iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1";
    private const string X509ConcreteSchema = "iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1";
    private const string X509StatementSchema = "iroha.privacy.statement.v1";
    private const string X509ActionSchema = "iroha_data_model::transaction::signed_attachments::ActionStatementV1";
    private const string X509PayloadSchema = "iroha_data_model::transaction::signed::model::TransactionPayload";

    internal static void ValidateX509Bindings(PrivacyExact12TypedFixtureRowV1 row)
    {
        var zero = new byte[32];
        var statementPayload = X509ExactFrame(row.StatementNoritoSpan, X509StatementSchema, 8, MaxStatementBytes);
        var statement = X509Fields(X509TaggedContent(statementPayload, 5), 20, MaxStatementBytes);
        var context = X509Fields(statement[0], 8, MaxStatementBytes);
        _ = NetworkId.FromBytes(context[0]);
        X509Equal(context[1], X509U32(0), "privacy ordinal");
        X509Equal(context[2], X509Struct(row.TransactionIntentDigest), "derived intent");
        foreach (var digest in context.Skip(3)) X509Digest(digest, true);
        var envelopePayload = X509ExactFrame(row.EnvelopeNoritoSpan, "iroha.privacy.proof-envelope.v1", 8, MaxEnvelopeBytes);
        var envelope = X509Fields(envelopePayload, 13, MaxEnvelopeBytes);
        X509Equal(envelope[0], new byte[] { 0x49, 0x52, 0x48, 0x5a, 0x4b, 0x31, 0xa5, 0x5a }, "envelope magic");
        X509Equal(envelope[1], Convert.FromHexString("5bcf366fb18d2e4361486c015da5b41a3fb00edf05611042ccc452120d176eb0"), "catalog commitment");
        X509Equal(envelope[2], X509U32(5), "protocol");
        X509Equal(envelope[3], X509U32(0), "proof system");
        X509Equal(envelope[4], X509U32(0), "engine");
        for (var index = 0; index < 5; index++) X509Equal(envelope[index + 5], context[index + 3], "governed tuple");
        X509Equal(envelope[11], statementPayload, "envelope statement");
        X509Equal(X509Digest(envelope[10], true), X509DomainHash("iroha:privacy:statement:v1", row.StatementNorito), "statement digest");
        var proof = X509Vector(X509Fields(X509TaggedContent(envelope[12], 5), 1, MaxEnvelopeBytes)[0], MaxEnvelopeBytes);
        if (proof.Length == 0 || proof.AsSpan().IndexOfAnyExcept((byte)0) < 0) throw X509Invalid("empty proof");

        var reference = X509Struct(X509U32(0), X509U32(0));
        var requestLimit = MaxStatementBytes - X509Frame([], X509StatementSchema, 8).Length - 4
            + X509Struct(reference).Length + X509Frame([], X509RequestSchema, 0).Length;
        var requestArchive = row.SubmitProofInstructionNorito;
        var request = X509Fields(X509ExactFrame(requestArchive, X509RequestSchema, 0, requestLimit), 2, requestLimit);
        context[2] = X509Struct(zero);
        statement[0] = X509Struct(context);
        X509Equal(request[0], X509Struct(statement), "ZERO-intent request statement");
        X509Equal(request[1], reference, "proof reference");
        var unsigned = X509Fields(row.UnsignedTransactionPayloadNoritoSpan, 9, MaxUnsignedTransactionBytes);
        X509Equal(unsigned[0], X509Tagged(0, context[0]), "transaction network");
        X509Equal(unsigned[1], statement[17], "wallet authority");
        X509Equal(unsigned[2], X509U64(1_700_000_000_005), "fixture creation time");
        X509Equal(unsigned[4], X509Concat([1], X509Struct(X509U64(60_000))), "fixture TTL");
        X509Equal(unsigned[5], X509Concat([1], X509Struct(X509U32(6))), "fixture nonce");
        var wire = X509Struct(Encoding.UTF8.GetBytes(SubmitX509WireId));
        var boxed = X509Struct(wire, X509Concat(X509U64((ulong)requestArchive.Length), requestArchive));
        X509Equal(unsigned[3], X509Tagged(0, X509Sequence(boxed)), "original instruction");
        var publicInputs = X509DomainHash("iroha.privacy.zk-x509-request-public-inputs.v1", X509Frame(request[0], X509ConcreteSchema, 0));
        byte[][] action = [X509U32(0), X509Tagged(0, wire), X509U32(0), X509U32(0), X509Tagged(1, X509U32(0)), X509Struct(publicInputs)];
        var originalAttachments = unsigned[8];
        unsigned[8] = X509Attachments(action, zero, [], zero);
        var ordered = X509DomainHash("iroha.transaction.ordered-intent-digest.v1", X509Frame(X509Struct(unsigned), X509PayloadSchema, 0));
        var actionStatement = X509Frame(X509Struct([X509Struct(ordered), ..action]), X509ActionSchema, 0);
        var actionDigest = X509DomainHash("iroha.transaction.action-statement-digest.v1", actionStatement);
        X509Equal(row.TransactionIntentProjectionNoritoSpan, actionStatement, "ActionStatement projection");
        X509Equal(row.TransactionIntentDigestSpan, actionDigest, "ActionStatement digest");
        X509Equal(originalAttachments, X509Attachments(action, actionDigest, proof, Blake3.Hash(proof)), "original occurrence");
        var signed = row.SignedTransactionVersionedNoritoSpan;
        if (signed.Length == 0 || signed[0] != 1) throw X509Invalid("signed version");
        var signedFields = X509Fields(signed[1..], 3, MaxSignedTransactionBytes);
        if (signedFields[0].Length == 0) throw X509Invalid("signature");
        X509Equal(signedFields[1], row.UnsignedTransactionPayloadNoritoSpan, "signed payload");
        X509Equal(signedFields[2], new byte[] { 0 }, "multisig marker");
        X509Equal(row.SignedTransactionHashSpan, IrohaHash.Hash(X509Tagged(0, row.UnsignedTransactionPayloadNorito)), "transaction hash");
    }

    private static byte[] X509Attachments(byte[][] action, byte[] digest, byte[] proof, byte[] hash)
    {
        var reference = X509Struct(X509U32(0), X509U32(0), X509U32(0), X509U64((ulong)proof.Length), X509Struct(hash));
        var descriptor = X509Struct([..action, X509Struct(digest), X509Sequence(reference)]);
        var occurrence = X509Struct(X509U32(0), X509U32(0), X509U64((ulong)proof.Length), X509Struct(hash), X509Concat(X509U64((ulong)proof.Length), proof));
        return X509Concat([1], X509Struct(X509Struct(X509Sequence(descriptor), X509Sequence(occurrence))));
    }
    private static byte[][] X509Fields(ReadOnlySpan<byte> bytes, int count, int maximum)
    {
        var reader = new CanonicalReader(bytes);
        var result = new byte[count][];
        for (var index = 0; index < count; index++)
        {
            var field = reader.ReadSizedField(maximum, "X509 field");
            result[index] = field.ReadBytes(field.Remaining, "X509 field");
        }
        reader.RequireEnd("X509 fields");
        return result;
    }
    private static byte[] X509TaggedContent(ReadOnlySpan<byte> bytes, uint expected)
    {
        if (bytes.Length < 4 || BinaryPrimitives.ReadUInt32LittleEndian(bytes) != expected) throw X509Invalid("protocol tag");
        return X509Fields(bytes[4..], 1, MaxEnvelopeBytes)[0];
    }
    private static byte[] X509Vector(byte[] bytes, int maximum)
    {
        if (bytes.Length < 8 || bytes.Length - 8 > maximum || BinaryPrimitives.ReadUInt64LittleEndian(bytes) != (ulong)(bytes.Length - 8)) throw X509Invalid("byte vector");
        return bytes[8..];
    }
    private static byte[] X509Digest(byte[] bytes, bool nonzero)
    {
        var digest = X509Fields(bytes, 1, 32)[0];
        if (digest.Length != 32 || (nonzero && digest.AsSpan().IndexOfAnyExcept((byte)0) < 0)) throw X509Invalid("digest");
        return digest;
    }
    private static byte[] X509ExactFrame(ReadOnlySpan<byte> bytes, string schema, int padding, int maximum)
    {
        if (bytes.Length > maximum) throw X509Invalid("frame bound");
        var decoded = NoritoCodec.Decode(schema, bytes);
        if (decoded.Flags != CanonicalFlags || !X509Frame(decoded.Payload, schema, padding).AsSpan().SequenceEqual(bytes)) throw X509Invalid("canonical frame");
        return decoded.Payload;
    }
    private static byte[] X509Frame(byte[] payload, string schema, int padding)
    {
        var raw = NoritoCodec.Encode(schema, payload, CanonicalFlags);
        return padding == 0 ? raw : X509Concat(raw[..NoritoHeader.EncodedLength], new byte[padding], raw[NoritoHeader.EncodedLength..]);
    }
    private static byte[] X509Struct(params byte[][] values)
    {
        var writer = new CanonicalWriter(MaxArchiveBytes);
        foreach (var value in values) { writer.WriteCompactLength((ulong)value.Length); writer.WriteBytes(value); }
        return writer.ToArray();
    }
    private static byte[] X509Sequence(params byte[][] values) => X509Concat(X509U64((ulong)values.Length), X509Struct(values));
    private static byte[] X509Tagged(uint tag, byte[] value) => X509Concat(X509U32(tag), X509Struct(value));
    private static byte[] X509U32(uint value) { var bytes = new byte[4]; BinaryPrimitives.WriteUInt32LittleEndian(bytes, value); return bytes; }
    private static byte[] X509U64(ulong value) { var bytes = new byte[8]; BinaryPrimitives.WriteUInt64LittleEndian(bytes, value); return bytes; }
    private static byte[] X509Concat(params byte[][] values)
    {
        var writer = new CanonicalWriter(MaxArchiveBytes);
        foreach (var value in values) writer.WriteBytes(value);
        return writer.ToArray();
    }
    private static byte[] X509DomainHash(string domain, byte[] value) => Blake3.Hash(X509Concat(Encoding.UTF8.GetBytes(domain), X509U64((ulong)value.Length), value));
    private static void X509Equal(ReadOnlySpan<byte> actual, ReadOnlySpan<byte> expected, string label)
    {
        if (!actual.SequenceEqual(expected)) throw X509Invalid(label);
    }
    private static ArgumentException X509Invalid(string label) => new($"X509 fixture binding differs: {label}");
}
