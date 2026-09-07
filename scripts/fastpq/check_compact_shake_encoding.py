#!/usr/bin/env python3
"""Check independent canonical Norito/SHAKE inputs and the fixed engine wrapper.

Uses only Python stdlib, complete type-tagged inputs and published SHAKE256.
These controls are encoding/work regressions, not cryptographic qualification.
"""
import hashlib
import json

IDENTITY=b"fastpq:compact-shake256:h16:g375:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1"
CONTEXT=b"fixed public candidate context"
MODULUS=2**64-2**32+1
RATE=136


def leb128(n):
    assert n>=0
    out=bytearray()
    while n>=128:
        out.append((n&127)|128)
        n>>=7
    out.append(n)
    return bytes(out)


def field(value):
    return leb128(len(value))+value


def vector(value):
    return len(value).to_bytes(8,'little')+value


def crc64_xz(value):
    crc=2**64-1
    for byte in value:
        crc^=byte
        for _ in range(8):
            crc=(crc>>1)^(0xc96c5795d7870f42 if crc&1 else 0)
    return crc^(2**64-1)


def canonical(schema, values):
    payload=b''.join(map(field,values))
    schema_hash=hashlib.sha256(b'norito:v1:type-name\0'+schema).digest()[:16]
    header=b'NRT0\0\0'+schema_hash+b'\0'+len(payload).to_bytes(8,'little')+crc64_xz(payload).to_bytes(8,'little')+b'\x02'
    assert len(header)==40
    return header+payload


def prefix(context=CONTEXT):
    assert 0<len(context)<=262144
    return canonical(b'fastpq_prover::compact_candidate::ShakePrefixV1',[(1).to_bytes(2,'little'),vector(IDENTITY),vector(context)])


def body(kind,oracle,round_number,output_bytes,fields,level=0,position=0):
    return canonical(b'fastpq_prover::compact_candidate::ShakeBodyV1',[
        bytes([kind]),bytes([oracle]),bytes([round_number]),level.to_bytes(4,'little'),position.to_bytes(4,'little'),output_bytes.to_bytes(4,'little'),
        len(fields).to_bytes(8,'little')+b''.join(field(vector(value)) for value in fields)])


def root(raw):
    assert len(raw)==128
    words=[raw[i:i+8] for i in range(0,128,8)]
    selected=[v for v in words if int.from_bytes(v,'little')<MODULUS]
    assert len(selected)>=6
    return b''.join(selected[:6])


def tapes():
    return [48,10992,29584,112]+[80]*17+[950]


def work(context_bytes):
    plen=len(prefix(bytes(context_bytes)))
    # A cache reuses floor(|P|/136) absorbed permutations. It clones the full
    # unfinished state, including |P| mod 136 bytes, for every independent call.
    shapes=[]
    for role,count,payload,parents in [(1,750,2736,7773),(2,375,32,4261),(3,375,32,4261),(4,4258,64,22486),(4,1,128,1)]:
        shapes.append((count,len(body(1,role,17 if payload==128 else 0,128,[bytes(payload)])),128))
        shapes.append((parents,len(body(2,role,17 if payload==128 else 0,128,[bytes(48),bytes(48)],level=1)),128))
    for r,tape in enumerate(tapes(),1):
        shapes.append((1,len(body(4,0,r,tape,[bytes(48)])),tape))
        if r<22:shapes.append((1,len(body(3,0,r,128,[bytes(tape),bytes(48)])),128))
    calls=sum(n for n,_,_ in shapes)
    suffix=sum(n*b for n,b,_ in shapes)
    # SHAKE needs one padded absorption permutation and ceil(output/136)-1
    # additional squeeze permutations after all complete input blocks.
    cold_perm=sum(n*((plen+b)//RATE+1+(out-1)//RATE) for n,b,out in shapes)
    cached_perm=plen//RATE+sum(n*(((plen%RATE)+b)//RATE+1+(out-1)//RATE) for n,b,out in shapes)
    assert calls==44584
    assert cold_perm-cached_perm==(calls-1)*(plen//RATE)
    return {
        'context_bytes':context_bytes,'canonical_prefix_bytes':plen,'prefix_partial_rate_bytes':plen%RATE,
        'verifier_expansions_bound':calls,'verifier_body_bytes_bound':suffix,
        'logical_input_bytes_bound':calls*plen+suffix,
        'cached_absorb_api_input_bytes_bound':plen+suffix,
        'cold_keccak_permutations_bound':cold_perm,'cached_keccak_permutations_bound':cached_perm,
        'saved_context_reabsorptions_bound':calls-1,
        'row_body_bytes':len(body(1,1,0,128,[bytes(2736)])),
        'parent_body_bytes':len(body(2,1,0,128,[bytes(48),bytes(48)],level=1)),
        'max_chain_body_bytes':len(body(3,0,3,128,[bytes(29584),bytes(48)])),
        'g_body_bytes':len(body(4,0,3,29584,[bytes(48)])),
        'qualification':'integer upper bounds for q375 expansion; not measured proof time or allocation limits; cloning and encoding work excluded',
    }


def calculate():
    assert crc64_xz(b'123456789')==0x995dc9bbdf1939fa
    p=prefix()
    b=body(1,2,0,128,[bytes(32)])
    raw=hashlib.shake_256(p+b).digest(128)
    dummy=hashlib.shake_256(p+body(4,0,1,48,[bytes(48)])).digest(48)
    chained=hashlib.shake_256(p+body(3,0,1,128,[dummy,bytes(48)])).digest(128)
    # hashlib's copy must also preserve partial rate blocks independently.
    cache=hashlib.shake_256(p)
    for n in [0,1,135,136,137,272,273,410]:
        suffix=bytes(n)
        c=cache.copy();c.update(suffix)
        assert c.digest(409)==hashlib.shake_256(p+suffix).digest(409)
    return {'identity':IDENTITY.decode(),'prefix_hex':p.hex(),'leaf_body_hex':b.hex(),'leaf_raw_tape_hex':raw.hex(),'leaf_root_hex':root(raw).hex(),'dummy_tape_hex':dummy.hex(),'state_after_dummy_zero_root_hex':root(chained).hex(),'resource_bounds':[work(30),work(262144)]}


u32=lambda v:v.to_bytes(4,'little')
u64=lambda v:v.to_bytes(8,'little')
def calculate_engine():
 schema=b'fastpq_prover::compact_candidate::ShakeEngineStatementV1'
 encoded=canonical(schema,[
  field(b'profile-binding:fixed-candidate:v1'),u32(65536),u32(524288),u32(342),u32(923),
  u64(MODULUS),u64(7),u64(0xa9c468a357df6e13),u32(19),u64(0xfd0e69f9a98ee946),
  u32(8),u32(2),u32(17),u32(4),u32(1),u32(375),
  vector(b'complete public context without a private witness')])
 encoded_prefix=prefix(encoded)
 roots={}
 for name,role,round_,index,payload in [('row',1,0,0,bytes(2736)),('mixed',2,0,0,bytes(32)),('quotient',3,0,0,bytes(32)),('fri0',4,0,0,bytes(64)),('terminal',4,17,0,bytes(128))]:
  encoded_body=body(1,role,round_,128,[payload],position=index);raw=hashlib.shake_256(encoded_prefix+encoded_body).digest(128);roots[name]=root(raw).hex()
 return {'context_frame_bytes':len(encoded),'context_frame_hex':encoded.hex(),'prefix_bytes':len(encoded_prefix),'zero_leaf_roots':roots}

EXPECTED_PREFIX = {'identity': 'fastpq:compact-shake256:h16:g375:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1',
 'prefix_hex': '4e52543000004fdd12ac5e6affa7dc0020d925a07956008d000000000000000dfd4ed02952f21902020100625a000000000000006661737470713a636f6d706163742d7368616b653235363a6831363a673337353a333432636f6c733a393233736c6f74733a3635353336726f77733a38626c6f7775703a3137666f6c64733a7072656669782d626f64793a7631261e000000000000006669786564207075626c69632063616e64696461746520636f6e74657874',
 'leaf_body_hex': '4e525430000085e95e1826c2b67068a640994478399e0047000000000000002109e3fab35cbe42020101010201000400000000040000000004800000003101000000000000002820000000000000000000000000000000000000000000000000000000000000000000000000000000',
 'leaf_raw_tape_hex': '1b124b38187f2b0c47d3e1a6fe202e9d5e3c06b5cae1c3fd5fbe3774b7fad606344d6eff3cb46f7785f616408d58422d12471f70c9fbd4d0069dd001faace0517e592ca7bb8ef461efb2a32f625080a3e1dbfa650bfcaeaf3864f3fa5e5362b85cef792ba3cac8f4a43b18bfa5cdae1c7cc7978402613fdc4dff02bfe517349a',
 'leaf_root_hex': '1b124b38187f2b0c47d3e1a6fe202e9d5e3c06b5cae1c3fd5fbe3774b7fad606344d6eff3cb46f7785f616408d58422d',
 'dummy_tape_hex': 'efe24717b872ecaef30c5b7feff38ec4439c0d14ea1aee210ec6f1fa70cb1ba58cbb8dd366174cb5f631301afbacfe22',
 'state_after_dummy_zero_root_hex': '55c3747cc39e0bbf5fc2d9778b55ce8f3317b6ba3913f11ead129572a2a3a0ee63c70b4762c56a6504254eab8a8192de',
 'resource_bounds': [{'context_bytes': 30,
                      'canonical_prefix_bytes': 181,
                      'prefix_partial_rate_bytes': 45,
                      'verifier_expansions_bound': 44584,
                      'verifier_body_bytes_bound': 9988761,
                      'logical_input_bytes_bound': 18058465,
                      'cached_absorb_api_input_bytes_bound': 9988942,
                      'cold_keccak_permutations_bound': 149354,
                      'cached_keccak_permutations_bound': 104771,
                      'saved_context_reabsorptions_bound': 44583,
                      'row_body_bytes': 2817,
                      'parent_body_bytes': 184,
                      'max_chain_body_bytes': 29724,
                      'g_body_bytes': 127,
                      'qualification': 'integer upper bounds for q375 expansion; not measured '
                                       'proof time or allocation limits; cloning and encoding work '
                                       'excluded'},
                     {'context_bytes': 262144,
                      'canonical_prefix_bytes': 262297,
                      'prefix_partial_rate_bytes': 89,
                      'verifier_expansions_bound': 44584,
                      'verifier_body_bytes_bound': 9988761,
                      'logical_input_bytes_bound': 11704238209,
                      'cached_absorb_api_input_bytes_bound': 10251058,
                      'cold_keccak_permutations_bound': 86101524,
                      'cached_keccak_permutations_bound': 145500,
                      'saved_context_reabsorptions_bound': 44583,
                      'row_body_bytes': 2817,
                      'parent_body_bytes': 184,
                      'max_chain_body_bytes': 29724,
                      'g_body_bytes': 127,
                      'qualification': 'integer upper bounds for q375 expansion; not measured '
                                       'proof time or allocation limits; cloning and encoding work '
                                       'excluded'}]}
EXPECTED_ENGINE = {'context_frame_bytes': 225,
 'context_frame_hex': '4e525430000047d26b623e365da9d63db2f594e53ee900b9000000000000008a36ae75d966e4b402232270726f66696c652d62696e64696e673a66697865642d63616e6469646174653a7631040000010004000008000456010000049b0300000801000000ffffffff08070000000000000008136edf57a368c4a904130000000846e98ea9f9690efd040800000004020000000411000000040400000004010000000477010000393100000000000000636f6d706c657465207075626c696320636f6e7465787420776974686f757420612070726976617465207769746e657373',
 'prefix_bytes': 377,
 'zero_leaf_roots': {'row': '9ad58fdc2cb76a3721ce39268c47edbcf2686e96e00a4d644e535c52d37e66239a4660942686b4b82b5b2a7ed0c902e2',
                     'mixed': 'fd0d82076991bbc83ca90f2c1c9633915e797e5ec5bc254f46123f32e92d4ab83ec065e3a0070f2bff2d1ea10c106b5c',
                     'quotient': '2e14eaeed8ccef024799bd89d5d9794c03bdc3357eb8db1a40646b01f2f35abc770d05e8af55df830996d205b9fd455d',
                     'fri0': '9c86c4026ab348e7c27db2bbab8340bae1a00befbd5ca0562d35d718f93422ba9f02b9970898633a08be1df247d1548f',
                     'terminal': 'b55e7b1b435626a444618ffb0b9b8a32e7d04876dc2827438028b004efa4ca2d51dc16edf60a0d9136215e9abb12a17c'}}

def main():
    if not __debug__:
        raise SystemExit('refusing optimized Python: controls require assertions')
    assert calculate() == EXPECTED_PREFIX, 'prefix/body encoding or resource bounds changed'
    assert calculate_engine() == EXPECTED_ENGINE, 'complete engine context encoding changed'
    print('PASS: canonical prefix/body and structured engine context, independent SHAKE KATs and exact resource bounds')


if __name__ == '__main__':
    main()
