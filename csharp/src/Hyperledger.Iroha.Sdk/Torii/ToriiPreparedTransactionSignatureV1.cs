using System.Buffers.Binary;
using System.Globalization;
using System.Text;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;

namespace Hyperledger.Iroha.Torii;

/// <summary>Builds and verifies the fixed V1 prepared-transaction signature transcript.</summary>
internal static class ToriiPreparedTransactionSignatureV1
{
    internal const string TranscriptSchema = "iroha.taira.prepared-signature-transcript.v1";

    private static readonly byte[] SignatureDomain =
        "iroha:taira:prepared-transaction:v1\0"u8.ToArray();

    internal static byte[] OnboardingPreparedTranscript(
        ToriiAccountOnboardingPreparedTransactionV1 prepared,
        ReadOnlySpan<byte> signedTransactionWire)
    {
        ArgumentNullException.ThrowIfNull(prepared);
        var transcript = BaseTranscript(prepared.Schema, prepared.Operation, prepared.Binding);
        AppendField(transcript, "semantic_hash_hex", prepared.SemanticHashHex);
        AppendField(transcript, "account_id", prepared.AccountId);
        AppendField(transcript, "alias", prepared.Alias);
        AppendField(transcript, "disposition", prepared.Disposition.Kind);
        AppendField(transcript, "transaction_hash_hex", prepared.TransactionHashHex);
        AppendField(
            transcript,
            "signed_transaction_wire_sha256",
            prepared.SignedTransactionWireSha256);
        AppendField(transcript, "signed_transaction_wire", signedTransactionWire);
        return transcript.ToArray();
    }

    internal static byte[] OnboardingProofRequiredTranscript(
        ToriiAccountOnboardingProofRequiredPrepareResponseV1 proofRequired)
    {
        ArgumentNullException.ThrowIfNull(proofRequired);
        var transcript = BaseTranscript(
            proofRequired.Schema,
            proofRequired.Operation,
            proofRequired.Binding);
        AppendField(transcript, "outcome", proofRequired.Outcome);
        AppendField(transcript, "proof_kind", proofRequired.ProofKind);
        AppendField(transcript, "semantic_hash_hex", proofRequired.SemanticHashHex);
        AppendField(transcript, "account_id", proofRequired.AccountId);
        AppendField(transcript, "alias", proofRequired.Alias);
        AppendField(transcript, "disposition", proofRequired.Disposition.Kind);
        return transcript.ToArray();
    }

    internal static byte[] FaucetPreparedTranscript(
        ToriiAccountFaucetPreparedTransactionV1 prepared,
        ReadOnlySpan<byte> signedTransactionWire)
    {
        ArgumentNullException.ThrowIfNull(prepared);
        var transcript = BaseTranscript(prepared.Schema, prepared.Operation, prepared.Binding);
        AppendField(transcript, "claim.account_id", prepared.Claim.AccountId);
        AppendField(
            transcript,
            "claim.pow_anchor_height",
            prepared.Claim.PowAnchorHeight is ulong anchor
                ? $"some:{anchor.ToString(CultureInfo.InvariantCulture)}"
                : "none");
        AppendField(
            transcript,
            "claim.pow_nonce_hex",
            prepared.Claim.PowNonceHex is string nonce ? $"some:{nonce}" : "none");
        AppendField(transcript, "semantic_hash_hex", prepared.SemanticHashHex);
        AppendField(transcript, "account_id", prepared.AccountId);
        AppendField(transcript, "asset_definition_id", prepared.AssetDefinitionId);
        AppendField(transcript, "asset_id", prepared.AssetId);
        AppendField(transcript, "amount", prepared.Amount);
        AppendField(transcript, "transaction_hash_hex", prepared.TransactionHashHex);
        AppendField(
            transcript,
            "signed_transaction_wire_sha256",
            prepared.SignedTransactionWireSha256);
        AppendField(transcript, "signed_transaction_wire", signedTransactionWire);
        return transcript.ToArray();
    }

    internal static void Verify(
        ReadOnlySpan<byte> transcript,
        string serverSignature,
        ReadOnlySpan<byte> signerPublicKey,
        string context)
    {
        if (serverSignature is null
            || serverSignature.Length != Ed25519Signer.SignatureLength * 2
            || serverSignature.Any(static value =>
                value is not (>= '0' and <= '9') and not (>= 'A' and <= 'F')))
        {
            throw new System.Text.Json.JsonException(
                $"{context}.server_signature must contain exactly one canonical uppercase Ed25519 signature.");
        }
        if (signerPublicKey.Length != Ed25519Signer.PublicKeyLength)
        {
            throw new System.Text.Json.JsonException(
                $"{context} server signer must be one Ed25519 public key.");
        }

        var signature = Convert.FromHexString(serverSignature);
        if (signature.All(static value => value == 0)
            || !Ed25519Signer.Verify(IrohaHash.Hash(transcript), signature, signerPublicKey))
        {
            throw new System.Text.Json.JsonException(
                $"{context}.server_signature does not authenticate the exact prepared envelope.");
        }
    }

    private static List<byte> BaseTranscript(
        string envelopeSchema,
        string operation,
        ToriiTairaPublicResetMutationBindingV1 binding)
    {
        ArgumentNullException.ThrowIfNull(binding);
        var transcript = new List<byte>();
        AppendFrame(transcript, SignatureDomain);
        AppendField(transcript, "transcript_schema", TranscriptSchema);
        AppendField(transcript, "envelope_schema", envelopeSchema);
        AppendField(transcript, "operation", operation);
        AppendField(transcript, "binding.schema", binding.Schema);
        AppendField(
            transcript,
            "binding.authorization_sha256",
            binding.AuthorizationSha256);
        AppendField(
            transcript,
            "binding.authorization_nonce",
            binding.AuthorizationNonce);
        AppendField(transcript, "binding.kind", binding.Kind);
        AppendField(transcript, "binding.phase", binding.Phase);
        AppendField(transcript, "binding.idempotency_key", binding.IdempotencyKey);
        AppendField(
            transcript,
            "binding.execution_expires_at_unix_ms",
            binding.ExecutionExpiresAtUnixMilliseconds.ToString(CultureInfo.InvariantCulture));
        return transcript;
    }

    private static void AppendField(List<byte> transcript, string label, string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        AppendFrame(transcript, Encoding.UTF8.GetBytes(label));
        AppendFrame(transcript, Encoding.UTF8.GetBytes(value));
    }

    private static void AppendField(
        List<byte> transcript,
        string label,
        ReadOnlySpan<byte> value)
    {
        AppendFrame(transcript, Encoding.UTF8.GetBytes(label));
        AppendFrame(transcript, value);
    }

    private static void AppendFrame(List<byte> transcript, ReadOnlySpan<byte> value)
    {
        Span<byte> length = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64BigEndian(length, checked((ulong)value.Length));
        transcript.AddRange(length);
        transcript.AddRange(value);
    }
}
