"""Synthetic raw-capture replay with independently refreshed adverse digests."""
from dataclasses import asdict, replace
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/tests'))
from signed_request_fixture import add_retention
import resource_replay as replay
import resource_probe as probe
import resource_probe_worker as worker
import resource_process as process
import kura_resource_metrics as metrics
import resource_evidence_budget as budget


def encode(value): return json.dumps(value, separators=(',', ':')).encode()
def sha(raw): return hashlib.sha256(raw).hexdigest()
def ref(path): return {'name': path.name, 'sha256': sha(path.read_bytes()), 'bytes': path.stat().st_size}


def body(storage):
    fields = ('resident_associations', 'persisted_entries', 'index_bytes', 'temporary_index_bytes', 'storage_bytes')
    rows = ['iroha_kura_resource_available 1', 'iroha_kura_resource_status{reason="available"} 1',
            'iroha_kura_resource_generation 7', 'iroha_kura_resource_fault_count 0']
    for family in metrics.FAMILIES:
        for field in fields:
            value = storage if family == 'storage_bytes' and field == 'storage_bytes' else (
                3 if family == 'resident_canonical' and field == 'resident_associations' else 0)
            rows.append(f'iroha_kura_resource_{field}{{family="{family}"}} {value}')
    rows.extend(f'iroha_kura_resource_{field}_sum {value}' for field, value in zip(fields, (3, 0, 0, 0, storage), strict=True))
    rows.append('iroha_kura_resource_represented_entries 3')
    return ('\n'.join(rows) + '\n').encode()


def identities(count=4):
    return tuple(replay.ExpectedPeer(f'peer{i}', process.ProcessIdentity(100+i, 501, 1, i, 3+i, '01'*16, 'a'*64))
                 for i in range(count))


def observation(peers, policy):
    rows = []
    for i, peer in enumerate(peers):
        status = encode({'queue_size': i+1, 'other': {'float': 1.5}})
        raw = body(100+i)
        def provenance(route, raw, kind): return probe.HttpProvenance(route, sha(raw), len(raw), kind, raw)
        rows.append(probe.PeerObservation(peer.peer_id, i+1, provenance('/status', status, 'application/json'),
                    provenance('/metrics', raw, 'text/plain'), metrics.parse_kura_resource_metrics(raw),
                    process.ProcessSample(peer.identity, 1000+i), process.ProcessSample(peer.identity, 1100+i)))
    return probe.ProbeObservation(tuple(rows), 1_000_000, sum(len(row.status.raw_body)+len(row.metrics.raw_body)+200 for row in rows),
                                  sum(1000+i for i in range(len(rows))), sum(1100+i for i in range(len(rows))),
                                  sum(range(1,len(rows)+1)), len(rows),
                                  probe.InventoryAggregate(sum(100+i for i in range(len(rows))),len(rows)*3), policy)


def admitted_allocation(peers, geometry, policy, journal_bytes=4*budget.MIB):
    capture_geometry=budget.CaptureGeometry(peers,geometry.interval_ns,geometry.measurement_ns,geometry.drain_ns)
    runs=[]
    for pair in range(1,6):
        for variant in ('one_lane','four_lane'):
            prefix=f'pair{pair}.{variant}'
            files=tuple(budget.FileBudget(f'{prefix}.{role}',journal_bytes if role=='journal' else budget.MIB)
                        for role in ('journal','trace','proof','log','raw'))
            runs.append(budget.RunBudget(pair,variant,capture_geometry,*files,()))
    experiment=budget.admit_experiment(policy=policy,runs=tuple(runs),
        static_files=(budget.StaticFile('input',budget.MIB),),
        manifest=budget.FileBudget('manifest',budget.MIB),report=budget.FileBudget('report',budget.MIB),other_control=())
    return budget.select_run_budget(experiment,1,'one_lane')


class Fixture:
    def __init__(self, root, peers=4):
        self.directory = root/'captures'
        self.directory.mkdir(mode=0o700)
        self.journal = root/'collector.jsonl'
        self.peers = identities(peers)
        self.geometry = replay.ReplayGeometry(100_000_000,200_000_000,10_000_000,20_000_000,
                                              10_000_000,4_000_000,2_000_000)
        self.policy=budget.CapturePolicy()
        self.allocation=admitted_allocation(peers,self.geometry,self.policy)
        geometry=self.geometry
        plan={key: 1 for key in replay.PLAN_FIELDS}
        plan.update(event='plan',schema=replay.JOURNAL_SCHEMA,scheduled_requests=1,pair_index=1,variant='one_lane',
                    **{name:getattr(geometry,name) for name in ('warmup_ns','measurement_ns','drain_ns','preparation_ahead_ns')})
        self.events=[plan]
        with worker.CaptureDirectory(self.directory,self.allocation) as directory:
            reference=directory.publish('preflight',0,observation(self.peers,self.policy),probe._Deadline(5))
            self.events += [{'event':'resource_preflight','sequence':0,'outcome':'complete','manifest':asdict(reference),'sampling':geometry.sampling()},
                            {'event':'clock_started','initial_offset_ns':-(geometry.warmup_ns+geometry.drain_ns+geometry.preparation_ahead_ns)}]
            for index in range(geometry.samples):
                scheduled=index*geometry.interval_ns
                reference=directory.publish('sample',index+1,observation(self.peers,self.policy),probe._Deadline(5))
                self.events += [
                    {'event':'resource_request','kind':'sample','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled},
                    {'event':'resource_observation','sequence':index+1,'scheduled_offset_ns':scheduled,'start_offset_ns':scheduled,
                     'end_offset_ns':scheduled+3_000_000,'outcome':'complete','manifest':asdict(reference)}]
        end=geometry.final+3_000_000
        self.events += [{'event':'resource_request','kind':'finish','sequence':geometry.samples+1,'start_offset_ns':end},
                        {'event':'resource_collection_finished','sequence':geometry.samples+1,'start_offset_ns':end,'end_offset_ns':end+1_000_000,'sampling':geometry.sampling()},
                        {'event':'request_final','plan':{'cohort':'measurement','sequence':1,'logical_id':'b'*64,'scheduled_offset_ns':0,'account_index':0},
                         'hash':'c'*63+'1','offer_offset_ns':0,'acknowledgment_offset_ns':1,'applied_offset_ns':geometry.final,
                         'block_height':1,'status_attempts':1,'submission_finished':True,'failure':None},
                        {'event':'collection_finished','passed':True,'failure':None}]
        self.events = add_retention(self.events)
        self.save()

    def save(self):
        raw=b''.join(encode(row)+b'\n' for row in self.events)
        self.journal.write_bytes(raw);self.journal.chmod(0o600)
        self.digest=sha(raw)

    def run(self):
        self.save()
        return replay.replay(self.directory,self.journal,self.digest,self.peers,self.geometry,
                             expected_policy=self.policy,allocation=self.allocation)

    def capture(self, sequence=1):
        event=next(row for row in self.events if row['event'] in ('resource_preflight','resource_observation') and row['sequence']==sequence)
        path=self.directory/event['manifest']['name']
        return event,path,json.loads(path.read_bytes())

    def rewrite(self, event, path, value):
        path.write_bytes(encode(value));event['manifest']=ref(path)


@pytest.fixture
def fixture(tmp_path): return Fixture(tmp_path)


def test_actual_frozen_worker_captures_replay_exact_values_and_final_deadline(fixture):
    result=fixture.run()
    assert len(result.samples)==22
    assert result.samples[-1].scheduled_offset_ns==fixture.geometry.final
    assert result.samples[-1].end_offset_ns>fixture.geometry.final
    assert result.finish_start_ns>=result.samples[-1].end_offset_ns
    assert result.capture_file_count==23*9==207
    assert result.capture_bytes==sum(path.stat().st_size for path in fixture.directory.iterdir())
    assert result.capture_bytes==result.raw_body_bytes+result.manifest_bytes
    assert result.reported_http_framing_bytes==23*4*200
    assert result.capture_file_count == result.admitted_resource_file_count
    assert result.admitted_experiment_resource_file_count == 2070
    assert result.admitted_experiment_resource_file_count > result.control_file_limit
    assert result.capture_bytes <= result.admitted_resource_byte_limit
    assert result.samples[0].capture.storage_bytes==406
    assert result.samples[0].capture.represented_entries==12
    assert result.samples[0].capture.queue_size_sum==10
    assert result.samples[0].capture.queue_size_max==4
    assert result.samples[0].capture.rss_before_bytes==4006
    assert result.samples[0].capture.rss_after_bytes==4406
    assert result.journal_sha256==fixture.digest
    assert not hasattr(result,'transaction_deadline_extension')


def test_six_peer_resource_namespace_uses_exact_admitted_scope(tmp_path):
    fixture=Fixture(tmp_path,peers=6)
    result=fixture.run()
    assert result.capture_file_count==23*13==299
    assert result.admitted_experiment_resource_file_count == 2990
    assert result.capture_bytes <= result.admitted_resource_byte_limit
    assert result.capture_file_count*10 == result.admitted_experiment_resource_file_count
    assert result.control_file_limit==256
    assert result.global_byte_limit==2*1024**3


@pytest.mark.parametrize('mutation', ['queue','storage','represented','rss_before','rss_after','vector','bool','extra','wire','elapsed','python_cli_offset'])
def test_rehashed_precomputed_values_are_not_trusted(fixture,mutation):
    event,path,value=fixture.capture()
    if mutation=='queue': value['peers'][0]['status']['queue_size']+=1
    if mutation=='storage': value['aggregates']['inventory']['storage_bytes']+=1
    if mutation=='represented': value['aggregates']['inventory']['represented_entries']+=1
    if mutation=='rss_before': value['aggregates']['rss_before_bytes']+=1
    if mutation=='rss_after': value['aggregates']['rss_after_bytes']+=1
    if mutation=='vector': value['peers'][0]['metrics']['kura']['components'][0]['usage']['resident_associations']+=1
    if mutation=='bool': value['peers'][0]['status']['queue_size']=True
    if mutation=='extra': value['legacy']={}
    if mutation=='wire': value['wire_bytes']=1
    if mutation=='elapsed': value['probe_local_elapsed_ns']=fixture.geometry.response_deadline_ns
    if mutation=='python_cli_offset': value['request_start_offset_ns']=0
    fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError):fixture.run()


@pytest.mark.parametrize('field', ['pid','uid','start_seconds','start_microseconds','start_abstime','image_uuid','executable_sha256'])
def test_rehashed_process_identity_must_match_prelaunch_across_every_bracket(fixture,field):
    event,path,value=fixture.capture(fixture.geometry.samples)
    identity=value['peers'][2]['process_after']['identity']
    identity[field]=('02'*16 if field=='image_uuid' else 'd'*64) if type(identity[field]) is str else identity[field]+1
    fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError,match='process_lifetime_mismatch'):fixture.run()


@pytest.mark.parametrize('mutation',['missing','duplicate','reordered','label','extra','rss_zero','rss_overflow'])
def test_full_peer_set_never_reduces_selected_or_partial_members(fixture,mutation):
    event,path,value=fixture.capture()
    if mutation=='missing':value['peers'].pop()
    if mutation=='duplicate':value['peers'][1]=value['peers'][0]
    if mutation=='reordered':value['peers'].reverse()
    if mutation=='label':value['peers'][1]['peer_id']='other'
    if mutation=='extra':value['peers'].append(value['peers'][0])
    if mutation=='rss_zero':value['peers'][0]['process_before']['rss_bytes']=0
    if mutation=='rss_overflow':
        for row in value['peers']:row['process_before']['rss_bytes']=2**53
    fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError):fixture.run()


@pytest.mark.parametrize('mutation',['duplicate_status','bool_status','metrics_missing','metrics_duplicate','unavailable','unknown_family','wrong_subtotal','bad_utf8'])
def test_raw_bodies_with_independently_refreshed_all_hashes_are_reparsed(fixture,mutation):
    event,path,value=fixture.capture()
    role='status' if mutation.endswith('status') else 'metrics'
    owner=value['peers'][0][role]
    bodypath=fixture.directory/owner['body']['name']
    raw=bodypath.read_bytes()
    if mutation=='duplicate_status':raw=b'{"queue_size":1,"queue_size":1}'
    if mutation=='bool_status':raw=b'{"queue_size":true}'
    if mutation=='metrics_missing':raw=b'iroha_kura_resource_available 1\n'
    if mutation=='metrics_duplicate':raw+=b'iroha_kura_resource_available 1\n'
    if mutation=='unavailable':raw=b'iroha_kura_resource_available 0\niroha_kura_resource_status{reason="busy"} 1\n'
    if mutation=='unknown_family':raw=raw.replace(b'family="resident_canonical"',b'family="unknown"')
    if mutation=='wrong_subtotal':raw=raw.replace(b'storage_bytes_sum 100\n',b'storage_bytes_sum 101\n')
    if mutation=='bad_utf8':raw=b'\xff'
    bodypath.write_bytes(raw);owner['body']=ref(bodypath)
    if role=='metrics':
        owner['kura']['raw_sha256']=sha(raw);owner['kura']['response_bytes']=len(raw)
    fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError):fixture.run()


@pytest.mark.parametrize('mutation',['missing_preflight','clock_before_preflight','missing_clock','duplicate_clock','request_missing','observation_missing',
                                     'sequence','schedule','early','late','timeout','final_cap','final_missing','finish_missing','finish_timeout',
                                     'outcome','failed_event','passed_false','extra_sampling','sampling_geometry','clock_origin','unknown_resource','extra_final'])
def test_exact_clock_sequence_cadence_and_complete_outcomes(fixture,mutation):
    rows=fixture.events
    observation=next(row for row in rows if row['event']=='resource_observation')
    request=next(row for row in rows if row['event']=='resource_request')
    if mutation=='missing_preflight':rows.pop(1)
    clock=next(row for row in rows if row['event']=='clock_started')
    if mutation=='clock_before_preflight':
        index=rows.index(clock);rows[1],rows[index]=rows[index],rows[1]
    if mutation=='missing_clock':rows.remove(clock)
    if mutation=='duplicate_clock':rows.insert(rows.index(clock)+1,clock.copy())
    if mutation=='request_missing':rows.remove(request)
    if mutation=='observation_missing':rows.remove(observation)
    if mutation=='sequence':request['sequence']=2;observation['sequence']=2
    if mutation=='schedule':request['scheduled_offset_ns']=1;observation['scheduled_offset_ns']=1
    if mutation=='early':request['start_offset_ns']=-1;observation['start_offset_ns']=-1
    if mutation=='late':request['start_offset_ns']=2_000_001;observation['start_offset_ns']=2_000_001
    if mutation=='timeout':observation['end_offset_ns']=4_000_000
    if mutation=='final_cap':
        final=next(row for row in rows if row['event']=='resource_observation' and row['sequence']==fixture.geometry.samples)
        final_request=rows[rows.index(final)-1]
        final_request['start_offset_ns']+=2_000_000;final['start_offset_ns']+=2_000_000
        final['end_offset_ns']=fixture.geometry.final+fixture.geometry.response_deadline_ns
    if mutation=='final_missing':
        final=next(row for row in rows if row['event']=='resource_observation' and row['sequence']==fixture.geometry.samples)
        rows.remove(rows[rows.index(final)-1]);rows.remove(final)
    if mutation=='finish_missing':rows[:]=[row for row in rows if row['event']!='resource_collection_finished']
    if mutation=='finish_timeout':
        row=next(row for row in rows if row['event']=='resource_collection_finished')
        row['end_offset_ns']=row['start_offset_ns']+fixture.geometry.response_deadline_ns
    if mutation=='outcome':observation['outcome']='unavailable'
    if mutation=='failed_event':rows.insert(3,{'event':'resource_collection_failed','failure':'bounded_resource_collection_failed'})
    if mutation=='passed_false':rows[-1]['passed']=False
    if mutation=='extra_sampling':rows[1]['sampling']['compatibility']=True
    if mutation=='sampling_geometry':rows[1]['sampling']['interval_ns']+=1
    if mutation=='clock_origin':clock['initial_offset_ns']+=1
    if mutation=='unknown_resource':rows.insert(3,{'event':'resource_legacy'})
    if mutation=='extra_final':rows.append(rows[-1].copy())
    with pytest.raises(replay.ReplayError):fixture.run()


def test_inclusive_last_start_lag_and_just_before_strict_cap_is_valid(fixture):
    rows=fixture.events
    final=next(row for row in rows if row['event']=='resource_observation' and row['sequence']==fixture.geometry.samples)
    request=rows[rows.index(final)-1]
    request['start_offset_ns']+=fixture.geometry.max_start_lag_ns
    final['start_offset_ns']=request['start_offset_ns']
    final['end_offset_ns']=fixture.geometry.final+fixture.geometry.response_deadline_ns-1
    finish_request=next(row for row in rows if row['event']=='resource_request' and row['kind']=='finish')
    finish=next(row for row in rows if row['event']=='resource_collection_finished')
    finish_request['start_offset_ns']=final['end_offset_ns'];finish['start_offset_ns']=final['end_offset_ns']
    finish['end_offset_ns']=final['end_offset_ns']+1
    result=fixture.run()
    assert result.samples[-1].end_offset_ns==fixture.geometry.final+fixture.geometry.response_deadline_ns-1


@pytest.mark.parametrize('cohort,field,value',['measurement_applied'.split('_')+[1], 'measurement_acknowledgment'.split('_')+[1],
                                             'warmup_applied'.split('_')+[0], 'warmup_acknowledgment'.split('_')+[0]])
def test_transaction_drain_never_uses_resource_finish_extension(fixture,cohort,field,value):
    row=next(row for row in fixture.events if row['event']=='request_final')
    row['plan']['cohort']=cohort
    if cohort=='warmup':row.update(offer_offset_ns=-10,acknowledgment_offset_ns=-5,applied_offset_ns=-1)
    row[field+'_offset_ns']=fixture.geometry.final+value if cohort=='measurement' else value
    with pytest.raises(replay.ReplayError,match='transaction_drain_extended'):fixture.run()


@pytest.mark.parametrize('mutation',['file_mode','file_symlink','file_hardlink','fifo','truncated','digest','unreferenced','directory_mode','directory_symlink','path_escape'])
def test_owner_only_exact_capture_files_fail_closed(fixture,mutation,tmp_path):
    event,path,value=fixture.capture()
    target=fixture.directory/value['peers'][0]['status']['body']['name']
    if mutation=='file_mode':target.chmod(0o644)
    if mutation=='file_symlink':
        other=tmp_path/'body';target.rename(other);target.symlink_to(other)
    if mutation=='file_hardlink':os.link(target,tmp_path/'other')
    if mutation=='fifo':target.unlink();os.mkfifo(target,0o600)
    if mutation=='truncated':target.write_bytes(b'x')
    if mutation=='digest':target.write_bytes(target.read_bytes().replace(b'1',b'2',1))
    if mutation=='unreferenced':(fixture.directory/'unreferenced').write_bytes(b'x')
    if mutation=='directory_mode':fixture.directory.chmod(0o755)
    if mutation=='directory_symlink':
        other=tmp_path/'retired';fixture.directory.rename(other);fixture.directory.symlink_to(other)
    if mutation=='path_escape':
        value['peers'][0]['status']['body']['name']='../outside';fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError):fixture.run()


def test_descriptor_file_changes_during_read_are_rejected(fixture,monkeypatch):
    original=replay.os.pread
    changed=[]
    def modify(fd,count,offset):
        raw=original(fd,count,offset)
        if not changed and b'public-never-used' not in raw and b'queue_size' in raw and len(raw)<1024:
            changed.append(True)
            name='sample-0000000001-peer-0000-status.body'
            target=fixture.directory/name
            target.write_bytes(target.read_bytes()+b' ')
        return raw
    monkeypatch.setattr(replay.os,'pread',modify)
    with pytest.raises(replay.ReplayError):fixture.run()
    assert changed


@pytest.mark.parametrize('mutation',['no_newline','duplicate_json','large_event','bad_hash','bad_mode','hardlink','bad_plan'])
def test_journal_raw_framing_digest_owner_and_schema_are_independent(fixture,mutation,tmp_path):
    fixture.save();raw=fixture.journal.read_bytes()
    if mutation=='no_newline':raw=raw[:-1]
    if mutation=='duplicate_json':raw=raw.replace(b'"sequence":0',b'"sequence":0,"sequence":0',1)
    if mutation=='large_event':raw=b' '*(replay.MAX_EVENT_BYTES+1)+b'\n'
    if mutation=='bad_plan':raw=raw.replace(replay.JOURNAL_SCHEMA.encode(),b'legacy',1)
    fixture.journal.write_bytes(raw)
    if mutation=='bad_mode':fixture.journal.chmod(0o644)
    if mutation=='hardlink':os.link(fixture.journal,tmp_path/'other')
    digest='0'*64 if mutation=='bad_hash' else sha(raw)
    with pytest.raises(replay.ReplayError):replay.replay(fixture.directory,fixture.journal,digest,fixture.peers,fixture.geometry,
                                                      expected_policy=fixture.policy,allocation=fixture.allocation)


@pytest.mark.parametrize('field,value',[('interval_ns',0),('measurement_ns',190_000_000),('drain_ns',0),('drain_ns',11_000_000),
                                      ('response_deadline_ns',6_000_000),('max_start_lag_ns',3_000_000),('warmup_ns',True),
                                      ('measurement_ns',10**50),('response_deadline_ns',1_000_001)])
def test_geometry_is_declared_exact_and_bounded(fixture,field,value):
    fixture.geometry=replace(fixture.geometry,**{field:value})
    with pytest.raises(replay.ReplayError):fixture.run()


def test_expected_identity_is_an_independent_required_input(fixture):
    fixture.peers=(replace(fixture.peers[0],identity=replace(fixture.peers[0].identity,start_abstime=999)),*fixture.peers[1:])
    with pytest.raises(replay.ReplayError,match='process_lifetime_mismatch'):fixture.run()


@pytest.mark.parametrize('owner', ['manifest','status','metrics'])
def test_reference_size_caps_apply_before_unbounded_read(fixture,owner):
    event,path,value=fixture.capture()
    if owner=='manifest':event['manifest']['bytes']=replay.MAX_MANIFEST_BYTES+1
    else:
        value['peers'][0][owner]['body']['bytes']=(replay.MAX_STATUS_BYTES if owner=='status' else replay.MAX_METRICS_BYTES)+1
        fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError,match='integer_outside_bounds'):fixture.run()


def test_directory_name_replaced_mid_replay_cannot_alias_retained_owner(fixture,monkeypatch,tmp_path):
    original=replay._capture
    changed=[]
    def replace_once(*args):
        result=original(*args)
        if not changed:
            changed.append(True)
            fixture.directory.rename(tmp_path/'retired')
            fixture.directory.mkdir(mode=0o700)
        return result
    monkeypatch.setattr(replay,'_capture',replace_once)
    with pytest.raises(replay.ReplayError,match='capture_directory_changed'):fixture.run()
    assert changed==[True]


def test_already_read_body_change_is_detected_by_final_identity_pass(fixture,monkeypatch):
    original=replay._capture
    def change_prior(*args):
        result=original(*args)
        if result.sequence==fixture.geometry.samples:
            path=fixture.directory/'preflight-0000000000-peer-0000-status.body'
            path.write_bytes(path.read_bytes()+b' ')
        return result
    monkeypatch.setattr(replay,'_capture',change_prior)
    with pytest.raises(replay.ReplayError,match='capture_changed'):fixture.run()


def test_all_unreferenced_complete_captures_are_rejected(fixture):
    path=fixture.directory/'sample-0000099999.json'
    path.write_bytes(b'{}');path.chmod(0o600)
    with pytest.raises(replay.ReplayError,match='capture_set_extra'):fixture.run()


def test_journal_resource_fields_reject_bool_even_when_equal_to_integer(fixture):
    row=next(row for row in fixture.events if row['event']=='resource_request')
    row['sequence']=True
    with pytest.raises(replay.ReplayError,match='request_identity_invalid'):fixture.run()


def test_missing_final_transaction_cannot_be_hidden_by_complete_resource_finish(fixture):
    fixture.events[:]=[row for row in fixture.events if row['event']!='request_final']
    with pytest.raises(replay.ReplayError,match='collection_finish_order_invalid'):fixture.run()


def test_sample_python_elapsed_never_becomes_a_clock_timestamp(fixture):
    event,path,value=fixture.capture(fixture.geometry.samples)
    value['probe_local_elapsed_ns']=1
    fixture.rewrite(event,path,value)
    result=fixture.run()
    assert result.samples[-1].start_offset_ns==fixture.geometry.final
    assert result.samples[-1].end_offset_ns==fixture.geometry.final+3_000_000


def test_generation_and_fault_vectors_are_exactly_replayed(fixture):
    event,path,value=fixture.capture()
    value['peers'][0]['metrics']['kura']['generation']+=1
    fixture.rewrite(event,path,value)
    with pytest.raises(replay.ReplayError,match='kura_projection_mismatch'):fixture.run()



def test_per_capture_wire_envelope_is_enforced_before_oversized_body_io(fixture,monkeypatch):
    event,path,value=fixture.capture()
    value['wire_bytes']=1024
    fixture.rewrite(event,path,value)
    original=replay._Captures.read
    attempted=[]
    def counted(self,reference,name,cap):
        if name=='sample-0000000001-peer-0000-metrics.body':
            attempted.append(cap)
            assert cap<1024
        return original(self,reference,name,cap)
    monkeypatch.setattr(replay._Captures,'read',counted)
    with pytest.raises(replay.ReplayError,match='integer_outside_bounds'):fixture.run()
    assert len(attempted)==1
