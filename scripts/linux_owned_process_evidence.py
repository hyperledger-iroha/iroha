"""Closed public Linux observer records and their deterministic replay joins.

This validates trusted native/reader projections and the admitted manifest bytes.
It does not reconstruct perf events or raw proc records, or turn a diagnostic
Cargo build receipt into independent release authority.
"""
from __future__ import annotations

import hashlib
import json
import os
import re

import linux_owned_process_observer as owned
import linux_root_observer_abi as abi
import private_settlement_process_observer as observer
import private_settlement_session_control as control
import resource_observation_contract as contracts

PATHS = {'admission':'observations/linux-root-admission.json',
         'lifecycle':'observations/linux-root-lifecycle.json',
         'terminal':'observations/linux-root-terminal.json'}
MAX_BYTES = 128 * 1024


def _hex(value):
    return type(value) is str and re.fullmatch(r'[0-9a-f]{64}',value) is not None and value!='0'*64


def _delivery(value,expected_delivery=None):
    """Admit only normal independent publisher delivery, never diagnostic receipts."""
    try:contracts.validate_projection(value,expected_delivery)
    except ValueError as error:
        raise observer.ProcessObservationError('independent observer delivery refused') from error


def _wait_join(native, waited, process):
    observer.require(type(waited) is dict and set(waited)=={
        'pid','status','cpu_user_ns','cpu_system_ns'}
        and all(owned._unsigned(waited[key]) for key in waited)
        and waited['pid']==native['root_pid']==process['pid']
        and waited['cpu_user_ns']==process['cpu_user_ns']
        and waited['cpu_system_ns']==process['cpu_system_ns'],
        'original wait4 resource join differs')
    observer.require(all(owned._unsigned(process[key]) for key in ('pid','cpu_user_ns','cpu_system_ns'))
                     and type(process['timed_out']) is bool, 'invalid outer process counters')
    terminal=native['terminal'];status=waited['status']
    observer.require(status<1<<32, 'wait4 status extent')
    if terminal['code']==os.CLD_EXITED:
        expected={'kind':'exited','code':terminal['status']}
        valid=os.WIFEXITED(status) and os.WEXITSTATUS(status)==terminal['status']
    else:
        expected={'kind':'signaled','signal':terminal['status']}
        valid=(os.WIFSIGNALED(status) and os.WTERMSIG(status)==terminal['status']
               and bool(os.WCOREDUMP(status))==(terminal['code']==os.CLD_DUMPED))
    observer.require(valid and control.canonical(process['exit'])==control.canonical(expected),
                     'original terminal and wait4 status differ')


def validate(admission,lifecycle,terminal,process,baseline,context,slack_ns,*,expected_delivery):
    """Validate complete original, native, sample and kernel-total joins together."""
    observer.require(type(admission) is dict and set(admission)=={
        'version','kind','reader','delivery','supervisor_pid'}
        and type(admission['version']) is int and admission['version']==1
        and admission['kind']=='linux_owned_initial_admission', 'invalid Linux admission record')
    owned.validate_admission(admission['reader']);_delivery(admission['delivery'],expected_delivery)
    reader=admission['reader'];capture=reader['capture'];sample=capture['sample'];identity=sample['identity']
    observer.require(type(admission['supervisor_pid']) is int
        and admission['supervisor_pid']==identity['ppid']
        and identity['pid']==process['pid']
        and type(context) is dict and context['artifact']=='sha256:'+identity['executable_sha256'],
        'Linux original executable or parent differs from run context')
    observer.require(type(lifecycle) is dict and set(lifecycle)=={
        'version','kind','native_json','wait4','wnowait','finish_before_reap'}
        and type(lifecycle['version']) is int and lifecycle['version']==1
        and lifecycle['kind']=='linux_owned_final_lifecycle'
        and lifecycle['finish_before_reap'] is True, 'invalid Linux lifecycle record')
    observer.require(type(lifecycle['native_json']) is str, 'missing exact frozen native bytes')
    native=abi.report(lifecycle['native_json'].encode('utf-8'),admission['supervisor_pid'],process['pid'])
    observer.require(native['lifecycle_certified'] and native['finalized'],
                     'Linux lifecycle not certified')
    observer.require(control.canonical(lifecycle['wnowait'])==control.canonical(native['terminal']),
                     'original WNOWAIT differs from native terminal')
    _wait_join(native,lifecycle['wait4'],process)
    observer.require(native['initial_exec_ns']<=capture['started_ns']<=capture['finished_ns']
        <=native['observed_through_ns'], 'initial live sample outside native image interval')
    observer.require(type(baseline) is dict and type(baseline.get('processes')) is list
        and len(baseline['processes'])==1, 'missing complete original baseline')
    row=baseline['processes'][0]
    observer.require(type(row) is dict and set(row)=={'label',*sample}
        and row['label']=='measured_command'
        and control.canonical({key:row[key] for key in sample})==control.canonical(sample)
        and owned._unsigned(baseline['started_monotonic_ns'])
        and owned._unsigned(baseline['finished_monotonic_ns'])
        and baseline['started_monotonic_ns']<=capture['started_ns']<=capture['finished_ns']
            <=baseline['finished_monotonic_ns'], 'baseline differs from actual owned initial sample')
    sampled=process['sampled']
    if sampled['state']=='validated':
        observer.require(owned._unsigned(sampled['sampled_cpu_time_ns'])
                         and type(sampled['observations']) is int and sampled['observations']>=2,
                         'invalid Linux sampled totals')
        last_cpu=sample['cpu_time_ns']+sampled['sampled_cpu_time_ns']
    elif sampled['state']=='not_sampled':
        observer.require(type(sampled.get('observations')) is int and sampled['observations']==1,
                         'Linux lifecycle lacks initial sample')
        last_cpu=sample['cpu_time_ns']
    else:
        # Failed records stay refused, even if a later lifecycle is complete.
        raise observer.ProcessObservationError('Linux process observation already failed')
    total=process['cpu_user_ns']+process['cpu_system_ns']
    observer.require(owned._unsigned(total) and last_cpu<=total+slack_ns,
                     'live CPU exceeds actual wait4 accounting')
    if terminal is not None:
        owned.validate_terminal(terminal,reader,native,last_cpu,total,slack_ns)


def publish(owner,records,status,user_ns,system_ns,sampled,slack_ns):
    """Publish exact independently frozen projections; do not invent missing facts."""
    observer.require(owner.joined and owner.finish_attempted and owner.waited is not None
                     and owner.native.final_raw is not None,'Linux observation is not completely joined')
    owner.module_owner.validate();owner.workload.validate()
    admission={'version':1,'kind':'linux_owned_initial_admission',
        'reader':owner.reader.admission(),'delivery':owner.module_owner.evidence(),
        'supervisor_pid':owner.native.supervisor}
    _delivery(admission['delivery']);owned.validate_admission(admission['reader'])
    native=abi.report(owner.native.final_raw,owner.native.supervisor,owner.child.pid)
    lifecycle={'version':1,'kind':'linux_owned_final_lifecycle',
        'native_json':owner.native.final_raw.decode('utf-8'),
        'wait4':{'pid':owner.waited[0],'status':status,'cpu_user_ns':user_ns,'cpu_system_ns':system_ns},
        'wnowait':owner.wnowait,'finish_before_reap':True}
    if owner.terminal_fact is not None:
        initial=admission['reader']['capture']['sample']['cpu_time_ns']
        last_cpu=initial+sampled.get('sampled_cpu_time_ns',0)
        owned.validate_terminal(owner.terminal_fact,admission['reader'],native,last_cpu,
                                user_ns+system_ns,slack_ns)
    result={}
    for key,value in (('admission',admission),('lifecycle',lifecycle),('terminal',owner.terminal_fact)):
        if value is None:
            result[key]=None
        else:
            raw=control.canonical(value)
            observer.require(len(raw)<=MAX_BYTES,'Linux observer evidence exceeds bound')
            result[key]=records.publish(PATHS[key],raw)
    return result


def replay(process,snapshot,context,slack_ns,*,required_linux,expected_delivery=None):
    """Return fixed rejection categories from the exact retained evidence snapshot."""
    try:
        contracts.validate(contracts.LINUX if required_linux else contracts.ORDINARY,expected_delivery)
        references=process['sampled'].get('linux_owned')
        present=set(PATHS.values())&set(snapshot.bindings)
        if references is None:
            return ['linux_owned_evidence_invalid'] if present or required_linux else []
        observer.require(required_linux is True and type(references) is dict and set(references)==set(PATHS),
                         'Linux evidence reference inventory differs')
        values={}
        for key,path in PATHS.items():
            reference=references[key]
            if reference is None:
                observer.require(key=='terminal' and path not in snapshot.bindings,
                                 'missing or additional Linux evidence')
                values[key]=None
                continue
            observer.require(type(reference) is dict and set(reference)=={'path','bytes','sha256'}
                and reference['path']==path and control.canonical(reference)==control.canonical(snapshot.bindings[path])
                and owned._unsigned(reference['bytes'],MAX_BYTES), 'Linux evidence reference differs')
            raw=snapshot.raw[path]
            observer.require(type(raw) is bytes and len(raw)==reference['bytes']
                and hashlib.sha256(raw).hexdigest()==reference['sha256'],
                'Linux evidence bytes differ from bound snapshot')
            values[key]=control.decode(raw)
        baseline=control.decode(snapshot.raw['observations/baseline.json'])
        validate(values['admission'],values['lifecycle'],values['terminal'],process,baseline,context,slack_ns,
                 expected_delivery=expected_delivery)
    except (KeyError,TypeError,ValueError,OSError,RuntimeError):
        return ['linux_owned_evidence_invalid']
    return []
