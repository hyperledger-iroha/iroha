"""Small honest k7 Direct69 proof fixture using existing official IPA parameters.

This zero-constraint diagnostic authenticates one transcript frame. It is not an
A relation, private simulator, parameter authority, native prover or ZK sampler.
"""

from dataclasses import dataclass
from functools import lru_cache
import hashlib
import json

import terminal


def uint(value, width=4):
    return value.to_bytes(width, 'little')


def frame(raw):
    size, prefix = len(raw), bytearray()
    while size >= 128:
        prefix.append((size & 127) | 128)
        size >>= 7
    prefix.append(size)
    return bytes(prefix)+raw


def sequence(values):
    return uint(len(values),8)+b''.join(frame(value) for value in values)


@dataclass(frozen=True)
class Originals:
    descriptor: bytes
    key: bytes
    parameters: bytes


def originals():
    """Check source custody even when immutable fixture originals are cached."""
    terminal.checked_sources()
    return _originals()


@lru_cache(maxsize=1)
def _originals():
    """One fixed diagnostic catalog, keeping the existing parameter authority intact."""
    ref = terminal.reference()
    data = json.loads((terminal.ROOT/'fixtures/native_prover/reference_v1.json').read_text())
    raw = bytes.fromhex(data['parameters']['eq/7'])
    curve = ref['transcript'].Curve(1)
    params = ref['parameters'].Parameters.decode(raw,curve,7)
    empty = sequence([])
    fields = [uint(1,2),uint(1),uint(curve.base,32),uint(curve.scalar,32),params.digest,
              uint(7,1),uint(2),uint(1),uint(1),uint(4,1),uint(5,2),uint(1,1),uint(3,1),
              uint(0),uint(0),uint(0),sequence([uint(69)]),empty,empty,empty,
              frame(uint(0,1))+frame(uint(0))+frame(empty),empty,empty,empty,
              sequence([uint(1)])]
    payload = b''.join(frame(field) for field in fields)
    schema = hashlib.sha256(b'norito:v1:type-name\0iroha.plonk.pipa.circuit_descriptor.v2').digest()[:16]
    descriptor = b'NRT0\0\0'+schema+b'\0'+uint(len(payload),8)+uint(ref['codec'].crc64(payload),8)+b'\x02'+payload
    parsed = ref['descriptor'].Descriptor.decode(descriptor,2)
    key = b'\x02'+uint(7)+b'\0'+uint(0)
    ref['verify'].key_points(parsed,key)
    return Originals(descriptor,key,raw)


def prove(words, seed):
    """Construct one deterministic honest proof; finite-point failure never retries.

    Polynomial coefficients and blinds are explicit diagnostic values. All group
    operations use official points, without their discrete logs. The unchanged
    verifier checks every resulting proof through its full generator decision.
    """
    ref = terminal.reference()
    source = originals()
    d = ref['descriptor'].Descriptor.decode(source.descriptor,2)
    c, m, n = d.curve, d.curve.scalar, d.n
    params = ref['parameters'].Parameters.decode(source.parameters,c,7)
    terminal.transcript.require(type(seed) is int and 0 <= seed < 1<<64, 'diagnostic seed')
    d.check_instances((words,))
    words = tuple(words)

    class Writer(ref['transcript'].Transcript):
        def __init__(self):
            super().__init__(c,2,b'')
            self.output = bytearray()
        def point(self,point):
            self.common_point(point)
            self.output.extend(c.encode(point))
        def scalar(self,value):
            self.common_scalar(value)
            self.output.extend(uint(value,32))

    def coin(label):
        return int.from_bytes(hashlib.sha256(uint(seed,8)+label.encode()).digest(),'little')%m
    def polynomial(label):return [coin(label+'/'+str(i)) for i in range(n)]
    def evaluate(a,x):
        result=0
        for value in reversed(a):result=(result*x+value)%m
        return result
    def commit(a,blind):return c.add(c.sum(zip(a,params.g)),c.multiply(params.w,blind))
    def inner(a,b):return sum(x*y for x,y in zip(a,b))%m

    t=Writer();t.common_native(d.repr(source.key))
    for value in (int.from_bytes(b'pipainst','little'),1,69,1):t.common_native(value)
    for word in words:t.common_scalar(word)
    theta=t.squeeze();beta=t.squeeze();gamma=t.squeeze()
    random=polynomial('random');random_blind=coin('random-blind')
    t.point(commit(random,random_blind));y=t.squeeze()
    h_blinds=[coin('h-blind/'+str(i)) for i in range(3)]
    for blind in h_blinds:t.point(c.multiply(params.w,blind))
    x=ref['transcript'].evaluation_challenge(t.squeeze(),m);xn=pow(x,n,m)
    t.scalar(evaluate(random,x))
    x1,x2=t.squeeze(),t.squeeze()
    # One opening group, ordered slots(h,random): P=random+x1*H, H=0.
    # Q=(P-P(x))/(X-x), with an independent commitment blind.
    q=[0]*n
    for i in range(n-2,-1,-1):q[i]=(random[i+1]+x*q[i+1])%m
    q_blind=coin('quotient-blind');t.point(commit(q,q_blind))
    x3=ref['transcript'].multiopening_challenge(t.squeeze(),m)
    t.scalar(evaluate(random,x3));x4=t.squeeze()
    opening=[(a+x4*b)%m for a,b in zip(random,q)]
    blind=(random_blind+x1*sum(pow(xn,i,m)*v for i,v in enumerate(h_blinds))+x4*q_blind)%m
    s=polynomial('ipa-mask');s[0]=(s[0]-evaluate(s,x3))%m
    s_blind=coin('ipa-mask-blind');t.point(commit(s,s_blind))
    xi=ref['transcript'].nonzero_challenge(t.squeeze(),m);zeta=t.squeeze()
    a=[(v+xi*w)%m for v,w in zip(opening,s)];a[0]=(a[0]-evaluate(a,x3))%m
    b=[pow(x3,i,m) for i in range(n)];g=list(params.g);blind=(blind+xi*s_blind)%m
    for round_index in range(7):
        half=len(a)//2
        al,ah=a[:half],a[half:];bl,bh=b[:half],b[half:];gl,gh=g[:half],g[half:]
        l_blind,r_blind=coin('left/'+str(round_index)),coin('right/'+str(round_index))
        left=c.add(c.add(c.sum(zip(ah,gl)),c.multiply(params.u,zeta*inner(ah,bl))),c.multiply(params.w,l_blind))
        right=c.add(c.add(c.sum(zip(al,gh)),c.multiply(params.u,zeta*inner(al,bh))),c.multiply(params.w,r_blind))
        t.point(left);t.point(right)
        u=ref['transcript'].nonzero_challenge(t.squeeze(),m);inv=pow(u,-1,m)
        a=[(lo+inv*hi)%m for lo,hi in zip(al,ah)]
        b=[(lo+u*hi)%m for lo,hi in zip(bl,bh)]
        g=[c.add(lo,c.multiply(hi,u)) for lo,hi in zip(gl,gh)]
        blind=(blind+inv*l_blind+u*r_blind)%m
    t.scalar(a[0]);t.scalar(blind);t.output.extend(c.encode(g[0]))
    proof=bytes(t.output)
    verified=ref['verify'].verify(source.descriptor,2,source.key,source.parameters,(words,),proof)
    return proof,verified
