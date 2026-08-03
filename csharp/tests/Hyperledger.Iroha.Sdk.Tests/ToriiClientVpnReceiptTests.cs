using System.Net;
using System.Text.Json;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    [Fact]
    public async Task SubmitVpnReceiptAsyncPostsEvidenceAndDeserializesSettlementInstruction()
    {
        const string quoteId = "1111111111111111111111111111111111111111111111111111111111111111";
        const string sessionId = "8989898989898989898989898989898989898989898989898989898989898989";

        using var handler = new RecordingHandler(request =>
        {
            var payload = ReadBodyAsJson(request);
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/vpn/receipts", request.RequestUri!.AbsolutePath);
            Assert.Equal("abcd", payload.RootElement.GetProperty("relay_receipt_hex").GetString());
            Assert.Equal("beef", payload.RootElement.GetProperty("client_voucher_hex").GetString());
            Assert.Equal(quoteId, payload.RootElement.GetProperty("lease_id_hex").GetString());

            return new HttpResponseMessage(HttpStatusCode.Created)
            {
                Content = new StringContent($$"""
                    {
                      "session_id": "{{sessionId}}",
                      "account_id": "{{VpnAccountId}}",
                      "exit_class": "standard",
                      "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                      "meter_family": "vpn-standard",
                      "connected_at_ms": 1699999400000,
                      "disconnected_at_ms": 1700000000000,
                      "duration_ms": 600000,
                      "bytes_in": 123,
                      "bytes_out": 456,
                      "status": "settled",
                      "receipt_source": "relay",
                      "quote_id": "{{quoteId}}",
                      "payment_tx_hash": "2222222222222222222222222222222222222222222222222222222222222222",
                      "fee_asset_id": "xor#universal.universal",
                      "escrow_account_id": "{{VpnEscrowAccountId}}",
                      "operator_account_id": "{{VpnOperatorAccountId}}",
                      "lease_fee": "1000000.25",
                      "earned_fee": "500000.125",
                      "refunded_fee": "500000.125",
                      "lease_id_hex": "{{quoteId}}",
                      "settle_lease_instruction": {
                        "wire_id": "SettleVpnLease",
                        "payload_hex": "cafe"
                      }
                    }
                    """),
            };
        });

        using var client = CreateSignedVpnClient(handler);
        var receipt = await client.SubmitVpnReceiptAsync(new ToriiVpnReceiptSubmitRequest
        {
            RelayReceiptHex = "0XABCD",
            ClientVoucherHex = "0xbeef",
            LeaseIdHex = "0X" + quoteId.ToUpperInvariant(),
        }, cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(sessionId, receipt.SessionId);
        Assert.Equal(VpnAccountId, receipt.AccountId);
        Assert.Equal(VpnEscrowAccountId, receipt.EscrowAccountId);
        Assert.Equal(VpnOperatorAccountId, receipt.OperatorAccountId);
        Assert.Equal("settled", receipt.Status);
        Assert.Equal("SettleVpnLease", receipt.SettleLeaseInstruction?.WireId);
        Assert.Equal("500000.125", receipt.EarnedFee);
    }

    public static IEnumerable<object[]> InvalidVpnReceiptSubmitRequests()
    {
        var valid = new ToriiVpnReceiptSubmitRequest
        {
            RelayReceiptHex = "abcd",
            ClientVoucherHex = "beef",
            LeaseIdHex = new string('d', 64),
        };

        yield return new object[] { valid with { RelayReceiptHex = null! }, "RelayReceiptHex", "null or whitespace" };
        yield return new object[] { valid with { RelayReceiptHex = "" }, "RelayReceiptHex", "null or whitespace" };
        yield return new object[] { valid with { RelayReceiptHex = " abcd" }, "RelayReceiptHex", "whitespace" };
        yield return new object[] { valid with { RelayReceiptHex = "ab cd" }, "RelayReceiptHex", "whitespace" };
        yield return new object[] { valid with { RelayReceiptHex = "abcd\u0001" }, "RelayReceiptHex", "control characters" };
        yield return new object[] { valid with { RelayReceiptHex = "abc" }, "RelayReceiptHex", "even number of hexadecimal characters" };
        yield return new object[] { valid with { RelayReceiptHex = "zz" }, "RelayReceiptHex", "even number of hexadecimal characters" };
        yield return new object[] { valid with { ClientVoucherHex = null! }, "ClientVoucherHex", "null or whitespace" };
        yield return new object[] { valid with { ClientVoucherHex = "" }, "ClientVoucherHex", "null or whitespace" };
        yield return new object[] { valid with { ClientVoucherHex = " beef" }, "ClientVoucherHex", "whitespace" };
        yield return new object[] { valid with { ClientVoucherHex = "bee" }, "ClientVoucherHex", "even number of hexadecimal characters" };
        yield return new object[] { valid with { ClientVoucherHex = "zz" }, "ClientVoucherHex", "even number of hexadecimal characters" };
        yield return new object[] { valid with { LeaseIdHex = " " }, "LeaseIdHex", "null or whitespace" };
        yield return new object[] { valid with { LeaseIdHex = " " + new string('d', 64) }, "LeaseIdHex", "whitespace" };
        yield return new object[] { valid with { LeaseIdHex = new string('d', 63) }, "LeaseIdHex", "32-byte hex string" };
        yield return new object[] { valid with { LeaseIdHex = new string('j', 64) }, "LeaseIdHex", "32-byte hex string" };
    }

    [Theory]
    [MemberData(nameof(InvalidVpnReceiptSubmitRequests))]
    public async Task SubmitVpnReceiptAsyncRejectsMalformedRequestBeforeDispatch(
        ToriiVpnReceiptSubmitRequest request,
        string expectedParamName,
        string expectedMessage)
    {
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("malformed VPN receipt request reached HTTP dispatch"));
        using var client = CreateSignedVpnClient(handler);

        var error = await Assert.ThrowsAnyAsync<ArgumentException>(() =>
            client.SubmitVpnReceiptAsync(request, cancellationToken: TestContext.Current.CancellationToken));

        Assert.Equal(expectedParamName, error.ParamName);
        Assert.Contains(expectedMessage, error.Message);
        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task ListVpnReceiptsAsyncDeserializesNativeSettlementItems()
    {
        const string quoteId = "3333333333333333333333333333333333333333333333333333333333333333";
        const string sessionId = "4545454545454545454545454545454545454545454545454545454545454545";

        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/v1/vpn/receipts", request.RequestUri!.AbsolutePath);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent($$"""
                    {
                      "items": [
                        {
                          "session_id": "{{sessionId}}",
                          "account_id": "{{VpnAccountId}}",
                          "exit_class": "standard",
                          "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                          "meter_family": "vpn-standard",
                          "connected_at_ms": 1699999400000,
                          "disconnected_at_ms": 1700000000000,
                          "duration_ms": 600000,
                          "bytes_in": 123,
                          "bytes_out": 456,
                          "status": "settled",
                          "receipt_source": "relay",
                          "quote_id": "{{quoteId}}",
                          "payment_tx_hash": "4444444444444444444444444444444444444444444444444444444444444444",
                          "fee_asset_id": "xor#universal.universal",
                          "escrow_account_id": "{{VpnEscrowAccountId}}",
                          "operator_account_id": "{{VpnOperatorAccountId}}",
                          "lease_fee": "1000000.25",
                          "earned_fee": "500000.125",
                          "refunded_fee": "500000.125",
                          "lease_id_hex": "{{quoteId}}",
                          "settle_lease_instruction": {
                            "wire_id": "SettleVpnLease",
                            "payload_hex": "cafe"
                          }
                        }
                      ],
                      "total": 1
                    }
                    """),
            };
        });

        using var client = CreateSignedVpnClient(handler);
        var receipts = await client.ListVpnReceiptsAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal((ulong)1, receipts.Total);
        var items = Assert.IsType<ToriiVpnReceipt[]>(receipts.Items);
        Assert.Single(items);
        Assert.Equal(sessionId, items[0].SessionId);
        Assert.Equal(VpnAccountId, items[0].AccountId);
        Assert.Equal(quoteId, items[0].LeaseIdHex);
        items[0] = ValidVpnReceipt() with { SessionId = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
        Assert.Equal(sessionId, receipts.Items[0].SessionId);
        Assert.Equal("SettleVpnLease", receipts.Items[0].SettleLeaseInstruction?.WireId);
    }
}
