using System.Buffers.Binary;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>
/// Exact decoding of the signature-bound <c>fee_payment</c> field that Torii
/// draft responses must carry before the SDK signs them locally.
/// </summary>
public sealed class ToriiSubmitValidationTests
{
    private const string SponsorAccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string ProgramName = "wallet_fx";
    private const ulong GasLimit = 700;

    [Fact]
    public void FeePaymentAcceptsCanonicalAuthorityAndSponsorIntents()
    {
        var encoding = new TransactionEncodingContext(SponsorAccountId);
        var programId = new FeeSponsorProgramId(SponsorAccountId, ProgramName);
        foreach (var intent in new[]
        {
            FeePaymentIntent.Authority([]),
            FeePaymentIntent.Authority([], gasLimit: GasLimit),
            FeePaymentIntent.Sponsor(programId, programRevision: 3, chargeLimits: []),
            FeePaymentIntent.Sponsor(
                programId,
                programRevision: 3,
                chargeLimits: [],
                gasLimit: GasLimit),
        })
        {
            ToriiSubmitValidation.RequireCanonicalTransactionFeePayment(
                encoding.EncodeFeePaymentIntent(intent));
        }
    }

    [Fact]
    public void FeePaymentRejectsNoncanonicalFramingPayerAndGasLimit()
    {
        var encoding = new TransactionEncodingContext(SponsorAccountId);
        var sponsor = encoding.EncodeFeePaymentIntent(FeePaymentIntent.Sponsor(
            new FeeSponsorProgramId(SponsorAccountId, ProgramName),
            programRevision: 3,
            chargeLimits: [],
            gasLimit: GasLimit));
        var authority = encoding.EncodeFeePaymentIntent(
            FeePaymentIntent.Authority([], gasLimit: GasLimit));

        // Authority layout: payer tag, one-byte value length, then the value
        // ending in the gas option (Some tag, framed length, UInt64).
        Assert.Equal((byte)(authority.Length - 5), authority[4]);
        Assert.Equal((byte)1, authority[^10]);
        Assert.Equal(
            GasLimit,
            BinaryPrimitives.ReadUInt64LittleEndian(
                authority.AsSpan(authority.Length - sizeof(ulong))));

        var unknownPayer = sponsor.ToArray();
        unknownPayer[0] = 2;
        var unknownGasTag = authority.ToArray();
        unknownGasTag[^10] = 2;
        var zeroGas = authority.ToArray();
        zeroGas.AsSpan(zeroGas.Length - sizeof(ulong)).Clear();
        var valueTrailing = new CanonicalNoritoWriter();
        valueTrailing.WriteUInt32LittleEndian(0);
        valueTrailing.WriteField(authority[5..].Concat(new byte[] { 0 }).ToArray());

        foreach (var malformed in new[]
        {
            Array.Empty<byte>(),
            sponsor[..^1],
            sponsor.Concat(new byte[] { 0 }).ToArray(),
            authority.Concat(new byte[] { 0 }).ToArray(),
            valueTrailing.ToArray(),
            unknownPayer,
            unknownGasTag,
            zeroGas,
        })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                ToriiSubmitValidation.RequireCanonicalTransactionFeePayment(malformed));
        }
    }

    [Fact]
    public void SponsorFeePaymentRejectsNoncanonicalSponsorProgramAndRevision()
    {
        var encoding = new TransactionEncodingContext(SponsorAccountId);
        var sponsor = encoding.EncodeAccountId(SponsorAccountId);
        Assert.Equal(
            encoding.EncodeFeePaymentIntent(FeePaymentIntent.Sponsor(
                new FeeSponsorProgramId(SponsorAccountId, ProgramName),
                programRevision: 3,
                chargeLimits: [],
                gasLimit: GasLimit)),
            SponsorFeePayment(encoding, sponsor, ProgramName, programRevision: 3));

        foreach (var malformed in new[]
        {
            SponsorFeePayment(encoding, sponsor, ProgramName, programRevision: 0),
            SponsorFeePayment(
                encoding,
                sponsor.Concat(new byte[] { 0 }).ToArray(),
                ProgramName,
                programRevision: 3),
            SponsorFeePayment(encoding, sponsor, string.Empty, programRevision: 3),
            SponsorFeePayment(encoding, sponsor, "wallet@fx", programRevision: 3),
            SponsorFeePayment(encoding, sponsor, "wallet fx", programRevision: 3),
            SponsorFeePayment(encoding, sponsor, "wallet/fx", programRevision: 3),
            SponsorFeePayment(encoding, sponsor, "wallet_fé", programRevision: 3),
        })
        {
            Assert.ThrowsAny<ArgumentException>(() =>
                ToriiSubmitValidation.RequireCanonicalTransactionFeePayment(malformed));
        }
    }

    private static byte[] SponsorFeePayment(
        TransactionEncodingContext encoding,
        byte[] sponsor,
        string programName,
        ulong programRevision)
    {
        var programId = new CanonicalNoritoWriter();
        programId.WriteField(sponsor);
        programId.WriteField(encoding.EncodeString(programName));
        var chargeLimits = new CanonicalNoritoWriter();
        chargeLimits.WriteSequenceLength(0);
        var value = new CanonicalNoritoWriter();
        value.WriteField(programId.ToArray());
        value.WriteField(encoding.EncodeUInt64(programRevision));
        value.WriteField(chargeLimits.ToArray());
        value.WriteField(encoding.EncodeOption<ulong>(GasLimit, encoding.EncodeUInt64));
        var intent = new CanonicalNoritoWriter();
        intent.WriteUInt32LittleEndian(1);
        intent.WriteField(value.ToArray());
        return intent.ToArray();
    }
}
