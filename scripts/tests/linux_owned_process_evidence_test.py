"""Closed Linux record joins without any native module or real process launch."""
from __future__ import annotations
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import types
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import linux_owned_process_evidence as evidence
import private_settlement_process_observer as observer
import private_settlement_session_control as control
from linux_owned_process_observer_test import fixture


def records_fixture(exit_code=0):
    """Construct explicit synthetic records for strict replay mutation controls."""
    reader,_,terminal=fixture();identity=reader['capture']['sample']['identity']
    native={'schema':'iroha.root-thread-group-observation.v1',
        'scope':'original_root_thread_group_lifecycle','supervisor_pid':23456,
        'root_pid':12345,'prepared_ns':1,'observed_through_ns':90,'initial_exec_ns':10,
        'possible_cpus':[0],'online_cpus':[0],'records':3,'target_tasks':1,
        'counters':[{'cpu':0,'lost':0}],'first_error':None,
        'terminal':{'pid':12345,'code':os.CLD_EXITED,'status':exit_code},
        'finalized':True,'lifecycle_certified':True,'image_authenticated':False,
        'descendant_cleanup':'root_thread_group_only'}
    admission={'version':1,'kind':'linux_owned_initial_admission','reader':reader,
        'supervisor_pid':23456,'delivery':{'version':1,'kind':'independently_pinned_native_observer',
            'module_sha256':'b'*64,'provenance_sha256':'c'*64,'source_manifest_sha256':'d'*64,
            'cargo_lock_sha256':'e'*64,'native_target':'x86_64-unknown-linux-gnu'}}
    lifecycle={'version':1,'kind':'linux_owned_final_lifecycle','native_json':control.canonical(native).decode(),
        'wait4':{'pid':12345,'status':exit_code<<8,'cpu_user_ns':150_000_000,'cpu_system_ns':180_000_000},
        'wnowait':dict(native['terminal']),'finish_before_reap':True}
    process={'pid':12345,'exit':{'kind':'exited','code':exit_code},'timed_out':False,
        'cpu_user_ns':150_000_000,'cpu_system_ns':180_000_000,
        'sampled':{'state':'not_sampled','observations':1}}
    baseline={'started_monotonic_ns':19,'finished_monotonic_ns':31,
        'processes':[dict(label='measured_command',**copy.deepcopy(reader['capture']['sample']))]}
    context={'artifact':'sha256:'+identity['executable_sha256']}
    return admission,lifecycle,terminal,process,baseline,context


EXPECTED={'module_sha256':'b'*64,'provenance_sha256':'c'*64,'source_manifest_sha256':'d'*64,
          'native_target':'x86_64-unknown-linux-gnu'}


class EvidenceControls(unittest.TestCase):
    """No schema, first-error, status, resource or reference mismatch may pass."""

    def validate(self,values):
        evidence.validate(*values,1000,expected_delivery=EXPECTED)

    def test_actual_exit101_fact_can_authenticate_without_becoming_success(self):
        values=records_fixture(101);self.validate(values)
        self.assertEqual(values[3]['exit'],{'kind':'exited','code':101})

    def test_initial_sample_required_even_without_a_terminal_stat(self):
        values=list(records_fixture());values[2]=None;self.validate(values)
        values[0]['reader']['capture']['sample']['rss_bytes']=0
        with self.assertRaises(observer.ProcessObservationError):self.validate(values)

    def test_every_top_level_admission_and_lifecycle_field_is_closed(self):
        for index in (0,1):
            for key in records_fixture()[index]:
                values=records_fixture();del values[index][key]
                with self.assertRaises((ValueError,KeyError,TypeError,RuntimeError)):self.validate(values)
            values=records_fixture();values[index]['extra']=None
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)

    def test_delivery_is_independently_pinned_and_exact_hash_shape(self):
        for key,value in [('kind','release_authority'),('module_sha256','0'*64),
                          ('provenance_sha256',None),('version',True)]:
            values=records_fixture();values[0]['delivery'][key]=value
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)

    def test_context_artifact_parent_and_pid_must_join_actual_initial_identity(self):
        for index,key,value in [(0,'supervisor_pid',23457),(3,'pid',12346),(5,'artifact','sha256:'+'f'*64)]:
            values=records_fixture();values[index][key]=value
            with self.assertRaises((ValueError,RuntimeError)):self.validate(values)

    def test_every_wait4_field_and_wnowait_join_refuses_mutation(self):
        for key in ('pid','status','cpu_user_ns','cpu_system_ns'):
            values=records_fixture();values[1]['wait4'][key]+=1
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)
        for key in ('pid','code','status'):
            values=records_fixture();values[1]['wnowait'][key]+=1
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)
        values=records_fixture();values[1]['finish_before_reap']=False
        with self.assertRaises(observer.ProcessObservationError):self.validate(values)

    def test_native_negative_loss_or_exec_cannot_authenticate_terminal(self):
        for key,value in [('lifecycle_certified',False),('finalized',False),
                          ('first_error',{'reason':'exec','errno':None})]:
            values=records_fixture();native=json.loads(values[1]['native_json']);native[key]=value
            values[1]['native_json']=control.canonical(native).decode()
            with self.assertRaises((ValueError,RuntimeError)):self.validate(values)
        values=records_fixture();native=json.loads(values[1]['native_json']);native['counters'][0]['lost']=1
        values[1]['native_json']=control.canonical(native).decode()
        with self.assertRaises((ValueError,RuntimeError)):self.validate(values)

    def test_baseline_sample_and_both_scope_endpoints_join_actual_capture(self):
        for key in ('cpu_time_ns','cpu_counter_unit_ns','rss_bytes'):
            values=records_fixture();values[4]['processes'][0][key]+=1
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)
        for key,value in [('started_monotonic_ns',21),('finished_monotonic_ns',29)]:
            values=records_fixture();values[4][key]=value
            with self.assertRaises(observer.ProcessObservationError):self.validate(values)

    def test_ordinary_failed_observation_remains_failed_despite_valid_terminal(self):
        values=records_fixture();values[3]['sampled']={'state':'failed','reason':'ValueError'}
        with self.assertRaisesRegex(observer.ProcessObservationError,'already failed'):self.validate(values)

    def snapshot(self,values):
        """Bind exact generated fixture bytes without claiming kernel source authority."""
        admission,lifecycle,terminal,process,baseline,context=values
        raw={evidence.PATHS['admission']:control.canonical(admission),
             evidence.PATHS['lifecycle']:control.canonical(lifecycle),
             'observations/baseline.json':control.canonical(baseline)}
        if terminal is not None:raw[evidence.PATHS['terminal']]=control.canonical(terminal)
        bindings={path:{'path':path,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
                  for path,data in raw.items()}
        process['sampled']['linux_owned']={key:bindings.get(path) for key,path in evidence.PATHS.items()}
        return process,types.SimpleNamespace(raw=raw,bindings=bindings),context

    def test_replay_requires_exact_reference_inventory_and_presence(self):
        process,snapshot,context=self.snapshot(records_fixture())
        self.assertEqual(evidence.replay(process,snapshot,context,1000, expected_delivery=EXPECTED, required_linux=True),[])
        for key in evidence.PATHS:
            p,s,c=self.snapshot(records_fixture());del p['sampled']['linux_owned'][key]
            self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),['linux_owned_evidence_invalid'])
            p,s,c=self.snapshot(records_fixture());p['sampled']['linux_owned'][key]=None
            self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),['linux_owned_evidence_invalid'])

    def test_missing_or_changed_snapshot_and_additional_terminal_cannot_pass(self):
        for key in evidence.PATHS:
            p,s,c=self.snapshot(records_fixture());del s.raw[evidence.PATHS[key]]
            self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),['linux_owned_evidence_invalid'])
        values=list(records_fixture());values[2]=None
        p,s,c=self.snapshot(values);self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),[])
        s.bindings[evidence.PATHS['terminal']]={'path':evidence.PATHS['terminal'],'bytes':2,'sha256':'d'*64}
        self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),['linux_owned_evidence_invalid'])

    def test_declared_owned_contract_cannot_drop_whole_bundle_or_change_mode(self):
        p,s,c=self.snapshot(records_fixture())
        del p['sampled']['linux_owned']
        for path in evidence.PATHS.values():
            s.raw.pop(path,None);s.bindings.pop(path,None)
        self.assertEqual(evidence.replay(p,s,c,1000,required_linux=True, expected_delivery=EXPECTED),['linux_owned_evidence_invalid'])
        p,s,c=self.snapshot(records_fixture())
        self.assertEqual(evidence.replay(p,s,c,1000,required_linux=False),['linux_owned_evidence_invalid'])

    def test_absent_reference_cannot_hide_existing_owned_evidence(self):
        p,s,c=self.snapshot(records_fixture());del p['sampled']['linux_owned']
        self.assertEqual(evidence.replay(p,s,c,1000, expected_delivery=EXPECTED, required_linux=True),['linux_owned_evidence_invalid'])


if __name__=='__main__':unittest.main()
