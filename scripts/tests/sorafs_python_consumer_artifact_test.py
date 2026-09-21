"""Synthetic identity/frame controls; these never qualify SDK execution."""
from __future__ import annotations

import base64
import copy
from dataclasses import FrozenInstanceError
import hashlib
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_consumer_artifact as artifact
from sorafs_python_consumer_cases import TEST_PATH, expected_node_ids

# A target-only support source copy is used when testing this review packet.
TEST_SOURCE = (ROOT / TEST_PATH).read_bytes()
INPUT = hashlib.sha256(b"independently held input").hexdigest()
PAYLOAD = b"observed pytest output\n"
DIGEST = hashlib.sha256(b"source").hexdigest()


def seal(inode, size=6):
    return f"{DIGEST}:1:{inode}:{size}:1:1:0o644"


def identity(path, data=b"source"):
    return {"path": path, "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}


@pytest.fixture
def report():
    sources = [identity(TEST_PATH, TEST_SOURCE), identity("python/norito_py/src/norito/__init__.py"),
               identity("python/iroha_torii_client/__init__.py"), identity("fixtures/sorafs_manifest/fixture.json")]
    sources.extend(identity(path) for path in artifact.FIXED_SOURCES if path != TEST_PATH)
    wheels = []
    for index, owner in enumerate(("iroha_native", "iroha_python")):
        names = [owner, owner + ("._crypto" if index == 0 else ".sorafs")]
        modules, files = [], []
        for offset, name in enumerate(names):
            member = owner+"/"+("__init__.py" if offset == 0 else "_crypto.abi3.so" if index == 0 else "sorafs.py")
            path = "/work/venv/site-packages/"+member
            files.append({"path": path, "seal": seal(10+index*10+offset)})
            modules.append({**identity(path), "name": name, "member": member,
                            "loader": "ExtensionFileLoader" if name.endswith("._crypto") else "SourceFileLoader"})
        wheels.append({"owner": owner, "path": "/work/"+owner+".whl", "seal": seal(index+1), "version": "0.0.1",
                       "installed_files": sorted(files, key=lambda x:x['path']), "loaded_modules": sorted(modules,key=lambda x:x['name'])})
    deps = []
    for owner, root, suffix in (("norito", "/work/snapshot/python/norito_py/src", "/norito/__init__.py"),
                                 ("iroha_torii_client", "/work/snapshot/python/iroha_torii_client", "/__init__.py")):
        deps.append({"module": owner, "root": root, "loaded_modules": [{**identity(root+suffix), "name": owner, "loader": "SourceFileLoader"}]})
    return {"schema": artifact.SCHEMA, "input_sha256": INPUT, "source_files": sorted(sources,key=lambda x:x['path']),
            "python": {**identity("/runtime/python3.12"), "version": "3.12.14"},
            "pytest": {**identity("/work/venv/site-packages/pytest/__init__.py"), "version": "9.0.3"},
            "wheels": wheels, "dependencies": deps,
            "cases": [{"nodeid": name, "phases": [{"phase": phase,"outcome":"passed"} for phase in ("setup","call","teardown")]} for name in expected_node_ids(TEST_SOURCE)],
            "captured_output": {"bytes":len(PAYLOAD),"sha256":hashlib.sha256(PAYLOAD).hexdigest()}}


def parse(report):
    return artifact.parse_report(artifact.canonical_json(report), expected_input_sha256=INPUT, test_source=TEST_SOURCE)


def frame(report, output=PAYLOAD):
    return output+artifact.REPORT_PREFIX+base64.b64encode(artifact.canonical_json(report))+b"\n"


def consume(raw):
    return artifact.consume_runtime_output(raw, expected_input_sha256=INPUT, test_source=TEST_SOURCE)


def test_frozen_report_preserves_all_77_cases_231_phases_and_long_parameter(report):
    observed = parse(report)
    assert len(observed.cases)==77
    assert sum(len(case.phases) for case in observed.cases)==231
    assert max(len(case.nodeid) for case in observed.cases)>10000
    with pytest.raises(FrozenInstanceError): observed.schema="other"
    with pytest.raises(FrozenInstanceError): observed.wheels[0].installed_files[0].seal.size=0
    assert consume(frame(report)).observations==observed
    assert consume(frame(report)).output==PAYLOAD


def test_empty_captured_stream_has_an_exact_join(report):
    report['captured_output']={"bytes":0,"sha256":hashlib.sha256(b'').hexdigest()}
    assert consume(frame(report,b'')).output==b''


@pytest.mark.parametrize('where', ('root','python','pytest','wheel','installed','module','dependency','case','phase','output','source'))
@pytest.mark.parametrize('mutation', ('extra','missing'))
def test_closed_objects_reject_unknown_or_missing_fields(report,where,mutation):
    targets={'root':report,'python':report['python'],'pytest':report['pytest'],'wheel':report['wheels'][0],
             'installed':report['wheels'][0]['installed_files'][0],'module':report['wheels'][0]['loaded_modules'][0],
             'dependency':report['dependencies'][0],'case':report['cases'][0], 'phase':report['cases'][0]['phases'][0],
             'output':report['captured_output'],'source':report['source_files'][0]}
    target=targets[where]
    if mutation=='extra': target['qualified']=True
    else: target.pop(next(iter(target)))
    with pytest.raises(artifact.ArtifactError): parse(report)


@pytest.mark.parametrize('mutation', (
    'input','zero_hash','uppercase_hash','bool_size','negative_size','overflow_size','python_version','pytest_version',
    'wheel_order','wheel_version','seal_format','seal_mode','seal_zero_inode','physical_alias','installed_path_alias',
    'source_duplicate','source_unsorted','source_escape','source_absolute','source_nul','source_test_bytes',
    'module_duplicate','module_unknown_owner','module_loader','module_byte_join','module_member','module_path','missing_native',
    'dependency_order','dependency_escape','dependency_digest','dependency_member_field','cases_missing','cases_duplicate',
    'cases_reordered','nodeid_changed','nodeid_excessive','phase_missing','phase_duplicate','phase_reordered','phase_skip',
    'phase_failed','phase_xpassed','phase_xfailed','output_bool','output_zero_hash',
))
def test_resealed_report_mutations_fail_closed(report,mutation):
    wheel=report['wheels'][0]; module=wheel['loaded_modules'][0]; source=report['source_files'][0]; case=report['cases'][0]
    if mutation=='input': report['input_sha256']='a'*64
    elif mutation=='zero_hash': report['python']['sha256']='0'*64
    elif mutation=='uppercase_hash': report['python']['sha256']='A'*64
    elif mutation=='bool_size': report['python']['size']=True
    elif mutation=='negative_size': source['size']=-1
    elif mutation=='overflow_size': source['size']=artifact.MAX_FILE_BYTES+1
    elif mutation=='python_version': report['python']['version']='3.13.0'
    elif mutation=='pytest_version': report['pytest']['version']='8.4.2'
    elif mutation=='wheel_order': report['wheels'].reverse()
    elif mutation=='wheel_version': wheel['version']='0.0.2'
    elif mutation=='seal_format': wheel['seal']=wheel['seal'].replace(':1:',':01:',1)
    elif mutation=='seal_mode': wheel['seal']=wheel['seal'].replace('0o644','0o10000')
    elif mutation=='seal_zero_inode': wheel['seal']=f'{DIGEST}:1:0:6:1:1:0o644'
    elif mutation=='physical_alias': report['wheels'][1]['seal']=wheel['seal']
    elif mutation=='installed_path_alias': wheel['installed_files'].append(copy.deepcopy(wheel['installed_files'][0]))
    elif mutation=='source_duplicate': report['source_files'].append(copy.deepcopy(source))
    elif mutation=='source_unsorted': report['source_files'].reverse()
    elif mutation=='source_escape': source['path']='../outside.py'
    elif mutation=='source_absolute': source['path']='/outside.py'
    elif mutation=='source_nul': source['path']='bad\x00.py'
    elif mutation=='source_test_bytes': next(s for s in report['source_files'] if s['path']==TEST_PATH)['sha256']='a'*64
    elif mutation=='module_duplicate': wheel['loaded_modules'].append(copy.deepcopy(module))
    elif mutation=='module_unknown_owner': module['name']='foreign'
    elif mutation=='module_loader': module['loader']='AssertionRewritingHook'
    elif mutation=='module_byte_join': module['sha256']='a'*64
    elif mutation=='module_member': module['member']='foreign/__init__.py'
    elif mutation=='module_path': module['path']='/shadow/native.py'
    elif mutation=='missing_native': wheel['loaded_modules']=[m for m in wheel['loaded_modules'] if m['name']!='iroha_native._crypto']
    elif mutation=='dependency_order': report['dependencies'].reverse()
    elif mutation=='dependency_escape': report['dependencies'][0]['loaded_modules'][0]['path']='/outside.py'
    elif mutation=='dependency_digest': report['dependencies'][0]['loaded_modules'][0]['sha256']='a'*64
    elif mutation=='dependency_member_field': report['dependencies'][0]['loaded_modules'][0]['member']='norito/__init__.py'
    elif mutation=='cases_missing': report['cases'].pop()
    elif mutation=='cases_duplicate': report['cases'][1]=copy.deepcopy(case)
    elif mutation=='cases_reordered': report['cases'].reverse()
    elif mutation=='nodeid_changed': case['nodeid']+='[invented]'
    elif mutation=='nodeid_excessive': case['nodeid']='x'*(artifact.MAX_NODE_BYTES+1)
    elif mutation=='phase_missing': case['phases'].pop()
    elif mutation=='phase_duplicate': case['phases'][1]=copy.deepcopy(case['phases'][0])
    elif mutation=='phase_reordered': case['phases'].reverse()
    elif mutation.startswith('phase_'): case['phases'][1]['outcome']=mutation.removeprefix('phase_')
    elif mutation=='output_bool': report['captured_output']['bytes']=True
    elif mutation=='output_zero_hash': report['captured_output']['sha256']='0'*64
    else: raise AssertionError(mutation)
    with pytest.raises(artifact.ArtifactError): parse(report)


@pytest.mark.parametrize('mutation', ('missing_frame','repeated','trailing','embedded','crlf','bad_base64','noncanonical_base64',
                                      'changed_output','fd_bypass','truncated','report_size','stream_size','mutable','duplicate_json','noncanonical_json','nan','deep_json'))
def test_actual_stream_join_and_frame_reject_mutation(report,mutation):
    raw=frame(report)
    if mutation=='missing_frame': raw=PAYLOAD
    elif mutation=='repeated': raw+=raw
    elif mutation=='trailing': raw+=b'after\n'
    elif mutation=='embedded': raw=raw.replace(b'\n'+artifact.REPORT_PREFIX,b' prefix '+artifact.REPORT_PREFIX)
    elif mutation=='crlf': raw=raw[:-1]+b'\r\n'
    elif mutation=='bad_base64': raw=PAYLOAD+artifact.REPORT_PREFIX+b'%%%%\n'
    elif mutation=='noncanonical_base64': raw=raw[:-1]+b'=\n'
    elif mutation=='changed_output': raw=raw.replace(b'observed',b'changed')
    elif mutation=='fd_bypass': raw=b'native fd output\n'+raw
    elif mutation=='truncated': raw=raw[:-1]
    elif mutation=='report_size': raw=artifact.REPORT_PREFIX+base64.b64encode(b'x'*(artifact.MAX_REPORT_BYTES+1))+b'\n'
    elif mutation=='stream_size': raw=b'x'*(artifact.MAX_OUTPUT_BYTES+1)+b'\n'
    elif mutation=='mutable': raw=bytearray(raw)
    elif mutation=='duplicate_json': raw=artifact.REPORT_PREFIX+base64.b64encode(b'{"schema":1,"schema":2}\n')+b'\n'
    elif mutation=='noncanonical_json': raw=artifact.REPORT_PREFIX+base64.b64encode(artifact.canonical_json(report)+b'\n')+b'\n'
    elif mutation=='nan': raw=artifact.REPORT_PREFIX+base64.b64encode(b'{"value":NaN}\n')+b'\n'
    elif mutation=='deep_json': raw=artifact.REPORT_PREFIX+base64.b64encode(b'['*2000+b'0'+b']'*2000+b'\n')+b'\n'
    with pytest.raises(artifact.ArtifactError): consume(raw)


def test_independent_input_and_test_source_are_required(report):
    with pytest.raises(artifact.ArtifactError): artifact.parse_report(artifact.canonical_json(report),expected_input_sha256='0'*64,test_source=TEST_SOURCE)
    with pytest.raises(artifact.ArtifactError): artifact.parse_report(artifact.canonical_json(report),expected_input_sha256=INPUT,test_source=TEST_SOURCE+b'\n')


@pytest.mark.parametrize("mutation", ("source_dot", "source_surrogate", "nodeid_surrogate", "source_unknown",
                                      "source_fixed_missing", "source_file_limit", "source_total_limit",
                                      "source_count_limit", "python_patch_limit", "module_renamed_member",
                                      "dependency_renamed_member", "wheel_split_roots"))
def test_resealed_owner_and_path_controls(report, mutation):
    if mutation == "source_dot": report["source_files"][0]["path"] = "."
    elif mutation == "source_surrogate": report["source_files"][0]["path"] = "bad\ud800"
    elif mutation == "nodeid_surrogate": report["cases"][0]["nodeid"] = "bad\ud800"
    elif mutation == "source_unknown": report["source_files"].append(identity("unknown/source.py"))
    elif mutation == "source_fixed_missing": report["source_files"] = [f for f in report["source_files"] if f["path"] != "ci/verify_privacy_python_wheel.py"]
    elif mutation == "source_file_limit": report["source_files"][0]["size"] = artifact.MAX_SOURCE_FILE_BYTES + 1
    elif mutation == "source_total_limit":
        for source in report["source_files"]:
            if source["path"] != TEST_PATH: source["size"] = artifact.MAX_SOURCE_FILE_BYTES
    elif mutation == "source_count_limit":
        report["source_files"].extend(identity(f"fixtures/sorafs_manifest/{i}.json") for i in range(artifact.MAX_SOURCE_FILES))
    elif mutation == "python_patch_limit": report["python"]["version"] = "3.12." + "1" * 65
    elif mutation == "module_renamed_member":
        wheel = report["wheels"][1]
        module = next(m for m in wheel["loaded_modules"] if m["name"] == "iroha_python.sorafs")
        old = module["path"]; module["member"] = "iroha_python/other.py"
        module["path"] = "/work/venv/site-packages/" + module["member"]
        next(f for f in wheel["installed_files"] if f["path"] == old)["path"] = module["path"]
        wheel["installed_files"].sort(key=lambda f: f["path"])
    elif mutation == "dependency_renamed_member":
        module = report["dependencies"][0]["loaded_modules"][0]
        module["path"] = "/work/snapshot/python/norito_py/src/norito/other.py"
        report["source_files"].append(identity("python/norito_py/src/norito/other.py"))
    elif mutation == "wheel_split_roots":
        wheel = report["wheels"][0]; module = wheel["loaded_modules"][1]
        old = module["path"]; module["path"] = "/other/site/" + module["member"]
        next(f for f in wheel["installed_files"] if f["path"] == old)["path"] = module["path"]
        wheel["installed_files"].sort(key=lambda f: f["path"])
    report["source_files"].sort(key=lambda f: f["path"])
    with pytest.raises(artifact.ArtifactError): parse(report)


def test_case_source_owner_errors_are_closed(report):
    for source in (b"syntax error!", b"pass", bytearray(TEST_SOURCE), b"\xff"):
        with pytest.raises(artifact.ArtifactError):
            artifact.parse_report(artifact.canonical_json(report), expected_input_sha256=INPUT, test_source=source)


def test_seal_uses_the_single_existing_ci_parser(report, monkeypatch):
    original = artifact._VERIFIER.FileSeal.parse
    calls = []
    def observed(value):
        calls.append(value)
        return original(value)
    monkeypatch.setattr(artifact._VERIFIER.FileSeal, "parse", observed)
    report["wheels"][0]["seal"] = report["wheels"][0]["seal"].replace("0o644", "0o4644")
    assert parse(report).wheels[0].seal.mode == 0o4644
    assert len(calls) == 6


def test_actual_log_ceiling_is_independent_of_final_report_size(report):
    output = b"x" * (artifact.MAX_LOG_BYTES - 1) + b"\n"
    report["captured_output"] = {"bytes": len(output), "sha256": hashlib.sha256(output).hexdigest()}
    raw = frame(report, output)
    assert artifact.MAX_LOG_BYTES < len(raw) < artifact.MAX_OUTPUT_BYTES
    assert consume(raw).output == output
    output += b"\n"
    report["captured_output"] = {"bytes": len(output), "sha256": hashlib.sha256(output).hexdigest()}
    with pytest.raises(artifact.ArtifactError): consume(frame(report, output))


def test_actual_report_ceiling_and_one_byte_over(report):
    # Fill only permitted, bounded source observation rows. No actual source or
    # process authority is claimed by this synthetic structural boundary check.
    prefix = "fixtures/sorafs_manifest/" + "a" * 240 + "/" + "b" * 240 + "/" + "c" * 240 + "/"
    template = identity(prefix + "0000-" + "d" * 240)
    row_size = len(artifact.canonical_json(template))
    pads = [identity("fixtures/sorafs_manifest/pad1"), identity("fixtures/sorafs_manifest/pad2")]
    report["source_files"].extend(pads)
    initial = len(artifact.canonical_json(report))
    count = (artifact.MAX_REPORT_BYTES - initial) // row_size
    report["source_files"].extend(identity(prefix + f"{i:04d}-" + "d" * 240) for i in range(count))
    missing = artifact.MAX_REPORT_BYTES - len(artifact.canonical_json(report))
    for pad, amount in zip(pads, (missing // 2, missing - missing // 2)):
        padding = "/".join("e" * 240 for _ in range((amount + 239) // 240))[:amount]
        pad["path"] += padding
    report["source_files"].sort(key=lambda row: row["path"])
    raw = artifact.canonical_json(report)
    assert len(raw) == artifact.MAX_REPORT_BYTES
    assert len(report["source_files"]) <= artifact.MAX_SOURCE_FILES
    assert artifact.parse_report(raw, expected_input_sha256=INPUT, test_source=TEST_SOURCE).cases[0].nodeid
    with pytest.raises(artifact.ArtifactError):
        artifact.parse_report(raw + b"\n", expected_input_sha256=INPUT, test_source=TEST_SOURCE)


@pytest.mark.parametrize("mutation", ("tool_path_alias", "tool_install_alias", "different_snapshot", "wrong_dependency_layout"))
def test_distinct_file_and_snapshot_owners_are_structural(report, mutation):
    if mutation == "tool_path_alias": report["pytest"]["path"] = report["python"]["path"]
    elif mutation == "tool_install_alias": report["pytest"]["path"] = report["wheels"][0]["installed_files"][0]["path"]
    else:
        dep = report["dependencies"][0]
        before = dep["root"]
        dep["root"] = before.replace("/work/snapshot", "/other/snapshot") if mutation == "different_snapshot" else "/work/foreign"
        for module in dep["loaded_modules"]: module["path"] = module["path"].replace(before, dep["root"])
    with pytest.raises(artifact.ArtifactError): parse(report)
