#!/usr/bin/env python3
"""Exact conditional field-block bundle accounting; no production qualification."""
from __future__ import annotations
import argparse
from hashlib import sha256
import json
from pathlib import Path
import check_compact_typed_profile as profile


def calculate(segments: int) -> dict:
    """Charge all child expansions under one adversary and adaptive-context family."""
    result=profile.aggregate_parts(375,segments)
    interval=result["aggregate_times_2_to_128_interval"]
    work=result["work_per_segment"]
    return {"segments":segments,"H_calls":segments*work["H_calls"],
        "whole_G_messages":22*segments,"G_blocks":931*segments,
        "physical_digest_calls":segments*work["verifier_digest_calls"],
        "group_query_budget":result["group_query_budget"],
        "passes_conditional_54_target_acceptance_bound":result["passes_strict_54_target_bound"],
        "scaled_acceptance_interval_over_1024":[interval["lower_numerator"],interval["upper_numerator"]],
        "query_candidates":401,"query_tape_bytes":3216,
        "total_honest_attempts":result["total_honest_attempts"],
        "honest_abort_below_2_to_minus":result["honest_abort_below_2_to_minus"],
        "honest_abort_below_2_to_minus_128":result["honest_abort_below_2_to_minus_128"]}


def controls() -> None:
    """Check bounds across the stated envelope and reject invalid input types."""
    single=calculate(1)
    assert single["group_query_budget"]==8590025578
    assert single["scaled_acceptance_interval_over_1024"]==[743,744]
    pair=calculate(2)
    assert pair["H_calls"]==89124 and pair["G_blocks"]==1862
    assert pair["group_query_budget"]==8590116564
    largest=calculate(128)
    assert largest["scaled_acceptance_interval_over_1024"]==[745,746]
    assert largest["honest_abort_below_2_to_minus"]==130
    for segments in range(1,129):
        result=calculate(segments)
        assert result["honest_abort_below_2_to_minus_128"]
        assert result["passes_conditional_54_target_acceptance_bound"]
    for invalid in [0,-1,129,True,1.5]:
        try:calculate(invalid)
        except ValueError:pass
        else:raise AssertionError(f"invalid segment count accepted: {invalid}")


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--segments",type=int,nargs="+",default=[1,2,128])
    parser.add_argument("--output",type=Path,default=profile.ROOT/"target/fastpq-production-validation/compact-bundle-profile-certificate.json")
    args=parser.parse_args()
    if any(not 1<=count<=128 for count in args.segments):
        parser.error("each segment count must be in 1..=128")
    before=profile.source_hashes()
    contracts=profile.check_source_contracts()
    provenance=profile.source_snapshot_controls()
    controls()
    results=[calculate(count) for count in args.segments]
    after=profile.assert_source_snapshot(before)
    report={"status":"pass","qualification":False,"query_candidates":401,
        "query_tape_bytes":3216,"honest_attempt_envelope":profile.HONEST_ATTEMPTS,
        "assumptions":[
            "ideal field-product oracle and injectively framed complete adaptive contexts",
            "a false accepted bundle implies a false child in the admissible family",
            "one shared 2^32 binary-query adversary budget and 54 external targets",
            "every physical H and G-block query for every child is charged",
            "two group queries jointly cover block selection and canonical binary encoding",
            "additional outer oracle calls, retries, authority and concrete errors require their own accounting"],
        "source_contracts":contracts,"source_snapshot_controls":provenance,"source_before_sha256":before,"source_after_sha256":after,
        "checker_sha256":sha256(Path(__file__).read_bytes()).hexdigest(),
        "reports":results,"production_qualification":False,
        "production_byte_caps_changed":False,"proof_or_hardware_test":False}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"status":"pass","qualification":False,"reports":results,"output":str(args.output)},indent=2))


if __name__=="__main__":
    main()
