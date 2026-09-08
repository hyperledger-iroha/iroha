#!/usr/bin/env python3
"""Exact conditional typed-compiler profile arithmetic; no production approval."""
from __future__ import annotations
from fractions import Fraction as F
from hashlib import sha256
from math import comb, isqrt
from pathlib import Path
import json
import argparse
import re

from check_compact_fri_bound import check_source_geometry

ROOT=Path(__file__).resolve().parents[2]
BASE=Path(__file__).resolve().parent
P=2**64-2**32+1
L=524288
PASSING=360447
TARGETS=54
# Chosen honest-abort envelope; this does not authorize production bundle limits.
MAX_BUNDLE_SEGMENTS=128
HONEST_ATTEMPTS=TARGETS*MAX_BUNDLE_SEGMENTS
ADVERSARY=2**32
TARGET=F(1,2**128)
FIELD_COUNTS=[1368,3692,8]+[4]*17

def dyadic(bits):
    return F(1,2**bits)

def certified_bits(value):
    assert value>0
    bits=value.denominator.bit_length()-value.numerator.bit_length()
    while value>=dyadic(bits):
        bits-=1
    while value<dyadic(bits+1):
        bits+=1
    assert dyadic(bits+1)<=value<dyadic(bits)
    return bits

def parents(depth,leaves):
    assert 1<=leaves<=2**depth
    return sum(min(leaves,2**level) for level in range(depth))

def counters(q):
    depths=list(range(18,1,-1))
    groups=[min(q,2**d) for d in depths]
    row_parents=parents(19,2*q)
    scalar_parents=parents(19,q)
    fri_parents=sum(parents(d,m) for d,m in zip(depths,groups))
    parent_total=row_parents+2*scalar_parents+fri_parents+1
    leaf_total=4*q+sum(groups)+1
    h=leaf_total+parent_total+21
    return {
        "row_leaves":2*q,"mixed_and_quotient_leaves":2*q,
        "fri_group_leaves":sum(groups),"terminal_leaves":1,
        "row_parents":row_parents,"mixed_parents":scalar_parents,
        "quotient_parents":scalar_parents,"fri_parents":fri_parents,
        "terminal_parents":1,"parent_total":parent_total,
        "leaf_total":leaf_total,"chain_H_calls":21,
        "H_calls":h,"G_calls":22,"verifier_calls":h+22,
        "FRI_group_counts":groups,
    }

def field_abort():
    rejection=F(2**32-1,2**64)
    return sum(F(comb(k+6,7))*rejection**7 for k in FIELD_COUNTS)

def query_abort(q,candidates):
    # Non-new indicators are zero after completion; before completion their
    # conditional probability is at most (q-1)/L.
    return F(comb(candidates,q-1))*F(q-1,L)**(candidates-q+1)

def tapes(q):
    candidates=q
    fa=field_abort()
    while HONEST_ATTEMPTS*(fa+query_abort(q,candidates))>=TARGET:
        candidates+=1
    sizes=[384,64*1374,64*3698,64*14]+[640]*17+[8*((19*candidates+7)//8)]
    assert len(sizes)==22
    assert sizes[0]>0
    assert all(s%8==0 for s in sizes)
    return candidates,sizes,fa,query_abort(q,candidates)

def errors(q):
    values=[F(0),F(4019707974324,P**4),F(1,P**4),F(8818589718,P**4)]
    values += [(F(134561,16)*(L//2**(i+1))+11)/P**4 for i in range(16)]
    values += [F(6,P**4),F(comb(PASSING,q),comb(L,q))]
    assert len(values)==22
    assert max(values[:-1])==F(4019707974324,P**4)
    return values

def aggregate_parts(q):
    work=counters(q)
    c,sizes,fa,qa=tapes(q)
    T=2*(ADVERSARY+work["verifier_calls"])
    candidates=[F(3*(T-1),P**6)]
    candidates += [e+F(T-1,2**bits) for e,bits in zip(errors(q),sizes)]
    delta=max(candidates)
    A=6*T*T*delta
    B=2*(F(work["H_calls"],P**6)+sum(F(1,2**bits) for bits in sizes))
    z=TARGET/TARGETS-A-B
    passes=z>0 and z*z>4*A*B
    # Rational enclosure of the square root: isqrt uses integers only.
    precision=384
    product=A*B
    floor=isqrt((product.numerator<<(2*precision))//product.denominator)
    rootlo=F(floor,2**precision)
    roothi=rootlo+dyadic(precision)
    assert rootlo*rootlo<=product<roothi*roothi
    lo=TARGETS*(A+B+2*rootlo)/TARGET
    hi=TARGETS*(A+B+2*roothi)/TARGET
    low_integer=lo.numerator*1024//lo.denominator
    high_integer=(hi.numerator*1024+hi.denominator-1)//hi.denominator
    assert F(low_integer,1024)<=lo<=hi<=F(high_integer,1024)
    return {
        "query_positions":q,"passes_strict_54_target_bound":passes,
        "work":work,"group_query_budget":T,
        "tape_bits":sizes,"query_raw_candidates":c,
        "total_G_output_bytes":sum(sizes)//8,
        "chain_bound_raw_tape_bytes":sum(sizes[:-1])//8,
        "aggregate_times_2_to_128_interval":{
            "lower_numerator":low_integer,"upper_numerator":high_integer,
            "denominator":1024,
        },
        "dominant_delta_entry":candidates.index(delta),
        "epsilon_query_below_2_to_minus":certified_bits(errors(q)[-1]),
        "epsilon_commit_below_2_to_minus":certified_bits(max(errors(q)[:-1])),
        "H_collision_attachment_below_2_to_minus":certified_bits(candidates[0]),
        "comparison_B_below_2_to_minus":certified_bits(B),
        "field_abort_below_2_to_minus":certified_bits(fa),
        "query_abort_below_2_to_minus":certified_bits(qa),
        "whole_abort_below_2_to_minus":certified_bits(fa+qa),
        "targets54_abort_below_2_to_minus":certified_bits(TARGETS*(fa+qa)),
        "max_bundle_honest_attempts":HONEST_ATTEMPTS,
        "max_bundle_abort_below_2_to_minus":certified_bits(HONEST_ATTEMPTS*(fa+qa)),
        "SHAKE256_output_blocks_only":sum((bits//8+135)//136 for bits in sizes),
    }

def varlen(n):
    return max(1,(n.bit_length()+6)//7)

def field(n):
    return n+varlen(n)

def sequence(count,size):
    return 8+count*field(size)

def wire_size(q,loose=False):
    # Exact size formula for the current compact-length canonical SharedProof
    # fields, projected to another query count, not measured new proof bytes.
    return _wire_size(q,loose,fp4_bytes=32)

def _wire_size(q,loose,*,fp4_bytes):
    # The explicit width also supports the retained predecessor's 37-byte
    # carrier; only wire_control uses that historical encoding.
    ds=list(range(18,1,-1))
    groups=[min(q,2**d) for d in ds]
    sibling=lambda d,m:m*d if loose else parents(d,m)-m+1
    row_siblings=sibling(19,2*q)
    scalar_siblings=sibling(19,q)
    fri_siblings=[sibling(d,m) for d,m in zip(ds,groups)]
    row_size=field(4)+field(sequence(342,8))
    query_size=field(4)+2*field(fp4_bytes)
    group_size=field(4)+field(2*field(fp4_bytes))
    rounds=8+sum(field(field(sequence(m,group_size))+field(sequence(s,48)))
                 for m,s in zip(groups,fri_siblings))
    sizes=[48,48,48,sequence(18,48),sequence(2*q,row_size),sequence(q,query_size),
           sequence(row_siblings,48),sequence(scalar_siblings,48),
           sequence(scalar_siblings,48),rounds,sequence(4,fp4_bytes)]
    return {"frame_bytes":40+sum(map(field,sizes)),
            "row_sibling_digests":row_siblings,
            "mixed_sibling_digests":scalar_siblings,
            "quotient_sibling_digests":scalar_siblings,
            "FRI_sibling_digests":sum(fri_siblings),
            "mode":"loose preflight shape" if loose else "valid maximal-byte shape; sibling counts are not independent frontier maxima"}

def framed_tree_monotonicity_controls():
    # More opened groups can reduce the sibling count; maximize combined
    # canonical framed values+frontier bytes, not the frontier alone.
    cases=0
    for depth in range(2,19):
        previous=0
        for leaves in range(1,min(512,2**depth)+1):
            sibling_count=parents(depth,leaves)-leaves+1
            combined=field(sequence(leaves,72))+field(sequence(sibling_count,48))
            assert combined>previous
            previous=combined;cases+=1
    previous=0
    for leaves in range(1,1025):
        sibling_count=parents(19,leaves)-leaves+1
        row_size=field(4)+field(sequence(342,8))
        combined=field(sequence(leaves,row_size))+field(sequence(sibling_count,48))
        assert combined>previous
        previous=combined;cases+=1
    return cases

def read_varint(data,offset):
    value=shift=0
    while True:
        byte=data[offset];offset+=1
        value|=(byte&127)<<shift
        if byte<128:return value,offset
        shift+=7

def parse_fields(data):
    out=[];offset=0
    while offset<len(data):
        size,start=read_varint(data,offset)
        end=start+size
        assert end<=len(data)
        out.append(data[start:end]);offset=end
    assert offset==len(data)
    return out

def wire_control(path):
    expected="72838ce11648e25f4c8b3e7496651d2e4eb63efdab08154d8cc5fd8ef0a574e7"
    # Independent Rust loose-shape controls for the current canonical carrier
    # and the retained predecessor are distinct encoding checks.
    assert wire_size(375,True)["frame_bytes"]==6713525
    assert _wire_size(136,True,fp4_bytes=37)["frame_bytes"]==2534462
    controls={"current_fp4_bytes":32,"current_Rust_loose_shape_bytes":6713525,
              "retained_fp4_bytes":37,"prior_Rust_loose_shape_bytes":2534462}
    if not path.exists():
        return {"retained_proof_present":False,"expected_public_artifact_sha256":expected,
                **controls}
    with path.open("rb") as stream:
        data=stream.read(1608632)
    assert len(data)==1608631, "retained 136-query proof has an unexpected byte length"
    assert sha256(data).hexdigest()=="72838ce11648e25f4c8b3e7496651d2e4eb63efdab08154d8cc5fd8ef0a574e7"
    assert data[:4]==b"NRT0" and data[39]==2
    fs=parse_fields(data[40:])
    assert len(fs)==11
    assert sum(field(len(f)) for f in fs)+40==len(data)==1608631
    rows=parse_fields(fs[4][8:])
    queries=parse_fields(fs[5][8:])
    assert len(rows)==272 and all(len(x)==3093 for x in rows)
    assert len(queries)==136 and all(len(x)==81 for x in queries)
    roots=parse_fields(fs[3][8:])
    assert len(roots)==18 and all(len(x)==48 for x in roots)
    for r in parse_fields(fs[9][8:]):
        rf=parse_fields(r)
        assert len(rf)==2
        assert all(len(g)==82 for g in parse_fields(rf[0][8:]))
    assert all(len(x)==37 for x in parse_fields(fs[10][8:]))
    return {"retained_proof_present":True,"public_artifact_sha256":sha256(data).hexdigest(),
            "measured_frame_bytes":len(data),**controls}

def parent_controls():
    # Enumerate all leaf subsets in small binary trees, independently count
    # ancestors and sibling frontiers, and check the per-tree maxima formula.
    cases=0
    for depth in range(1,5):
        leaf_count=2**depth
        actual_max=[0]*(leaf_count+1)
        for mask in range(1,1<<leaf_count):
            current={i for i in range(leaf_count) if mask&(1<<i)}
            leaves=len(current);siblings=total=0
            for _ in range(depth):
                siblings+=sum((i^1) not in current for i in current)
                current={i//2 for i in current};total+=len(current)
            assert siblings==total-leaves+1
            actual_max[leaves]=max(actual_max[leaves],total);cases+=1
        assert all(actual_max[m]==parents(depth,m) for m in range(1,leaf_count+1))
    return cases

SOURCE_PATHS = [
    "specs/fastpq_compact_round_by_round.md",
    "specs/fastpq_compact_air_bound.md",
    "specs/fastpq_compact_air_degree_ledger.md",
    "specs/fastpq_compact_typed_compiler.md",
    "specs/fastpq_compact_typed_profile.md",
    "crates/fastpq_isi/src/params.rs",
    "crates/fastpq_isi/src/poseidon_digest384.rs",
    "crates/fastpq_prover/src/backend/compact_protocol.rs",
    "crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs",
    "crates/fastpq_prover/src/backend/merkle_multiproof.rs",
    "crates/fastpq_prover/src/backend/compact_protocol/shared_openings/codec.rs",
    "crates/fastpq_prover/src/backend/compact_transfer_air.rs",
    "crates/fastpq_prover/src/field.rs",
    "crates/fastpq_prover/src/backend/compact_shake_candidate.rs",
    "crates/iroha_data_model/src/privacy.rs",
    "crates/norito/src/core.rs",
    "crates/norito/src/lib.rs",
]


def source_hashes():
    """Record the complete implementation context; hashes are provenance only."""
    return {p:sha256((ROOT/p).read_bytes()).hexdigest() for p in SOURCE_PATHS}


def check_source_contracts():
    """Fail on changed geometry, digest widths or wire field/layout assumptions."""
    check_source_geometry(ROOT)
    shared=(ROOT/"crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs").read_text()
    expected={
        "SharedProof":"row_root: WireDigest,mixed_root: WireDigest,quotient_root: WireDigest,fri_roots: Vec<WireDigest>,rows: Vec<SharedRow>,queries: Vec<SharedQuery>,row_siblings: Vec<WireDigest>,mixed_siblings: Vec<WireDigest>,quotient_siblings: Vec<WireDigest>,rounds: Vec<SharedRound>,terminal_values: Vec<GoldilocksFp4V1>,",
        "SharedRow":"index: u32,values: Vec<u64>,",
        "SharedQuery":"index: u32,mixed: GoldilocksFp4V1,quotient: GoldilocksFp4V1,",
        "SharedRound":"groups: Vec<SharedGroup>,siblings: Vec<WireDigest>,",
        "SharedGroup":"index: u32,values: [GoldilocksFp4V1; 2],",
    }
    for name,fields in expected.items():
        found=re.search(r"\bstruct\s+"+name+r"\s*\{([^}]*)\}",shared,re.S)
        assert found, name
        assert re.sub(r"\s+","",found.group(1))==re.sub(r"\s+","",fields), name
    assert 'schema_name = "fastpq_prover::compact_prototype::SharedProofV1"' in shared
    protocol=(ROOT/"crates/fastpq_prover/src/backend/compact_protocol.rs").read_text()
    assert "use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;" in protocol
    field_source=(ROOT/"crates/fastpq_prover/src/field.rs").read_text()
    assert re.search(r"struct GoldilocksFp4V1\s*\{\s*coefficients: \[u64; 4\],\s*\}",field_source)
    assert re.search(r"impl GoldilocksFp4V1\s*\{\s*///[^\n]*\n\s*pub const BYTES: usize = 32;",field_source)
    assert re.search(r"impl SerializePayload for GoldilocksFp4V1\s*\{\s*fn serialize\([^\n]*\) -> Result<\(\), norito::Error>\s*\{\s*writer.write_all\(&self.to_le_bytes\(\)\)\?;\s*Ok\(\(\)\)\s*\}",field_source)
    assert "GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001" in field_source
    digest=(ROOT/"crates/fastpq_isi/src/poseidon_digest384.rs").read_text()
    assert "GOLDILOCKS_DIGEST384_LANES_V1: usize = 6;" in digest
    assert "GOLDILOCKS_DIGEST384_BYTES_V1: usize = GOLDILOCKS_DIGEST384_LANES_V1 * 8;" in digest
    codec=(ROOT/"crates/norito/src/core.rs").read_text()
    assert re.search(r"fn default_encode_flags\(\) -> u8\s*\{\s*header_flags::COMPACT_LEN\s*\}",codec)
    assert "pub const SIZE: usize = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1;" in codec
    air=(ROOT/"crates/fastpq_prover/src/backend/compact_transfer_air.rs").read_text()
    assert "COLUMN_COUNT != 342" in air and "CONSTRAINT_COUNT != 923" in air
    sampler=(ROOT/"crates/fastpq_prover/src/backend/compact_shake_candidate.rs").read_text()
    assert 'h16:g375:c401:342cols' in sampler
    assert re.search(r"const QUERY_COUNT: usize = 375;", sampler)
    assert re.search(r"const QUERY_CANDIDATES: usize = 401;", sampler)
    assert re.search(r"const QUERY_LABEL_BITS: usize = 19;", sampler)
    assert '(QUERY_CANDIDATES * QUERY_LABEL_BITS).div_ceil(8)' in sampler
    return {"kind":"explicit structural and geometry guards, not semantic source equivalence",
            "shared_schema":"fastpq_prover::compact_prototype::SharedProofV1",
            "digest_bytes":48,"fp4_bytes":32,"row_columns":342,"AIR_constraints":923,
            "canonical_header_bytes":40,"canonical_layout":"COMPACT_LEN"}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-typed-profile-certificate.json")
    parser.add_argument("--retained-proof",type=Path,default=ROOT/"target/fastpq-production-validation/compact-shared-transfer-72838ce11648e25f4c8b3e7496651d2e4eb63efdab08154d8cc5fd8ef0a574e7.bin",
                        help="Optional exact retained 136-query proof; missing files are reported, present files are checked")
    args=parser.parse_args()
    contracts=check_source_contracts()
    results=[aggregate_parts(q) for q in [136,237,374,375,376,396]]
    assert not results[2]["passes_strict_54_target_bound"]
    assert results[3]["passes_strict_54_target_bound"]
    # For every q<=374, the displayed compiler bound itself is at least this
    # unavoidable query term, since T>=2*ADVERSARY and the ratio decreases in q.
    lower374=TARGETS*6*(2*ADVERSARY)**2*errors(374)[-1]
    assert lower374>TARGET
    candidate=results[3]
    assert candidate["query_raw_candidates"]==401
    assert TARGETS*(field_abort()+query_abort(375,399))>=TARGET
    # Retain the prior 54-single-attempt control; the selected envelope is larger.
    assert TARGETS*(field_abort()+query_abort(375,400))<TARGET
    assert HONEST_ATTEMPTS*(field_abort()+query_abort(375,400))>=TARGET
    assert HONEST_ATTEMPTS*(field_abort()+query_abort(375,401))<TARGET
    assert all(HONEST_ATTEMPTS*(field_abort()+query_abort(375,c))>=TARGET for c in range(375,401))
    assert candidate["group_query_budget"]==8590023760
    assert candidate["work"]["H_calls"]==44562
    assert candidate["total_G_output_bytes"]==43049
    assert candidate["SHAKE256_output_blocks_only"]==326
    report={
        "scope":"conditional ideal theorem arithmetic; no production profile approval",
        "status":"pass","targets":TARGETS,"binary_adversary_queries":ADVERSARY,
        "honest_abort_max_bundle_segments":MAX_BUNDLE_SEGMENTS,
        "honest_attempts_envelope":HONEST_ATTEMPTS,
        "query_candidate_count_for_chosen_envelope":401,
        "candidate400_passes_single_attempt_union_but_fails_max_bundle_envelope":True,
        "goal_bound":"strictly below 2^-128 after union over 54 targets",
        "smallest_certified_initial_position_count":375,
        "all_counts_at_most_374_fail_the_displayed_bound":True,
        "candidate_comparisons":results,
        "candidate_valid_shared_wire_upper":wire_size(375),
        "candidate_loose_preflight_wire_upper":wire_size(375,True),
        "wire_formula_controls":wire_control(args.retained_proof),
        "source_contracts":contracts,
        "exact_framed_tree_monotonicity_cases":framed_tree_monotonicity_controls(),
        "exhaustive_tree_subset_controls":parent_controls(),
        "source_sha256":source_hashes(),
        "checker_sha256":sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    path=args.output
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"status":"pass","smallest_conditional_q":375,
                      "H_calls":candidate["work"]["H_calls"],
                      "G_calls":22,"group_budget":candidate["group_query_budget"],
                      "aggregate_scaled_interval":candidate["aggregate_times_2_to_128_interval"],
                      "wire_upper":wire_size(375)["frame_bytes"],
                      "output":str(path)},indent=2))

if __name__=="__main__":
    if not __debug__:
        raise SystemExit("Exact certificate requires assertions; remove -O.")
    main()
