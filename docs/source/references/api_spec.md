# API Specification for Client Libraries

## Endpoints

### Submit Instructions

**Protocol**: HTTP

**Encoding**: Parity Scale Codec

**Endpoint**: `/transaction`

**Method**: `POST`

**Expects**: Body: `Transaction`

**Responses**:
- 200 OK - Transaction Accepted (But not guaranteed to have passed consensus yet)
- 500 Internal Server Error - Transaction Rejected (Malformed or improperly signed)

### Submit Query

**Protocol**: HTTP

**Encoding**: Parity Scale Codec

**Endpoint**: `/query`

**Method**: `POST`

**Expects**:
- Body: `SignedQueryRequest`
- Query parameters:
 + `start` - Optional parameter in queries where results can be indexed. Use to return results from specified point. Results are ordered where can be by id which uses rust's [PartialOrd](https://doc.rust-lang.org/std/cmp/trait.PartialOrd.html#derivable) and [Ord](https://doc.rust-lang.org/std/cmp/trait.Ord.html) traits.
 + `limit` - Optional parameter in queries where results can be indexed. Use to return specific number of results.

**Responses**:
- 200 OK - Query Executed Successfuly. Body: `QueryResult`
- 500 Internal Server Error - Query Rejected (Failed to parse/execute or improperly signed)

### Listen to Events

**Protocol**: HTTP

**Protocol Upgrade**: `WebSocket`

**Encoding**: JSON

**Endpoint**: `/events`

**Expects**: 

First message after handshake from client: `SubscriptionRequest`

When server is ready to transmit events it sends: `SubscriptionAccepted`

Server sends `Event` and expects `EventReceived` after each, before sending the next event.

### Metrics

**Protocol**: HTTP

**Encoding**: Parity Scale Codec

**Endpoint**: `/metrics`

**Method**: `GET`

**Expects**: -

**Responses**:
- 200 OK - Metrics Calculated Successfully. Body: `Metrics`
- 500 Internal Server Error - Failed to get metrics

### Configuration

**Protocol**: HTTP

**Encoding**: Json

**Endpoint**: `/configure`

**Method**: `GET` or `PUT`

**Expects** on `GET`:
Query parameter "docs" can be set to boolean (indecates whether should return documentation or field value)
Request body is json with field "field", which is array of strings.

**Responses** on `GET`:
- 200 OK - Field was found and either doc or value is returned in json body.
- 404 Not Found - Field wasn't found

**Expects** on `PUT`:
Json body with fields "field" (array of strings) and "value" which can be anything.

**Responses** on `PUT`:
- 200 OK - Field was found and value is valid
- 400 Bad Request - Value for field is invalid
- 404 Not Found - Field wasn't found

### Health

**Protocol**: HTTP

**Encoding**: Json

**Endpoint**: `/health`

**Method**: `GET`

**Expects**: -

**Responses**:
- 200 OK - The peer is up.
Also returns current status of peer in json string:
```
"Healthy"
```

## Parity Scale Codec

For more information on codec check [Substrate Dev Hub](https://substrate.dev/docs/en/knowledgebase/advanced/codec) and codec's [Github repository](https://github.com/paritytech/parity-scale-codec).

## Reference Iroha Client Implementation

[Iroha client in Rust.](../../../iroha_client)

## Iroha Structures

- `Transaction` - `iroha_data_model::transaction::Transaction`
- `SignedQueryRequest` - `iroha_data_model::query::SignedQueryRequest`
- `QueryResult` - `iroha_data_model::query::QueryResult`
- `SubscriptionRequest` - `iroha_data_model::events::EventSocketMessage::SubscriptionRequest`
- `SubscriptionAccepted` - `iroha_data_model::events::EventSocketMessage::SubscriptionAccepted`
- `Event` - `iroha_data_model::events::EventSocketMessage::Event`
- `EventReceived` - `iroha_data_model::events::EventSocketMessage::EventReceived`
- `Metrics` - `iroha::maintenance::Metrics`
