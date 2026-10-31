"""Bounded verified-source terminal projection; never native TerminalPlan admission."""

import hashlib
import importlib
import json
from pathlib import Path
import sys

import transcript

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FILES = frozenset(('README.md', 'terminal.py', 'terminal_fixture.py', 'test_terminal.py'))
# The retained exact inventory is generated once in the proposal, then literal.
SOURCES = frozenset((
    'crates/iroha_kagemusha_proof/src/private_fold.rs',
    'crates/iroha_pasta/src/curve/hash_to_curve.rs',
    'crates/iroha_pasta/src/curve/mod.rs',
    'crates/iroha_pasta/src/fft.rs',
    'crates/iroha_pasta/src/field/fp.rs',
    'crates/iroha_pasta/src/field/fq.rs',
    'crates/iroha_pasta/src/field/mod.rs',
    'crates/iroha_pasta/src/params.rs',
    'crates/iroha_pasta/src/poseidon/mod.rs',
    'crates/iroha_pasta/src/poseidon/rp57_fp.bin',
    'crates/iroha_pasta/src/poseidon/rp57_fq.bin',
    'crates/iroha_plonk/src/cs/descriptor_v2.rs',
    'crates/iroha_plonk/src/keys/vk.rs',
    'crates/iroha_plonk/src/pcs/ipa/generator_prefix.rs',
    'crates/iroha_plonk/src/pcs/ipa/mod.rs',
    'crates/iroha_plonk/src/pcs/ipa/prover.rs',
    'crates/iroha_plonk/src/pcs/multiopen/prover.rs',
    'crates/iroha_plonk/src/transcript/mod.rs',
    'crates/iroha_plonk/src/transcript/pipa_r.rs',
    'crates/iroha_plonk/src/verifier.rs',
    'crates/iroha_plonk_gadgets/src/poseidon/sponge.rs',
    'crates/iroha_plonk_gadgets/src/pow5_fq/duplex.rs',
    'crates/iroha_plonk_recursion/src/accumulation.rs',
    'crates/iroha_plonk_recursion/src/accumulation_circuit.rs',
    'crates/iroha_plonk_recursion/src/claim.rs',
    'crates/iroha_plonk_recursion/src/fold_tape.rs',
    'crates/iroha_plonk_recursion/src/lib.rs',
    'crates/iroha_plonk_recursion/src/purpose.rs',
    'crates/iroha_plonk_recursion/src/terminal.rs',
    'crates/iroha_plonk_recursion/src/tests.rs',
    'crates/iroha_plonk_recursion/src/transcript.rs',
    'fixtures/native_prover/kats_v1.json',
    'fixtures/native_prover/reference_v1.json',
    'fixtures/native_prover/reference_verifier/__init__.py',
    'fixtures/native_prover/reference_verifier/codec.py',
    'fixtures/native_prover/reference_verifier/curve.py',
    'fixtures/native_prover/reference_verifier/descriptor.py',
    'fixtures/native_prover/reference_verifier/parameters.py',
    'fixtures/native_prover/reference_verifier/transcript.py',
    'fixtures/native_prover/reference_verifier/verify.py',
))


def checked_sources():
    """Bind every imported verifier/fixture and selected native correspondence owner."""
    transcript.checked_sources()
    transcript.require(sys.flags.optimize == 0, 'unoptimized terminal controls only')
    manifest = json.loads((HERE/'terminal_source_manifest.json').read_text())
    transcript.require(type(manifest) is dict and set(manifest) == {'schema','files','sources'},
                       'terminal manifest grammar')
    transcript.require(manifest['schema'] == 'kagemusha.fold.terminal-controls.v1',
                       'terminal manifest schema')
    for section, roles, root in (('files',FILES,HERE), ('sources',SOURCES,ROOT)):
        transcript.require(type(manifest[section]) is dict and set(manifest[section]) == roles,
                           'terminal inventory')
        for name, digest in manifest[section].items():
            transcript.require(type(digest) is str and len(digest) == 64 and
                               all(c in '0123456789abcdef' for c in digest), 'terminal digest')
            transcript.require(hashlib.sha256((root/name).read_bytes()).hexdigest() == digest,
                               'changed terminal source '+name)
    return manifest


def reference():
    """Import unchanged full verifier modules only after all exact source checks."""
    checked_sources()
    package = transcript.reference().__package__
    return {name:importlib.import_module(package+'.'+name)
            for name in ('verify','descriptor','codec','parameters','transcript')}


def _point(words, curve):
    """Four canonical u128 limbs encode a full-group Vesta G, including identity."""
    transcript.require(len(words) == 4 and all(type(v) is int and 0 <= v < 1<<128 for v in words),
                       'terminal coordinate limbs')
    point = (words[0]+(words[1]<<128),words[2]+(words[3]<<128))
    transcript.require(all(value < curve.base for value in point), 'terminal coordinate field')
    if point != (0,0):transcript.point_words(1,point)
    return point


def _claim(point, challenges, width, curve):
    """The native sixteen-coordinate normalized source grammar, including k7 here."""
    transcript.require(type(width) is int and 1 <= width <= 16, 'terminal source width')
    transcript.require(len(challenges) == 16 and
                       all(type(v) is int and 0 <= v < curve.scalar for v in challenges),
                       'terminal source scalars')
    transcript.require(tuple(challenges[:16-width]) == (0,)*(16-width) and
                       all(value != 0 for value in challenges[16-width:]), 'terminal source padding')
    return point,width,tuple(challenges)


def _trivial(curve):
    """Read the exact native constant as DATA, without asserting its k16 decision."""
    import re
    source=(ROOT/'crates/iroha_plonk_recursion/src/lib.rs').read_text()
    match=re.search(r'pub const VESTA_TRIVIAL_GENERATOR: \[u8; 32\] = \[(.*?)\];',source,re.S)
    transcript.require(match is not None, 'trivial generator owner')
    tokens=re.findall(r'0x([0-9a-f]{2})',match.group(1))
    transcript.require(len(tokens) == 32, 'trivial generator extent')
    return curve.affine(curve.decode(bytes.fromhex(''.join(tokens)))),16,(1,)*16


class BoundedTerminalFrame:
    """One immutable k7 diagnostic source verified before any terminal projection.

    Construction accepts only original proof material selected by a fixed local
    fixture catalog; there is no raw, own-claim or slots constructor. This Python
    owner guards its ordinary API, not hostile code/reflection in the interpreter.
    Its minimal relation is NOT A, and its own k7 opening is padded with nine
    zeros rather than satisfying native TerminalPlan's k16 admission rule.
    """
    __slots__=('__originals','__words','__proof','__binding','__raw','__inputs','__prefix')

    def __setattr__(self,name,value):
        raise AttributeError('immutable verified source')

    def __init__(self,originals,words,proof,*,key_index=0):
        import terminal_fixture
        transcript.require(not hasattr(self,'_BoundedTerminalFrame__originals'),
                           'source already initialized')
        ref=reference();expected=terminal_fixture.originals()
        transcript.require(type(key_index) is int and key_index == 0, 'diagnostic catalog index')
        transcript.require(type(originals) is terminal_fixture.Originals and
                           all(type(getattr(originals,name)) is bytes
                               for name in ('descriptor','key','parameters')) and originals == expected,
                           'diagnostic catalog originals')
        transcript.require(type(proof) is bytes, 'original proof bytes')
        # Copy the frame before verification. A later caller list edit cannot
        # retarget the stored projection or transcript prefix.
        transcript.require(type(words) in (list,tuple), 'original frame words')
        frame=tuple(words)
        verified=ref['verify'].verify(originals.descriptor,2,originals.key,originals.parameters,
                                      (frame,),proof)
        d=ref['descriptor'].Descriptor.decode(originals.descriptor,2);curve=d.curve
        transcript.require((d['curve'],d['k'],d['transcript'],d['instance_mode'],d['proof_suffix'])
                           == (1,7,2,1,1) and d['instance_lengths'] == [69] and
                           d['instance_types'] == [(1,None)] and d['num_advice_columns'] == 0,
                           'bounded diagnostic source shape')
        transcript.require(verified.descriptor_digest == d.digest and len(verified.rounds) == 7,
                           'same verified own opening')
        # This minimal source has no advice commitments before Theta. Replay its
        # exact admitted prefix only AFTER full verification, with no raw setter.
        prefix=(int.from_bytes(b'pipa-rb1','little'),d.repr(originals.key),
                int.from_bytes(b'pipainst','little'),1,69,1)+frame
        prefix_trace=transcript.trace_script(1,(prefix,))
        transcript.check_trace(1,(prefix,),prefix_trace)
        raw=prefix_trace['raw'][0]
        transcript.require(raw % curve.scalar == verified.challenges[0], 'same raw prefix map')
        own_point=(0,0) if verified.generator[2] == 0 else curve.affine(verified.generator)
        own=_claim(own_point,(0,)*9+verified.rounds,7,curve)
        transcript.require(frame[1] in (12,14,16), 'native part width')
        part=_claim(_point(frame[2:6],curve),frame[6:22],frame[1],curve)
        predecessor=_claim(_point(frame[22:26],curve),frame[26:42],16,curve)
        incoming=_claim(_point(frame[42:46],curve),frame[46:62],16,curve)
        corrected=_point(frame[65:69],curve)  # validated even when not selected
        mode=frame[62:65]
        if mode == (1,0,0):selected=incoming
        elif mode == (0,1,0):selected=_trivial(curve)
        elif mode == (0,0,1):
            transcript.require(corrected != incoming[0], 'unchanged corrected claim')
            selected=_claim(corrected,incoming[2],16,curve)
        else:raise ValueError('terminal mode')
        for name,value in (('originals',originals),('words',frame),('proof',proof),
                           ('binding',d.digest),('raw',raw),
                           ('inputs',(part,own,predecessor,selected)),('prefix',prefix_trace)):
            object.__setattr__(self,'_BoundedTerminalFrame__'+name,value)

    def snapshot(self):
        """Return immutable-valued diagnostic observations, never an intake handle."""
        return {'descriptor':self.__binding,'first_raw':self.__raw,'inputs':self.__inputs,
                'lineage':self.__words[0],'words':self.__words,'proof':self.__proof,
                'prefix_calls':len(self.__prefix['edges'])}

    def trace(self,masks,rho):
        """Prefix-bound Terminal digest and downstream syntactic challenge schedule.

        Source fields always come from this verified owner. This does not create
        a fold proof, decide the other slots or prove correction/burn validity.
        """
        curve=transcript.curve(1)
        transcript.require(type(masks) is tuple and len(masks) == 32, 'terminal mask extent')
        for value in (*masks,rho):transcript.scalar_words(1,value)
        parameter_digest=hashlib.sha256(self.__originals.parameters).digest()
        context=(int.from_bytes(parameter_digest[:16],'little'),
                 int.from_bytes(parameter_digest[16:],'little'),
                 int.from_bytes(b'wrapv001','little'),4)
        descriptor=(int.from_bytes(self.__binding[:16],'little'),
                    int.from_bytes(self.__binding[16:],'little'))
        point,width,challenges=self.__inputs[1]
        payload=context+descriptor+(self.__raw,)+point+(width,)+challenges+masks
        words=(int.from_bytes(b'pipa-tm1','little'),len(payload))+payload
        digest_trace=transcript.trace_script(1,(words,));transcript.check_trace(1,(words,),digest_trace)
        h=digest_trace['raw'][0]
        ref=reference();params=ref['parameters'].Parameters.decode(self.__originals.parameters,curve,7)
        d=curve.sum(((h&((1<<128)-1),params.g[0]),(h>>128,params.g[1]),(rho,params.w)))
        transcript.require(d[2] != 0,'identity terminal D')
        d=curve.affine(d)
        first=(transcript.DOMAIN,)+context+transcript.point_words(1,d)
        pair=(curve.base-1,2,curve.base-1,curve.base-2)
        buffers=(first,(),())+(pair,)*16
        challenges_trace=transcript.trace_script(1,buffers)
        transcript.check_trace(1,buffers,challenges_trace)
        return {'digest_words':words,'input_digest':h,'commitment':d,
                'digest_trace':digest_trace,'challenge_buffers':buffers,
                'challenge_trace':challenges_trace}
