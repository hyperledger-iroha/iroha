"""Cargo-log/output provenance controls, including the actual withdrawn capture."""
from pathlib import Path
import copy, json, unittest
import source_admission as m

class CargoRecordTests(unittest.TestCase):
    def row(self):
        return dict(text='cargo:rustc-env=DATA=a=b\ncargo::rustc-cfg=feature="data"\ncargo:rustc-link-lib=static=data\ncargo:rustc-link-search=native=/DATA\ncargo:rustc-check-cfg=cfg(data)\n',
            cargo_record=dict(reason='build-script-executed',env=[['DATA','a=b']],cfgs=['feature="data"'],linked_libs=['static=data'],linked_paths=['native=/DATA']))
    def test_all_four_compiler_fields_join_exactly(self):
        self.assertEqual(m.check_cargo_output(self.row())['env'],[['DATA','a=b']])
    def test_changed_missing_or_extra_cargo_values_refuse(self):
        for key in ('env','cfgs','linked_libs','linked_paths'):
            for value in ([],['foreign'],None):
                row=self.row();row['cargo_record'][key]=value
                with self.assertRaisesRegex(ValueError,'disagree'):m.check_cargo_output(row)
    def test_duplicate_env_and_unrepresented_compiler_flags_refuse(self):
        for line in ('cargo:rustc-env=DATA=x','cargo:rustc-env=broken','cargo:rustc-flags=-l x','cargo:rustc-link-arg=-foreign'):
            row=self.row();row['text']+=line+'\n'
            with self.assertRaises(ValueError):m.check_cargo_output(row)
    def test_foreign_record_kind_refuses(self):
        row=self.row();row['cargo_record']['reason']='compiler-artifact'
        with self.assertRaisesRegex(ValueError,'build-script'):m.check_cargo_output(row)
    def test_actual_capture_three_metadata_contradiction_refuses(self):
        p=Path(__file__).resolve().parent/'fixtures/cargo-record-contradiction.json'
        rows=json.loads(p.read_bytes());accepted=[];refused=[]
        for row in rows:
            try:m.check_cargo_output(row)
            except ValueError as e:
                self.assertIn('disagree with Cargo record: env',str(e));refused.append(row['cargo_record']['package_id'])
            else:accepted.append(row)
        self.assertEqual(len(accepted),11)
        self.assertEqual({x.split('#')[0].split('/crates/')[1] for x in refused},{'iroha_cli/bins','irohad/bins'})

if __name__=='__main__':unittest.main()
