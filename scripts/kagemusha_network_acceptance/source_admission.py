"""Current compiler-capture admission; no historical source-equivalence path."""
from pathlib import Path
import hashlib
import capture
import source_audit
from paths import ROOT, QUAL


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    import compiler_capture
    return compiler_capture.sha(Path(path))


def check_runtime_custody(owner):
    import compiler_capture
    compiler_capture.recheck(owner)


def check_cargo_output(output):
    """Join retained compiler-affecting directives to the actual captured Cargo record."""
    parsed = {'env': [], 'cfgs': [], 'linked_libs': [], 'linked_paths': []}
    names = set()
    keys = {'rustc-cfg': 'cfgs', 'rustc-link-lib': 'linked_libs',
            'rustc-link-search': 'linked_paths'}
    for line in output['text'].splitlines():
        if line.startswith('cargo::'):
            directive = line[len('cargo::'):]
        elif line.startswith('cargo:'):
            directive = line[len('cargo:'):]
        else:
            continue
        key, separator, value = directive.partition('=')
        if key == 'rustc-env':
            name, equal, data = value.partition('=')
            if not separator or not equal or not name or name in names:
                raise ValueError('ambiguous retained Cargo environment directive')
            names.add(name)
            parsed['env'].append([name, data])
        elif key in keys:
            if not separator or not value:
                raise ValueError('malformed retained Cargo compiler directive')
            parsed[keys[key]].append(value)
        elif key.startswith('rustc-') and key != 'rustc-check-cfg':
            raise ValueError('unreviewed retained Cargo compiler directive')
    record = output['cargo_record']
    if record.get('reason') != 'build-script-executed':
        raise ValueError('captured Cargo build-script record required')
    for key, values in parsed.items():
        if record.get(key) != values:
            raise ValueError('retained compiler directives disagree with Cargo record: ' + key)
    return parsed



def admit_selection(selection, expected):
    import compiler_capture
    return compiler_capture.admit(selection, expected)
