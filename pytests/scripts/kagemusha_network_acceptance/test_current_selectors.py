"""Lexical selector drift guards; actual prebuilt --list remains authoritative.

These checks read only the current native module declarations and test attributes.
They do not compile Rust or establish that any native test has executed.
"""
from pathlib import Path
import re
import tempfile
import unittest
import run as r

SOURCE_ROOT = r.ROOT


def preceding_attributes(source, start):
    """Read adjacent one-line attributes used by the selected native modules."""
    result = []
    for line in reversed(source[:start].splitlines()):
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith('#[') and stripped.endswith(']'):
            result.append(stripped)
        else:
            break
    return result


def resolve(root, selector, ignored, reader=None):
    """Follow actual file/path module names; refuse missing or ambiguous declarations."""
    reader = reader or (lambda path: path.read_text())
    names = selector.split('::')
    if names[0] == 'kagemusha_network_setup':
        path = root / 'integration_tests/tests/sumeragi.rs'
    elif names[0] in ('kagemusha_wallet_advance_v1', 'kagemusha_wallet_artifacts_v1'):
        path = root / 'crates/iroha_core_zk/src/lib.rs'
    else:
        raise ValueError('unknown current native module root')
    source = reader(path)
    observed = [path]
    owner_uses_path = False
    for name in names[:-1]:
        found = list(re.finditer(r'(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?mod\s+' + re.escape(name) + r'\s*([;{])', source))
        if len(found) != 1:
            raise ValueError('missing or ambiguous module: ' + name)
        item = found[0]
        if item.group(1) == '{':
            if name != 'tests':
                raise ValueError('unreviewed inline module')
            source = source[item.end():]
            continue
        selected = [re.fullmatch(r'#\[path\s*=\s*"([^"]+)"\]', attr) for attr in preceding_attributes(source,item.start())]
        selected = [match.group(1) for match in selected if match]
        if len(selected) > 1:
            raise ValueError('ambiguous module path')
        if selected:
            child = path.parent / selected[0]
        else:
            # An explicit #[path] owner resolves default children beside its selected file.
            base = path.parent if owner_uses_path or path.name in ('lib.rs','mod.rs','sumeragi.rs') else path.with_suffix('')
            child = base / (name + '.rs')
        if not child.is_file():
            raise ValueError('missing native module original: ' + str(child))
        path = child
        owner_uses_path = bool(selected)
        source = reader(path)
        observed.append(path)
    functions = list(re.finditer(r'(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+' + re.escape(names[-1]) + r'\s*\(',source))
    if len(functions) != 1:
        raise ValueError('missing or ambiguous native function: ' + names[-1])
    attributes = preceding_attributes(source,functions[0].start())
    if '#[test]' not in attributes:
        raise ValueError('selected function is not a native test')
    if any(attr.startswith('#[ignore') for attr in attributes) != ignored:
        raise ValueError('selected native ignore status differs')
    return observed


class CurrentSelectorTests(unittest.TestCase):
    def test_every_selected_current_native_test_is_declared_with_exact_ignore_status(self):
        ordinary = (*r.HELPERS, *r.NATIVE_HELPERS)
        phases = tuple(selector for _, selector in r.PREFLIGHT_TESTS)
        self.assertEqual(len(ordinary),33)
        self.assertEqual(len(set((*ordinary,*phases))),41)
        for selector in ordinary:
            with self.subTest(selector=selector):resolve(SOURCE_ROOT,selector,False)
        for selector in phases:
            with self.subTest(selector=selector):resolve(SOURCE_ROOT,selector,True)

    def test_every_current_issuer_control_is_selected_and_creator_is_only_listed(self):
        issuer = SOURCE_ROOT / 'integration_tests/tests/kagemusha_network_setup/continuation/issuer'
        declared = set()
        for suffix, path in [('tests::', issuer / 'tests.rs'),
                             ('provisioning::tests::', issuer / 'provisioning/tests.rs'),
                             ('provisioning::scopes::tests::', issuer / 'provisioning/scopes/tests.rs')]:
            source = path.read_text()
            for function in re.finditer(r'(?m)^fn\s+([A-Za-z0-9_]+)\s*\(', source):
                attributes = preceding_attributes(source, function.start())
                if '#[test]' in attributes and not any(attr.startswith('#[ignore') for attr in attributes):
                    declared.add(r.ISSUER + suffix + function.group(1))
        self.assertEqual(len(declared), 18)
        self.assertEqual({test for test in r.HELPERS if test.startswith(r.ISSUER)}, declared)
        self.assertNotIn(r.ISSUER_COMPOSER, (*r.HELPERS, *r.NATIVE_HELPERS))
        self.assertEqual([role for role, test in r.PREFLIGHT_TESTS if test == r.ISSUER_COMPOSER], ['sumeragi'])

    def test_retired_filename_based_namespace_and_missing_helper_are_refused(self):
        for selector in [
            'kagemusha_wallet_enrollment_v1::tests::native_exchange::export_actual_a_load_target_with_retained_native_custody',
            'kagemusha_network_setup::registration_incarnation_requires_one_isolated_signed_creation',
        ]:
            with self.subTest(selector=selector),self.assertRaises(ValueError):resolve(SOURCE_ROOT,selector,False)

    def test_source_rename_removed_test_attribute_and_ignore_changes_refuse(self):
        selector=r.HELPERS[0]
        name=selector.split('::')[-1]
        for old,new in [('fn '+name+'(', 'fn retired_'+name+'('),
                        ('#[test]\nfn '+name, 'fn '+name),
                        ('#[test]\nfn '+name, '#[test]\n#[ignore]\nfn '+name)]:
            def changed(path):return path.read_text().replace(old,new)
            with self.subTest(replacement=new),self.assertRaises(ValueError):resolve(SOURCE_ROOT,selector,False,changed)

    def test_foreign_or_ambiguous_module_path_refuses(self):
        def changed(path):
            text=path.read_text()
            if path.name=='kagemusha_wallet_advance_v1.rs':
                text=text.replace('mod enrollment_owner_tests;', 'mod wrong_owner_tests;')
            return text
        with self.assertRaises(ValueError):resolve(SOURCE_ROOT,r.TARGET,True,changed)
        def duplicate(path):
            text=path.read_text()
            if path.name=='lib.rs':text+='\npub mod kagemusha_wallet_advance_v1;\n'
            return text
        with self.assertRaises(ValueError):resolve(SOURCE_ROOT,r.TARGET,True,duplicate)

    def test_explicit_path_owner_default_children_resolve_beside_selected_file(self):
        for explicit in (False, True):
            with self.subTest(explicit=explicit), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                tests = root / 'integration_tests/tests'
                tests.mkdir(parents=True)
                owner = tests / ('selected.rs' if explicit else 'kagemusha_network_setup.rs')
                declaration = ('#[path = "selected.rs"]\n' if explicit else '')
                (tests / 'sumeragi.rs').write_text(declaration + 'mod kagemusha_network_setup;\n')
                owner.write_text('mod tests;\n')
                expected = (owner.parent if explicit else owner.with_suffix('')) / 'tests.rs'
                wrong = (owner.with_suffix('') if explicit else owner.parent) / 'tests.rs'
                for path, function in [(expected, 'selected'), (wrong, 'wrong_owner')]:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text('#[test]\nfn ' + function + '() {}\n')
                observed = resolve(root, 'kagemusha_network_setup::tests::selected', False)
                self.assertEqual(observed[-1], expected)

    def test_issuer_children_keep_explicit_file_ownership(self):
        issuer = SOURCE_ROOT / 'integration_tests/tests/kagemusha_network_setup/continuation/issuer'
        for owner, children in [
            (issuer / 'provisioning.rs', {
                'configuration': 'provisioning/configuration.rs',
                'inputs': 'provisioning/inputs.rs',
                'scopes': 'provisioning/scopes.rs',
                'tests': 'provisioning/tests.rs',
            }),
            (issuer / 'provisioning/scopes.rs', {'tests': 'scopes/tests.rs'}),
        ]:
            source = owner.read_text()
            for name, relative in children.items():
                with self.subTest(owner=owner, child=name):
                    matches = list(re.finditer(r'(?m)^mod ' + re.escape(name) + r';$', source))
                    self.assertEqual(len(matches), 1)
                    attributes = preceding_attributes(source, matches[0].start())
                    self.assertIn('#[path = "' + relative + '"]', attributes)
                    self.assertTrue((owner.parent / relative).is_file())


if __name__ == '__main__':unittest.main()
