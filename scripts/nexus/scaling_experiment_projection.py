"""Explicit public run projections with allocation checked before copying values."""
import base64
import json
import re

from scaling_completed_authority import (PublicRunProjection, PublicCommand, PublicArtifact,
    PublicRequest, ResourceSnapshot, _PUBLIC_RECORDS)
from scaling_fixed_trial import TrialPlan
from scaling_generator import GenerationReceipt
from scaling_readiness_inputs import GeneratedAccount
from scaling_native_load import NativeLoadReceipt
from scaling_experiment_plan import encode, require

_RECORD_TYPES=frozenset((*_PUBLIC_RECORDS.values(),PublicCommand,PublicArtifact,PublicRequest,ResourceSnapshot))


class _PublicTree:
    def __init__(self, cap):
        require(type(cap) is int and 0 < cap <= 256 * 1024 * 1024)
        self.remaining = cap
        self.nodes = 2_000_000

    def charge(self, size):
        self.remaining -= size
        require(self.remaining >= 0)

    def copy(self, value, depth=0):
        self.nodes -= 1
        require(self.nodes >= 0 and depth <= 20)
        if type(value) is int:
            require(-(1 << 127) < value < 1 << 128)
            self.charge(len(str(value))); return value
        if type(value) is str:
            require(len(value) <= 4096)
            self.charge(len(json.dumps(value, ensure_ascii=True))); return value
        if type(value) is bytes:
            size = 4 * ((len(value) + 2) // 3)
            self.charge(size + 2)
            return base64.b64encode(value).decode('ascii')
        if type(value) in _RECORD_TYPES:
            require(type(value._fields) is tuple and len(value._fields)<=64)
            value = {name: getattr(value, name) for name in value._fields}
        if type(value) is dict:
            require(len(value) <= 64 and all(type(key) is str and len(key) <= 128 for key in value))
            self.charge(2 + max(0, len(value) - 1))
            result = {}
            for key, item in value.items():
                self.charge(len(json.dumps(key)) + 1)
                result[key] = self.copy(item, depth + 1)
            return result
        require(type(value) is tuple and len(value) <= 100_000)
        self.charge(2 + max(0, len(value) - 1))
        return [self.copy(item, depth + 1) for item in value]


def raw_run(value: PublicRunProjection, cap: int) -> bytes:
    """Project complete original timings and reduced captures; derive no verdict."""
    require(type(value) is PublicRunProjection)
    require(type(value.plan) is _PUBLIC_RECORDS[TrialPlan])
    fields = dict(schema='iroha.sumeragi_v2.multilane_scaling.raw_run.v1',
        pair_index=value.pair_index, variant=value.variant, load=value.plan.load,
        geometry=value.geometry, resources=value.resources, requests=value.requests)
    return encode(_PublicTree(cap).copy(fields), cap)


def run_receipt(value: PublicRunProjection, raw_sha256: str, cap: int) -> bytes:
    """Publish exact native/public identities without private configs or bodies."""
    require(type(value) is PublicRunProjection)
    require(type(raw_sha256) is str and re.fullmatch(r'[a-f0-9]{64}',raw_sha256))
    generation = value.generation
    require(type(generation) is _PUBLIC_RECORDS[GenerationReceipt]
            and type(generation.accounts) is tuple and 4<=len(generation.accounts)<=64
            and all(type(item) is _PUBLIC_RECORDS[GeneratedAccount] for item in generation.accounts))
    require(type(value.load) is _PUBLIC_RECORDS[NativeLoadReceipt])
    fields = dict(schema='iroha.sumeragi_v2.multilane_scaling.run_receipt.v1',
        pair_index=value.pair_index, variant=value.variant,
        original_deadline_ns=value.original_deadline_ns,
        raw_run_sha256=raw_sha256,
        generation=dict(network_id=generation.network_id, genesis_hash=generation.genesis_hash,
            context_id=generation.context_id, genesis_public_key=generation.genesis_public_key,
            chain_discriminant=generation.chain_discriminant, anchors_sha256=generation.anchors_sha256,
            generator_sha256=generation.generator_sha256, process=generation.process,
            accounts=tuple(dict(index=item.index, account_id=item.account_id) for item in generation.accounts)),
        readiness=value.readiness, canonical=value.canonical,
        load_process=value.load.process, peers=value.peers,
        artifacts=value.artifacts)
    # All three proof-stage process identities must survive private trial close.
    fields['proof_commands'] = value.proof_commands
    return encode(_PublicTree(cap).copy(fields), cap)
