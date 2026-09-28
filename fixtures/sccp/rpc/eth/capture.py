#!/usr/bin/env python3
"""Capture Ethereum mainnet light-client and execution responses for SCCP fixtures.

Usage: python3 fixtures/sccp/rpc/eth/capture.py <output directory>

Captures, within a few minutes (so that the finalized block is still inside the
eth_getProof window):

1. the beacon finality update, which fixes the finalized execution block E;
2. a bootstrap at an epoch-boundary root of the previous sync-committee period;
3. the light-client updates of the previous and the current period;
4. the headers of the event block B (the block with the fewest transactions
   among the 64 before E) through E, the receipts of B, and the EIP-2935
   history-contract proof of slot B mod 8191 at E.

See README.md in this directory for the recorded summary.
"""
import json
import os
import sys
import urllib.error
import urllib.request

BEACON = "https://ethereum-beacon-api.publicnode.com"
EXEC = "https://ethereum-rpc.publicnode.com"
# publicnode refuses eth_getProof beyond a short window behind the head.
PROOF_EXEC = "https://eth.drpc.org"
HISTORY_CONTRACT = "0x0000F90827F1C53a10cb7A02335B175320002935"
HISTORY_SERVE_WINDOW = 8191
SLOTS_PER_PERIOD = 8192
HEADERS = {"user-agent": "curl/8.7.1"}


def get(path):
    request = urllib.request.Request(
        BEACON + path, headers={"accept": "application/json", **HEADERS}
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def rpc(method, params, url=EXEC):
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = urllib.request.Request(
        url, data=body, headers={"content-type": "application/json", **HEADERS}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.status, response.read()


def main(out):
    os.makedirs(out, exist_ok=True)
    index = {}

    def save(name, status, raw, request, source, trimmed=None):
        with open(os.path.join(out, name), "wb") as handle:
            handle.write(raw)
        entry = {"body": name, "request": request, "source": source, "status": status}
        if trimmed:
            entry["trimmed"] = trimmed
        index[name.rsplit(".", 1)[0]] = entry

    beacon_request = lambda path: {"method": "GET", "path": path, "accept": "application/json"}

    path = "/eth/v1/beacon/light_client/finality_update"
    status, raw = get(path)
    finality = json.loads(raw)
    save("finality_update.json", status, raw, beacon_request(path), BEACON)
    finalized_slot = int(finality["data"]["finalized_header"]["beacon"]["slot"])
    period = finalized_slot // SLOTS_PER_PERIOD
    e_number = int(finality["data"]["finalized_header"]["execution"]["block_number"])

    bootstrap_slot = None
    for back in range(1, 64):
        slot = period * SLOTS_PER_PERIOD - back * 32 * 40
        status, raw = get(f"/eth/v1/beacon/headers/{slot}")
        root = json.loads(raw).get("data", {}).get("root")
        if not root:
            continue
        path = f"/eth/v1/beacon/light_client/bootstrap/{root}"
        status, raw = get(path)
        if status == 200:
            bootstrap_slot = slot
            save("bootstrap.json", status, raw, beacon_request(path), BEACON)
            break
    assert bootstrap_slot is not None, "no bootstrap served for the previous period"

    path = f"/eth/v1/beacon/light_client/updates?start_period={period - 1}&count=2"
    status, raw = get(path)
    save("updates.json", status, raw, beacon_request(path), BEACON)

    best = None
    blocks = {}
    for number in range(e_number - 64, e_number + 1):
        status, raw = rpc("eth_getBlockByNumber", [hex(number), False])
        block = json.loads(raw)["result"]
        blocks[number] = (status, block)
        count = len(block["transactions"])
        if number < e_number and count > 0 and (best is None or count < best[1]):
            best = (number, count)
    b_number = best[0]
    for number in range(b_number, e_number + 1):
        status, block = blocks[number]
        count = len(block["transactions"])
        block["transactions"] = []
        raw = json.dumps(
            {"jsonrpc": "2.0", "id": 1, "result": block}, sort_keys=True, indent=1
        ).encode()
        save(
            f"block_{number}.json",
            status,
            raw,
            {"method": "eth_getBlockByNumber", "params": [hex(number), False]},
            EXEC,
            trimmed=f"transactions array ({count} hashes) removed; header fields unchanged",
        )

    params = [hex(b_number)]
    status, raw = rpc("eth_getBlockReceipts", params)
    save(
        f"receipts_{b_number}.json",
        status,
        raw,
        {"method": "eth_getBlockReceipts", "params": params},
        EXEC,
    )

    slot_key = "0x" + format(b_number % HISTORY_SERVE_WINDOW, "064x")
    params = [HISTORY_CONTRACT, [slot_key], hex(e_number)]
    status, raw = rpc("eth_getProof", params, PROOF_EXEC)
    assert b"accountProof" in raw, raw
    save(
        "history_proof.json",
        status,
        raw,
        {"method": "eth_getProof", "params": params},
        PROOF_EXEC,
    )

    summary = {
        "bootstrap_slot": bootstrap_slot,
        "event_block": b_number,
        "finalized_block": e_number,
        "finalized_slot": finalized_slot,
        "period": period,
        "signature_slot": int(finality["data"]["signature_slot"]),
    }
    with open(os.path.join(out, "recorded.json"), "w") as handle:
        json.dump({"summary": summary, "exchanges": index}, handle, sort_keys=True, indent=2)
        handle.write("\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main(sys.argv[1])
