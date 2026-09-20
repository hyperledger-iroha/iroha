"""Bounded public experiment identity derived without runtime paths or secrets.

These inputs commit declared hardware/source identity and retained main images.
RuntimeAdmission separately verifies the existing release source, binary and
Python runtime contracts. Resource measurements describe the declared peer and
storage scope; these projections do not attest continuous whole-host quotas.
"""
from dataclasses import dataclass, fields
import hashlib
import re
from pathlib import Path

from scaling_fixed_trial import TrialRuntime
from scaling_experiment_plan import encode, require
from scaling_worker_sources import WorkerSourceFiles
from resource_process import ExecutableImage


@dataclass(frozen=True, slots=True)
class ExperimentIdentity:
    """Declared hardware and build identity; native executable hashes are derived."""
    machine_id: str
    cpu_model: str
    storage_model: str
    physical_cores: int
    logical_cores: int
    memory_bytes: int
    os: str
    kernel: str
    architecture: str
    python_version: str
    rustc_version: str
    source_revision: str
    workspace_source_sha256: str


def public_inputs(identity: ExperimentIdentity, runtime: TrialRuntime,
                  plan: bytes, workers: WorkerSourceFiles) -> tuple[bytes, bytes]:
    """Derive two closed public projections from bounded exact typed inputs."""
    require(type(identity) is ExperimentIdentity and type(runtime) is TrialRuntime
            and type(workers) is WorkerSourceFiles
            and type(plan) is bytes and 0 < len(plan) <= 1024 * 1024)
    workers.validate()
    require(type(runtime.resource_worker) is type(Path('/'))
            and type(runtime.resource_worker_sha256) is str
            and re.fullmatch(r'[a-f0-9]{64}',runtime.resource_worker_sha256))
    require(runtime.resource_worker == workers.worker_path
            and runtime.resource_worker_sha256 == workers.pins[0].sha256)
    counts = ('physical_cores', 'logical_cores', 'memory_bytes')
    for field in fields(ExperimentIdentity):
        item = getattr(identity, field.name)
        if field.name in counts:
            require(type(item) is int and 0 < item < 1 << 63)
        else:
            require(type(item) is str and 0 < len(item) <= 512
                    and all(32 <= ord(char) < 127 for char in item))
    require(identity.physical_cores <= identity.logical_cores <= 65_536)
    require(re.fullmatch(r'(?:[a-f0-9]{40}|[a-f0-9]{64})', identity.source_revision))
    require(re.fullmatch(r'[a-f0-9]{64}', identity.workspace_source_sha256))
    images = {}
    for role in ('kagami', 'cli', 'daemon', 'resource_program'):
        image = getattr(runtime, role)
        require(type(image) is ExecutableImage)
        image.validate()
        require(type(image.sha256) is str and re.fullmatch(r'[a-f0-9]{64}', image.sha256))
        images[role] = image.sha256
    require(type(runtime.resource_worker_sha256) is str
            and re.fullmatch(r'[a-f0-9]{64}', runtime.resource_worker_sha256))
    hardware = {name: getattr(identity, name) for name in
                ('machine_id', 'cpu_model', 'storage_model', *counts)}
    software = {name: getattr(identity, name) for name in
                ('os', 'kernel', 'architecture', 'python_version', 'rustc_version',
                 'source_revision', 'workspace_source_sha256')}
    software.update(plan_sha256=hashlib.sha256(plan).hexdigest(),
                    irohad_sha256=images['daemon'], iroha_cli_sha256=images['cli'])
    public = encode(dict(schema='iroha.sumeragi_v2.multilane_scaling.fixed_identity.v1',
                         hardware=hardware, software=software))
    # Explicit scope: these hashes cannot attest interpreter dependencies or
    # continuous host quota enforcement. The experiment owner issues no PASS.
    source = encode(dict(schema='iroha.sumeragi_v2.multilane_scaling.source_inputs.v1',
        source_revision=identity.source_revision,
        workspace_source_sha256=identity.workspace_source_sha256,
        executable_images=images,
        resource_sources=[dict(name=item.name, sha256=item.sha256, bytes=item.bytes)
                          for item in workers.pins],
        scope='main_executable_images_and_fixed_resource_sources'))
    workers.validate()
    return public, source
