"""Real archive/source joins reuse the unchanged original wheel fixture harness.

The subprocess executes that maintained harness, then its actual ZIP/RECORD
builders and sole parser. No package/native binary or release candidate executes.
"""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

ROOT = next(parent for parent in Path(__file__).resolve().parents
            if (parent / "ci/privacy_sdk_cargo_lockfile_test.sh").is_file())
HELPER = Path(__file__).resolve().parents[1] / "sorafs_python_package_source.py"


def test_exact_candidate_package_source_and_same_version_mutations(tmp_path):
    source = r'''
import importlib.util, json, shutil, stat, sys
from pathlib import Path
root, helper, fixture = map(Path, sys.argv[1:])
fixture.mkdir()
sys.path.insert(0, str(root / 'scripts'))
import sorafs_python_producer_inputs as inputs
spec = importlib.util.spec_from_file_location('original_byte_controls', root / 'scripts/tests/python_wheel_byte_owner_test.py')
controls = importlib.util.module_from_spec(spec); spec.loader.exec_module(controls)
original = {'__name__': '__main__', '__file__': str(controls.SHELL_HARNESS)}
sys.argv = [str(controls.SHELL_HARNESS), str(root / 'ci/verify_privacy_python_wheel.py'), str(fixture / 'wheel-controls')]
exec(compile(controls.extract_original_harness(controls.SHELL_HARNESS.read_bytes()), str(controls.SHELL_HARNESS), 'exec'), original)
spec = importlib.util.spec_from_file_location('package_source_join', helper)
join = importlib.util.module_from_spec(spec); spec.loader.exec_module(join)
verifier, member, with_record, write_wheel = inputs.verifier, original['member'], original['with_record'], original['write_wheel']
results = []

def candidate(label, owner):
    selected = fixture / label
    project = selected / 'python' / owner.package
    package = project / 'src' / owner.package
    package.mkdir(parents=True)
    for filename in ('pyproject.toml', 'README.md'):
        (project / filename).write_bytes((root / 'python' / owner.package / filename).read_bytes())
    content = {'__init__.py': b'# fixed candidate initializer\n'}
    if owner.native:
        content['_loader.py'] = b'# fixed native loader\n'
    else:
        content.update({'client.py': b'# fixed SDK client\n', 'examples/__init__.py': b'',
                        'py.typed': b'', 'examples/connect_app_metadata.json': b'{"candidate":1}\n'})
    for name, raw in content.items():
        path = package / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
    return selected, {owner.package + '/' + name: raw for name, raw in content.items()}

sequence = 0
def parse(owner, content, version='0.0.1', directories=()):
    global sequence
    sequence += 1
    dist = owner.package + '-' + version + '.dist-info'
    metadata = ('Metadata-Version: 2.3\nName: ' + owner.distribution + '\nVersion: ' + version + '\n')
    if not owner.native:
        metadata += 'Requires-Dist: iroha-native==' + version + '\n'
    entries = [member(name, raw) for name, raw in sorted(content.items())]
    if owner.native:
        entries.append(member(owner.package + '/_crypto.abi3.so', original['native_bytes']))
    entries.extend(member(name, b'', stat.S_IFDIR | 0o755) for name in directories)
    entries += [member(dist + '/METADATA', metadata.encode()),
                member(dist + '/WHEEL', ('Wheel-Version: 1.0\nRoot-Is-Purelib: ' + ('false' if owner.native else 'true') + '\nTag: ' + ('cp312-abi3-any' if owner.native else 'py3-none-any') + '\n').encode())]
    path = write_wheel(fixture / ('candidate-' + str(sequence) + '.whl'), with_record(entries, dist + '/RECORD'))
    return verifier.parse_wheel_bytes(path.read_bytes(), owner=owner, extension_suffixes=('.abi3.so',))

def authenticate(parsed, selected):
    with inputs.OriginalInputs() as owner:
        first = join.authenticate_package_source(parsed, selected, owner)
        second = join.authenticate_package_source(parsed, selected, owner)
        assert first == second
        owner.recheck()
    return first

def refused(label, operation):
    try:
        operation()
    except (inputs.ArtifactError, inputs.child.QualificationError):
        results.append(label)
    else:
        raise AssertionError('accepted changed candidate ownership: ' + label)

for owner in (verifier.NATIVE_OWNER, verifier.SDK_OWNER):
    selected, content = candidate(owner.package + '-original', owner)
    parsed = parse(owner, content)
    retained = authenticate(parsed, selected)
    assert retained == {'python/' + owner.package + '/' + key: raw for key, raw in
                        [('pyproject.toml', (selected / 'python' / owner.package / 'pyproject.toml').read_bytes()),
                         ('README.md', (selected / 'python' / owner.package / 'README.md').read_bytes()),
                         *(('src/' + name, raw) for name, raw in content.items())]}
    results.append(owner.package + ':exact-and-recheck')
    changed = dict(content); changed[owner.initializer] += b'# different same-version code\n'
    refused(owner.package + ':changed-same-version', lambda: authenticate(parse(owner, changed), selected))
    added = dict(content); added[owner.package + '/unowned.py'] = b'# unowned\n'
    refused(owner.package + ':added-same-version', lambda: authenticate(parse(owner, added), selected))
    missing = dict(content); missing.pop(owner.package + ('/_loader.py' if owner.native else '/client.py'))
    refused(owner.package + ':missing-same-version', lambda: authenticate(parse(owner, missing), selected))
    refused(owner.package + ':other-version', lambda: authenticate(parse(owner, content, '0.0.2'), selected))
    refused(owner.package + ':empty-wheel-directory', lambda: authenticate(parse(owner, content, directories=(owner.package + '/foreign/',)), selected))
    assert authenticate(parse(owner, content, directories=(owner.package + '/',)), selected) == retained
    results.append(owner.package + ':owned-wheel-directory')
    project = selected / 'python' / owner.package
    for name in ('setup.py', 'setup.cfg', 'MANIFEST.in'):
        extra = project / name; extra.write_bytes(b'# unowned build hook\n')
        refused(owner.package + ':' + name, lambda: authenticate(parsed, selected))
        extra.unlink()
    recipe = project / 'pyproject.toml'; before = recipe.read_bytes()
    recipe.write_bytes(before + b'\n# changed reviewed build recipe\n')
    refused(owner.package + ':recipe-pin', lambda: authenticate(parsed, selected))
    recipe.write_bytes(before)
    package = project / 'src' / owner.package
    unknown = package / 'unowned.json'; unknown.write_bytes(b'{}')
    refused(owner.package + ':unowned-source-data', lambda: authenticate(parsed, selected))
    unknown.unlink()
    foreign = project / 'src/foreign'; foreign.mkdir()
    refused(owner.package + ':extra-source-package', lambda: authenticate(parsed, selected))
    foreign.rmdir()
    alias = package / 'alias.py'; alias.symlink_to(package / '__init__.py')
    refused(owner.package + ':source-symlink', lambda: authenticate(parsed, selected))
    alias.unlink()
    # Explicitly excluded cache/native build outputs do not become source owners.
    for name in ('old.pyc', 'old.pyo', 'old.so', 'old.dylib', 'old.pyd'):
        (package / name).write_bytes(b'not source')
    (package / '__pycache__').mkdir(); (package / '__pycache__/old.pyc').write_bytes(b'not source')
    (project / 'src' / (owner.package + '.egg-info')).mkdir()
    assert authenticate(parsed, selected) == retained
    results.append(owner.package + ':excluded-build-outputs')
    # An original same-byte rewrite must not replace the held source owner.
    def drift():
        with inputs.OriginalInputs() as originals:
            join.authenticate_package_source(parsed, selected, originals)
            path = package / '__init__.py'; raw = path.read_bytes(); path.write_bytes(raw)
            originals.recheck()
    refused(owner.package + ':same-byte-original-rewrite', drift)
    # The final helper recapture detects a new source, even with prior files intact.
    def addition():
        with inputs.OriginalInputs() as originals:
            join.authenticate_package_source(parsed, selected, originals)
            (package / 'late.py').write_bytes(b'# late source\n')
            join.authenticate_package_source(parsed, selected, originals)
    refused(owner.package + ':late-source-addition', addition)

# SDK package data is required and byte-identical independently of Python code.
for action in ('missing', 'changed'):
    selected, content = candidate('sdk-data-' + action, verifier.SDK_OWNER)
    name = 'iroha_python/examples/connect_app_metadata.json'
    if action == 'missing': content.pop(name)
    else: content[name] = b'{"candidate":2}\n'
    refused('sdk-data:' + action, lambda: authenticate(parse(verifier.SDK_OWNER, content), selected))

# Reuse actual current package trees/recipe bytes as an additional positive join.
for owner in (verifier.NATIVE_OWNER, verifier.SDK_OWNER):
    package = root / 'python' / owner.package / 'src' / owner.package
    content = {owner.package + '/' + path.relative_to(package).as_posix(): path.read_bytes()
               for path in package.rglob('*') if path.is_file() and '__pycache__' not in path.parts
               and (path.suffix == '.py' or path.relative_to(package).as_posix() in ('py.typed', 'examples/connect_app_metadata.json'))}
    actual = authenticate(parse(owner, content), root)
    assert len(actual) == len(content) + 2
    results.append(owner.package + ':actual-current-source')
print('PACKAGE_SOURCE_CONTROLS=' + json.dumps(results, separators=(',', ':')))
'''
    result = subprocess.run((sys.executable, "-I", "-B", "-c", source,
                             str(ROOT), str(HELPER), str(tmp_path / "fixture")),
                            capture_output=True, text=True, timeout=120, check=False)
    (tmp_path / "original-package-source-harness.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "two-wheel bounded archive, installed-origin, loader, missing-owner and tamper checks passed" in result.stdout
    reports = [line.removeprefix("PACKAGE_SOURCE_CONTROLS=") for line in result.stdout.splitlines()
               if line.startswith("PACKAGE_SOURCE_CONTROLS=")]
    assert len(reports) == 1
    rows = json.loads(reports[0])
    assert len(rows) == len(set(rows)) == 38
