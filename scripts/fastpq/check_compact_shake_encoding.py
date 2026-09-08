#!/usr/bin/env python3
"""Check independent canonical Norito/SHAKE inputs and the fixed engine wrapper.

Uses only Python stdlib, complete type-tagged inputs and published SHAKE256.
These controls are encoding/work regressions, not cryptographic qualification.
"""
import hashlib
import json

IDENTITY=b"fastpq:compact-shake256:h16:g375:c401:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1"
CONTEXT=b"fixed public candidate context"
MODULUS=2**64-2**32+1
RATE=136
QUERY_COUNT=375
QUERY_CANDIDATES=401
QUERY_LABEL_BITS=19
QUERY_TAPE_BYTES=(QUERY_CANDIDATES*QUERY_LABEL_BITS+7)//8


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
    return [48,10992,29584,112]+[80]*17+[QUERY_TAPE_BYTES]


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

def query_controls():
    # Independent whole-integer decoding differs from Rust's per-bit extraction.
    def pack(labels):
        labels=list(labels)
        assert len(labels)==QUERY_CANDIDATES
        assert all(0<=value<2**QUERY_LABEL_BITS for value in labels)
        return sum(value<<(QUERY_LABEL_BITS*i) for i,value in enumerate(labels)).to_bytes(QUERY_TAPE_BYTES,'little')
    def decode(raw):
        if len(raw)!=QUERY_TAPE_BYTES:
            raise ValueError('fixed query tape length')
        word=int.from_bytes(raw,'little'); selected=set()
        for i in range(QUERY_CANDIDATES):
            selected.add((word>>(QUERY_LABEL_BITS*i))&((1<<QUERY_LABEL_BITS)-1))
            if len(selected)==QUERY_COUNT:
                return sorted(selected)
        raise ValueError('fixed query tape exhausted')
    first400=list(range(374))+[0]*26
    tape=pack(first400+[374])
    expected=list(range(375))
    assert decode(tape)==expected
    for padding in range(32):
        padded=tape[:-1]+bytes([(tape[-1]&7)|(padding<<3)])
        assert decode(padded)==expected
    high=pack(first400+[2**19-1])
    assert decode(high)==list(range(374))+[2**19-1]
    changed=high[:-1]+bytes([high[-1]^4])
    assert decode(changed)==list(range(374))+[2**19-1-(1<<18)]
    for invalid in [pack(first400+[0]),tape[:950],tape[:-1],tape+b'\0']:
        try:
            decode(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError('invalid/exhausted tape accepted')
    assert QUERY_CANDIDATES*QUERY_LABEL_BITS==7619
    assert (QUERY_CANDIDATES-1)*QUERY_LABEL_BITS==950*8
    assert QUERY_TAPE_BYTES==953
    assert QUERY_TAPE_BYTES*8-QUERY_CANDIDATES*QUERY_LABEL_BITS==5
    return {'candidates':QUERY_CANDIDATES,'bytes':QUERY_TAPE_BYTES,'used_bits':7619,'padding_variants':32,'narrow_profile_sha256':hashlib.sha256(IDENTITY).hexdigest()}


EXPECTED_PREFIX = {'identity': 'fastpq:compact-shake256:h16:g375:c401:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1',
 'prefix_hex': '4e52543000004fdd12ac5e6affa7dc0020d925a07956009200000000000000263b808fe774e20b02020100675f000000000000006661737470713a636f6d706163742d7368616b653235363a6831363a673337353a633430313a333432636f6c733a393233736c6f74733a3635353336726f77733a38626c6f7775703a3137666f6c64733a7072656669782d626f64793a7631261e000000000000006669786564207075626c69632063616e64696461746520636f6e74657874',
 'leaf_body_hex': '4e525430000085e95e1826c2b67068a640994478399e0047000000000000002109e3fab35cbe42020101010201000400000000040000000004800000003101000000000000002820000000000000000000000000000000000000000000000000000000000000000000000000000000',
 'leaf_raw_tape_hex': '15a77b743718a950590e1883df6ec7985f7d134e4042ad579df0104539ea7acd4a606b8a981d289b74592a968f821a66baf7a2d27489d6ddba39f13f7cedf3df335af74ba20774c75fb28477021cc011292bd8eb309c594cad6a97f26df72fd301b899d6cee0ffee6e9417c9b83bcb1ec341845e4b3f0c98fc60dc178882cd9a',
 'leaf_root_hex': '15a77b743718a950590e1883df6ec7985f7d134e4042ad579df0104539ea7acd4a606b8a981d289b74592a968f821a66',
 'dummy_tape_hex': '343f90e3854092b180adf8b291a03857a47c62af13de8314cf9cb178720c656e42dd41dc0e245aa07bcae130182d9342',
 'state_after_dummy_zero_root_hex': '11cae1e579a7989a33ecb7357e6fbc3c49ff1e2fc71e5aeafbc38704800205875cf4167029e93ea5159df935f7755190',
 'resource_bounds': [{'context_bytes': 30,
                      'canonical_prefix_bytes': 186,
                      'prefix_partial_rate_bytes': 50,
                      'verifier_expansions_bound': 44584,
                      'verifier_body_bytes_bound': 9988761,
                      'logical_input_bytes_bound': 18281385,
                      'cached_absorb_api_input_bytes_bound': 9988947,
                      'cold_keccak_permutations_bound': 149355,
                      'cached_keccak_permutations_bound': 104772,
                      'saved_context_reabsorptions_bound': 44583,
                      'row_body_bytes': 2817,
                      'parent_body_bytes': 184,
                      'max_chain_body_bytes': 29724,
                      'g_body_bytes': 127,
                      'qualification': 'integer upper bounds for q375 expansion; not measured '
                                       'proof time or allocation limits; cloning and encoding work '
                                       'excluded'},
                     {'context_bytes': 262144,
                      'canonical_prefix_bytes': 262302,
                      'prefix_partial_rate_bytes': 94,
                      'verifier_expansions_bound': 44584,
                      'verifier_body_bytes_bound': 9988761,
                      'logical_input_bytes_bound': 11704461129,
                      'cached_absorb_api_input_bytes_bound': 10251063,
                      'cold_keccak_permutations_bound': 86101525,
                      'cached_keccak_permutations_bound': 145501,
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
 'prefix_bytes': 382,
 'zero_leaf_roots': {'row': 'b46a0302a2054203efc8e8e4efc034d0d41df4e289af4799c76e63d73072364cfdea264a713b93294634c85f8246a31b',
                     'mixed': '74ce1c1f2a26bc6b49b5cf91a98f9ba951006c06f153abbcf4fe943742e8e9536ec0278da7e1896d09ba5aec5ffb728e',
                     'quotient': '5b1429cce353590adbfb099dfbaeac29c3365addf2781a39d4b1bcbbd4817cf8cdebb8a4b5a13fd754823161d80da41e',
                     'fri0': '490044daca5b1017f7a1191b8d2a3718db199cf388337c09cd6cc863e21d6e1a738f405d46fff73b5f7250cfe10d456a',
                     'terminal': '0a812fa9936286dc4beb14e3cc646cfbdac68d554607867ace2fcea5233d0bb87be416416609b1d3e3375e4212ecfb0a'}}
EXPECTED_QUERY = {'candidates': 401,
 'bytes': 953,
 'used_bits': 7619,
 'padding_variants': 32,
 'narrow_profile_sha256': '19093354f57a228cf17a92d94212a4419167225d04e4e2ab46d0a2a6c6860ba4'}

def main():
    if not __debug__:
        raise SystemExit('refusing optimized Python: controls require assertions')
    assert calculate() == EXPECTED_PREFIX, 'prefix/body encoding or resource bounds changed'
    assert calculate_engine() == EXPECTED_ENGINE, 'complete engine context encoding changed'
    assert query_controls() == EXPECTED_QUERY, '401-label query/padding controls changed'
    print('PASS: canonical prefix/body and structured engine context, independent SHAKE KATs and exact resource bounds')


if __name__ == '__main__':
    main()
