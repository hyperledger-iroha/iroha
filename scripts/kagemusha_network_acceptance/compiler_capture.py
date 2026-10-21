"""Explicit current compiler capture and replay admission for engineering networks.

This producer runs only on its explicit CLI. Admission never builds or silently
repairs a receipt. The maintained Rust census and generated-input auditor are
used together with prospectively captured dependency bytes, not as substitutes.
"""
from pathlib import Path
from types import ModuleType, SimpleNamespace
import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import time

import capture
import child_process
import source_audit
import monetary_depfile
from paths import ROOT, QUAL

AUTHENTICATED_RUNTIME = {}
SCHEMA = 'iroha.kagemusha.current-compiler-capture.v1'
REVIEW_SCHEMA = 'iroha.kagemusha.current-compiler-review.v1'
ROLES = {
    'native': ({'iroha_core_zk'}, {'iroha_core_zk'}, ['test', '-p', 'iroha_core_zk', '--lib', '--no-run']),
    'network': ({'integration_tests'}, {'sumeragi'}, ['test', '-p', 'integration_tests', '--test', 'sumeragi', '--no-run']),
    'nodes': ({'irohad', 'iroha_cli'}, {'iroha3d', 'iroha'}, ['build', '-p', 'irohad', '-p', 'iroha_cli', '--bin', 'iroha3d', '--bin', 'iroha']),
}
MAINTAINED = ('audit_native_source_inputs.py', 'native_sdk_source_custody.py', 'kagemusha_qualify.py')


def require(ok, message):
    if not ok:
        raise ValueError(message)


def exact_path(value, *, absent=False):
    path = Path(value)
    require(path.is_absolute() and str(path) == str(value) and path == Path(os.path.abspath(path)), 'absolute original path required')
    for part in (*reversed(path.parents), path):
        require(not part.is_symlink(), 'aliased compiler original')
    require(path.resolve(strict=not absent) == path, 'aliased compiler original')
    return path


def sha(path):
    path = exact_path(path)
    before = path.stat()
    require(stat.S_ISREG(before.st_mode), 'regular compiler original required')
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        opened = os.fstat(stream.fileno())
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            digest.update(chunk)
        after = os.fstat(stream.fileno())
    final = path.stat()
    identity = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
    require(len({identity(info) for info in (before, opened, after, final)}) == 1, 'compiler original changed during hash')
    return digest.hexdigest()


def pin(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) and value != '0' * 64, 'nonzero SHA256 required')
    return value


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate evidence field')
        result[key] = value
    return result


def read(path):
    return json.loads(capture.stable_read(exact_path(path)), object_pairs_hook=unique)


def save(path, value):
    with path.open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')


def maintained():
    """Execute the verified current source, never an importlib bytecode cache."""
    result = {}
    for name in MAINTAINED:
        path = exact_path(ROOT / 'scripts' / name)
        raw = capture.stable_read(path)
        module = ModuleType('_kagemusha_capture_' + path.stem)
        module.__file__ = str(path)
        module.__source_sha256__ = hashlib.sha256(raw).hexdigest()
        exec(compile(raw, str(path), 'exec'), module.__dict__)
        require(sha(path) == hashlib.sha256(raw).hexdigest(), 'maintained helper changed during load')
        if name in ('audit_native_source_inputs.py', 'kagemusha_qualify.py'):
            module.subprocess = SimpleNamespace(run=child_process.natural_run)
        result[name] = module
    return result


def tool_files():
    paths = [ROOT / 'scripts' / name for name in MAINTAINED]
    paths += [Path(__file__).resolve().with_name(name) for name in
              ('compiler_capture.py', 'child_process.py', 'capture_build.py', 'source_admission.py', 'capture.py', 'source_audit.py', 'monetary_depfile.py', 'paths.py')]
    pins = {str(path): sha(path) for path in paths}
    require(AUTHENTICATED_RUNTIME and all(AUTHENTICATED_RUNTIME.get(name) == digest for name, digest in pins.items()), 'capture helpers differ from externally selected runtime')
    return pins


def configuration(role, profile, slot, jobs):
    require(role in ROLES and profile in ('debug', 'release'), 'reviewed compiler role/profile required')
    require(role == 'nodes' or profile == 'release', 'native and network test owners require release profile')
    require(type(slot) is str and re.fullmatch('[A-Za-z0-9][A-Za-z0-9._-]*', slot), 'simple selected Cargo slot required')
    require(type(jobs) is int and 1 <= jobs <= 64, 'bounded positive build jobs required')
    return {'role': role, 'profile': profile, 'slot': slot, 'jobs': jobs}


def command(config):
    config = configuration(**config)
    target = ROOT / 'target/cargo-fast' / config['slot']
    args = list(ROLES[config['role']][2])
    if config['profile'] == 'release':
        args.append('--release')
    return ['scripts/cargo_fast.sh', '--stable-local-metadata', '--target-dir', str(target),
            '--jobs', str(config['jobs']), '--', *args, '--locked', '--offline', '--message-format=json']


def metadata():
    completed = child_process.natural_run(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'],
                               cwd=ROOT, capture_output=True, text=True, check=True)
    return json.loads(completed.stdout, object_pairs_hook=unique)


def source_snapshot(meta, role, modules):
    """Prospective full local packages plus exact external metadata dependencies."""
    capture.require_resolved_checkout()
    custody = modules['native_sdk_source_custody.py']
    require(meta['workspace_root'] == str(ROOT), 'foreign Cargo workspace')
    roots = source_audit.local_roots(ROOT, meta)
    value = capture.snapshot(roots)
    require(value['branch'] == 'optimizations' and value['merge_head'] is False
            and not value['unmerged'] and not value['read_failures'], 'unresolved source capture')
    dependency = custody.source_identity(meta, ROOT, ROLES[role][0], sha)
    for name, digest in dependency.items():
        if not name.startswith(('registry:', 'git:')):
            require(value['files'].get(name) == digest, 'dependency absent from prospective local census: ' + name)
    value['dependencies'] = dependency
    # Native census administration is a separate presence/value binding. Never
    # normalize a changed index or synthesize missing build-time Git bytes.
    candidates = {ROOT / '.git/index', ROOT / '.git/info/exclude', ROOT / '.git/HEAD', ROOT / '.git/packed-refs'}
    symbolic = capture.git('symbolic-ref', '-q', 'HEAD')
    require(symbolic == 'refs/heads/optimizations', 'resolved optimizations ref required')
    candidates.add(ROOT / '.git' / symbolic)
    require((ROOT / '.git').is_dir() and not (ROOT / '.git/commondir').exists(), 'original checkout required')
    value['administration'] = {str(path.relative_to(ROOT)): sha(path) if path.exists() else None for path in sorted(candidates)}
    return value


def validate_source_pair(before, after):
    """Original captured values only; no synthesized Git or source equivalence."""
    fields = {'files', 'links', 'read_failures', 'head', 'branch', 'unmerged', 'merge_head', 'dependencies', 'administration'}
    administration = {'.git/HEAD', '.git/index', '.git/info/exclude', '.git/packed-refs', '.git/refs/heads/optimizations'}
    for value in (before, after):
        require(type(value) is dict and set(value) == fields and type(value['administration']) is dict
                and set(value['administration']) == administration, 'incomplete original source/metadata inventory')
        require(value['branch'] == 'optimizations' and re.fullmatch('[0-9a-f]{40}', value['head'])
                and value['merge_head'] is False and not value['unmerged']
                and not value['read_failures'], 'unresolved original source/metadata')
    require(before == after, 'prospective source/metadata originals changed')


def check_pins(pins):
    for name, digest in pins.items():
        require(sha(Path(name)) == pin(digest), 'retained compiler original changed')


def check_output_namespace(root, expected):
    require(output_map(root) == expected, 'generated output namespace or contents changed')


def check_tools(captured, current):
    require(captured == current, 'compiler tools/configuration changed')
    check_pins(captured['files'])
    for tool in captured['compiler'].values():
        if 'sha256' in tool:
            check_pins({tool['path']: tool['sha256']})


def compiler_environment():
    # Preserve values only for build controls, never private issuer/SDK custody.
    forbidden = ('RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_TARGET',
                 'CARGO_TARGET_DIR', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTFLAGS')
    require(not any(os.environ.get(name) for name in forbidden), 'unreviewed compiler override')
    native_controls = re.compile(r'(?:(?:HOST_|TARGET_)?(?:CC|CXX|AR|RANLIB|AS|OBJC|OBJCXX|CFLAGS|CXXFLAGS|ARFLAGS)(?:_.*)?|CARGO_TARGET_.*_(?:LINKER|RUNNER|RUSTFLAGS)|CMAKE_.*|PKG_CONFIG.*|SDKROOT|DEVELOPER_DIR)')
    require(not any(native_controls.fullmatch(name) and value for name, value in os.environ.items()), 'unreviewed native compiler/linker override')
    require(not any(name.startswith(('KAGEMUSHA_', 'IROHA_TEST_', 'IROHA_RELEASE_', 'TEST_NETWORK_'))
                    or name in ('LD_PRELOAD', 'DYLD_INSERT_LIBRARIES', 'PYTHONOPTIMIZE') for name in os.environ),
            'inherited runtime or compiler override')
    return {name: os.environ.get(name) for name in forbidden}


def tools(modules):
    for name, module in modules.items():
        require(sha(ROOT / 'scripts' / name) == module.__source_sha256__, 'executed maintained helper bytes changed')
    cargo_tools = modules['kagemusha_qualify.py'].tool_identity()
    for name, digest in cargo_tools['cargo_configuration'].items():
        if digest is None:
            continue
        value = modules['kagemusha_qualify.py'].tomllib.loads(capture.stable_read(Path(name)).decode())
        require(not value.get('build', {}).get('rustflags'), 'configured native rustflags require review')
        for target in value.get('target', {}).values():
            require(isinstance(target, dict) and not any(target.get(key) for key in ('linker', 'runner', 'rustflags')), 'configured target compiler/linker requires review')
    return {'files': tool_files(), 'compiler': source_audit.tools_snapshot(ROOT, capture.stable_read),
            'cargo': cargo_tools, 'environment': compiler_environment(),
            'environment_hashes': {name: hashlib.sha256(value.encode()).hexdigest() for name, value in sorted(os.environ.items())}}


def source_paths(snapshot):
    result = {str(ROOT / name): value for name, value in snapshot['files'].items()}
    result.update({str(ROOT / name): value for name, value in snapshot['administration'].items()})
    for name, value in snapshot['dependencies'].items():
        if name.startswith(('registry:', 'git:')):
            result[name.split(':', 1)[1]] = value
    return result


def build_events(path, modules):
    return modules['native_sdk_source_custody.py'].cargo_messages(capture.stable_read(path).decode(), object_pairs_hook=unique)


def artifact_hashes(events):
    """Pin every actual emitted artifact, including evaluator dependency closure."""
    pins = {}
    for event in events:
        if event.get('reason') != 'compiler-artifact':
            continue
        require(event.get('filenames'), 'compiler artifact lacks filenames')
        for name in event['filenames']:
            path = exact_path(name)
            require(path.is_relative_to(ROOT / 'target'), 'emitted compiler artifact outside checkout target')
            digest = sha(path)
            require(name not in pins or pins[name] == digest, 'emitted artifact changed while captured')
            pins[name] = digest
    require(pins, 'no emitted compiler artifacts')
    return pins


def output_map(directory):
    """Bind every emitted file, rejecting aliases and special files."""
    directory = exact_path(directory)
    require(directory.is_dir(), 'Cargo output directory required')
    result = {}
    for parent, children, files in os.walk(directory, followlinks=False):
        for name in children:
            exact_path(Path(parent) / name)
        for name in files:
            path = exact_path(Path(parent) / name)
            result[str(path)] = sha(path)
    return result


def environment_dependencies(text, event, package, events):
    """Bind rustc env! inputs to exact Cargo-derived values or captured process env."""
    scripts = [row for row in events if row.get('reason') == 'build-script-executed' and row['package_id'] == event['package_id']]
    derived = {'CARGO_MANIFEST_DIR': str(Path(package['manifest_path']).parent),
               'CARGO_MANIFEST_PATH': package['manifest_path'],
               'CARGO_CRATE_NAME': event['target']['name'].replace('-', '_')}
    if event['target']['kind'] == ['bin']:
        derived['CARGO_BIN_NAME'] = event['target']['name']
    for key, field in (('VERSION', 'version'), ('NAME', 'name'), ('DESCRIPTION', 'description'),
                       ('HOMEPAGE', 'homepage'), ('REPOSITORY', 'repository'), ('LICENSE', 'license'),
                       ('LICENSE_FILE', 'license_file'), ('RUST_VERSION', 'rust_version')):
        derived['CARGO_PKG_' + key] = package.get(field) or ''
    derived['CARGO_PKG_AUTHORS'] = ':'.join(package.get('authors', []))
    version = re.fullmatch(r'(\d+)\.(\d+)\.(\d+)(?:-([^+]+))?(?:\+.*)?', package['version'])
    require(version is not None, 'invalid Cargo package version')
    for key, value in zip(('MAJOR', 'MINOR', 'PATCH', 'PRE'), version.groups()):
        derived['CARGO_PKG_VERSION_' + key] = value or ''
    if scripts:
        require(len(scripts) == 1, 'ambiguous package build environment')
        derived['OUT_DIR'] = scripts[0]['out_dir']
        for key, value in scripts[0]['env']:
            require(key not in derived, 'build script overrides Cargo-derived environment')
            derived[key] = value
    observed = {}
    for line in text.splitlines():
        if not line.startswith('# env-dep:'):
            continue
        name, separator, value = line[len('# env-dep:'):].partition('=')
        require(name and name not in observed, 'ambiguous rustc environment dependency')
        expected = value if separator else None
        if name == 'CARGO_TARGET_TMPDIR':
            require(expected is not None and exact_path(expected, absent=True).is_relative_to(ROOT / 'target'), 'foreign Cargo test temporary owner')
        elif name in derived:
            require(expected == derived[name], 'Cargo-derived compiler environment differs: ' + name)
        elif name.startswith('CARGO_') or name == 'OUT_DIR':
            raise ValueError('unreviewed derived compiler environment: ' + name)
        else:
            require(expected == os.environ.get(name), 'compiler environment changed: ' + name)
        observed[name] = None if expected is None else hashlib.sha256(expected.encode()).hexdigest()
    return observed


def exact_depfiles(events, meta, before, after, audit, native):
    """Join each actual Cargo artifact to its exact Make target and input rule."""
    packages = {row['id']: row for row in meta['packages']}
    before_paths, after_paths = source_paths(before), source_paths(after)
    generated = {row['path']: row['sha256'] for key in ('canonicalGeneratedInputs', 'externalDependencyGeneratedInputs', 'generatedExceptions') for row in native[key]}
    absent = {row['path'] for row in native['absentBuildWatchExceptions']}
    audited = {row['path']: row['sha256'] for row in native['depfiles']}
    pins, compiled, mapping = artifact_hashes(events), set(), []
    for event in events:
        if event.get('reason') != 'compiler-artifact':
            continue
        require(event['package_id'] in packages, 'Cargo artifact outside prospective metadata')
        package = packages[event['package_id']]
        manifest = exact_path(package['manifest_path'])
        require(event['manifest_path'] == str(manifest), 'Cargo artifact manifest differs')
        if 'targets' in package:
            require(any(row['name'] == event['target']['name'] and row['kind'] == event['target']['kind']
                        and row['src_path'] == event['target']['src_path'] for row in package['targets']), 'Cargo artifact target differs from prospective metadata')
        if manifest.is_relative_to(ROOT):
            compiled.add(str(manifest.parent.relative_to(ROOT)))
        require(event['filenames'], 'Cargo artifact has no output')
        for filename in event['filenames']:
            path = exact_path(filename)
            require(path.is_relative_to(ROOT / 'target'), 'compiler artifact outside selected checkout outputs')
            target = path
            stem = path.stem.removeprefix('lib') if path.suffix else path.name
            dep = path.with_name(stem + '.d')
            if path.name in ('build-script-build', 'build-script-script'):
                matches = [p for p in path.parent.glob('build_script_*') if p.suffix == '' and p.with_suffix('.d').is_file() and sha(p) == sha(path)]
                require(len(matches) == 1, 'missing or ambiguous exact build-script alias')
                target = exact_path(matches[0]); dep = target.with_suffix('.d')
                pins[str(target)] = sha(target)
            require(str(dep) in audited and sha(dep) == audited[str(dep)], 'exact emitted-target depfile absent from maintained audit')
            text = capture.stable_read(dep).decode()
            names = monetary_depfile.target_inputs(text, str(target))
            original_inputs = {str(Path(name) if Path(name).is_absolute() else ROOT / name) for name in names}
            audited_inputs = {str(p) for p in audit.inputs(dep, {Path(p) for p in absent})}
            require(original_inputs == audited_inputs, 'auditor depfile rule differs from actual emitted target')
            for name in original_inputs:
                if name in absent:
                    require(not os.path.lexists(name), 'exact absent native watcher appeared')
                elif name in generated:
                    require(sha(Path(name)) == generated[name], 'generated compiler input changed')
                else:
                    require(before_paths.get(name) is not None and before_paths.get(name) == after_paths.get(name) == sha(Path(name)), 'compiler input lacks unchanged prospective bytes: ' + name)
            pins[str(dep)] = audited[str(dep)]
            environment = environment_dependencies(text, event, package, events)
            mapping.append({'package': event['package_id'], 'target': str(target), 'depfile': str(dep), 'inputs': sorted(original_inputs), 'environment_hashes': environment})
    require(compiled, 'no local compiler package inputs')
    return sorted(compiled), pins, mapping


def bind_watches(watches, before, after):
    before_paths, after_paths = source_paths(before), source_paths(after)
    for path in watches:
        name = str(path)
        if path.is_symlink():
            relative = str(path.relative_to(ROOT))
            require(before['links'].get(relative) == after['links'].get(relative) == os.readlink(path), 'native watched link was not prospectively bound')
        elif path.exists():
            require(before_paths.get(name) is not None and before_paths.get(name) == after_paths.get(name) == sha(path), 'native watched input lacks prospective byte custody: ' + name)
        else:
            relative = str(path.relative_to(ROOT))
            require(relative in before['administration'] and relative in after['administration'] and before['administration'][relative] is None and after['administration'][relative] is None, 'unclassified absent native watcher')


def native_audit(events, directory, modules, *, create):
    audit = modules['audit_native_source_inputs.py']
    evaluator_dir = directory / 'native-evaluator'
    if create:
        evaluator_dir.mkdir()
        tool = audit.build_snapshot_tool(evaluator_dir, events)
    else:
        tool = evaluator_dir / 'source-snapshot'
    evaluator = read(tool.with_name('source-snapshot-build.json'))
    require(evaluator['binaryPath'] == str(tool), 'foreign snapshot evaluator')
    for key in ('helper', 'driver', 'binary', 'sha2'):
        require(sha(Path(evaluator[key + 'Path'])) == evaluator[key + 'Sha256'], 'snapshot evaluator input changed')
    digest, watches = audit.actual_snapshot(tool)
    depfiles, out_dirs, artifacts, mappings = set(), set(), [], []
    for event in events:
        if event.get('reason') == 'compiler-artifact':
            selected = audit.artifact_depfiles(event, mappings)
            require(selected, 'exact selected compiler depfile missing')
            depfiles.update(selected)
            artifacts.append({'packageId': event['package_id'], 'target': event['target']['name'], 'depfiles': sorted(map(str, selected))})
        if event.get('reason') == 'build-script-executed' and event['package_id'].startswith('path+file://' + str(ROOT / 'crates/iroha_plonk') + '#'):
            out_dirs.add(exact_path(event['out_dir']))
    require(out_dirs, 'complete native identity producer required')
    # Auditor output is retained to a new path on creation; replay goes to a
    # disposable in-memory sink so admission never rewrites old evidence.
    class Sink:
        def write_text(self, text):
            self.value = json.loads(text, object_pairs_hook=unique)
    sink = Sink()
    # Arguments are expected digest and observed BEFORE digest. The maintained
    # auditor independently computes/compares its AFTER snapshot and watches.
    audit.audit(SimpleNamespace(output=sink), events, depfiles, out_dirs, artifacts, tool,
                digest, digest, watches, mappings)
    result = sink.value
    require(result['passed'] is True and not result['omittedCanonicalInputs'] and result['actualSourceVerifiedBeforeAndAfter'] is True,
            'incomplete maintained compiler closure')
    require(result['identityBuilds'] and all(row['targetSupported'] is True for row in result['identityBuilds']), 'unsupported compiled native target')
    version = child_process.natural_run(['rustc', '-vV'], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    hosts = re.findall(r'^host: ([a-zA-Z0-9_-]+)$', version, re.MULTILINE)
    require(len(hosts) == 1, 'one selected native host target required')
    expected_identity = modules['kagemusha_qualify.py'].native_identity_source(digest, {'triple': hosts[0]})
    for out in out_dirs:
        identity = out / 'native_source_identity.rs'
        require(capture.stable_read(identity).decode() == expected_identity, 'generated native identity differs from exact complete current host output')
        require(set(output_map(out)) == {str(identity)}, 'unreviewed native generator output')
        declared = set()
        for line in capture.stable_read(out.parent / 'output').decode().splitlines():
            if line.startswith('cargo:rerun-if-changed='):
                declared.add(line.split('=', 1)[1])
        require(declared == {str(p) for p in watches} | {str(out / 'native-source-rescan-required')}, 'exact native watcher namespace differs')
    if create:
        save(directory / 'native-audit.json', result)
    else:
        require(result == read(directory / 'native-audit.json'), 'current generated/dependency audit differs from captured original')
    return result


def evidence(events, meta, before, after, native, modules):
    audit = modules['audit_native_source_inputs.py']
    _, watches = audit.actual_snapshot(Path(native['snapshotEvaluator']['binaryPath']))
    bind_watches(watches, before, after)
    from source_admission import check_cargo_output
    packages, pins, mappings = exact_depfiles(events, meta, before, after, modules['audit_native_source_inputs.py'], native)
    outputs = []
    for event in events:
        if event.get('reason') != 'build-script-executed':
            continue
        out = exact_path(event['out_dir'])
        output = exact_path(out.parent / 'output')
        text = capture.stable_read(output).decode()
        check_cargo_output({'text': text, 'cargo_record': event})
        generated = output_map(out)
        pins[str(output)] = sha(output); pins.update(generated)
        outputs.append({'record': event, 'output': str(output), 'sha256': sha(output), 'files': generated})
    for key in ('externalDependencyGeneratedInputs', 'externalBuildScriptsWithoutGeneratedSource'):
        for row in native[key]:
            pins[row['registryArchive']] = row['registryArchiveSha256']
            pins.update({item['path']: item['sha256'] for item in row['maintainedGeneratorInputs']})
    return {'actual_packages': packages, 'depfiles': mappings, 'build_outputs': outputs, 'current_originals': pins}


def default_features(package):
    """Expand this package's actual metadata defaults, not dependency features."""
    feature_map = package.get('features')
    require(type(feature_map) is dict and 'default' in feature_map
            and all(type(key) is str and key and type(values) is list
                    and all(type(value) is str and value for value in values)
                    for key, values in feature_map.items()), 'complete selected feature metadata required')
    features, pending = set(), ['default']
    while pending:
        feature = pending.pop()
        if feature in features:
            continue
        features.add(feature)
        for edge in feature_map[feature]:
            if edge in feature_map:
                pending.append(edge)
            else:
                require(edge.startswith('dep:') or '/' in edge, 'unknown local default feature edge')
    return features


def selected_artifact(event, meta, config):
    """Join one selected executable to prospective metadata and actual Cargo facts."""
    config = configuration(**config)
    contracts = {
        'iroha_core_zk': ('native', 'iroha_core_zk', 'crates/iroha_core_zk', ['lib'], ['lib'], 'src/lib.rs'),
        'sumeragi': ('network', 'integration_tests', 'integration_tests', ['test'], ['bin'], 'tests/sumeragi.rs'),
        'iroha3d': ('nodes', 'irohad', 'crates/irohad/bins', ['bin'], ['bin'], 'src/bin/iroha3d.rs'),
        'iroha': ('nodes', 'iroha_cli', 'crates/iroha_cli/bins', ['bin'], ['bin'], 'src/bin/iroha.rs'),
    }
    require(type(event) is dict and event.get('reason') == 'compiler-artifact'
            and type(event.get('target')) is dict, 'actual selected Cargo artifact required')
    target = event['target']
    name = target.get('name')
    require(type(name) is str and name in ROLES[config['role']][1], 'foreign selected executable role')
    role, package_name, relative, kind, crate_types, source = contracts[name]
    package_root = ROOT / relative
    manifest = str(package_root / 'Cargo.toml')
    require(type(meta) is dict and meta.get('version') == 1 and type(meta.get('version')) is int
            and meta.get('workspace_root') == str(ROOT) and type(meta.get('packages')) is list
            and type(meta.get('workspace_members')) is list
            and all(type(value) is str for value in meta['workspace_members'])
            and len(set(meta['workspace_members'])) == len(meta['workspace_members'])
            and type(meta.get('resolve')) is dict and type(meta['resolve'].get('nodes')) is list,
            'complete prospective selected package metadata required')
    packages = meta['packages']
    require(all(type(row) is dict and type(row.get('id')) is str and row['id'] for row in packages)
            and len({row['id'] for row in packages}) == len(packages), 'unique selected metadata package IDs required')
    matches = [row for row in packages if row.get('manifest_path') == manifest]
    require(len(matches) == 1, 'one exact selected metadata package required')
    package = matches[0]
    require(package.get('name') == package_name and 'source' in package and package['source'] is None
            and package['id'].startswith('path+file://' + str(package_root) + '#')
            and package['id'] in meta['workspace_members'], 'selected executable has foreign package owner')
    nodes = meta['resolve']['nodes']
    require(all(type(row) is dict and type(row.get('id')) is str for row in nodes)
            and len({row['id'] for row in nodes}) == len(nodes)
            and sum(row['id'] == package['id'] for row in nodes) == 1, 'selected package missing from complete Cargo resolution')
    require(type(package.get('targets')) is list and all(type(row) is dict for row in package['targets']),
            'complete selected target metadata required')
    targets = [row for row in package['targets'] if row.get('name') == name]
    require(len(targets) == 1 and target == targets[0]
            and target.get('kind') == kind and target.get('crate_types') == crate_types
            and target.get('src_path') == str(package_root / source) and target.get('test') is True
            and event.get('package_id') == package['id'] and event.get('manifest_path') == manifest,
            'selected executable differs from exact Cargo package/target/source')
    profile = event.get('profile')
    require(type(profile) is dict and profile.get('test') is (role != 'nodes')
            and type(profile.get('opt_level')) is str and profile['opt_level'] in ('0', '1', '2', '3', 's', 'z')
            and type(profile.get('debug_assertions')) is bool, 'selected executable has wrong actual profile')
    if config['profile'] == 'release':
        require(profile['opt_level'] == '3' and profile['debug_assertions'] is False,
                'actual optimized selected artifact required')
    features = default_features(package)
    actual = event.get('features')
    require(type(actual) is list and all(type(value) is str for value in actual)
            and len(actual) == len(features) and set(actual) == features, 'exact default selected artifact features required')
    require(not any(feature.startswith(('test-network-', 'fault-'))
                    or feature in ('sim', 'dev-tools', 'cli_integration_harness') for feature in actual),
            'nonshipping executable feature selected')
    filenames = event.get('filenames')
    require(type(event.get('executable')) is str and type(filenames) is list
            and all(type(value) is str for value in filenames) and len(set(filenames)) == len(filenames)
            and event['executable'] in filenames, 'selected executable absent from actual emitted filenames')
    original = exact_path(event['executable'])
    require(original.is_relative_to(ROOT / 'target'), 'selected executable outside current target tree')
    return original


def selected_binaries(events, meta, config, directory, *, create):
    config = configuration(**config)
    names = ROLES[config['role']][1]
    require(type(events) is list and all(type(event) is dict for event in events), 'Cargo event objects required')
    selected = {}
    for event in events:
        if event.get('reason') != 'compiler-artifact' or not event.get('executable'):
            continue
        require(type(event.get('target')) is dict and type(event['target'].get('name')) is str,
                'actual executable target object required')
        if event['target']['name'] not in names:
            continue
        name = event['target']['name']
        require(name not in selected, 'ambiguous selected executable')
        selected[name] = selected_artifact(event, meta, config)
    require(set(selected) == names, 'selected executable inventory incomplete')
    result = {}
    for name, original in selected.items():
        copied = directory / ('core-zk-tests' if name == 'iroha_core_zk' else 'sumeragi-tests' if name == 'sumeragi' else name)
        digest = sha(original)
        if create:
            with original.open('rb') as source, copied.open('xb') as stream:
                for chunk in iter(lambda: source.read(1 << 20), b''):
                    stream.write(chunk)
            copied.chmod(0o555)
        require(sha(copied) == digest, 'retained executable differs from actual Cargo original')
        result[name] = {'path': str(copied), 'sha256': digest, 'original': str(original)}
    return result


def producer(output, config):
    """Explicit capture only; failures retain all originals and never kill a child."""
    capture.require_resolved_checkout()
    output = exact_path(output, absent=True)
    require(output.is_relative_to(QUAL) and not output.exists(), 'fresh absent qualification output required')
    require(Path.cwd() == ROOT, 'run from original checkout')
    config = configuration(**config)
    modules = maintained()
    compiler_environment()
    output.mkdir(mode=0o700)
    live = False
    child = None
    try:
        before_tools = tools(modules); save(output / 'tools-before.json', before_tools)
        meta = metadata(); save(output / 'cargo-metadata-before.json', meta)
        before = source_snapshot(meta, config['role'], modules); save(output / 'source-before.json', before)
        argv = command(config)
        save(output / 'started.json', {'cwd': str(ROOT), 'command': argv, 'configuration': config,
                                      'source_before_sha256': sha(output / 'source-before.json')})
        with (output / 'build.log').open('xb') as stdout, (output / 'build.stderr.log').open('xb') as stderr:
            live = True  # Unknown spawn outcome also forbids follow-up work.
            child = subprocess.Popen(argv, cwd=ROOT, stdout=stdout, stderr=stderr)
            save(output / 'child.json', {'pid': child.pid, 'command': argv})
            code = child.wait()
            live = False
        save(output / 'build-result.json', {'exit': code})
        require(code == 0, 'Cargo build failed; retained partial capture')
        events = build_events(output / 'build.log', modules)
        artifact_before = artifact_hashes(events); save(output / 'artifact-originals.json', artifact_before)
        native = native_audit(events, output, modules, create=True)
        after_meta = metadata(); save(output / 'cargo-metadata-after.json', after_meta)
        after = source_snapshot(after_meta, config['role'], modules); save(output / 'source-after.json', after)
        require(artifact_hashes(events) == artifact_before, 'compiler artifacts changed during evaluator/source audit')
        after_tools = tools(modules); save(output / 'tools-after.json', after_tools)
        validate_source_pair(before, after)
        require(meta == after_meta, 'prospective dependency metadata changed')
        check_tools(before_tools, after_tools)
        inventory = evidence(events, meta, before, after, native, modules); save(output / 'inventory.json', inventory)
        binaries = selected_binaries(events, meta, config, output, create=True); save(output / 'binaries.json', binaries)
        protected = {name: sha(output / name) for name in ('started.json', 'build-result.json', 'build.log', 'build.stderr.log',
                    'source-before.json', 'source-after.json', 'cargo-metadata-before.json', 'cargo-metadata-after.json',
                    'tools-before.json', 'tools-after.json', 'native-audit.json', 'inventory.json', 'binaries.json',
                    'native-evaluator/source-snapshot-build.json', 'native-evaluator/source-snapshot', 'artifact-originals.json')}
        save(output / 'result.json', {'schema': SCHEMA, 'build_exit': 0, 'configuration': config, 'binaries': binaries,
             'files': protected, 'source_digest': native['sourceDigest'], 'source_before_sha256': protected['source-before.json'],
             'profile': config['profile'], 'command': argv, 'classification': 'current_complete_compiler_closure',
             'scope': 'Current engineering compiler input and emitted-artifact custody under Cargo source-to-artifact mapping trust. No independently rebuilt compiler or OS/sysroot closure. Root review, full catalog, network, resources and phones remain separate.'})
    except BaseException as error:
        try:
            save(output / 'failure.json', {'error': repr(error), 'child_may_be_running': live or isinstance(error, child_process.UnfinishedChild),
                                          'pid': error.pid if isinstance(error, child_process.UnfinishedChild) else None if child is None else child.pid,
                                          'scope': 'Failed or interrupted capture; no source admission'})
        except OSError:
            pass
        raise


def selection_paths(selection):
    require(type(selection) is dict and set(selection) == {'directory', 'result_sha256', 'review_sha256'}, 'exact current compiler capture; no supplements')
    directory = exact_path(selection['directory'])
    require(directory.is_dir() and directory.is_relative_to(QUAL), 'retained compiler capture required')
    require(not os.path.lexists(directory / 'withdrawal.json'), 'selected compiler review withdrawn')
    for name, expected in (('result.json', selection['result_sha256']), ('review.json', selection['review_sha256'])):
        require(sha(directory / name) == pin(expected), 'selected compiler original changed')
    return directory


def admit(selection, expected):
    capture.require_resolved_checkout()
    directory = selection_paths(selection)
    review, result = read(directory / 'review.json'), read(directory / 'result.json')
    require(review == {'schema': REVIEW_SCHEMA, 'result_sha256': selection['result_sha256'], 'accepted_current_compiler_closure': True,
                       'reviewer_role': 'root', 'scope': 'engineering compiler inputs only'}, 'explicit exact root compiler review required')
    require(result['schema'] == SCHEMA and result['classification'] == 'current_complete_compiler_closure' and
            type(result['build_exit']) is int and result['build_exit'] == 0, 'successful current complete compiler capture required')
    config = configuration(**result['configuration'])
    require(result['profile'] == config['profile'] and result['command'] == command(config)
            and result['source_before_sha256'] == result['files']['source-before.json'], 'compiler result command/source binding differs')
    require(ROLES[config['role']][1] == expected == set(result['binaries']), 'different executable inventory')
    require(set(result['files']) == {'started.json', 'build-result.json', 'build.log', 'build.stderr.log', 'source-before.json',
            'source-after.json', 'cargo-metadata-before.json', 'cargo-metadata-after.json', 'tools-before.json', 'tools-after.json',
            'native-audit.json', 'inventory.json', 'binaries.json', 'native-evaluator/source-snapshot-build.json', 'native-evaluator/source-snapshot', 'artifact-originals.json'},
            'incomplete retained compiler evidence')
    retained = {str(directory / name): pin(digest) for name, digest in result['files'].items()}
    retained.update({str(directory / name): selection[key] for name, key in [('result.json', 'result_sha256'), ('review.json', 'review_sha256')]})
    for name, digest in retained.items():
        require(sha(Path(name)) == digest, 'retained compiler evidence changed')
    started = read(directory / 'started.json')
    require(started == {'cwd': str(ROOT), 'command': command(config), 'configuration': config,
                       'source_before_sha256': result['files']['source-before.json']}, 'selected compiler invocation differs')
    require(read(directory / 'build-result.json') == {'exit': 0}, 'natural successful compiler terminal required')
    modules = maintained()
    captured_tools = read(directory / 'tools-before.json')
    check_tools(captured_tools, read(directory / 'tools-after.json'))
    check_tools(captured_tools, tools(modules))
    meta = read(directory / 'cargo-metadata-before.json')
    require(meta == read(directory / 'cargo-metadata-after.json') == metadata(), 'compiler dependency metadata changed')
    before = read(directory / 'source-before.json')
    validate_source_pair(before, read(directory / 'source-after.json'))
    validate_source_pair(before, source_snapshot(meta, config['role'], modules))
    events = build_events(directory / 'build.log', modules)
    artifact_before = read(directory / 'artifact-originals.json')
    require(artifact_hashes(events) == artifact_before, 'captured emitted compiler artifacts changed')
    native = native_audit(events, directory, modules, create=False)
    require(native['sourceDigest'] == result['source_digest'], 'compiled native identity differs')
    inventory = evidence(events, meta, before, before, native, modules)
    require(inventory == read(directory / 'inventory.json'), 'exact compiler dependency/generated inventory differs')
    require(artifact_hashes(events) == artifact_before, 'compiler artifacts changed during admission')
    binaries = selected_binaries(events, meta, config, directory, create=False)
    require(binaries == result['binaries'] == read(directory / 'binaries.json'), 'retained Cargo executable map differs')
    retained.update(inventory['current_originals'])
    packages = inventory['actual_packages']
    local = before['files']
    custody = {'compiler_selection': selection, 'expected_binaries': sorted(expected), 'before_head': before['head'], 'after_head': before['head'],
               'reviewed_originals': retained, 'tools': captured_tools, 'execution_tools': captured_tools,
               'withdrawal_path': str(directory / 'withdrawal.json'), 'packages': packages,
               'namespace': sorted(name for name in before['files'] if source_audit.in_package(name, packages))}
    return result, inventory, local, custody


def recheck(owner):
    require(source_audit.package_paths(ROOT, owner['packages']) == set(owner['namespace']), 'compiled package namespace changed')
    admit(owner['compiler_selection'], set(owner['expected_binaries']))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--role', choices=sorted(ROLES), required=True)
    parser.add_argument('--profile', choices=('release', 'debug'), default='release')
    parser.add_argument('--slot', required=True)
    parser.add_argument('--jobs', type=int, default=2)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    producer(args.output, configuration(args.role, args.profile, args.slot, args.jobs))


if __name__ == '__main__':
    raise SystemExit('use authenticated capture_build.py launcher')
