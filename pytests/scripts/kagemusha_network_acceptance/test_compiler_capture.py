"""Pure DATA mutations for current compiler closure; never starts Cargo/native work."""
from pathlib import Path
from paths import QUAL
from types import SimpleNamespace
import copy
import hashlib
import json
import os
import tempfile
import unittest
from unittest.mock import patch
import compiler_capture as c
import child_process
import capture_build as launcher


class CurrentCompilerCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=QUAL)
        self.assertTrue(Path(self.temp.name).is_relative_to(QUAL))
        self.assertEqual(Path(self.temp.name).stat().st_mode & 0o777, 0o700)
        self.root = Path(self.temp.name).resolve()
        self.addCleanup(self.temp.cleanup)
        self.patchers = [patch.object(c, 'ROOT', self.root), patch.object(c, 'QUAL', self.root / 'target/qualification')]
        for item in self.patchers:
            item.start(); self.addCleanup(item.stop)

    def file(self, name, data=b'DATA'):
        path = self.root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
        return path

    def test_current_slot_is_explicit_and_has_no_historical_pin(self):
        for slot in ('fresh-current', 'another-slot'):
            cfg = c.configuration('native', 'release', slot, 2)
            argv = c.command(cfg)
            self.assertIn(str(self.root / 'target/cargo-fast' / slot), argv)
            self.assertIn('--locked', argv); self.assertIn('--offline', argv)
            self.assertEqual(argv[-1], '--message-format=json')
            self.assertIn('--no-run', argv)
        for slot in ('../foreign', '/absolute', '', 'a/b'):
            with self.assertRaises(ValueError): c.configuration('native', 'release', slot, 2)
        for jobs in (True, 0, 65):
            with self.assertRaises(ValueError): c.configuration('native', 'release', 'current', jobs)
        for role in ('native', 'network'):
            with self.assertRaises(ValueError): c.configuration(role, 'debug', 'current', 2)

    def test_selection_refuses_every_historical_supplement_before_io(self):
        selected = {'directory': '/DATA', 'result_sha256': '12'*32, 'review_sha256': '34'*32}
        with patch.object(c, 'exact_path', side_effect=AssertionError('no original read')):
            for extra in ('source_equivalence', 'historical_runtime_supplement', 'closure_directory', 'bytes_retirement', 'closure_sha256', 'inventory_tool', 'closure_tool'):
                with self.assertRaisesRegex(ValueError, 'no supplements'):
                    c.selection_paths(selected | {extra: {}})

    def test_unpinned_native_tool_overrides_refuse(self):
        for name in ('CC', 'HOST_CC', 'CC_aarch64_unknown_linux_gnu', 'CXX', 'AR', 'RANLIB', 'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER', 'CMAKE_TOOLCHAIN_FILE', 'SDKROOT'):
            with patch.dict(os.environ, {name: '/unreviewed/tool'}, clear=True):
                with self.assertRaisesRegex(ValueError, 'native compiler/linker'): c.compiler_environment()

    def test_review_withdrawal_and_original_change_refuse(self):
        directory = self.root / 'target/qualification/fresh'; directory.mkdir(parents=True)
        for name in ('result.json', 'review.json'): (directory/name).write_text('{}')
        selected = {'directory': str(directory), 'result_sha256': c.sha(directory/'result.json'), 'review_sha256': c.sha(directory/'review.json')}
        self.assertEqual(c.selection_paths(selected), directory)
        (directory/'withdrawal.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'withdrawn'): c.selection_paths(selected)
        (directory/'withdrawal.json').unlink(); (directory/'review.json').write_text('{"changed":true}')
        with self.assertRaisesRegex(ValueError, 'changed'): c.selection_paths(selected)

    def test_prospective_watched_input_cannot_be_pinned_after_build(self):
        watched = self.file('ignored/source.rs')
        before = {'files': {}, 'dependencies': {}, 'administration': {}, 'links': {}}
        after = copy.deepcopy(before); after['files']['ignored/source.rs'] = c.sha(watched)
        with self.assertRaisesRegex(ValueError, 'prospective'): c.bind_watches({watched}, before, after)
        before['files'] = dict(after['files'])
        c.bind_watches({watched}, before, after)
        watched.write_text('mutated')
        with self.assertRaises(ValueError): c.bind_watches({watched}, before, after)

    def test_absent_native_watcher_presence_is_not_a_source_omission(self):
        watched = self.root / '.git/info/exclude'
        snap = {'files': {}, 'dependencies': {}, 'administration': {'.git/info/exclude': None}, 'links': {}}
        c.bind_watches({watched}, snap, snap)
        self.file('.git/info/exclude')
        with self.assertRaises(ValueError): c.bind_watches({watched}, snap, snap)
        with self.assertRaises(ValueError): c.bind_watches({self.root/'foreign-absent'}, snap, snap)

    def dep_fixture(self):
        manifest = self.file('crates/pkg/Cargo.toml')
        source = self.file('crates/pkg/src/lib.rs')
        artifact = self.file('target/current/deps/libpkg-1.rlib')
        dep = artifact.with_name('pkg-1.d')
        dep.write_text(str(artifact)+': '+str(source)+'\n')
        package = {'id': 'local-pkg', 'name': 'pkg', 'version': '1.2.3', 'manifest_path': str(manifest)}
        event = {'reason': 'compiler-artifact', 'package_id': 'local-pkg', 'manifest_path': str(manifest), 'filenames': [str(artifact)], 'target': {'name': 'pkg', 'kind': ['lib']}}
        snapshot = {'files': {'crates/pkg/src/lib.rs': c.sha(source)}, 'dependencies': {}, 'administration': {}}
        native = {key: [] for key in ('canonicalGeneratedInputs', 'externalDependencyGeneratedInputs', 'generatedExceptions', 'absentBuildWatchExceptions')}
        native['depfiles'] = [{'path': str(dep), 'sha256': c.sha(dep)}]
        audit = SimpleNamespace(inputs=lambda path, absent: {source})
        return event, {'packages': [package]}, snapshot, native, audit, dep, source

    def test_exact_emitted_target_and_prospective_inputs_are_both_required(self):
        event, meta, before, native, audit, dep, source = self.dep_fixture()
        packages, _, mapping = c.exact_depfiles([event], meta, before, before, audit, native)
        self.assertEqual(packages, ['crates/pkg']); self.assertEqual(mapping[0]['inputs'], [str(source)])
        after = copy.deepcopy(before); before['files'] = {}
        with self.assertRaisesRegex(ValueError, 'prospective'): c.exact_depfiles([event], meta, before, after, audit, native)
        before['files'] = dict(after['files']); dep.write_text('/foreign-target: '+str(source)+'\n')
        native['depfiles'][0]['sha256'] = c.sha(dep)
        with self.assertRaisesRegex(ValueError, 'target rule'): c.exact_depfiles([event], meta, before, after, audit, native)

    def test_auditor_first_rule_cannot_substitute_for_actual_emitted_rule(self):
        event, meta, snap, native, audit, dep, source = self.dep_fixture()
        audit.inputs = lambda path, absent: {self.file('different.rs')}
        with self.assertRaisesRegex(ValueError, 'actual emitted target'): c.exact_depfiles([event], meta, snap, snap, audit, native)

    def test_generated_input_requires_exact_typed_disposition_and_bytes(self):
        event, meta, snap, native, audit, dep, source = self.dep_fixture()
        generated = self.file('target/current/build/out/native_source_identity.rs')
        dep.write_text(event['filenames'][0]+': '+str(source)+' '+str(generated)+'\n')
        native['depfiles'][0]['sha256'] = c.sha(dep)
        audit.inputs = lambda path, absent: {source, generated}
        with self.assertRaisesRegex(ValueError, 'prospective'): c.exact_depfiles([event], meta, snap, snap, audit, native)
        native['generatedExceptions'] = [{'path': str(generated), 'sha256': c.sha(generated)}]
        c.exact_depfiles([event], meta, snap, snap, audit, native)
        generated.write_text('incomplete=false')
        with self.assertRaisesRegex(ValueError, 'generated compiler input changed'): c.exact_depfiles([event], meta, snap, snap, audit, native)

    def test_external_dependency_byte_is_not_merely_metadata(self):
        event, meta, snap, native, audit, dep, source = self.dep_fixture()
        external = self.file('registry/dependency/lib.rs')
        dep.write_text(event['filenames'][0]+': '+str(external)+'\n'); native['depfiles'][0]['sha256']=c.sha(dep)
        audit.inputs=lambda path, absent:{external}
        snap['dependencies']['registry:'+str(external)] = c.sha(external)
        c.exact_depfiles([event], meta, snap, snap, audit, native)
        external.write_text('changed')
        with self.assertRaisesRegex(ValueError, 'prospective'): c.exact_depfiles([event], meta, snap, snap, audit, native)

    def test_every_emitted_dependency_artifact_is_pinned(self):
        event, meta, snap, native, audit, dep, source = self.dep_fixture()
        _, pins, _ = c.exact_depfiles([event], meta, snap, snap, audit, native)
        filename = event['filenames'][0]
        self.assertEqual(pins[filename], c.sha(Path(filename)))
        captured = c.artifact_hashes([event]); Path(filename).write_text('substituted')
        self.assertNotEqual(c.artifact_hashes([event]), captured)

    def test_derived_environment_foreign_output_and_unknown_cargo_input_refuse(self):
        event, meta, snap, native, audit, dep, source = self.dep_fixture(); package=meta['packages'][0]
        c.environment_dependencies('# env-dep:CARGO_PKG_VERSION=1.2.3\n', event, package, [event])
        for text in ('# env-dep:CARGO_PKG_VERSION=4.5.6\n', '# env-dep:OUT_DIR=/foreign\n', '# env-dep:CARGO_UNKNOWN=1\n'):
            with self.assertRaises(ValueError): c.environment_dependencies(text, event, package, [event])
        with patch.dict(os.environ, {'CONTROL':'current'}):
            with self.assertRaisesRegex(ValueError, 'environment changed'): c.environment_dependencies('# env-dep:CONTROL=old\n', event, package, [event])

    def test_natural_child_interruption_never_signals_or_starts_followup(self):
        class Child:
            pid=1234
            def communicate(self): raise KeyboardInterrupt()
            def kill(self): raise AssertionError('kill forbidden')
            def terminate(self): raise AssertionError('terminate forbidden')
        with patch.object(c.subprocess, 'Popen', return_value=Child()) as spawn:
            with self.assertRaises(child_process.UnfinishedChild) as raised: child_process.natural_run(['cargo','metadata'], capture_output=True)
            self.assertEqual(raised.exception.pid, 1234); self.assertEqual(spawn.call_count, 1)
        with patch.object(c.subprocess, 'Popen', side_effect=KeyboardInterrupt()):
            with self.assertRaises(child_process.UnfinishedChild) as raised: child_process.natural_run(['cargo','metadata'])
            self.assertIsNone(raised.exception.pid)

    def test_precampaign_unknown_child_identity_survives_uncaught_diagnostic(self):
        import traceback
        argv = ['cargo', 'metadata', '--locked', '--offline']
        for pid in (None, 741):
            with self.subTest(pid=pid):
                error = child_process.UnfinishedChild(argv, pid)
                # Before a campaign exists, the interpreter prints this exact
                # diagnostic; recording storage is neither needed nor assumed.
                rendered = ''.join(traceback.format_exception_only(type(error), error))
                self.assertIn(f'pid={pid!r}', rendered)
                self.assertIn(f'argv={argv!r}', rendered)
                self.assertIn('no signals sent', rendered)
                self.assertIn('reconciliation required', rendered)
                self.assertEqual(error.argv, argv)
                self.assertEqual(error.pid, pid)

    def test_natural_nonzero_child_retains_completed_failure(self):
        child=SimpleNamespace(returncode=7, communicate=lambda:(b'partial',b'error'))
        with patch.object(c.subprocess, 'Popen', return_value=child):
            result=child_process.natural_run(['cargo'], capture_output=True)
            self.assertEqual((result.returncode,result.stdout,result.stderr),(7,b'partial',b'error'))

    def test_aliases_and_nonregular_inputs_refuse(self):
        original=self.file('original'); link=self.root/'alias'; link.symlink_to(original)
        with self.assertRaisesRegex(ValueError,'aliased'):c.sha(link)
        with self.assertRaises(ValueError):c.sha(self.root)

    def test_executed_maintained_source_hash_must_join_tool_capture(self):
        path=self.file('scripts/helper.py',b'value=1\n')
        module=SimpleNamespace(__source_sha256__=c.sha(path))
        path.write_text('value=2\n')
        with self.assertRaisesRegex(ValueError,'executed maintained helper'):c.tools({'helper.py':module})

    def test_capture_bootstrap_refuses_changed_or_preloaded_helpers(self):
        here=self.root/'scripts/capture';here.mkdir(parents=True)
        self.file('Cargo.toml');(self.root/'.git').mkdir()
        local={}
        for name in launcher.LOCAL:
            p=here/name;p.write_text('SAFE=True\n');local[p.name]=hashlib.sha256(p.read_bytes()).hexdigest()
        external={}
        for name in launcher.EXTERNAL:
            p=self.file(name,b'SAFE=True\n');external[name]=hashlib.sha256(p.read_bytes()).hexdigest()
        manifest=here/'manifest.json';manifest.write_text(json.dumps({'schema':launcher.SCHEMA,'runtime':{'local':local,'external':external}}))
        expected=hashlib.sha256(manifest.read_bytes()).hexdigest()
        with self.assertRaisesRegex(ValueError,'preloaded'):launcher.bootstrap(manifest,expected)
        (here/'capture.py').write_text('SAFE=False\n')
        with self.assertRaisesRegex(ValueError,'changed'):launcher.bootstrap(manifest,expected)


if __name__ == '__main__':
    unittest.main()
