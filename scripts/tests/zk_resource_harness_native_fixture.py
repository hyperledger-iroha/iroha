"""Explicit normal-publication inputs for real public harness tests.

These values are supplied by the independent test invoker after normal publisher
validation. This fixture never locates a module, computes authoritative pins, or
loads a diagnostic artifact. The actual public handler retains and validates all
source and artifact owners on every invocation.
"""
from dataclasses import dataclass
from pathlib import Path
import resource_observation_contract as contracts


@dataclass(frozen=True)
class NormalPublicationFixture:
    """Six explicit run inputs and independent reader requirements, not admission."""
    source_root: str
    bridge_root: str
    module_sha256: str
    provenance_sha256: str
    source_manifest_sha256: str
    native_target: str

    def __post_init__(self):
        """Reject incomplete fixture inputs without opening source or artifacts."""
        for value in (self.source_root, self.bridge_root):
            if type(value) is not str or not Path(value).is_absolute():
                raise ValueError('fixture roots must be explicit absolute paths')
        contracts.validate(contracts.LINUX, self.pins())

    def pins(self):
        """Return the independently provided native delivery anchors."""
        return {key: getattr(self, key) for key in contracts.PIN_FIELDS}

    def requirement(self):
        """Supply the required mode before any evidence is opened."""
        return {'expected_observation_contract': contracts.LINUX,
                'expected_delivery': self.pins()}

    def arguments(self, arguments):
        """Bind explicit public CLI flags without bypassing its normal loader."""
        if not arguments or arguments[0] not in ('run', 'salvage'):
            raise ValueError('fixture selects only public run or salvage')
        common = ['--trusted-observer-sha256', self.module_sha256,
                  '--trusted-observer-provenance-sha256', self.provenance_sha256,
                  '--trusted-source-manifest-sha256', self.source_manifest_sha256,
                  '--observer-native-target', self.native_target]
        selected = (['--observer-source-root', self.source_root,
                     '--observer-bridge-root', self.bridge_root] if arguments[0] == 'run'
                    else ['--require-observation-contract', contracts.LINUX])
        flags = {*common[::2], *selected[::2]}
        if any(value in flags for value in arguments[1:]):
            raise ValueError('fixture inputs cannot be overridden by a test command')
        return [arguments[0], *selected, *common, *arguments[1:]]
