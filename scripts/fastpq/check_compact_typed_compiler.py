#!/usr/bin/env python3
"""Finite-alphabet controls for the typed ideal compiler derivation.

These finite checks do not qualify cryptographic parameters or concrete hashes.
They exercise the new operator identities and exceptional-set arithmetic.
"""
from __future__ import annotations
import argparse
import cmath
import hashlib
import itertools
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TOL = 2e-10

def product_group(moduli):
    return list(itertools.product(*(range(m) for m in moduli)))

def character(moduli, frequency, value):
    phase = sum(a*b/m for a,b,m in zip(frequency,value,moduli))
    return cmath.exp(2j*cmath.pi*phase)

def matmul(a,b):
    return [[sum(a[i][k]*b[k][j] for k in range(len(b)))
             for j in range(len(b[0]))] for i in range(len(a))]

def adjoint(a):
    return [list(map(complex.conjugate,col)) for col in zip(*a)]

def error(a,b):
    return max(abs(x-y) for ar,br in zip(a,b) for x,y in zip(ar,br))

def matrix_controls(moduli):
    values=product_group(moduli)
    s=len(values)
    d=s+1
    ident=[[complex(i==j) for j in range(d)] for i in range(d)]
    jmat=[[0j for _ in range(d)] for _ in range(d)]
    for i in range(s):
        jmat[0][i+1]=jmat[i+1][0]=1/s**0.5
        for j in range(s):
            jmat[i+1][j+1]=complex(i==j)-1/s
    assert error(matmul(jmat,jmat),ident)<TOL
    characters=0
    partitions=0
    largest_frobenius_ratio=0.0
    for freq in values:
        chi=[character(moduli,freq,y) for y in values]
        diagonal=[[0j for _ in range(d)] for _ in range(d)]
        diagonal[0][0]=1
        for i,v in enumerate(chi):
            diagonal[i+1][i+1]=v
        actual=matmul(matmul(jmat,diagonal),jmat)
        assert error(matmul(adjoint(actual),actual),ident)<TOL
        if all(f==0 for f in freq):
            assert error(actual,ident)<TOL
            continue
        assert abs(sum(chi))<TOL
        expected=[[0j for _ in range(d)] for _ in range(d)]
        for i in range(s):
            expected[i+1][0]=chi[i]/s**0.5
            expected[0][i+1]=chi[i]/s**0.5
            for j in range(s):
                expected[i+1][j+1]=(1-chi[j]-chi[i])/s
                if i==j:
                    expected[i+1][j+1]+=chi[i]
        assert error(actual,expected)<TOL
        for w in range(s):
            row_sum=sum(abs(1-chi[w]-cy)**2 for cy in chi)
            assert abs(row_sum-s*(3-2*chi[w].real))<TOL
            assert row_sum<=5*s+TOL
        # Exhaust every membership assignment to bot and all output symbols.
        for mask in range(1<<d):
            membership=[bool(mask&(1<<i)) for i in range(d)]
            b=membership[0]
            delta=sum(v!=b for v in membership[1:])/s
            crossing=sum(abs(actual[i][j])**2
                         for i in range(d) if membership[i]
                         for j in range(d) if not membership[j])
            assert crossing<=6*delta+TOL,(moduli,freq,mask,crossing,delta)
            if delta:
                largest_frobenius_ratio=max(largest_frobenius_ratio,crossing/delta)
            partitions+=1
        characters+=1
    return {"moduli":list(moduli),"cardinality":s,"nontrivial_characters":characters,
            "membership_partitions":partitions,
            "max_cross_frobenius_over_instability":largest_frobenius_ratio}

def comparison_controls():
    tested=0
    # The exact rank-one off-diagonal norm formula is checked against the
    # explicitly tensor-multiplied selected row, for unequal output sizes.
    for sizes in [(2,),(3,),(5,),(2,3),(3,4),(2,3,5),(3,3,2),(2,2,2,2)]:
        rows=[]
        for s in sizes:
            # Selected value is y=0; row <0|J includes bot then all values.
            rows.append([1/s**0.5]+[complex(i==0)-1/s for i in range(s)])
        tensor=[1+0j]
        selected_index=0
        radix=1
        for row in reversed(rows):
            selected_index+=radix # local selected value index is one
            radix*=len(row)
        for row in rows:
            tensor=[a*b for a in tensor for b in row]
        assert abs(sum(abs(x)**2 for x in tensor)-1)<TOL
        a=1.0
        for s in sizes:
            a*=1-1/s
        off=sum(abs(v)**2 for i,v in enumerate(tensor) if i!=selected_index)
        assert abs(off-(1-a*a))<TOL
        assert off<=2*sum(1/s for s in sizes)+TOL
        tested+=1
    return tested

def binary_simulation_controls():
    tested=0
    # Exhaust finite cyclic response arithmetic with invalid bit encodings
    # in the arbitrary XOR destination; the temporary response is canonical.
    for modulus in range(2,18):
        bits=(modulus-1).bit_length()
        for value in range(modulus):
            for destination in range(1<<bits):
                temporary=0
                temporary=(temporary+value)%modulus
                result=destination^temporary
                temporary=(-temporary)%modulus
                temporary=(temporary+value)%modulus
                temporary=(-temporary)%modulus
                assert temporary==0
                assert result==(destination^value)
                tested+=1
    return tested

def counting_controls():
    tested=0
    for parents,chains,leaves,malformed,gj,other_g in itertools.product(range(4),repeat=6):
        nh=parents+chains+leaves+malformed
        ng=gj+other_g
        n=nh+ng
        short_pointers=2*parents+chains+ng
        assert short_pointers+nh<=3*n
        assert chains+gj<=n
        tested+=1
    return tested

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-typed-compiler-controls.json")
    args=parser.parse_args()
    controls=[matrix_controls(m) for m in [(2,),(3,),(4,),(5,),(7,),(2,2),(3,3)]]
    files=[Path(p) for p in [
        "specs/fastpq_compact_round_by_round.md",
        "specs/fastpq_compact_typed_compiler.md",
        "specs/fastpq_compact_adaptive_context.md",
        "specs/fastpq_compact_projected_xof.md",
    ]]
    primary=ROOT/"target/fastpq-production-validation/soundness-paper-text/cms2019.pdf"
    primary_sha="c3258e2faa339bdc441403d73aba9fee7d03687121c3369ef007ccce71cd2b41"
    if primary.exists():
        assert hashlib.sha256(primary.read_bytes()).hexdigest()==primary_sha, "retained CMS PDF hash differs"
    report={
        "kind":"finite-alphabet-controls-not-cryptographic-qualification",
        "status":"pass",
        "operator_controls":controls,
        "total_membership_partitions":sum(c["membership_partitions"] for c in controls),
        "unequal_alphabet_comparison_cases":comparison_controls(),
        "canonical_binary_simulation_cases":binary_simulation_controls(),
        "typed_pointer_count_cases":counting_controls(),
        "sources":{str(p):hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in files},
        "primary_pdf_sha256":primary_sha,"retained_primary_pdf_present_and_verified":primary.exists(),
        "primary_theorem":"Chiesa-Manohar-Spooner, 2020-01-14, https://eprint.iacr.org/2019/834",
        "checker_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    out=args.output
    out.parent.mkdir(parents=True,exist_ok=True)
    out.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({"status":report["status"],"total_membership_partitions":report["total_membership_partitions"],
                      "canonical_binary_simulation_cases":report["canonical_binary_simulation_cases"],
                      "typed_pointer_count_cases":report["typed_pointer_count_cases"],
                      "output":str(out)},indent=2))

if __name__=="__main__":
    if not __debug__:
        raise SystemExit("Do not disable assertions for finite-alphabet controls.")
    main()
