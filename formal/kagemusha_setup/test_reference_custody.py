"""Reference and public-byte controls; the obsolete A1 descriptor is DATA only."""
import ast
import copy
import inspect
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from . import custody, rebind
from .custody import HERE, ROOT, checked_sources, sha

DATA_PIN=(39386,'e7e535287ff5b2f41ff3c4a92dac549981b1dea243ba191930cc24932a51c087')


class ReferenceCustodyTests(unittest.TestCase):
    def temporary(self):
        result=tempfile.TemporaryDirectory(prefix='formal-reference-custody-')
        self.addCleanup(result.cleanup)
        return Path(result.name).resolve()

    def test_exact_descriptor_data_has_no_constructor_or_override(self):
        self.assertFalse((HERE/'load_a1_case.py').exists())
        self.assertNotIn('resource_curve',inspect.signature(rebind._reference).parameters)
        self.assertNotIn('load_a1_case.py',custody.FILES)
        manifest=checked_sources()
        self.assertEqual(manifest['files']['load_a1_descriptor.norito'],DATA_PIN[1])
        missing=copy.deepcopy(manifest);del missing['files']['load_a1_descriptor.norito']
        with self.assertRaisesRegex(ValueError,'exact maintained inventory'):
            custody.validate_manifest(missing)
        fixture=HERE/'load_a1_descriptor.norito';raw=fixture.read_bytes()
        self.assertEqual((len(raw),sha(raw)),DATA_PIN)
        read=Path.read_bytes
        def changed(path):return read(path)+b'changed' if path==fixture else read(path)
        with patch.object(Path,'read_bytes',changed):
            with self.assertRaisesRegex(ValueError,'changed source load_a1_descriptor.norito'):
                checked_sources()
        modules=rebind.reference(self.temporary()/'official')
        with self.assertRaisesRegex(ValueError,'reference domain exponent'):
            modules['descriptor'].Descriptor.decode(raw,2)
        # Canonical frame fields inspected as DATA; no semantic admission or
        # resource override is used to turn a historical descriptor into a case.
        cursor=modules['codec'].descriptor_frame(raw,2);fields=[]
        while cursor.position<len(cursor.data):fields.append(cursor.read(cursor.length()))
        cursor.finish()
        self.assertEqual((len(fields),fields[5],fields[9],fields[11]),(25,b'\x10',b'\x08',b'\x06'))
        self.assertNotEqual(int.from_bytes(fields[11],'little'),(int.from_bytes(fields[9],'little')-2)//2)
        self.assertEqual((int.from_bytes(fields[9],'little')-2)//2,3)
        with self.assertRaises(TypeError):
            rebind._reference(self.temporary()/'forbidden',resource_curve=1)

    def test_canonical_header_key_and_scalar_binding(self):
        from .public_setup import PublicSetup
        class Descriptor:
            n=1;digest=b'd'*32;curve=SimpleNamespace(scalar=101)
            def __getitem__(self,name):return {'num_fixed_columns':1,'permutation':[0]}[name]
        d=Descriptor();key=b'key';tail=(5).to_bytes(32,'little')+(7).to_bytes(32,'little')
        raw=b'PIPAPK01'+d.digest+len(key).to_bytes(4,'little')+key+b'c'*32+tail
        got=PublicSetup.decode(raw,d,key,sha(raw));self.assertEqual(got.fixed,((5,),));self.assertEqual(got.sigma,((7,),))
        for offset in (0,8,40,44):
            changed=bytearray(raw);changed[offset]^=1
            with self.assertRaises(ValueError):PublicSetup.decode(bytes(changed),d,key,sha(changed))
        with self.assertRaises(ValueError):PublicSetup.decode(raw[:-1],d,key,sha(raw[:-1]))
        changed=raw[:-32]+(101).to_bytes(32,'little')
        with self.assertRaisesRegex(ValueError,'canonical scalar'):PublicSetup.decode(changed,d,key,sha(changed))

    def test_private_authority_precedes_any_import(self):
        from . import rebind as shared
        root=self.temporary();wire=(6).to_bytes(4,'little')+bytes(64*64+64)
        def before():
            kats=json.loads((root/'private/kats_v1.json').read_text())
            self.assertEqual(kats['params_ipa'],{'ep':[],'eq':[{'k':6,'byte_len':len(wire),'sha256':sha(wire)}]})
            receipt=json.loads((root/'private/copy-receipt.json').read_text())
            self.assertTrue(receipt['authority_before_import'])
            raise RuntimeError('stop before loader')
        with patch.object(shared.importlib.util,'spec_from_file_location',side_effect=AssertionError('early import')) as loader:
            with self.assertRaisesRegex(RuntimeError,'stop before loader'):
                shared._reference(root/'private',parameter_authority=(1,6,wire),before_import=before)
            loader.assert_not_called()

    def test_no_production_authority_bridge(self):
        from . import rebind as shared
        root=self.temporary();modules=shared.reference(root/'official')
        official=(ROOT/'fixtures/native_prover/kats_v1.json').read_bytes()
        self.assertEqual(json.loads((root/'official/kats_v1.json').read_text()),json.loads(official))
        wire=(6).to_bytes(4,'little')+bytes(64*64+64)
        pin=next(row for row in json.loads(official)['params_ipa']['eq'] if row['k']==6)
        self.assertNotEqual(sha(wire),pin['sha256'])
        with self.assertRaisesRegex(ValueError,'unpinned parameters'):
            modules['parameters'].Parameters.decode(wire,modules['curve'].Curve(1),6)
        self.assertEqual((ROOT/'fixtures/native_prover/kats_v1.json').read_bytes(),official)

    def test_reference_copy_delta_namespace_and_digest_guards(self):
        root=self.temporary()
        modules=rebind.reference(root/'small')
        receipt=json.loads((root/'small/copy-receipt.json').read_text())
        self.assertTrue(receipt['authority_before_import'])
        for row in receipt['source_changes']:
            self.assertEqual(row['before_sha256'],row['after_sha256'])
        with self.assertRaises(FileExistsError):rebind.reference(root/'small')
        self.assertIn('_decide',vars(modules['parameters']))
        with self.assertRaises(TypeError):
            rebind.reference(root/'bad',exact_descriptor_sha="bad' injection")
        self.assertFalse((root/'bad').exists())
        with self.assertRaisesRegex(ValueError,'small reference authority'):
            rebind.reference(root/'no-large',parameter_authority=(1,16,b''))
        self.assertFalse((root/'no-large').exists())

    def test_verbatim_reference_has_no_large_resource_selector(self):
        root=self.temporary();seen=[]
        def before():
            seen.append(True)
            self.assertEqual(json.loads((root/'ordinary/kats_v1.json').read_text()),
                             json.loads((ROOT/'fixtures/native_prover/kats_v1.json').read_text()))
        modules=rebind._reference(root/'ordinary',before_import=before)
        self.assertEqual(seen,[True])
        receipt=json.loads((root/'ordinary/copy-receipt.json').read_text())
        self.assertNotIn('exact_k16_descriptor',receipt)
        self.assertNotIn('OMEGA',vars(rebind))
        for row in receipt['source_changes']:
            self.assertEqual(row['before_sha256'],row['after_sha256'])
            copied=Path(modules['descriptor'].__file__).parent/row['name']
            self.assertEqual(copied.read_bytes(),(custody.REFERENCE/row['name']).read_bytes())
        for entry in (rebind.reference,rebind._reference,rebind.make_case):
            self.assertNotIn('allow_large',inspect.signature(entry).parameters)
            self.assertNotIn('exact_descriptor_sha',inspect.signature(entry).parameters)
        for entry in (rebind.reference,rebind._reference):
            for options in ({'allow_large':True},{'exact_descriptor_sha':'0'*64},
                            {'resource_curve':0}):
                with self.assertRaises(TypeError):entry(root/'forbidden',**options)
                self.assertFalse((root/'forbidden').exists())
        for tag in (0,1):
            with self.assertRaisesRegex(ValueError,'reference parameter exponent'):
                modules['parameters'].Parameters.decode(b'',modules['curve'].Curve(tag),16)

    def test_same_ordered_parameter_reconstruction_and_checkpoints(self):
        seen=[];ticks=[]
        curve=SimpleNamespace(multiply=lambda base,value:seen.append((base,value)) or value,
                              encode=lambda value:value.to_bytes(32,'little'))
        encoded=rebind._parameter_bytes(curve,'sentinel-base',1,(1,2),(3,4),5,6,lambda:ticks.append(True))
        self.assertEqual(encoded,(1).to_bytes(4,'little')+b''.join(i.to_bytes(32,'little') for i in range(1,7)))
        self.assertEqual(seen,[('sentinel-base',i) for i in range(1,7)])
        self.assertEqual(len(ticks),6)
        node=next(n for n in ast.walk(ast.parse(inspect.getsource(rebind._finish_case)))
                  if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id=='scalar_ifft')
        self.assertEqual(ast.unparse(node.args[-1]),'checkpoint')
