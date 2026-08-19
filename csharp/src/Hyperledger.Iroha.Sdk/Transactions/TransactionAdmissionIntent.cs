using System.Text.Json;
using System.Text.Json.Serialization;

namespace Hyperledger.Iroha.Transactions;

/// <summary>Signature-bound admission protocol required before a transaction may execute.</summary>
[JsonConverter(typeof(TransactionAdmissionIntentJsonConverter))]
public enum TransactionAdmissionIntent : uint
{
    /// <summary>Ordinary queue admission without a globally certified QueuePlan owner.</summary>
    Ordinary = 0,

    /// <summary>Require an exact quorum-certified QueuePlan registry owner before execution.</summary>
    QueuePlanSynced = 1,
}

internal sealed class TransactionAdmissionIntentJsonConverter
    : JsonConverter<TransactionAdmissionIntent>
{
    public override TransactionAdmissionIntent Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options)
    {
        using var document = JsonDocument.ParseValue(ref reader);
        var fields = FeePaymentIntentJsonConverter.RequireExactObject(
            document.RootElement,
            "transaction admission intent",
            ["intent", "value"]);
        if (fields["value"].ValueKind != JsonValueKind.Null)
        {
            throw new JsonException("transaction admission intent.value must be null.");
        }

        if (fields["intent"].ValueKind != JsonValueKind.String)
        {
            throw new JsonException("transaction admission intent.intent must be a string.");
        }

        return fields["intent"].GetString() switch
        {
            "ordinary" => TransactionAdmissionIntent.Ordinary,
            "queue_plan_synced" => TransactionAdmissionIntent.QueuePlanSynced,
            _ => throw new JsonException(
                "transaction admission intent.intent must be ordinary or queue_plan_synced."),
        };
    }

    public override void Write(
        Utf8JsonWriter writer,
        TransactionAdmissionIntent value,
        JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WriteString("intent", value switch
        {
            TransactionAdmissionIntent.Ordinary => "ordinary",
            TransactionAdmissionIntent.QueuePlanSynced => "queue_plan_synced",
            _ => throw new JsonException("Unknown transaction admission intent."),
        });
        writer.WriteNull("value");
        writer.WriteEndObject();
    }
}
