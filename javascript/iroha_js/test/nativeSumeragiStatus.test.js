import test from "node:test";
import assert from "node:assert/strict";
import { parseSumeragiStatusJson, parseSumeragiStatusPayload } from "../src/sumeragiTyped.js";
// Syntax fixture only: no generated native finality or execution-capture claim.
const BASE = {"protocol_version": 1, "config_fingerprint": "hash:0101010101010101010101010101010101010101010101010101010101010101#B86C", "beacon_horizon": null, "instance": "0000000000000000000000000000000000000000000000000000000000000000", "height": 1, "view": 0, "stage": 0, "leader": null, "proxy_tail": null, "high_qc_view": null, "level": 0, "start_level": 0, "t_retx_ms": 1, "committed_height": 0, "applied_height": 0, "awaiting": false, "signer": null, "unanchored": true, "abstaining": true, "halted": null, "footprint": {"votes": 0, "timeouts": 0, "blocks": 0, "exec_entries": 0, "wants": 0, "pending_apply": 0, "sync_entries": 0, "sync_bytes": 0, "peers": 0, "recent_headers": 0, "configs": 0, "cert_cache": 0, "evidence_keys": 0, "probe": 0}};
const copy=()=>structuredClone(BASE);
const parse=(value)=>parseSumeragiStatusJson(JSON.stringify(value));

test("native observer status is exact and immutable",()=>{
  const value=copy();const result=parse(value);value.footprint.votes=10;
  assert.equal(result.protocol_version,1);assert.equal(result.leader,null);
  assert.equal(result.footprint.votes,0);assert.ok(Object.isFrozen(result));assert.ok(Object.isFrozen(result.footprint));
});
for (const field of Object.keys(BASE)) test(`native status requires ${field}`,()=>{
  const value=copy();delete value[field];assert.throws(()=>parse(value));
});
for (const field of Object.keys(BASE.footprint)) test(`native footprint requires ${field}`,()=>{
  const value=copy();delete value.footprint[field];assert.throws(()=>parse(value));
});
for (const field of ["height","view","t_retx_ms","committed_height","applied_height"]) test(`native u64 ${field} remains exact`,()=>{
  const value=copy();value[field]=(1n<<64n)-1n;
  assert.equal(parseSumeragiStatusPayload(value)[field],(1n<<64n)-1n);
  for(const bad of [(1n<<64n),-1n,-0,true,1.5,"1"]){value[field]=bad;assert.throws(()=>parseSumeragiStatusPayload(value));}
});
for(const token of ["-0","-1","1.0","1e0","NaN","Infinity"])test(`native JSON rejects numeric token ${token}`,()=>{
 assert.throws(()=>parseSumeragiStatusJson(JSON.stringify(BASE).replace('"height":1','"height":'+token)));
});
for(const reason of ["safety_record_corrupt","safety_record_inconsistent","driver_anomaly","safety_violation","apply_diverged","publication_recovery_required"])test(`native halt tag ${reason} binds details`,()=>{
 const value=copy();const unit=["safety_record_corrupt","safety_record_inconsistent","driver_anomaly"].includes(reason);
 value.halted={reason,details:unit?null:9};assert.equal(parse(value).halted.reason,reason);
 value.halted.details=unit?1:null;assert.throws(()=>parse(value));
});
test("native beacon readiness requires the session and demand",()=>{
 const value=copy();value.beacon_horizon={epoch_length_blocks:10,next_required_pulse_height:20,active_session_id:"AB".repeat(32),session_covers_next_pulse:true,local_provider_ready:true};
 assert.equal(parse(value).beacon_horizon.active_session_id,"AB".repeat(32));
 for(const field of Object.keys(value.beacon_horizon)){const bad=structuredClone(value);delete bad.beacon_horizon[field];assert.throws(()=>parse(bad));}
 for(const field of ["active_session_id","next_required_pulse_height"]){const bad=structuredClone(value);bad.beacon_horizon[field]=null;assert.throws(()=>parse(bad));}
});
test("native status rejects duplicates, old fields, malformed fingerprints and size excess",()=>{
 for(const wire of ["", " ".repeat(1024*1024+1),'{"protocol_version":4}',JSON.stringify(BASE).replace('"height":1','"height":1,"height":1')])assert.throws(()=>parseSumeragiStatusJson(wire));
 const value=copy();value.execution_commitment={};assert.throws(()=>parse(value));
 for(const field of ["config_fingerprint","instance"]){const bad=copy();bad[field]="00";assert.throws(()=>parse(bad));}
});

for (const field of ["leader", "proxy_tail", "signer"]) test(`native ${field} admits canonical key material`, () => {
 const ed = "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A";
 const bls = "ea013097F1D3A73197D7942695638C4FA9AC0FC3688C4F9774B905A14E3A3F171BAC586C55E83FF97A1AEFFB3AF00ADB22C6BB";
 const value = copy();
 for (const key of [ed, bls]) { value[field] = key; assert.equal(parse(value)[field], key); }
 for (const key of [ed.toLowerCase(), bls.toLowerCase(), "ed0120" + "00".repeat(32), "ea0130C0" + "00".repeat(47), "ea0130" + "00".repeat(48), "bls_normal:" + bls, "ed810020" + ed.slice(6), ed.slice(0, -2)]) {
   value[field] = key; assert.throws(() => parse(value), key);
 }
});

for (const version of [0, 2, 4, 8]) test(`first release rejects protocol ${version}`, () => {
  const value = copy(); value.protocol_version = version; assert.throws(() => parse(value));
});
