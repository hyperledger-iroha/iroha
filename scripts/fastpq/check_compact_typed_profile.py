#!/usr/bin/env python3
"""Exact conditional six-lane field-block profile; no production qualification."""
from __future__ import annotations
from fractions import Fraction as F
from hashlib import sha256
from math import comb, isqrt
from itertools import product
from collections import Counter
from pathlib import Path
import json
import argparse
import re
import shutil
from tempfile import TemporaryDirectory

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
FIXED_BLOCKS=(1,228,616,2)+(1,)*17+(67,)

if not __debug__:
    raise SystemExit("Exact field-block certificate requires assertions; remove -O or -OO.")

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
        "H_calls":h,"whole_G_messages":22,
        "FRI_group_counts":groups,
    }

def query_abort(q,candidates):
    """Bound failure under iid canonical F_p coordinates; not uniform bytes."""
    if type(q) is not int or not 1<=q<=512:
        raise ValueError("queries must be an integer in 1..=512")
    if type(candidates) is not int or candidates<q:
        raise ValueError("candidate count must be an integer at least q")
    k,remainder=divmod(P,L)
    assert remainder==1
    # Before completion, at most q-1 labels are already present, each with k
    # field preimages; the one rejected canonical residue also fails to add.
    nonnew=F(remainder+(q-1)*k,P)
    return F(comb(candidates,q-1))*nonnew**(candidates-q+1)


def field_tape_schedule(q):
    """Fixed native field groups; other q values are arithmetic comparisons only."""
    candidates=q
    while HONEST_ATTEMPTS*query_abort(q,candidates)>=TARGET:
        candidates+=1
    blocks=(1,)+(tuple((k+5)//6 for k in FIELD_COUNTS))+((candidates+5)//6,)
    assert len(blocks)==22 and all(b>0 for b in blocks)
    return {"query_candidates":candidates,"blocks":blocks,
            "output_bytes":tuple(48*b for b in blocks),
            "group_orders":tuple(P**(6*b) for b in blocks),
            "query_abort":query_abort(q,candidates)}


def errors(q):
    values=[F(0),F(4019707974324,P**4),F(1,P**4),F(8818589718,P**4)]
    values += [(F(134561,16)*(L//2**(i+1))+11)/P**4 for i in range(16)]
    values += [F(6,P**4),F(comb(PASSING,q),comb(L,q))]
    assert len(values)==22
    assert max(values[:-1])==F(4019707974324,P**4)
    return values

def aggregate_parts(q,segments=1):
    """Exact conditional bound with two group queries per physical digest call."""
    if type(segments) is not int or not 1<=segments<=MAX_BUNDLE_SEGMENTS:
        raise ValueError("segments must be an integer in 1..=128")
    schedule=field_tape_schedule(q)
    work=counters(q)
    work["G_blocks"]=sum(schedule["blocks"])
    work["verifier_digest_calls"]=work["H_calls"]+work["G_blocks"]
    T=2*(ADVERSARY+segments*work["verifier_digest_calls"])
    entries=[F(3*(T-1),P**6)]
    entries += [e+F(T-1,order) for e,order in zip(errors(q),schedule["group_orders"])]
    delta=max(entries)
    A=6*T*T*delta
    B=2*segments*(F(work["H_calls"],P**6)+sum(F(1,order) for order in schedule["group_orders"]))
    z=TARGET/TARGETS-A-B
    passes=z>0 and z*z>4*A*B
    precision=384
    product_ab=A*B
    floor=isqrt((product_ab.numerator<<(2*precision))//product_ab.denominator)
    rootlo=F(floor,2**precision)
    roothi=rootlo+dyadic(precision)
    assert rootlo*rootlo<=product_ab<roothi*roothi
    lo=TARGETS*(A+B+2*rootlo)/TARGET
    hi=TARGETS*(A+B+2*roothi)/TARGET
    low_integer=lo.numerator*1024//lo.denominator
    high_integer=(hi.numerator*1024+hi.denominator-1)//hi.denominator
    assert F(low_integer,1024)<=lo<=hi<=F(high_integer,1024)
    abort=schedule["query_abort"]
    return {
        "query_positions":q,"segments":segments,
        "passes_strict_54_target_bound":passes,"work_per_segment":work,
        "group_query_budget":T,"blocks_per_message":schedule["blocks"],
        "group_order_p_exponents":[6*b for b in schedule["blocks"]],
        "query_raw_candidates":schedule["query_candidates"],
        "total_G_output_bytes":sum(schedule["output_bytes"]),
        "chain_bound_raw_tape_bytes":sum(schedule["output_bytes"][:-1]),
        "aggregate_times_2_to_128_interval":{
            "lower_numerator":low_integer,"upper_numerator":high_integer,"denominator":1024},
        "dominant_delta_entry":entries.index(delta),
        "epsilon_query_below_2_to_minus":certified_bits(errors(q)[-1]),
        "epsilon_commit_below_2_to_minus":certified_bits(max(errors(q)[:-1])),
        "H_collision_attachment_below_2_to_minus":certified_bits(entries[0]),
        "comparison_B_below_2_to_minus":certified_bits(B),
        "field_coefficient_abort_exact_zero_in_ideal_model":True,
        "query_abort_below_2_to_minus":certified_bits(abort),
        "total_honest_attempts":TARGETS*segments,
        "honest_abort_below_2_to_minus":certified_bits(TARGETS*segments*abort),
        "honest_abort_below_2_to_minus_128":TARGETS*segments*abort<TARGET,
        "raw_row_and_query_value_lower_bytes":q*(342*8+64),
    }


def varlen(n):
    return max(1,(n.bit_length()+6)//7)

def field(n):
    return n+varlen(n)

def sequence(count,size):
    return 8+count*field(size)

def wire_size(q,loose=False):
    """Exact canonical DTO shape formula; no valid proof or runtime claim."""
    fp4_bytes=32
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

# All entry points use this one inventory, including their executable checker
# owners, imported geometry helper and the adaptive-context theorem premise.
CHECKER_SPEC_PATHS = (
    "scripts/fastpq/check_compact_typed_profile.py",
    "scripts/fastpq/check_compact_bundle_profile.py",
    "scripts/fastpq/check_compact_projected_xof.py",
    "scripts/fastpq/check_compact_fri_bound.py",
    "specs/fastpq_compact_adaptive_context.md",
    "specs/fastpq_compact_projected_xof.md",
)
SOURCE_PATHS = list(CHECKER_SPEC_PATHS) + [
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
    "crates/fastpq_prover/src/backend/compact_v1.rs",
    "crates/fastpq_prover/src/backend/compact_protocol/profile.rs",
    "crates/fastpq_isi/src/poseidon_digest384_prefix.rs",
    "specs/fastpq_compact_v1_framing.md",
    "specs/fastpq_compact_protocol_contract.md",
    "crates/iroha_crypto/Cargo.toml",
    "crates/iroha_data_model/src/privacy.rs",
    "crates/norito/src/core.rs",
    "crates/norito/src/lib.rs",
]


def source_hashes(root=None):
    """Record the explicit input inventory; hashes are provenance only."""
    root=ROOT if root is None else root
    assert len(SOURCE_PATHS)==len(set(SOURCE_PATHS)), "duplicate source inventory entry"
    return {p:sha256((root/p).read_bytes()).hexdigest() for p in SOURCE_PATHS}


def assert_source_snapshot(before, root=None):
    """Reject changed or omitted declared inputs across a checker execution."""
    after=source_hashes(root)
    assert before==after, "source changed during check"
    return after


def source_snapshot_controls():
    """Actually mutate copied inputs to exercise before/after provenance checks."""
    reports=[]
    with TemporaryDirectory(prefix="fastpq-checker-inputs-") as directory:
        root=Path(directory)
        for relative in SOURCE_PATHS:
            destination=root/relative
            destination.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(ROOT/relative,destination)
        before=source_hashes(root)
        assert_source_snapshot(before,root)
        for relative in CHECKER_SPEC_PATHS:
            path=root/relative
            original=path.read_bytes()
            path.write_bytes(original+b"\n# private changed-input control\n")
            try:
                assert_source_snapshot(before,root)
            except AssertionError:
                reports.append({"path":relative,"changed_file_rejected":True})
            else:
                raise AssertionError(f"changed declared input accepted: {relative}")
            finally:
                path.write_bytes(original)
            assert_source_snapshot(before,root)
        omitted=dict(before)
        del omitted[CHECKER_SPEC_PATHS[0]]
        try:
            assert_source_snapshot(omitted,root)
        except AssertionError:
            reports.append({"control":"omitted declared checker","rejected":True})
        else:
            raise AssertionError("incomplete source inventory accepted")
    return reports


def check_source_contracts(overrides=None):
    """Fail on changed geometry, digest widths or wire field/layout assumptions."""
    check_source_geometry(ROOT)
    overrides={} if overrides is None else overrides
    def read(relative):
        return overrides[relative] if relative in overrides else (ROOT/relative).read_text()
    shared=read("crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs")
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
    assert 'frame = "fastpq_prover::compact_v1::SharedProofV1"' in shared
    protocol=read("crates/fastpq_prover/src/backend/compact_protocol.rs")
    assert "use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;" in protocol
    field_source=read("crates/fastpq_prover/src/field.rs")
    assert re.search(r"struct GoldilocksFp4V1\s*\{\s*coefficients: \[u64; 4\],\s*\}",field_source)
    assert re.search(r"impl GoldilocksFp4V1\s*\{\s*///[^\n]*\n\s*pub const BYTES: usize = 32;",field_source)
    assert re.search(r"impl SerializePayload for GoldilocksFp4V1\s*\{\s*fn serialize\([^\n]*\) -> Result<\(\), norito::Error>\s*\{\s*writer.write_all\(&self.to_le_bytes\(\)\)\?;\s*Ok\(\(\)\)\s*\}",field_source)
    assert "GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001" in field_source
    digest=read("crates/fastpq_isi/src/poseidon_digest384.rs")
    assert "GOLDILOCKS_DIGEST384_LANES_V1: usize = 6;" in digest
    assert "GOLDILOCKS_DIGEST384_BYTES_V1: usize = GOLDILOCKS_DIGEST384_LANES_V1 * 8;" in digest
    codec=read("crates/norito/src/core.rs")
    assert re.search(r"fn default_encode_flags\(\) -> u8\s*\{\s*header_flags::COMPACT_LEN\s*\}",codec)
    assert "pub const SIZE: usize = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1;" in codec
    air=read("crates/fastpq_prover/src/backend/compact_transfer_air.rs")
    assert "COLUMN_COUNT != 342" in air and "CONSTRAINT_COUNT != 923" in air
    sampler=read("crates/fastpq_prover/src/backend/compact_v1.rs")
    compact=sampler.split("#[cfg(test)]\nmod tests",1)[0]
    normal=lambda value:re.sub(r"\s+","",re.sub(r"//[^\n]*","",value))
    flat=normal(compact)
    binding=read("crates/fastpq_prover/src/backend/compact_protocol/profile.rs")
    binding_flat=normal(binding)
    for fragment in [
        'const QUERY_COUNT: usize = 375;',
        'const QUERY_CANDIDATES: usize = 401;',
        'const QUERY_TAPE_BYTES: usize = QUERY_CANDIDATES.div_ceil(6) * 48;',
        'const MAX_CONTEXT_BYTES: usize = 256 * 1024;',
        'const H_CACHE_ROUNDS: usize = 22;',
        'const H_CACHE_LEVELS: usize = 20;',
        'const G_CACHE_ROUNDS: usize = 22;',
        'protocol: Arc::from(FASTPQ_FINAL_V1.name.as_bytes()),',
        'catalog: Arc::from(FASTPQ_CATALOG_V1.as_bytes()),',
        'profile: self.prefix.encoded.clone(),',
        'index: 0, counter: u64::from(round),',
        '.hash_at(index, &[body])',
        'for (block, target) in output.chunks_exact_mut(48).enumerate()',
        'b"compact-transcript", b"whole-field-tape-block", round.0, 0, block as u64, body,',
        'b"compact-commitment", b"typed-h", frame.round, frame.level, u64::from(frame.position), &encoded,',
        '1 => 48, 2 => 10_944, 3 => 29_568, 4 => 96, 5..=21 => 48, 22 => QUERY_TAPE_BYTES,',
        '2 => Some(1368), 3 => Some(3692), 4 => Some(8), 5..=21 => Some(4),',
        'let rejection_limit = MODULUS - MODULUS % u64::from(LDE_ROWS);',
        'for chunk in raw.chunks_exact(8).take(QUERY_CANDIDATES)',
        'if candidate >= rejection_limit { continue; }',
        'let value = (candidate % u64::from(LDE_ROWS)) as u32;',
        'Err(position) => indices.insert(position, value),',
        'if indices.len() == QUERY_COUNT { return Ok(Message::Queries(indices)); }',
        'let values = raw[..needed * 8].chunks_exact(32)',
        'vec![tape, root.to_le_bytes().to_vec()],',
        'predecessor: Digest::default(), phase: Phase::Ready(Round(1)),',
        'Phase::Pending { round, raw }',
        'self.phase = Phase::Aborted;',
        'if round.0 == 22 { return Err(CandidateError::Phase); }',
        '(leaves == 1 && left != right)',
    ]:
        assert normal(fragment) in flat, fragment
    assert 'goldilocks-six-lane:h6:g-field-blocks:q375:c401:342cols:923slots:65536rows:8blowup:17folds:v1' in compact
    assert not re.search(r"Shake|SHAKE|Prototype|QUERY_LABEL_BITS",compact)
    # Reject suffix-skipping canonicality before interpreting any message.
    canonical='raw.chunks_exact(8).any(|chunk|u64::from_le_bytes(chunk.try_into().expect("exactfieldword"))>=MODULUS)'
    assert canonical in flat
    assert flat.index(canonical)<flat.index('ifround.0==1{returnOk(Message::Dummy);}')
    assert flat.index(canonical)<flat.index('forchunkinraw.chunks_exact(8).take(QUERY_CANDIDATES)')
    for name,fields in {
        'PrefixFrame':'version:u16,identity:Vec<u8>,context:Vec<u8>,',
        'Frame':'kind:u8,oracle:u8,round:u8,level:u32,position:u32,output_bytes:u32,fields:Vec<Vec<u8>>,',
    }.items():
        found=re.search(r"\bstruct\s+"+name+r"\s*\{([^}]*)\}",compact,re.S)
        assert found and normal(found.group(1))==fields,name
    assert 'frame = "fastpq_prover::compact_v1::ProfileContextV1"' in compact
    assert 'frame = "fastpq_prover::compact_v1::BodyV1"' in compact
    for fragment in [
        'geometry.schema.trace_rows != 65_536', 'geometry.schema.width != 342',
        'geometry.schema.constraints != 923', 'geometry.lde_rows != 524_288',
        'FASTPQ_FINAL_V1.fri.arity != 2', 'FASTPQ_FINAL_V1.fri.blowup_factor != 8',
        'geometry.fri_lengths != (0..=17).map(|r| 524_288 >> r).collect::<Vec<_>>()',
        'geometry.terminal_degree != 1', 'compact::Context::new(&encoded)',
        'let encoded = norito::encode_canonical(&context)?;',
        'statement: relation.statement_bytes().to_vec(),',
        'context: compact::Context,',
    ]:
        assert normal(fragment) in binding_flat,fragment
    statement_fields='relation:String,trace_rows:u32,lde_rows:u32,width:u32,constraints:u32,base_modulus:u64,extension_nonresidue:u64,lde_root:u64,lde_log_size:u32,coset_offset:u64,blowup:u32,arity:u32,folds:u32,terminal_values:u32,terminal_degree:u32,queries:u32,statement:Vec<u8>,'
    found=re.search(r"struct StatementContext\s*\{([^}]*)\}",binding,re.S)
    assert found and normal(found.group(1))==statement_fields
    return {"kind":"explicit structural/ordering guards, not complete semantic source equivalence",
            "shared_schema":"fastpq_prover::compact_v1::SharedProofV1",
            "digest_alphabet":"Fp^6; serialized coordinates are not uniform bits",
            "digest_bytes":48,"fp4_bytes":32,"row_columns":342,"AIR_constraints":923,
            "trace_rows":65536,"lde_rows":524288,"folds":17,"terminal_degree":1,
            "queries":375,"query_candidates":401,"G_blocks":931,
            "canonical_header_bytes":40,"canonical_layout":"COMPACT_LEN"}




def finite_simulation():
    """Enumerate all toy tables, query registers and response-group ancillas."""
    # C=F3 is a toy group, not the concrete six-coordinate field alphabet.
    # The two H cells include one malformed/auxiliary raw input. G round one
    # has one block. Round two has two blocks and two distinct full contexts.
    routes = [
        ('H', 0, (0,), 0), ('H', 1, (1,), 0),
        ('G1', 0, (2,), 0),
        ('G2', 0, (3, 4), 0), ('G2', 0, (3, 4), 1),
        ('G2', 1, (5, 6), 0), ('G2', 1, (5, 6), 1),
    ]
    query_inputs = {(kind, context, block) for kind, context, _, block in routes}
    assert len(query_inputs) == 7
    assert {cells[block] for _, _, cells, block in routes} == set(range(7))
    tables = basis_checks = workspace_checks = 0
    tuple_tables = set()
    missing_unquery_detected = context_alias_detected = False
    for table in product(range(3), repeat=7):
        tables += 1
        grouped = (table[0], table[1], table[2], table[3:5], table[5:7])
        tuple_tables.add(grouped)
        for _, _, cells, selected in routes:
            whole = tuple(table[cell] for cell in cells)
            outputs = set()
            for ancilla in product(range(3), repeat=len(cells)):
                for response in range(4):
                    computed = tuple((a + f) % 3 for a, f in zip(ancilla, whole))
                    encoded_xor = response ^ computed[selected]
                    cleared = tuple((a - f) % 3 for a, f in zip(computed, whole))
                    # Addition followed by inverse is implemented over the
                    # entire tuple; there is no selected-block side channel.
                    assert cleared == ancilla
                    outputs.add((encoded_xor, cleared))
                    workspace_checks += 1
                    if not any(ancilla):
                        assert encoded_xor == response ^ table[cells[selected]]
                        assert not any(cleared)
                        basis_checks += 1
                        missing_unquery_detected |= any(computed)
            # A phase-free bijection on the full workspace gives a unitary
            # extension, while the zero-ancilla subspace has exact semantics.
            assert len(outputs) == 4 * 3**len(cells)
        context_alias_detected |= table[3:5] != table[5:7]
    assert tables == len(tuple_tables) == 3**7
    assert basis_checks == tables * 7 * 4
    assert missing_unquery_detected and context_alias_detected
    # A repeated address cannot simulate independent tuple coordinates.
    diagonal = {(x, x) for x in range(3)}
    assert len(diagonal) == 3 < 3**2
    return {
        'complete_random_tables': tables,
        'zero_ancilla_basis_checks': basis_checks,
        'full_workspace_permutation_checks': workspace_checks,
        'coherent_scope': 'Basis identity with no phases and cleared ancilla implies superposition identity in this finite model.',
        'negative_controls': ['missing unquery', 'omitted full context', 'aliased block address'],
        'concrete_hash_test': False,
    }



def sampler_controls():
    """Enumerate complete toy field tapes, including rejection and duplicates."""
    counts=Counter();biased=Counter();aborts=0
    for raw in product(range(5),repeat=4):
        selected=set()
        for value in raw:
            if value<4:
                selected.add(value)
            if len(selected)==2:break
        if len(selected)==2:counts[tuple(sorted(selected))]+=1
        else:aborts+=1
        selected=set()
        for value in raw:
            selected.add(value%4)
            if len(selected)==2:break
        if len(selected)==2:biased[tuple(sorted(selected))]+=1
    assert len(counts)==6 and len(set(counts.values()))==1
    assert sum(counts.values())+aborts==625
    assert len(set(biased.values()))>1, "modulo without field rejection must expose bias"
    k,remainder=divmod(P,L)
    assert remainder==1 and k*L==P-1
    exact_bound=F(comb(401,374))*F(1+374*k,P)**27
    assert exact_bound==query_abort(375,401)
    assert exact_bound<F(1,2**143)
    assert HONEST_ATTEMPTS*exact_bound<F(1,2**130)
    assert TARGETS*query_abort(375,399)>=TARGET
    assert TARGETS*query_abort(375,400)<TARGET
    assert HONEST_ATTEMPTS*query_abort(375,400)>=TARGET
    assert all(HONEST_ATTEMPTS*query_abort(375,c)>=TARGET for c in range(375,401))
    assert field_tape_schedule(375)["blocks"]==FIXED_BLOCKS
    assert sum(FIXED_BLOCKS)==931 and sum(FIXED_BLOCKS)*48==44688
    # Canonical binary strings have a different distribution from F_p^6.
    assert P**6!=2**384
    for q,c in [(0,401),(True,401),(513,520),(375,374),(375,True),(375,1.5)]:
        try:query_abort(q,c)
        except ValueError:pass
        else:raise AssertionError("invalid sampler arithmetic input accepted")
    return {"complete_toy_tapes":625,"preimages_per_successful_subset":next(iter(counts.values())),
            "aborts":aborts,"modulo_without_rejection_bias_detected":True,
            "query_abort_bound":"binom(401,374)*((1+374*((p-1)/L))/p)^27",
            "abort_per_attempt_below_2_to_minus":143,
            "abort_6912_attempts_below_2_to_minus":130,
            "uniformity_scope":"iid uniform canonical field coordinates; not concrete-hash security"}


def negative_source_controls():
    """Fail changed source assumptions using in-memory copies, never live edits."""
    context="crates/fastpq_prover/src/backend/compact_v1.rs"
    binding="crates/fastpq_prover/src/backend/compact_protocol/profile.rs"
    shared="crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs"
    mutations=[
        (context,'const QUERY_COUNT: usize = 375;','const QUERY_COUNT: usize = 374;'),
        (context,'const QUERY_CANDIDATES: usize = 401;','const QUERY_CANDIDATES: usize = 400;'),
        (context,'QUERY_CANDIDATES.div_ceil(6) * 48','(QUERY_CANDIDATES * 19).div_ceil(8)'),
        (context,'protocol: Arc::from(FASTPQ_FINAL_V1.name.as_bytes())','protocol: Arc::from(IDENTITY)'),
        (context,'profile: self.prefix.encoded.clone()','profile: Arc::from(b"context digest")'),
        (context,'counter: u64::from(round)','counter: 0'),
        (context,'.hash_at(index, &[body])','.hash_at(0, &[body])'),
        (context,'block as u64,','0,'),
        (context,'3 => Some(3692)','3 => Some(3688)'),
        (context,'if candidate >= rejection_limit','if candidate > rejection_limit'),
        (context,'raw.chunks_exact(8).take(QUERY_CANDIDATES)','raw.chunks_exact(8).take(QUERY_CANDIDATES - 1)'),
        (context,'vec![tape, root.to_le_bytes().to_vec()]','vec![root.to_le_bytes().to_vec()]'),
        (context,'(leaves == 1 && left != right)','false'),
        (binding,'geometry.schema.width != 342','geometry.schema.width != 343'),
        (binding,'geometry.terminal_degree != 1','geometry.terminal_degree != 2'),
        (binding,'statement: relation.statement_bytes().to_vec()','statement: vec![0]'),
        (shared,'fastpq_prover::compact_v1::SharedProofV1','fastpq_prover::compact_prototype::SharedProofV1'),
        (shared,'    quotient: GoldilocksFp4V1,','    quotient: u64,'),
    ]
    reports=[]
    for path,before,after in mutations:
        original=(ROOT/path).read_text()
        assert before in original,(path,before)
        changed=original.replace(before,after,1)
        try:check_source_contracts({path:changed})
        except AssertionError:reports.append({"path":path,"mutation":before,"rejected":True})
        else:raise AssertionError(f"changed source assumption was accepted: {before}")
    return reports


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-typed-profile-certificate.json")
    args=parser.parse_args()
    before=source_hashes()
    contracts=check_source_contracts()
    provenance=source_snapshot_controls()
    negatives=negative_source_controls()
    sampler=sampler_controls()
    simulation=finite_simulation()
    results=[aggregate_parts(q) for q in [136,237,374,375,376,396]]
    assert not results[2]["passes_strict_54_target_bound"]
    candidate=results[3]
    assert candidate["passes_strict_54_target_bound"]
    assert TARGETS*6*(2*ADVERSARY)**2*errors(374)[-1]>TARGET
    assert candidate["query_raw_candidates"]==401
    assert candidate["group_query_budget"]==8590025578
    assert candidate["work_per_segment"]["H_calls"]==44562
    assert candidate["work_per_segment"]["G_blocks"]==931
    assert candidate["work_per_segment"]["verifier_digest_calls"]==45493
    assert candidate["total_G_output_bytes"]==44688
    assert candidate["aggregate_times_2_to_128_interval"]=={
        "lower_numerator":743,"upper_numerator":744,"denominator":1024}
    assert wire_size(375,True)["frame_bytes"]==6713525
    tree_cases=parent_controls();monotonicity=framed_tree_monotonicity_controls()
    after=assert_source_snapshot(before)
    report={
        "scope":"Conditional ideal F_p^6 block/whole-tuple arithmetic; no production qualification",
        "status":"pass","qualification":False,"targets":TARGETS,
        "binary_adversary_queries":ADVERSARY,
        "assumptions":[
            "uniform ideal field-product function over an explicitly bounded auxiliary domain",
            "injective reversible full-context framing and disjoint H/G message images",
            "fixed per-round block counts and every-prefix AIR/FRI/context hypotheses",
            "two group queries include both coordinate selection and canonical binary encoding",
            "all physical verifier digest calls and adversary retries are charged",
            "concrete related-lane/internal-permutation errors and caller authority remain unqualified"],
        "source_mapping":"Explicit structural/order checks are not a complete semantic equivalence proof",
        "honest_attempts_envelope":HONEST_ATTEMPTS,"query_candidate_count":401,
        "smallest_certified_initial_position_count":375,
        "all_counts_at_most_374_fail_this_sufficient_bound":True,
        "candidate_comparisons":results,"sampler_controls":sampler,
        "finite_group_oracle_controls":simulation,"negative_source_controls":negatives,
        "source_snapshot_controls":provenance,
        "candidate_valid_shared_wire_upper":wire_size(375),
        "candidate_loose_preflight_wire_upper":wire_size(375,True),
        "raw_row_and_query_value_lower_bytes":1050000,
        "production_512KiB_and_AXT_1MiB_caps_fit":False,
        "proof_executed":False,"resource_or_hardware_qualification":False,
        "source_contracts":contracts,"exhaustive_tree_subset_controls":tree_cases,
        "exact_framed_tree_monotonicity_cases":monotonicity,
        "source_before_sha256":before,"source_after_sha256":after,
        "checker_sha256":sha256(Path(__file__).read_bytes()).hexdigest()}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"status":"pass","qualification":False,"smallest_conditional_q":375,
        "G_blocks":931,"group_budget":candidate["group_query_budget"],
        "aggregate_scaled_interval":candidate["aggregate_times_2_to_128_interval"],
        "source_negative_controls":len(negatives),"output":str(args.output)},indent=2))


if __name__=="__main__":
    main()
