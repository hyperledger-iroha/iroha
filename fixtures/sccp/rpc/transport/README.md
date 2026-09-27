# SCCP RPC transport fixtures

Recorded responses replayed by `crates/iroha_sccp_rpc/tests/transport.rs`
through an in-process mock HTTP server. The tests never reach the network.

`recorded.json` indexes every exchange: the public endpoint it was recorded
from, the request that produced it, the HTTP status, content type, relevant
response headers and the body file. Bodies are stored byte-for-byte unless the
entry has a `trimmed` note; trimming only drops array elements or bulky
object members (transactions, receipts, ABI) and never edits a kept value.

Recorded on 2026-09-26 (Ethereum block `0x18daf08`, beacon slot `15301120`,
TRON block `86588651`) from:

- `https://ethereum-rpc.publicnode.com` (Ethereum execution JSON-RPC),
- `https://bsc-dataseed.bnbchain.org` (BSC JSON-RPC),
- `https://lodestar-mainnet.chainsafe.io` (beacon light-client API, SSZ),
- `https://ethereum-beacon-api.publicnode.com` (answers the SSZ light-client
  routes with JSON, kept to exercise failover to an SSZ-capable endpoint),
- `https://api.trongrid.io` (TRON HTTP API).

The TRON `/walletsolidity/getcontractinfo` exchange records the HTTP 405 that
java-tron returns for that route; runtime code is read from
`/wallet/getcontractinfo` instead.

Synthetic replies (HTTP 429 and 5xx, timeouts, malformed hex, a successful
`eth_sendRawTransaction` and `broadcasthex`) are built inline by the tests and
are not stored here. JSON-RPC ids in replayed bodies are rewritten by the mock
server to the id of the request it answers.
