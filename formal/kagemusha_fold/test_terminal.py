"""Real reference-proof acceptance versus bounded terminal projection controls."""

import copy
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import terminal as subject
import terminal_fixture as fixture


def coordinates(point):
    return tuple(v for coordinate in point for v in (coordinate&((1<<128)-1),coordinate>>128))


def frame(mode=(1,0,0), width=12, correction=None):
    c=subject.transcript.curve(1)
    positive=(c.base-1,2);negative=(c.base-1,c.base-2)
    if correction is None:correction=positive
    return (7,width,*coordinates(positive),*((0,)*(16-width)+(1,)*width),
            *coordinates(positive),*((1,)*16),*coordinates(negative),*((1,)*16),
            *mode,*coordinates(correction))


def changed(words,offset,value):
    return words[:offset]+(value,)+words[offset+1:]


class TerminalSource(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subject.checked_sources()
        cls.ref=subject.reference();cls.original=fixture.originals()
        cls.words=frame();cls.proof,cls.verified=fixture.prove(cls.words,17)
        cls.other_proof,cls.other_verified=fixture.prove(cls.words,18)

    @classmethod
    def tearDownClass(cls):
        subject.checked_sources()

    def read(self,words=None,proof=None):
        return subject.BoundedTerminalFrame(self.original,
                    self.words if words is None else words,self.proof if proof is None else proof)

    def test_full_reference_accepts_genuine_official_k7_direct69_proofs(self):
        d=self.ref['descriptor'].Descriptor.decode(self.original.descriptor,2)
        params=self.ref['parameters'].Parameters.decode(self.original.parameters,d.curve,7)
        self.assertEqual((d['k'],d['instance_lengths'],d['instance_types']), (7,[69],[(1,None)]))
        self.assertEqual((d['transcript'],d['instance_mode'],d['proof_suffix']), (2,1,1))
        self.assertEqual((len(self.original.parameters),len(self.proof)), (8260,800))
        self.assertEqual(params.digest.hex(),'432e492288c43ccabdffb913625109ca76f66312cd4625c9a258a2e33a28c400')
        self.assertEqual(len(self.verified.rounds),7)
        result=self.ref['verify'].verify(self.original.descriptor,2,self.original.key,
                                        self.original.parameters,(self.words,),self.proof)
        self.assertEqual(result,self.verified)
        self.assertNotEqual(self.proof,self.other_proof)
        self.assertNotEqual(self.verified.rounds,self.other_verified.rounds)
        # Minimal relation has no A predicates, selectors, queries or private
        # witness. Its full proof verdict is solely this declared diagnostic.
        self.assertEqual((d['gates'],d['permutation'],d['lookups'],d['advice_queries']),([],[],[],[]))

    def test_verified_source_derives_exact_order_and_explicit_width_adaptation(self):
        source=self.read();observation=source.snapshot();part,own,predecessor,incoming=observation['inputs']
        self.assertEqual(part,((self.ref['transcript'].Curve(1).base-1,2),12,(0,)*4+(1,)*12))
        self.assertEqual(own[1:],(7,(0,)*9+self.verified.rounds))
        c=self.ref['transcript'].Curve(1)
        self.assertEqual(own[0],c.affine(self.verified.generator))
        self.assertEqual(predecessor,(part[0],16,(1,)*16))
        self.assertEqual(incoming,((c.base-1,c.base-2),16,(1,)*16))
        self.assertEqual(observation['lineage'],self.words[0])
        self.assertEqual(observation['words'],self.words)
        self.assertEqual(observation['proof'],self.proof)
        self.assertEqual(observation['descriptor'],self.verified.descriptor_digest)
        self.assertEqual(observation['prefix_calls'],38)
        self.assertEqual(observation['first_raw']%c.scalar,self.verified.challenges[0])
        # k7 is deliberately not the native TerminalPlan source-k16 requirement.
        self.assertNotEqual(own[1],16)

    def test_prefix_and_digest_bind_same_verified_source_and_all_masks(self):
        source=self.read();snapshot=source.snapshot();masks=tuple(range(32))
        trace=source.trace(masks,42);words=trace['digest_words']
        self.assertEqual((len(words),words[:2]),(60,(int.from_bytes(b'pipa-tm1','little'),58)))
        parameter=hashlib.sha256(self.original.parameters).digest()
        context=(int.from_bytes(parameter[:16],'little'),int.from_bytes(parameter[16:],'little'),
                 int.from_bytes(b'wrapv001','little'),4)
        self.assertEqual(words[2:6],context)
        self.assertEqual(words[6:8],tuple(int.from_bytes(snapshot['descriptor'][i:i+16],'little') for i in (0,16)))
        self.assertEqual(words[8],snapshot['first_raw'])
        point,width,rounds=snapshot['inputs'][1]
        self.assertEqual(words[9:28],point+(width,)+rounds)
        self.assertEqual(words[28:],masks)
        self.assertEqual(len(trace['digest_trace']['edges']),31)
        self.assertEqual((len(trace['challenge_trace']['edges']),len(trace['challenge_trace']['raw'])),(54,19))
        self.assertEqual(trace['challenge_buffers'][0][1:5],context)
        for i in range(32):
            mutated=source.trace(changed(masks,i,masks[i]+1),42)
            self.assertNotEqual(mutated['input_digest'],trace['input_digest'])
        other_rho=source.trace(masks,43)
        self.assertEqual(other_rho['input_digest'],trace['input_digest'])
        self.assertNotEqual(other_rho['commitment'],trace['commitment'])

    def test_two_valid_same_prefix_proofs_require_separate_own_slot_binding(self):
        left=self.read();right=self.read(proof=self.other_proof)
        l,r=left.snapshot(),right.snapshot()
        self.assertEqual(l['first_raw'],r['first_raw'])
        self.assertEqual(l['descriptor'],r['descriptor'])
        self.assertEqual(l['words'],r['words'])
        self.assertNotEqual(l['inputs'][1],r['inputs'][1])
        a,b=left.trace((0,)*32,0),right.trace((0,)*32,0)
        self.assertEqual(a['digest_words'][:9],b['digest_words'][:9])
        self.assertEqual(a['digest_words'][28:],b['digest_words'][28:])
        self.assertNotEqual(a['input_digest'],b['input_digest'])
        def omit_own(words):
            shortened=words[:9]+words[28:]
            return (shortened[0],len(shortened)-2)+shortened[2:]
        broken_a,broken_b=omit_own(a['digest_words']),omit_own(b['digest_words'])
        self.assertEqual(broken_a,broken_b)
        self.assertEqual(subject.transcript.trace_script(1,(broken_a,))['raw'],
                         subject.transcript.trace_script(1,(broken_b,))['raw'])

    def test_full_reference_rejects_each_of69_original_frame_substitutions(self):
        for index in range(69):
            with self.subTest(word=index):
                altered=changed(self.words,index,self.words[index]+1)
                with self.assertRaises(ValueError):
                    self.ref['verify'].verify(self.original.descriptor,2,self.original.key,
                                             self.original.parameters,(altered,),self.proof)

    def test_original_proof_substitution_refuses_before_a_projection_is_returned(self):
        c=self.ref['transcript'].Curve(1)
        invalid=(self.proof[:-1],self.proof+b'\0',bytes(32)+self.proof[32:],
                 self.proof[:-32]+c.encode((c.base-1,2,1)),
                 self.proof[:-96]+bytes([255])*32+self.proof[-64:])
        for proof in invalid:
            with self.subTest(sha=hashlib.sha256(proof).hexdigest()):
                with self.assertRaises(ValueError):self.read(proof=proof)
        # Refusal happens before prefix replay: the return is never fabricated.
        with patch.object(subject.transcript,'trace_script',side_effect=AssertionError('late replay')):
            with self.assertRaises(ValueError):self.read(proof=self.proof[:-1])
        # Refused reads cannot poison a later successful read of the originals.
        healthy=self.read().snapshot()
        self.assertEqual(healthy['proof'],self.proof)
        self.assertEqual(healthy['inputs'][1][2],(0,)*9+self.verified.rounds)

    def test_synthetic_digest_words_keep_unreduced_raw_before_scalar_mapping(self):
        # This is word-level DATA, not a fabricated admitted source or forced
        # reference squeeze. Neither altered word tuple is passed to the owner.
        c=self.ref['transcript'].Curve(1)
        words=self.read().trace((0,)*32,0)['digest_words']
        zero=changed(words,8,0);raw_p=changed(words,8,c.scalar)
        self.assertLess(c.scalar,c.base)
        self.assertEqual((zero[8]%c.scalar,raw_p[8]%c.scalar),(0,0))
        self.assertEqual(zero[:8]+zero[9:],raw_p[:8]+raw_p[9:])
        self.assertNotEqual(subject.transcript.trace_script(1,(zero,))['raw'],
                            subject.transcript.trace_script(1,(raw_p,))['raw'])

    def test_catalog_originals_and_index_are_checked_before_verification(self):
        for name in ('descriptor','key','parameters'):
            raw=getattr(self.original,name)
            for value in (raw+b'\0',bytearray(raw),list(raw),None):
                altered=replace(self.original,**{name:value})
                with self.subTest(field=name,kind=type(value).__name__):
                    with self.assertRaisesRegex(ValueError,'diagnostic catalog originals'):
                        subject.BoundedTerminalFrame(altered,self.words,self.proof)
        for index in (-1,1,True,'0'):
            with self.assertRaisesRegex(ValueError,'catalog index'):
                subject.BoundedTerminalFrame(self.original,self.words,self.proof,key_index=index)
        with self.assertRaises(ValueError):subject.BoundedTerminalFrame(None,self.words,self.proof)
        for proof in (bytearray(self.proof),list(self.proof),None):
            with self.assertRaises(ValueError):
                subject.BoundedTerminalFrame(self.original,self.words,proof)

    def test_proof_valid_accept_trivial_corrected_projection_modes(self):
        c=self.ref['transcript'].Curve(1)
        for mode,width in (((1,0,0),12),((0,1,0),14),((0,0,1),16)):
            words=frame(mode,width);proof,verified=fixture.prove(words,20+width)
            self.assertEqual(verified.descriptor_digest,self.verified.descriptor_digest)
            source=self.read(words,proof);slots=source.snapshot()['inputs']
            self.assertEqual(slots[0][1],width)
            if mode == (1,0,0):expected=((c.base-1,c.base-2),16,(1,)*16)
            elif mode == (0,1,0):expected=subject._trivial(c)
            else:expected=((c.base-1,2),16,(1,)*16)
            self.assertEqual(slots[3],expected)
            self.assertEqual(slots[3][2],(1,)*16)
        # A well-formed inactive correction may equal the incoming G, and need
        # not be zero. Only the selected Corrected mode enforces inequality.
        words=frame(correction=(c.base-1,c.base-2));proof,_=fixture.prove(words,39)
        self.assertEqual(self.read(words,proof).snapshot()['inputs'][3][0],(c.base-1,c.base-2))
        # Identity G and correction are legal syntax even though claims may not
        # decide. This zero-constraint proof does not assert A or fold validity.
        words=frame((0,0,1),correction=(0,0));proof,_=fixture.prove(words,40)
        self.assertEqual(self.read(words,proof).snapshot()['inputs'][3][0],(0,0))

    def test_proof_valid_malformed_modes_and_inactive_fields_refuse_projection(self):
        c=self.ref['transcript'].Curve(1)
        cases=(('mode-zero',changed(self.words,62,0),'terminal mode'),
               ('mode-two',changed(self.words,63,1),'terminal mode'),
               ('mode-nonboolean',changed(self.words,62,2),'terminal mode'),
               ('part-width',changed(self.words,1,11),'native part width'),
               ('part-prefix',changed(self.words,6,1),'source padding'),
               ('predecessor-zero',changed(self.words,26,0),'source padding'),
               ('inactive-incoming-zero',changed(frame((0,1,0)),46,0),'source padding'),
               ('inactive-correction-limb',changed(self.words,65,1<<128),'coordinate limbs'),
               ('inactive-correction-field',self.words[:65]+coordinates((c.base,0)),'coordinate field'),
               ('inactive-correction-curve',self.words[:65]+coordinates((1,1)),'point on curve'),
               ('unchanged-correction',frame((0,0,1),correction=(c.base-1,c.base-2)), 'unchanged corrected'))
        for index,(label,words,error) in enumerate(cases):
            with self.subTest(case=label):
                proof,accepted=fixture.prove(words,70+index)
                self.assertEqual(accepted.descriptor_digest,self.verified.descriptor_digest)
                with self.assertRaisesRegex(ValueError,error):self.read(words,proof)

    def test_source_owns_copies_and_does_not_offer_raw_or_slot_retargeting(self):
        supplied=list(self.words);source=self.read(supplied);before=source.snapshot()
        supplied[0]+=1
        self.assertEqual(source.snapshot(),before)
        for name,value in (('first_raw',0),('inputs',()),('words',()),('proof',b'')):
            with self.assertRaises(AttributeError):setattr(source,name,value)
        with self.assertRaises(TypeError):
            subject.BoundedTerminalFrame(self.original,self.words,self.proof,raw=0)
        with self.assertRaises(TypeError):
            subject.BoundedTerminalFrame(self.original,self.words,self.proof,slots=())
        with self.assertRaisesRegex(ValueError,'already initialized'):
            source.__init__(self.original,self.words,self.other_proof)
        observed=source.snapshot();observed['first_raw']=0;observed['inputs']=()
        self.assertEqual(source.snapshot(),before)

    def test_mask_and_diagnostic_seed_canonicality_refuse(self):
        source=self.read();c=self.ref['transcript'].Curve(1);masks=(0,)*32
        for invalid in ((),masks[:-1],masks+(0,),list(masks),(-1,)+masks[1:],
                        (c.scalar,)+masks[1:],(True,)+masks[1:]):
            with self.assertRaises(ValueError):source.trace(invalid,0)
        for rho in (-1,c.scalar,True):
            with self.assertRaises(ValueError):source.trace(masks,rho)
        for seed in (-1,1<<64,True):
            with self.assertRaises(ValueError):fixture.prove(self.words,seed)

    def test_exact_source_inventory_and_added_owner_mutations_refuse(self):
        raw=(subject.HERE/'terminal_source_manifest.json').read_text()
        manifest=json.loads(raw);read=Path.read_text
        for transform in ('missing','extra','schema','hash'):
            changed=copy.deepcopy(manifest)
            if transform == 'missing':del changed['sources']['fixtures/native_prover/reference_verifier/verify.py']
            elif transform == 'extra':changed['sources']['../foreign.py']='0'*64
            elif transform == 'schema':changed['schema']='unknown'
            else:changed['files']['terminal.py']='A'*64
            def substitute(path,*args,**kwargs):
                return json.dumps(changed) if path == subject.HERE/'terminal_source_manifest.json' else read(path,*args,**kwargs)
            with patch.object(Path,'read_text',substitute):
                with self.assertRaises(ValueError):subject.checked_sources()
        read_bytes=Path.read_bytes
        for name in ('fixtures/native_prover/reference_v1.json',
                     'fixtures/native_prover/reference_verifier/verify.py',
                     'fixtures/native_prover/reference_verifier/descriptor.py',
                     'fixtures/native_prover/reference_verifier/codec.py',
                     'crates/iroha_plonk/src/verifier.rs',
                     'crates/iroha_plonk_recursion/src/terminal.rs'):
            def substitute(path):
                value=read_bytes(path)
                return value+b'changed' if path == subject.ROOT/name else value
            with self.subTest(source=name):
                with patch.object(Path,'read_bytes',substitute):
                    with self.assertRaises(ValueError):subject.reference()
                    with self.assertRaises(ValueError):fixture.originals()


if __name__ == '__main__':unittest.main()
