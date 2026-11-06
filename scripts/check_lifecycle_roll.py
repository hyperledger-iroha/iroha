#!/usr/bin/env python3
"""Read-only all-seat committed lifecycle rollout compatibility check."""
import argparse
import json
from urllib.request import Request, urlopen


def check_reports(reports, height):
    """Require one committed network/runtime identity and an active governed boundary."""
    if not reports:
        raise ValueError("at least one validator seat is required")
    if type(height) is not int or height < 2:
        raise ValueError("activation height must be after genesis")
    expected = None
    for report in reports:
        fields = {"network_id", "v2_activation_height", "committed_height", "active", "consensus_config_fingerprint"}
        if type(report) is not dict or set(report) != fields:
            raise ValueError("malformed lifecycle activation response")
        if report["v2_activation_height"] != height or type(report["v2_activation_height"]) is not int:
            raise ValueError("validator activation height mismatch")
        tip = report["committed_height"]
        if type(tip) is not int or tip < 1 or type(report["active"]) is not bool:
            raise ValueError("malformed validator tip/activation state")
        if report["active"] is not True or tip < height:
            raise ValueError("validator has not committed the governed lifecycle activation")
        identity = (report["network_id"], report["consensus_config_fingerprint"])
        if any(type(value) is not str or not value for value in identity):
            raise ValueError("missing rollout identity")
        if expected is not None and identity != expected:
            raise ValueError("validator consensus configuration fingerprint or network mismatch")
        expected = identity
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--height", type=int, required=True)
    parser.add_argument("seats", nargs="+", help="every validator's Torii root URL")
    args = parser.parse_args()
    reports = []
    for seat in args.seats:
        request = Request(seat.rstrip("/") + "/v1/triggers/lifecycle-activation", headers={"Accept": "application/json"})
        with urlopen(request, timeout=30) as response:
            data = response.read(4097)
        if len(data) > 4096:
            raise ValueError("activation report exceeds 4 KiB")
        reports.append(json.loads(data))
    network, fingerprint = check_reports(reports, args.height)
    print(json.dumps({"compatible": True, "seats": len(reports), "network_id": network,
                      "v2_activation_height": args.height, "consensus_config_fingerprint": fingerprint,
                      "all_seats_active": all(r["active"] for r in reports)}))


if __name__ == "__main__":
    main()
