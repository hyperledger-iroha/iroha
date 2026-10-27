"""Synthetic current-stage record and process-lifecycle mutations; no native work."""
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import protocol as p
import run as r
from test_current_protocol import A, B, C, D, PINS, REFERENCE


class CurrentRecordsTests(unittest.TestCase):
    def tearDown(self):
        r._UNFINISHED_CHILDREN.clear()

    def test_runtime_root_tracks_its_enclosing_checkout_not_ambient_cwd(self):
        import paths
        self.assertEqual(paths.repository_root(__file__), r.ROOT)
        with tempfile.TemporaryDirectory(dir=r.QUAL, prefix='root-resolver-fake-') as tmp:
            root=Path(tmp); (root/'Cargo.toml').write_text('[workspace]');(root/'.git').write_text('synthetic checkout marker')
            inner=root/'scripts';inner.mkdir();source=inner/'candidate.py';source.write_text('# fake source')
            self.assertEqual(paths.repository_root(source),root)

    def test_selected_tool_accepts_only_exact_unaliased_bounded_review_original(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL, prefix='tool-selection-fake-') as tmp:
            root=Path(tmp);tool=root/'tool.py';tool.write_text('# retained source DATA')
            selected=r.ref(r.observe(tool));self.assertEqual(r.selected_tool(selected),tool)
            for wrong in [dict(selected,sha256=A),dict(selected,extra='forbidden')]:
                with self.assertRaises(ValueError):r.selected_tool(wrong)
            alias=root/'alias.py';alias.symlink_to(tool)
            with self.assertRaises(ValueError):r.selected_tool(dict(selected,path=str(alias)))
            with self.assertRaises(ValueError):r.selected_tool(selected,cap=1)

    def test_uncertain_compiler_child_is_retained_before_failure_io(self):
        for pid in [None, 741]:
            for broken_storage in [False, True]:
                r._UNFINISHED_CHILDREN.clear()
                error=r.child_process.UnfinishedChild(['cargo','metadata'],pid)
                with patch.object(r,'save',side_effect=OSError('fake disk failure') if broken_storage else None) as saved:
                    value=r.retain_failure(Path('/fake/campaign'),error)
                self.assertEqual(r._UNFINISHED_CHILDREN,[{'pid':pid,'command':['cargo','metadata'],
                    'directory':'/fake/campaign','role':'compiler_custody'}])
                self.assertEqual(saved.call_count,1)
                if broken_storage:self.assertIsNone(value)
        r._UNFINISHED_CHILDREN.clear()
        with patch.object(r,'save',side_effect=OSError('fake disk failure')):
            with self.assertRaises(OSError):r.retain_failure(Path('/fake/campaign'),ValueError('completed refusal'))
        self.assertEqual(r._UNFINISHED_CHILDREN,[])

    def test_registration_requires_actual_setup_cut_and_every_current_catalog_pin(self):
        value = {'schema':'iroha.kagemusha.network-registered.v1', 'setup_sha256':A,
                 'scheme_id':A, 'manifest_digest':A, 'verifier_pack_sha256':B,
                 'predecessor_height':6, 'certified_height':7,
                 'registration_transaction_hash_hex':C, 'registration_source_sha256':D}
        p.registered(value, A, {'live_height':6}, PINS, {'sha256':B})
        for field in ['setup_sha256','scheme_id','manifest_digest','verifier_pack_sha256',
                      'predecessor_height','certified_height','registration_source_sha256']:
            with self.subTest(field=field), self.assertRaises(ValueError):
                p.registered(dict(value, **{field:None}), A, {'live_height':6}, PINS, {'sha256':B})
        for cut in [True, 1, 62]:
            with self.assertRaises(ValueError):
                p.registered(value, A, {'live_height':cut}, PINS, {'sha256':B})

    def test_issuer_ready_requires_same_current_genesis_height_peer_and_private_config(self):
        selected={'peer_index':2, 'configuration':{'sha256':C}}
        value={'schema':'iroha.kagemusha.network-issuer-ready.v1', 'setup_sha256':A,
               'registered_sha256':B, 'configuration_sha256':C, 'peer_index':2,
               'certified_height':7, 'signed_genesis_sha256':D}
        p.issuer_ready(value,A,{'sha256':B},{'certified_height':7},selected,D)
        for field in value:
            with self.subTest(field=field), self.assertRaises(ValueError):
                p.issuer_ready(dict(value,**{field:None}),A,{'sha256':B},{'certified_height':7},selected,D)

    def test_activation_retention_records_preserve_same_wire_no_new_signatures(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL, prefix='activation-records-test-') as tmp:
            output=Path(tmp); wire=r.bounded.save(output/'signed-activation.wire',b'fake signed activation data')
            prepared=r.save(output/'activation-prepared.json', {'synthetic':'prepared'})
            wire=r.observe(output/'signed-activation.wire')
            receipt={'schema':'iroha.kagemusha.wallet-activation-retained.v1','action':'retain-activation',
                     'signedWireSHA256':wire['sha256'],'signedWireBytes':str(wire['bytes']),
                     'nativeResult':'awaiting','entrypointHex':C,'nativeReturnedTicks':'1234',
                     'invocation':'12345678-abcd-1234-abcd-123456789abc','sequence':'0'}
            retained=r.save(output/'activation-retained.json',receipt)
            scope={'schema':'iroha.kagemusha.native-software-activation-retention.v1','source_pins':PINS,
                   'prepared_sha256':prepared['sha256'],'target_sha256':A,'retention_sha256':retained['sha256'],
                   'new_payment_signatures':0,'physical_device_qualified':False}
            r.save(output/'component-scope.json',scope)
            env=PINS|{'KAGEMUSHA_NATIVE_ACTIVATION_PREPARED_SHA256':prepared['sha256'],
                'KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256':A,'KAGEMUSHA_NATIVE_ACTIVATION_INVOCATION':receipt['invocation']}
            got=r.component.output_records('activation-retention',output,env)
            self.assertEqual(got['wire'],wire)
            for field,bad in [('new_payment_signatures',1),('new_payment_signatures',False),
                              ('physical_device_qualified',True),('target_sha256',B)]:
                path=output/'component-scope.json'; path.unlink(); r.save(path,dict(scope,**{field:bad}))
                with self.assertRaises(ValueError):r.component.output_records('activation-retention',output,env)
                path.unlink(); r.save(path,scope)
            # Malformed ticks and a foreign invocation remain refused even if the
            # enclosing DATA digest is updated; DATA never turns into authority.
            for field,bad in [('nativeReturnedTicks','0'),('nativeReturnedTicks','01'),
                              ('nativeReturnedTicks',str(1<<64)),('invocation','foreign'),('sequence','1'),
                              ('signedWireSHA256',B),('nativeResult','started'),
                              ('nativeResult','not_started'),('nativeResult','confirmed'),('nativeResult','rejected'),
                              ('entrypointHex',None),('entrypointHex',0),('entrypointHex',''),
                              ('entrypointHex','0'*64),('entrypointHex',C.upper()),
                              ('entrypointHex',C[:-1]),('entrypointHex','g'*64)]:
                path=output/'activation-retained.json';path.unlink()
                changed=r.save(path,dict(receipt,**{field:bad}))
                scope_path=output/'component-scope.json';scope_path.unlink()
                r.save(scope_path,dict(scope,retention_sha256=changed['sha256']))
                with self.assertRaises(ValueError):r.component.output_records('activation-retention',output,env)

            # The current closed receipt has nine fields. Recomputing the enclosing
            # DATA digest cannot admit a retired receipt, missing field or extension.
            retired=dict(receipt,nativeResult='not_started');del retired['entrypointHex']
            candidates=[retired,dict(receipt,extra='forbidden')]
            for field in receipt:
                missing=receipt.copy();del missing[field];candidates.append(missing)
            for candidate in candidates:
                path=output/'activation-retained.json';path.unlink()
                changed=r.save(path,candidate)
                scope_path=output/'component-scope.json';scope_path.unlink()
                r.save(scope_path,dict(scope,retention_sha256=changed['sha256']))
                with self.assertRaises(ValueError):r.component.output_records('activation-retention',output,env)

    def test_listing_retains_child_and_exact_census_without_subprocess_run(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL,prefix='listing-fake-test-') as tmp:
            directory=Path(tmp); child=Mock(pid=553);child.wait.return_value=0
            def spawn(command,**kwargs):
                self.assertEqual(command,['/fake/current','selected','--exact','--list','--ignored'])
                kwargs['stdout'].write('selected: test\n\n1 test, 0 benchmarks\n')
                return child
            with patch.object(r.subprocess,'Popen',side_effect=spawn), \
                    patch.object(r.subprocess,'run',side_effect=AssertionError('subprocess.run may signal')):
                r.retained_listing({'path':'/fake/current','sha256':A},'selected',directory,'list',['--ignored'],{})
            row=r.read_document(directory/'list-process.json')
            self.assertEqual(row['pid'],553);self.assertFalse(row['signals_permitted'])
            child.kill.assert_not_called();child.terminate.assert_not_called()
            self.assertEqual(r.read_document(directory/'list-exit.json'),{'exit':0})

    def test_listing_receipt_failure_waits_naturally_and_never_retries(self):
        with tempfile.TemporaryDirectory(dir=r.QUAL,prefix='listing-failure-test-') as tmp:
            child=Mock(pid=554);child.wait.return_value=0
            with patch.object(r.subprocess,'Popen',return_value=child) as spawned, \
                    patch.object(r,'save',side_effect=OSError('simulated retained write failure')):
                with self.assertRaises(OSError):
                    r.retained_listing({'path':'/fake/current','sha256':A},'selected',Path(tmp),'list',[],{})
            self.assertEqual(spawned.call_count,1);self.assertEqual(child.wait.call_count,1)
            child.kill.assert_not_called();child.terminate.assert_not_called()


if __name__=='__main__': unittest.main()
