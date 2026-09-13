"""Generate synthetic shared Rust/Python V1 report contract fixtures.

These files are parser fixtures, never device or release qualification evidence.
Run from the repository root with
`python3 -m scripts.fastpq.tests.generate_contract_fixtures`.
"""
from pathlib import Path
import copy
import hashlib
import json

from scripts.fastpq import wrap_benchmark as wrapper
from scripts.fastpq.report_projection import project_bundle
from scripts.fastpq.tests.test_digest384_evidence import primitive_report


def generate(out: Path) -> None:
    """Write deterministic positive and adversarial schema fixtures."""
    out.mkdir(parents=True, exist_ok=True)
    rows=[]
    for operation in ['digest384_trace_columns','digest384_merkle_pairs']:
     for schema,backend in [('metal_flat','metal'),('cuda_nested','cuda')]:
      for gpu in [False,True]:
       report=primitive_report(operation,backend if gpu else 'none')
       report['producer_schema']=schema
       if schema=='cuda_nested': report['operations'][0].pop('gpu_recorded',None)
       flat,_=wrapper.summarize_operations(report,schema)
       bundle={'producer_schema':schema,'metadata':{'notes':['Synthetic schema fixture. Device and timing claims are invented; this is not qualification evidence.']},'report':report,'benchmarks':{**report,'operations':flat}}
       bundle['benchmarks'].pop('producer_schema')
       wrapper.validate_report_header(wrapper.normalize_report(bundle),schema)
       project_bundle(bundle)
       name=f'{backend}-{"gpu" if gpu else "cpu"}-{operation}.json'
       encoded=json.dumps(bundle,indent=2,sort_keys=True)+'\n';(out/name).write_text(encoded)
       rows.append({'path':name,'sha256':hashlib.sha256(encoded.encode()).hexdigest(),'expected':'accept'})
    # Each rejection is one well-typed mutation from a valid matching report pair.
    valid=json.loads((out/'metal-gpu-digest384_trace_columns.json').read_text())
    mutants=[]
    def mutant(name,change):
     data=copy.deepcopy(valid);change(data)
     try: project_bundle(data)
     except (ValueError,SystemExit): pass
     else: raise AssertionError(name+' accepted')
     encoded=json.dumps(data,indent=2,sort_keys=True)+'\n';name='reject-'+name+'.json';(out/name).write_text(encoded)
     rows.append({'path':name,'sha256':hashlib.sha256(encoded.encode()).hexdigest(),'expected':'reject'})
    mutant('missing-raw-report',lambda d:d.pop('report'))
    mutant('missing-flat-nullable-field',lambda d:d['benchmarks']['operations'][0].pop('speedup_delta_ms'))
    mutant('missing-raw-producer-tag',lambda d:d['report'].pop('producer_schema'))
    mutant('missing-raw-column-count',lambda d:d['report'].pop('column_count'))
    mutant('typed-column-count-alias',lambda d:d['benchmarks'].__setitem__('column_count',2.0))
    mutant('partial-parity',lambda d:d['benchmarks']['operations'][0]['digest384']['gpu'].__setitem__('parity_checked_lanes',11))
    mutant('scalar-queue',lambda d:d['report'].__setitem__('metal_dispatch_queue',{'poseidon':None}))
    mutant('scalar-geometry',lambda d:d['benchmarks'].__setitem__('metal_heuristics',{'batch_columns':{'poseidon':None}}))
    (out/'manifest.json').write_text(json.dumps({'schema':'fastpq-benchmark-v1-synthetic-contract-fixtures','hardware_evidence':False,'files':rows},indent=2)+'\n')
    (out/'README.md').write_text('# FASTPQ V1 benchmark schema fixtures\n\nThese fixtures exercise the Rust and Python report readers against one explicit\nV1 contract. Every timing and device count is synthetic. No fixture establishes\nGPU execution, parity, source provenance or proof qualification.\n\nEight accepted cases cover both six-lane operations, Metal/CUDA producer forms\nand explicit CPU/GPU modes. Rejected cases remove required copies/fields,\nsubstitute numeric types or retain retired scalar claims. The manifest records\nexpected parser outcomes and exact file hashes. The Python fixture generator\nuses maintained projection/validation helpers; native Rust acceptance must be\nverified independently against the same bytes.\n')


if __name__ == "__main__":
    generate(Path(__file__).resolve().parents[3] / "fixtures/fastpq/benchmark_v1")
