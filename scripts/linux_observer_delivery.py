"""Explicit independently pinned Linux observer delivery through existing native owners.

This module never discovers a library or adopts a hash calculated from it. The
installed Python entry is application trust; the existing reviewed registry binds
its custody/admission helpers before their execution. Observation resources borrow
the complete original source/artifact stack until the native close boundary.
"""
from __future__ import annotations
from contextlib import ExitStack
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import sys
import time

import pipeline_registry
import resource_observation_contract as contracts
import linux_root_observer_abi as abi
import linux_owned_process_lifetime as lifetime

# Generated from the same copied canonical modules; changing an owner requires
# explicitly reviewing its new literal alongside this caller.
REVIEWED_MODULES = {'workspace_source_keeper.py': '6eaea08b23cad1cebae891456d99cfc3a3f61c163c91bd9048b8c53841652d01', 'workspace_source_keeper_windows.py': '8dea082905fca974c65cb44731c2c4a3411dbb4f2a46c0d63bd08627c525d3a2', 'compute_workspace_source_manifest.py': 'e49fefa6d19de36773c01661717956e70fa21e8508fe24ab16230d984f2c79d3', 'sumeragi_prebuilt_bundle.py': '427b2eeaafe69e050b57971bc15a3a40cdeaa60520e15583577e7e2326215478', 'release_entry_owner.py': '4e3e0142434c28ce489fffc829469666311582c8c05933516552e499c056c050', 'release_process_bridge.py': '13796955c7cd0215df4cc62a33e9de0c4df45446a78096f18151ef5781da1710', 'release_bridge_admission.py': 'fe47ddde0cb35c5a35620ff8c312334f6c878d2cbe80a4a29399d538b430e6c9', 'environment-policy.json': 'dd7ea93cde4e2a6fa3c1564b0fca87e4357e1273bb6384d318d6c2d49eed13b8'}
MAX_LOCK_BYTES=16*1024*1024
_RETAINED_DELIVERIES=[]


def require(value,message):
    """Refuse incomplete public delivery inputs without any ambient fallback."""
    if not value:raise ValueError(message)


@dataclass(frozen=True)
class DeliveryPins:
    """Independent caller authority for exactly one native source/artifact pair."""
    module_sha256:str
    provenance_sha256:str
    source_manifest_sha256:str
    native_target:str

    def validate(self):
        """Validate exact lowercase anchors; native-machine admission remains canonical."""
        contracts.validate(contracts.LINUX,self.expected())

    def expected(self):
        """Return the four independently supplied pins, without reading evidence."""
        return dict(zip(contracts.PIN_FIELDS,(self.module_sha256,self.provenance_sha256,
                    self.source_manifest_sha256,self.native_target),strict=True))


def pins_from_arguments(arguments):
    """Require the complete explicit Linux run input group before artifact acquisition."""
    values=[getattr(arguments,key,None) for key in ('observer_source_root','observer_bridge_root',
        'trusted_observer_sha256','trusted_observer_provenance_sha256',
        'trusted_source_manifest_sha256','observer_native_target')]
    require(all(type(value) is str and value for value in values),'Linux run requires all independent observer inputs')
    pins=DeliveryPins(*values[2:]);pins.validate()
    return Path(values[0]),Path(values[1]),pins


class DeliveryLifetime(lifetime.OriginalLifetime):
    """Complete native close while the enclosing workload and delivery owners survive."""
    def prepare(self,timeout_ms,output_drain_seconds):
        """Finish full census before perf can observe this supervisor's later forks."""
        self.module_owner.begin_observation(self)
        super().prepare(timeout_ms,output_drain_seconds)

    def close(self):
        """Preserve ordinary cleanup and refuse CLI completion on unknown native close."""
        # A pre-reap refusal must return to enclosing original cleanup, never
        # enter observer-close retries ahead of the existing deadline/reap owner.
        super().close()
        self.module_owner.wait_closed(self)
        self.module_owner.finish_observation(self)


class ObserverDelivery:
    """One synchronous observer invocation with the complete retained owner stack."""
    def __init__(self,repository,bridge_root,pins,*,cleanup_original,
                 registry_type=pipeline_registry.ReviewedRegistry):
        """Allocate all ownership state before opening source or artifact originals."""
        self.repository=Path(repository);self.bridge_root=Path(bridge_root);self.pins=pins
        require(callable(cleanup_original),'original lifetime cleanup callback is required')
        self.cleanup_original=cleanup_original
        self.registry_type=registry_type;self.stack=ExitStack();self.registry=None
        self.source=None;self.bridge=None;self.library=None;self.used=False
        self.closed=False;self.first=None;self.active=None;self.delivery=None
        self.registered=False
        self.observing=False;self.observation_checked=False;self.observation_validated=False

    def retain(self,error):
        """Latch the first original object before any diagnostic or cleanup allocation."""
        if self.first is None:self.first=error

    def __enter__(self):
        """Reuse canonical registry/source/lock/provenance/original-FD admission once."""
        require(sys.platform.startswith('linux'),'Linux observer delivery requested on another host')
        self.pins.validate()
        try:
            # Register custody before any native observation token can be allocated.
            _RETAINED_DELIVERIES.append(self);self.registered=True
            self.registry=self.stack.enter_context(self.registry_type(Path(__file__).parent,REVIEWED_MODULES))
            modules=self.registry.modules;entry=modules['release_entry_owner']
            custody=modules['workspace_source_keeper'];source_module=modules['compute_workspace_source_manifest']
            repository=entry._path(str(self.repository));bridge_root=entry._path(str(self.bridge_root))
            source_root=self.stack.enter_context(entry.RootSecurity(repository,custody))
            artifact_root=self.stack.enter_context(entry.RootSecurity(bridge_root,custody))
            require(source_root.identity not in artifact_root.ancestry and artifact_root.identity not in source_root.ancestry,
                    'observer artifact and canonical source roots physically overlap')
            self.source=self.stack.enter_context(source_module.retained_workspace_source(
                repository,self.pins.source_manifest_sha256,custody_module=custody))
            bridge_module=modules['release_bridge_admission']
            native_pins=bridge_module.BridgePins(self.pins.module_sha256,self.pins.provenance_sha256,
                                                self.pins.source_manifest_sha256,self.pins.native_target)
            self.bridge=bridge_module.load_original(bridge_root,native_pins,self.source,self.stack,
                entry_module=entry,custody_module=custody,
                environment_policy=self.registry.payloads['environment-policy.json'])
            self.library=self.bridge.library
            lock=hashlib.sha256();length=0
            with self.source.reader(repository/'Cargo.lock') as original:
                while block:=original.read(64*1024):
                    length+=len(block);require(length<=MAX_LOCK_BYTES,'retained observer lock exceeds bound');lock.update(block)
            self.delivery={'version':1,'kind':'independently_pinned_native_observer',
                'module_sha256':self.pins.module_sha256,'provenance_sha256':self.pins.provenance_sha256,
                'source_manifest_sha256':self.pins.source_manifest_sha256,'cargo_lock_sha256':lock.hexdigest(),
                'native_target':self.pins.native_target}
            # All five required symbols must exist before a workload can be launched.
            for suffix in ('prepare','bind','poll','finish','close'):
                getattr(self.library,'iroha_process_root_observer_'+suffix+'_v1')
            self.validate()
            return self
        except BaseException as error:
            self.retain(error)
            try:self.stack.close()
            except BaseException:pass
            self.closed=True
            if self.registered:_RETAINED_DELIVERIES.remove(self);self.registered=False
            raise

    def validate(self):
        """Keep live checks process-free; full census is mandatory around observation."""
        require(not self.closed and self.bridge is not None,'observer delivery is closed or incomplete')
        self.registry.keeper.revalidate()
        if self.observing:
            require(callable(self.bridge.validate_originals),'process-free original validation is unavailable')
            self.bridge.validate_originals()
        else:
            self.bridge.validate()

    def begin_observation(self,owner):
        """Complete canonical admission before native preparation and the sole child fork."""
        require(owner is self.active and not self.observing and not self.observation_checked,
                'invalid observation validation stage')
        self.validate()
        self.observing=True

    def finish_observation(self,owner):
        """Require closed native resources and a full census before report acceptance."""
        require(owner is self.active and owner.native.closed,'observer remains open at final census')
        if owner.first is not None:self.retain(owner.first)
        self.observing=False
        if not self.observation_checked:
            try:
                self.validate()
                self.observation_validated=True
            except BaseException as failure:
                self.retain(failure)
                owner.retain('outer_close',failure)
                raise
            finally:
                # A completed, refused census is not an unknown native close.
                # Preserve its first failure without parking already-closed owners.
                self.observation_checked=True

    def evidence(self):
        """Project the independent pins and canonical original lock, never self-adopt hashes."""
        self.validate()
        return dict(self.delivery)

    def factory(self,workload,tail_bytes):
        """Create only one original lifetime, before prepare/Popen or stream readers."""
        require(not self.used,'observer delivery is single use')
        self.used=True
        self.validate()
        self.active=DeliveryLifetime(self,workload,tail_bytes)
        return self.active

    def wait_closed(self,owner):
        """Retain source/module originals until observation close is genuinely known.

        Called from original-lifetime close only after its owned root is reaped.
        Unknown preparation or unsupported descendants stay refused and retained;
        no additional timeout, signal, or successful command inference is added.
        """
        require(owner.module_owner is self,'foreign observer lifetime')
        while True:
            try:
                if owner.first is not None:self.retain(owner.first)
                if owner.native.closed:return
                status=owner.native.close()
                require(status==abi.OK,'observer close remains unknown')
            except BaseException as error:
                self.retain(error)
                owner.retain('native_close',error)
            if not owner.native.closed:
                try:time.sleep(.01)
                except BaseException as error:self.retain(error)

    def _complete(self):
        """Retry guarded original cleanup and native close before releasing the stack.

        This encompasses acquisition/iteration and callback errors, including a
        one-shot unwind outside an inner waiter. A registered but unassigned
        observation is also retained; missing creation evidence never means no
        child. The callback is the harness's existing original-only cleanup.
        """
        while True:
            try:
                if self.active is not None:
                    if self.active.first is not None:self.retain(self.active.first)
                    if not self.active.closed:
                        self.cleanup_original(self.active)
                    self.wait_closed(self.active)
                    self.finish_observation(self.active)
                pending=None
                for native in abi._RETAINED_MODULES:
                    if native.image is self:
                        pending=native
                        break
                if pending is None:return
                if pending.closed:
                    abi._RETAINED_MODULES.remove(pending)
                    continue
                require(pending.close()==abi.OK,'observer preparation close remains unknown')
            except BaseException as failure:
                self.retain(failure)
                try:time.sleep(.01)
                except BaseException as later:self.retain(later)

    def __exit__(self,kind,error,traceback):
        """No normal CLI unwind releases originals before actual observation close."""
        try:
            if error is not None:self.retain(error)
            self._complete()
        except BaseException as failure:
            self.retain(failure)
        finally:
            # Actual final supervision, not a global leak: a one-shot unwind at
            # the first cleanup entry/header must still finish the same owners.
            self._complete()
        try:self.validate()
        except BaseException as failure:self.retain(failure)
        first=self.first
        try:
            try:
                self.stack.__exit__(type(first) if first is not None else kind,
                    first if first is not None else error,
                    first.__traceback__ if first is not None else traceback)
            except BaseException:
                if first is not None:raise first
                raise
        finally:
            self.closed=True
            if self.registered:
                _RETAINED_DELIVERIES.remove(self)
                self.registered=False
        if first is not None and first is not error:raise first
        return False
