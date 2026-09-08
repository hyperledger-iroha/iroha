#!/usr/bin/env python3
"""Exact controls for the projected raw-XOF H lemma; not hash qualification."""
from __future__ import annotations
from collections import Counter
from fractions import Fraction as F
from hashlib import sha256
from itertools import product
from math import comb,isqrt
from pathlib import Path
import argparse
import check_compact_typed_profile as B
import json

BASE=Path(__file__).resolve().parent
ROOT=Path(__file__).resolve().parents[2]

def eta_exact(count):
    r=F(2**32-1,2**64)
    return sum(F(comb(count,a))*(1-r)**a*r**(count-a) for a in range(6))

def eta_upper(count):
    return F(comb(count,count-5))*F(2**32-1,2**64)**(count-5)

def sqrt_certified_bits(square):
    bits=0
    while square<F(1,2**(2*(bits+1))):
        bits+=1
    assert square<F(1,2**(2*bits))
    assert square>=F(1,2**(2*(bits+1)))
    return bits

def projection_controls():
    reports=[]
    for bits,p,k,c in [(2,3,2,3),(3,5,2,4),(3,5,3,5)]:
        counts=Counter()
        space=2**bits
        for raw in product(range(space),repeat=c):
            accepted=tuple(x for x in raw if x<p)
            output=accepted[:k] if len(accepted)>=k else None
            counts[output]+=1
        successful=[n for y,n in counts.items() if y is not None]
        assert len(successful)==p**k and len(set(successful))==1
        total=space**c
        r=F(space-p,space)
        eta=sum(F(comb(c,a))*(1-r)**a*r**(c-a) for a in range(k))
        assert F(counts[None],total)==eta
        assert F(successful[0],total)==(1-eta)/p**k
        assert eta<=F(comb(c,c-k+1))*r**(c-k+1)
        # Any root-pointer subset has exactly its projected density; include
        # every subset cardinality without choosing only one lucky root.
        for size in range(p**k+1):
            assert F(size*successful[0],total)==(1-eta)*F(size,p**k)
        reports.append({"word_bits":bits,"field_size":p,"coordinates":k,
                        "candidate_words":c,"enumerated_tapes":total,
                        "abort_tapes":counts[None],"preimages_per_root":successful[0]})
    return reports

def historical_binary_tapes(q):
    """Original theorem's binary-XOF model only; no current protocol selector."""
    rejection=F(2**32-1,2**64)
    field_abort=sum(F(comb(k+6,7))*rejection**7 for k in B.FIELD_COUNTS)
    candidates=q
    while B.HONEST_ATTEMPTS*(field_abort+F(comb(candidates,q-1))*F(q-1,B.L)**(candidates-q+1))>=B.TARGET:
        candidates+=1
    sizes=[384,64*1374,64*3698,64*14]+[640]*17+[8*((19*candidates+7)//8)]
    assert len(sizes)==22
    return sizes


def historical_xof_work(q):
    """Count whole binary-XOF calls in this original model, not field blocks."""
    work=B.counters(q)
    return {**work,"whole_G_messages":22,"raw_XOF_calls":work["H_calls"]+22}


def raw_bound(q,overhead,h_count=12):
    work=historical_xof_work(q)
    sizes=historical_binary_tapes(q)
    Q=B.ADVERSARY+work["raw_XOF_calls"]
    T=overhead*Q
    delta=max(F(3*(T-1),B.P**6),
              max(e+F(T-1,2**z) for e,z in zip(B.errors(q),sizes)))
    a=6*T*T*delta
    b=2*(F(work["H_calls"],2**(64*h_count))+sum(F(1,2**z) for z in sizes))
    target=B.TARGET/B.TARGETS
    z=target-a-b
    good=z>0 and z*z>4*a*b
    product_ab=a*b
    rootfloor=isqrt((product_ab.numerator<<768)//product_ab.denominator)
    rootlo=F(rootfloor,2**384)
    roothi=rootlo+F(1,2**384)
    assert rootlo*rootlo<=product_ab<roothi*roothi
    lo=54*(a+b+2*rootlo)/B.TARGET
    hi=54*(a+b+2*roothi)/B.TARGET
    return {"q":q,"binary_to_group_factor":overhead,"T":T,
            "H_tape_bits":64*h_count,"passes":good,
            "scaled_bound_interval":{
                "lower":lo.numerator*1024//lo.denominator,
                "upper":(hi.numerator*1024+hi.denominator-1)//hi.denominator,
                "denominator":1024},
            "verifier_work":work,"wire_upper":B.wire_size(q)["frame_bytes"]}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-projected-xof-certificate.json")
    args=parser.parse_args()
    before=B.source_hashes()
    provenance=B.source_snapshot_controls()
    contracts=B.check_source_contracts()
    assert historical_binary_tapes(375)[-1]==953*8
    assert sum(historical_binary_tapes(375))//8==43049
    Q=2**32+historical_xof_work(375)["raw_XOF_calls"]
    full_prover_H=3*(2*B.L-1)+sum(2*2**d-1 for d in range(2,19))+2+21
    assert full_prover_H==4194299
    rows=[]
    for count in [12,14,16,18]:
        exact=eta_exact(count);upper=eta_upper(count)
        assert 0<exact<=upper
        coupling_square=(54*2*Q)**2*upper
        abort_search=54*(2*Q+1)**2*upper
        honest_H_abort=54*full_prover_H*upper
        rows.append({
            "candidates":count,"raw_bits":64*count,"raw_bytes":8*count,
            "eta_H_upper_below_2_to_minus":B.certified_bits(upper),
            "interface_only_coupling54_below_2_to_minus":sqrt_certified_bits(coupling_square),
            "raw_abort_search54_below_2_to_minus":B.certified_bits(abort_search),
            "honest_prover_H_abort54_below_2_to_minus":B.certified_bits(honest_H_abort),
            "raw_SHAKE256_output_blocks":(8*count+135)//136,
            "exact_eta_le_union_upper":True,
        })
    scenarios=[raw_bound(370,1),raw_bound(371,1),raw_bound(374,2),raw_bound(375,2),raw_bound(375,1,16)]
    assert [x["passes"] for x in scenarios]==[False,True,False,True,True]
    assert 54*6*(2**32)**2*B.errors(370)[-1]>B.TARGET
    toy_controls=projection_controls()
    hashes=B.assert_source_snapshot(before)
    report={
        "scope":"Historical projected ideal raw-XOF theorem controls; no current H/G selector or concrete qualification",
        "status":"pass","H_candidate_comparisons":rows,
        "oracle_interface_only_hybrid_is_not_a_raw_XOF_security_reduction":True,
        "projected_lemma_additive_abort_soundness_error":0,
        "concrete_SHAKE_instantiation_error":"unquantified; requires explicit joint assumption/review",
        "honest_full_prover_H_calls_projection":full_prover_H,
        "raw_binary_query_scenarios":scenarios,
        "toy_uniform_projection_controls":toy_controls,
        "sources":hashes,"source_contracts":contracts,
        "source_before_sha256":before,"source_after_sha256":hashes,
        "source_snapshot_controls":provenance,
        "checker_sha256":sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    output=args.output
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"status":"pass","H_candidate_comparisons":rows,
                      "raw_one_query_minimum":371,"conservative_two_query_minimum":375,
                      "output":str(output)},indent=2))

if __name__=="__main__":
    if not __debug__:
        raise SystemExit("Exact projected-XOF controls require assertions.")
    main()
