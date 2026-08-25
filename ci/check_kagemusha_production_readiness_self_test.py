"""Negative controls for the Kagemusha production-readiness gate.

The main gate executes these tests in its established Python namespace.
Promotion mode supplies only source-closure-authenticated helper bytes.
"""

if globals().get("_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1") is not True:
    raise RuntimeError("readiness self-test helper requires the authenticated gate context")

def expect_value_error(action: Callable[[], object], failure: str,
        expected: str | None = None) -> None:
    try:
        action()
    except ValueError as error:
        if expected is not None and expected not in str(error):
            raise
    else:
        errors.append(failure)
native_binding_builder = b"reviewed native builder bytes"
native_binding_launch = {
    "builder_entrypoint_sha256": hashlib.sha256(native_binding_builder).hexdigest(),
    "controller_sha256": "1" * 64,
    "python_interpreter_sha256": "2" * 64,
    "python_runtime_tree_sha256": "3" * 64,
    "macos_build": "25A1",
    "os_tcb_sha256": "4" * 64,
}
native_binding_report: dict[str, object] = {
    "native_launch_attestation": native_binding_launch
}
try:
    validate_native_build_launch_binding(
        native_binding_report, "1" * 64, "2" * 64, "3" * 64, "25A1", "4" * 64
    )
    validate_native_builder_entrypoint_binding(
        native_binding_report, native_binding_builder
    )
except ValueError as error:
    errors.append(f"native build-launch binding control failed unexpectedly: {error}")
for field, hostile in (
    ("controller_sha256", "5" * 64),
    ("python_interpreter_sha256", "5" * 64),
    ("python_runtime_tree_sha256", "5" * 64),
    ("macos_build", "25B2"),
    ("os_tcb_sha256", "5" * 64),
):
    mutated_launch = dict(native_binding_launch)
    mutated_launch[field] = hostile
    mutated_report: dict[str, object] = {
        "native_launch_attestation": mutated_launch
    }
    expect_value_error(
        lambda report=mutated_report: validate_native_build_launch_binding(
            report, "1" * 64, "2" * 64, "3" * 64, "25A1", "4" * 64
        ),
        f"self-test failed to reject native build-launch {field} substitution",
        "differs from the authenticated readiness controller",
    )
expect_value_error(
    lambda: validate_native_builder_entrypoint_binding(
        native_binding_report, b"substituted native builder bytes"
    ),
    "self-test failed to reject native builder entrypoint substitution",
    "differs from the reviewed signed source closure",
)
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-git-isolation-self-test-') as temporary:
        temporary_root = Path(temporary)
        repository = temporary_root / 'repository'
        repository.mkdir()
        git_home = temporary_root / 'home'
        git_home.mkdir()
        ordinary_environment = {
            'GIT_CONFIG_GLOBAL': '/dev/null',
            'GIT_CONFIG_NOSYSTEM': '1',
            'HOME': str(git_home),
            'LANG': 'C',
            'LC_ALL': 'C',
            'PATH': '/usr/bin:/bin',
        }
        def ordinary_git(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
            return subprocess.run(
                [
                    str(SOURCE_GIT), '-C', str(repository),
                    '-c', 'core.hooksPath=/dev/null',
                    '-c', 'user.name=Kagemusha Self Test',
                    '-c', 'user.email=kagemusha-self-test.invalid',
                    *arguments,
                ],
                cwd=Path('/'),
                env=ordinary_environment,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=check,
                close_fds=True,
            )
        ordinary_git('init', '-q', '-b', 'main')
        conflict = repository / 'conflict.txt'
        conflict.write_text('base\n', encoding='utf-8')
        ordinary_git('add', '--', conflict.name)
        ordinary_git('commit', '-qm', 'base')
        ordinary_git('checkout', '-qb', 'other')
        conflict.write_text('other\n', encoding='utf-8')
        ordinary_git('commit', '-qam', 'other')
        ordinary_git('checkout', '-q', 'main')
        conflict.write_text('main\n', encoding='utf-8')
        ordinary_git('commit', '-qam', 'main')
        merge = ordinary_git('merge', '--no-edit', 'other', check=False)
        if merge.returncode == 0:
            raise ValueError('self-test could not construct an unmerged Git index')
        hostile_hook = temporary_root / 'hostile-git-hook'
        hostile_marker = temporary_root / 'hostile-git-hook.executed'
        hostile_hook.write_text('#!/bin/sh\n: > "${0}.executed"\n', encoding='utf-8')
        hostile_hook.chmod(0o700)
        ordinary_git('config', 'core.fsmonitor', str(hostile_hook))
        ordinary_git('config', 'diff.external', str(hostile_hook))
        redirected = temporary_root / 'redirected-worktree'
        ordinary_git('config', 'core.worktree', str(redirected))
        isolated_environment = {
            'GIT_CONFIG_COUNT': '0',
            'GIT_CONFIG_GLOBAL': '/dev/null',
            'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_LITERAL_PATHSPECS': '1',
            'GIT_NO_REPLACE_OBJECTS': '1',
            'GIT_OPTIONAL_LOCKS': '0',
            'GIT_PAGER': 'cat',
            'GIT_TERMINAL_PROMPT': '0',
            'HOME': '/var/empty',
            'LANG': 'C',
            'LC_ALL': 'C',
            'PATH': '/usr/bin:/bin',
        }
        isolated_prefix = [
            str(SOURCE_GIT), '-C', str(repository), f'--work-tree={repository}',
            '-c', 'core.attributesFile=/dev/null', '-c', 'core.excludesFile=/dev/null',
            '-c', 'core.fsmonitor=false', '-c', 'core.hooksPath=/dev/null',
            '-c', 'core.preloadIndex=false', '-c', 'core.untrackedCache=false',
            '-c', 'submodule.recurse=false',
        ]
        configured = subprocess.run(
            [*isolated_prefix, 'config', '--get-all', 'core.worktree'],
            cwd=Path('/'), env=isolated_environment, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, text=True,
            close_fds=True,
        )
        if configured.returncode != 0 or configured.stdout.strip() != str(redirected):
            raise ValueError('self-test did not observe the hostile local core.worktree redirect')
        ordinary_git('config', 'core.worktree', str(repository))
        unmerged = subprocess.run(
            [*isolated_prefix, 'diff', '--cached', '--quiet', '--no-ext-diff',
             '--no-textconv', '--diff-filter=U', '--'],
            cwd=Path('/'), env=isolated_environment, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, close_fds=True,
        )
        if unmerged.returncode != 1:
            raise ValueError('self-test isolated Git did not reject an unmerged index')
        if hostile_marker.exists():
            raise ValueError('self-test isolated Git executed a hostile fsmonitor or diff hook')
except (OSError, subprocess.CalledProcessError, ValueError) as error:
    errors.append(f'Git worktree-isolation self-test failed unexpectedly: {error}')
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-symlink-invocation-self-test-') as temporary:
        invocation = Path(temporary) / 'readiness-symlink'
        invocation.symlink_to(root / READINESS)
        rejected = subprocess.run(['/bin/bash', str(invocation), 'candidate'], cwd=Path('/'), env={'LANG': 'C', 'LC_ALL': 'C',
            'PATH': '/untrusted/bin'}, stdin=subprocess.DEVNULL, check=False, capture_output=True, text=True, close_fds=True)
        if rejected.returncode != 2 or 'rejects missing or symlinked script invocation' not in rejected.stderr:
            errors.append('self-test failed to reject symlinked gate invocation')
except OSError as error:
    errors.append(f'symlink invocation self-test failed unexpectedly: {error}')
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-untrusted-gate-self-test-') as temporary:
        untrusted_checkout = Path(temporary).resolve(strict=True) / 'untrusted-checkout'
        untrusted_ci = untrusted_checkout / 'ci'
        untrusted_ci.mkdir(parents=True)
        untrusted_gate = untrusted_ci / Path(READINESS).name
        untrusted_gate.write_bytes((root / READINESS).read_bytes())
        untrusted_gate.chmod(0o700)
        untrusted_ci.chmod(0o700)
        untrusted_checkout.chmod(0o770)
        rejected = subprocess.run(['/bin/bash', str(untrusted_gate), 'promotion'], cwd=Path('/'), env={'LANG': 'C', 'LC_ALL': 'C',
            'PATH': '/usr/bin:/bin'}, stdin=subprocess.DEVNULL, check=False, capture_output=True, text=True, close_fds=True)
        if (
            rejected.returncode != 2
            or 'promotion readiness checkout' not in rejected.stderr
            or not any(
                marker in rejected.stderr
                for marker in (
                    'not root-owned',
                    'group/world writable',
                    'has an extended ACL',
                    'has unbound extended attributes',
                )
            )
        ):
            errors.append('self-test failed to reject a user-controlled promotion gate checkout')
except OSError as error:
    errors.append(f'untrusted gate self-test failed unexpectedly: {error}')
try:
    head_result = subprocess.run([str(SOURCE_GIT), '-C', str(root), 'rev-parse', '--verify', 'HEAD'], cwd=Path('/'),
        env=source_git_environment(), stdin=subprocess.DEVNULL, check=False, capture_output=True, text=True, close_fds=True)
    head_commit = head_result.stdout.strip()
    if head_result.returncode != 0 or head_result.stderr or re.fullmatch('[0-9a-f]{40}', head_commit) is None:
        raise ValueError('could not resolve self-test HEAD')
    expect_value_error(lambda: authenticate_reviewed_source_file(
        SOURCE_TREE_SEAL, b'mutated helper bytes', head_commit, MAX_REVIEWED_HELPER_BYTES),
        'self-test failed to reject a source-helper byte mutation', 'differs from the source closure')
except (OSError, ValueError) as error:
    errors.append(f'source-helper authentication self-test failed unexpectedly: {error}')
system_git_descriptor = -1
try:
    (system_git_descriptor, system_git_fingerprint) = pin_regular_metadata(SOURCE_GIT, 'self-test source-authentication Git',
        require_single_link=False)
    require_production_root_custody(system_git_descriptor, 'self-test source-authentication Git')
    if not system_git_fingerprint[3] & 0o111:
        raise ValueError('fixed source-authentication Git is not executable')
    revalidate_pinned_metadata(SOURCE_GIT, system_git_descriptor, system_git_fingerprint, 'self-test source-authentication Git')
except (OSError, ValueError) as error:
    errors.append(f'fixed Git custody self-test failed unexpectedly: {error}')
finally:
    if system_git_descriptor >= 0:
        os.close(system_git_descriptor)
# Validate closure-bound source trust and its exact byte ceilings.
prior_runtime_root = trusted_python_runtime_root
prior_runtime_sha256 = trusted_python_sha256
prior_runtime_tree_sha256 = trusted_python_runtime_tree_sha256
try:
    source_commit = '1' * 40
    source_tree_sha256 = '2' * 64
    closure_value: dict[str, object] = {'schema': 'iroha.reviewed-source-closure.v1', 'base_commit': source_commit,
        'source_commit': source_commit, 'source_repo_dirty': False, 'source_tree_sha256': source_tree_sha256,
        'tracked_binary_diff_sha256': '3' * 64, 'untracked_file_count': 0, 'untracked_path_mode_blob_oid_manifest': [],
        'untracked_path_mode_blob_oid_manifest_sha256': '4' * 64, 'tracked_cargo_lock_size_bytes': 1,
        'tracked_cargo_lock_sha256': '5' * 64, 'combined_source_fingerprint_sha256': '6' * 64}
    closure_bytes = (json.dumps(closure_value, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')
    closure_sha256 = hashlib.sha256(closure_bytes).hexdigest()
    allowed_sha256 = '7' * 64
    revocation_sha256 = '8' * 64
    trusted_python_runtime_root = Path(
        '/private/var/db/iroha-kagemusha-python-runtime-v1/self-test'
    )
    trusted_python_sha256 = 'a' * 64
    trusted_python_runtime_tree_sha256 = 'b' * 64
    tree_identity = {'bytes': 1, 'files': 1, 'records': 1, 'sha256': 'c' * 64}
    build_inputs = {
        'cargo_home': {'roots': ['git', 'registry'], 'tree': tree_identity},
        'cargo_toolchain': {'cargo_relative_path': 'bin/cargo', 'tree': tree_identity},
        'developer_dir': {
            'path': '/private/var/db/kagemusha/Xcode/Developer',
            'tree': tree_identity,
        },
        'host_tools': [],
        'platform': 'darwin',
        'python_runtime': {
            'interpreter_path': str(trusted_python_runtime_root / 'bin/python3'),
            'interpreter_sha256': trusted_python_sha256,
            'root': str(trusted_python_runtime_root),
            'tree_sha256': trusted_python_runtime_tree_sha256,
        },
        'runtime_identity': {},
        'rust_toolchain': {'rustc_relative_path': 'bin/rustc', 'tree': tree_identity},
        'sandbox': {},
        'schema': 'iroha.kagemusha.build_input_closure.v1',
        'sdkroot': {
            'path': (
                '/private/var/db/kagemusha/Xcode/Developer/Platforms/'
                'MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk'
            ),
            'tree': tree_identity,
        },
    }
    build_inputs_bytes = (
        json.dumps(build_inputs, sort_keys=True, separators=(',', ':')) + '\n'
    ).encode('ascii')
    build_inputs_sha256 = hashlib.sha256(build_inputs_bytes).hexdigest()
    cargo_sha256 = '4' * 64
    rustc_sha256 = '5' * 64
    raw_graph_sha256 = '1' * 64
    source_stderr_sha256 = hashlib.sha256(b'').hexdigest()
    capture_receipt = {
        'build_inputs_sha256': build_inputs_sha256,
        'cargo_binary_sha256': cargo_sha256,
        'exit_status': 0,
        'raw_stdout_sha256': raw_graph_sha256,
        'raw_stdout_size_bytes': 1,
        'rustc_binary_sha256': rustc_sha256,
        'schema': 'iroha.kagemusha.cargo_unit_graph_capture_receipt.v1',
        'source_commit': source_commit,
        'source_tree_sha256': source_tree_sha256,
        'stderr_sha256': source_stderr_sha256,
        'stderr_size_bytes': 0,
    }
    projection = {'build_script_observed': {}, 'outer_policy': {
        'build_inputs_hex': build_inputs_bytes.hex(),
        'build_inputs_sha256': build_inputs_sha256,
        'cargo': {'binary': 'kagemusha_recursive_spend_v4_bundle', 'explicit_features': [], 'package': 'iroha_core',
        'profile': 'release', 'semantic_argv': [], 'target': 'aarch64-apple-darwin', 'unit_graph': {
        'capture_receipt': capture_receipt, 'custom_build_packages': 0,
        'custom_build_units': 0, 'iroha_core_units': 1,
        'normalization': SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION, 'packages': 1,
        'raw_sha256': raw_graph_sha256, 'raw_size_bytes': 1,
        'sha256': '2' * 64, 'size_bytes': 1, 'units': 1}},
        'execution_policy_sha256': '3' * 64, 'schema': 'iroha.kagemusha.cprime_source_seal_outer_policy.v1',
        'toolchain': {'cargo': {'binary_sha256': cargo_sha256, 'binary_size_bytes': 1},
        'rustc': {'binary_sha256': rustc_sha256, 'binary_size_bytes': 1}}}, 'reviewed_source_closure_hex': closure_bytes.hex(),
        'reviewed_source_closure_sha256': closure_sha256, 'schema': 'iroha.kagemusha.authenticated_source_seal_projection.v1',
        'source_authority': {'commit': source_commit, 'commit_object_sha256': '9' * 64, 'commit_object_size': 1, 'committer_epoch': 1,
        'git_tree': 'a' * 40, 'ordered_parents': ['b' * 40], 'parent_commit': 'b' * 40, 'parent_tree': 'c' * 40,
        'signature': {'allowed_signers_sha256': allowed_sha256, 'mechanism': 'git-commit-ssh-signature-v1',
        'principal': 'reviewer@example.test', 'public_key_sha256': 'd' * 64, 'revocation_sha256': revocation_sha256,
        'signature_namespace': 'git'}}, 'source_commit': source_commit, 'source_date_epoch': 1, 'source_repo_dirty': False,
        'source_tree_sha256': source_tree_sha256}
    projection_bytes = (json.dumps(projection, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')
    validate_source_trust_projection(projection_bytes, closure_bytes, closure_sha256, source_commit, allowed_sha256, revocation_sha256)
    expect_value_error(lambda: validate_source_trust_projection(
        projection_bytes, closure_bytes, closure_sha256, source_commit, 'e' * 64, revocation_sha256),
        'self-test failed to bind SSH trust-policy digests to the reviewed closure', 'trust-policy digests')
except (UnicodeError, ValueError, json.JSONDecodeError) as error:
    errors.append(f'source trust-projection self-test failed unexpectedly: {error}')
finally:
    trusted_python_runtime_root = prior_runtime_root
    trusted_python_sha256 = prior_runtime_sha256
    trusted_python_runtime_tree_sha256 = prior_runtime_tree_sha256
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-source-projection-bound-self-test-') as temporary:
        boundary_root = Path(temporary)
        exact = boundary_root / 'exact-projection'
        exact.write_bytes(b'x' * MAX_SOURCE_SEAL_PROJECTION_BYTES)
        (descriptor, fingerprint) = pin_regular_metadata(exact, 'self-test exact source projection')
        try:
            payload = read_pinned_descriptor(descriptor, fingerprint, MAX_SOURCE_SEAL_PROJECTION_BYTES,
                'self-test exact source projection')
        finally:
            os.close(descriptor)
        if len(payload) != MAX_SOURCE_SEAL_PROJECTION_BYTES:
            errors.append('self-test failed at the exact source-projection byte bound')
        oversized = boundary_root / 'oversized-projection'
        oversized.write_bytes(b'x' * (MAX_SOURCE_SEAL_PROJECTION_BYTES + 1))
        (descriptor, fingerprint) = pin_regular_metadata(oversized, 'self-test oversized source projection')
        try:
            expect_value_error(lambda: read_pinned_descriptor(descriptor, fingerprint, MAX_SOURCE_SEAL_PROJECTION_BYTES,
                'self-test oversized source projection'), 'self-test accepted an oversized source projection',
                '16384-byte size limit')
        finally:
            os.close(descriptor)
except (OSError, ValueError) as error:
    errors.append(f'source-projection bound self-test failed unexpectedly: {error}')
if (
    'recursive-step-two-qualification-v4.norito' not in FINAL_METADATA
    or 'internal-validation-receipt-v1.norito' not in FINAL_METADATA
    or MAX_RELEASE_INVENTORY_ENTRIES != 18
    or MAX_INTERNAL_VALIDATION_RECEIPT_BYTES != 1_048_576
    or MAX_QUALIFICATION_RECEIPT_BYTES != 802_816
):
    errors.append('self-test failed to pin the final internal and recursive receipt inventory')
for invalid_catalog_path in (Path('relative/catalog'), Path('/trusted/staging/../catalog')):
    expect_value_error(lambda: absolute_directory_chain(invalid_catalog_path),
        'self-test failed to reject a noncanonical catalog path chain')
aggregate_boundary = 0
try:
    for release_bytes in (MAX_CATALOG_AGGREGATE_BYTES // 2, MAX_CATALOG_AGGREGATE_BYTES // 2):
        aggregate_boundary = checked_catalog_aggregate_total(aggregate_boundary, release_bytes)
    checked_catalog_aggregate_total(aggregate_boundary, 1)
except ValueError:
    if aggregate_boundary != MAX_CATALOG_AGGREGATE_BYTES:
        errors.append('self-test failed at the whole-catalog byte boundary')
else:
    errors.append('self-test failed to reject an oversized multi-release catalog')
try:
    with (
        tempfile.TemporaryDirectory(
            prefix='kagemusha-self-test-staging-parent-'
        ) as staging_text,
        tempfile.TemporaryDirectory(
            prefix='kagemusha-self-test-attacker-tmpdir-'
        ) as attacker_tmpdir,
    ):
        staging_parent = Path(staging_text).resolve(strict=True)
        prior_tmpdir = os.environ.get('TMPDIR')
        os.environ['TMPDIR'] = attacker_tmpdir
        try:
            (snapshot, snapshot_path) = snapshot_private_bytes(b'authenticated physical-iOS evidence bytes', 'evidence.json',
                'self-test evidence', staging_parent)
            try:
                snapshot_metadata = snapshot_path.lstat()
                if (
                    snapshot_path.read_bytes()
                    != b'authenticated physical-iOS evidence bytes'
                    or snapshot_path.parent.parent != staging_parent
                    or Path(attacker_tmpdir).resolve(strict=True)
                    in snapshot_path.parents
                    or stat.S_IMODE(snapshot_metadata.st_mode) != 0o600
                    or stat.S_IMODE(snapshot_path.parent.lstat().st_mode) != 0o700
                ):
                    errors.append('self-test failed to create an exact fixed-parent evidence snapshot')
            finally:
                snapshot.cleanup()
            (empty_snapshot, empty_snapshot_path) = snapshot_private_bytes(b'', 'revocation', 'self-test empty SSH revocation policy',
                staging_parent, allow_empty=True)
            try:
                if (
                    empty_snapshot_path.stat().st_size != 0
                    or hashlib.sha256(empty_snapshot_path.read_bytes()).hexdigest()
                    != hashlib.sha256(b'').hexdigest()
                ):
                    errors.append('self-test failed to preserve an explicitly pinned empty revocation policy')
            finally:
                empty_snapshot.cleanup()
            (allowed_snapshot, allowed_path) = snapshot_private_bytes(b'reviewer@example.test ssh-ed25519 AAAA\n', 'allowed-signers',
                'self-test SSH allowed-signers policy', staging_parent)
            (revocation_snapshot, revocation_path) = snapshot_private_bytes(b'', 'revocation', 'self-test SSH revocation policy',
                staging_parent, allow_empty=True)
            config_snapshot: tempfile.TemporaryDirectory[str] | None = None
            try:
                config_payload = isolated_source_trust_git_config(allowed_path, revocation_path)
                (config_snapshot, config_path) = snapshot_private_bytes(config_payload, '.gitconfig',
                    'self-test isolated source SSH Git config', staging_parent)
                config_environment = source_git_environment()
                config_environment.pop('GIT_CONFIG_GLOBAL', None)
                config_environment['HOME'] = str(config_path.parent)
                for (key, expected) in (('gpg.ssh.allowedSignersFile', allowed_path), ('gpg.ssh.revocationFile', revocation_path)):
                    configured = subprocess.run([str(SOURCE_GIT), 'config', '--global', '--path', '--get', key], cwd=Path('/'),
                        env=config_environment, stdin=subprocess.DEVNULL, check=False, capture_output=True, text=True, close_fds=True)
                    if configured.returncode != 0 or configured.stderr or configured.stdout != f'{expected}\n':
                        errors.append('self-test failed to expose only the snapshotted source SSH trust policy')
            finally:
                if config_snapshot is not None:
                    config_snapshot.cleanup()
                revocation_snapshot.cleanup()
                allowed_snapshot.cleanup()
        finally:
            if prior_tmpdir is None:
                os.environ.pop('TMPDIR', None)
            else:
                os.environ['TMPDIR'] = prior_tmpdir
except (OSError, ValueError) as error:
    errors.append(f'private evidence snapshot self-test failed unexpectedly: {error}')
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-custody-self-test-') as temporary:
        writable = Path(temporary) / 'writable'
        writable.write_bytes(b'untrusted')
        writable.chmod(402)
        (descriptor, _) = pin_regular_metadata(writable, 'self-test writable file')
        try:
            expect_value_error(lambda: require_production_root_custody(descriptor, 'self-test writable file'),
                'self-test failed to reject a caller-writable production input')
        finally:
            os.close(descriptor)
except (OSError, ValueError) as error:
    errors.append(f'production custody self-test failed unexpectedly: {error}')
real_fstat = os.fstat
real_require_no_macos_extended_acl = require_no_macos_extended_acl
try:
    root_custodied_metadata = types.SimpleNamespace(
        st_uid=PRODUCTION_TRUSTED_UID,
        st_mode=stat.S_IFDIR | 0o755,
    )
    os.fstat = lambda _descriptor: root_custodied_metadata
    require_no_macos_extended_acl = lambda _descriptor, _label: None
    try:
        require_production_root_custody(-1, 'self-test root-custodied ancestor')
    except (OSError, ValueError) as error:
        errors.append(
            f'self-test rejected a legitimate root-custodied production ancestor: {error}'
        )
    non_root_metadata = types.SimpleNamespace(
        st_uid=PRODUCTION_TRUSTED_UID + 1,
        st_mode=stat.S_IFDIR | 0o755,
    )
    os.fstat = lambda _descriptor: non_root_metadata
    expect_value_error(
        lambda: require_production_root_custody(-1, 'self-test non-root ancestor'),
        'self-test failed to reject a non-root production ancestor',
        'must be owned by root',
    )
    writable_metadata = types.SimpleNamespace(
        st_uid=PRODUCTION_TRUSTED_UID,
        st_mode=stat.S_IFDIR | 0o775,
    )
    os.fstat = lambda _descriptor: writable_metadata
    expect_value_error(
        lambda: require_production_root_custody(-1, 'self-test writable ancestor'),
        'self-test failed to reject a group-writable production ancestor',
        'must not be group/world writable',
    )
finally:
    os.fstat = real_fstat
    require_no_macos_extended_acl = real_require_no_macos_extended_acl
if sys.platform == 'darwin':
    try:
        acl_free_path = Path('/usr/bin/true')
        (descriptor, _) = pin_regular_metadata(
            acl_free_path, 'self-test ACL-free macOS input', require_single_link=False
        )
        try:
            require_no_macos_extended_acl(descriptor, 'self-test ACL-free macOS input')
        finally:
            os.close(descriptor)
        with tempfile.TemporaryDirectory(prefix='kagemusha-acl-custody-self-test-') as temporary:
            acl_path = Path(temporary) / 'acl-input'
            acl_path.write_bytes(b'root-custody-acl-test')
            acl_path.chmod(384)
            added_acl = subprocess.run(['/bin/chmod', '+a', 'everyone allow read', str(acl_path)], cwd=Path('/'), env={'LANG': 'C',
                'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'}, stdin=subprocess.DEVNULL, check=False, capture_output=True, close_fds=True)
            if added_acl.returncode != 0:
                errors.append('self-test could not install a macOS extended ACL')
            else:
                (descriptor, _) = pin_regular_metadata(acl_path, 'self-test extended-ACL macOS input')
                try:
                    try:
                        require_no_macos_extended_acl(descriptor, 'self-test extended-ACL macOS input')
                    except ValueError as error:
                        if 'must not have an extended ACL' not in str(error):
                            errors.append('self-test produced a nondeterministic macOS ACL rejection')
                    else:
                        errors.append('self-test failed to reject a macOS extended ACL')
                finally:
                    os.close(descriptor)
    except (OSError, ValueError) as error:
        errors.append(f'macOS ACL custody self-test failed unexpectedly: {error}')
# Build one canonical authenticated report fixture for all report mutations.
report_manifest_artifacts = [{'file_name': name, 'size_bytes': index + 1, 'sha256': f'{index + 1:x}' * 64,
    'payload_size_bytes': index + 2, 'payload_sha256': f'{index + 2:x}' * 64} for (index, name) in enumerate(ARTIFACTS)]
report_manifest = {'generation': 'self-test', 'generation_memory_limit_bytes': 1,
    'generation_memory_enforcement_profile': 'self-test-profile', 'network_id': 'self-test-network', 'asset': 'self-test-asset',
    'asset_scale': 2, 'authenticated_source_seal_projection_sha256': 'b' * 64, 'reviewed_cargo_binary_sha256': 'c' * 64,
    'reviewed_rustc_binary_sha256': 'd' * 64, 'qualified_candidate_sha256': '7' * 64,
    'generator_binary_sha256': 'e' * 64, 'sealed_candidate_build_report_sha256': 'f' * 64,
    'profiles': [{'artifacts': report_manifest_artifacts[:4]}, {'artifacts': report_manifest_artifacts[4:]}],
    'topup_finality_roster_artifact': {'file_name': 'topup-finality-roster-v4.norito', 'size_bytes': 17, 'sha256': 'a' * 64}}
report_artifacts = [{'purpose': purpose, 'file_name': artifact['file_name'], 'size_bytes': artifact['size_bytes'],
    'sha256': artifact['sha256'], 'payload_size_bytes': artifact['payload_size_bytes'],
    'payload_sha256': artifact['payload_sha256']} for (purpose, artifact) in zip(REPORT_ARTIFACT_PURPOSES, report_manifest_artifacts,
    strict=True)]
report_artifacts.append({'purpose': 'topup_finality_roster', 'file_name': 'topup-finality-roster-v4.norito', 'size_bytes': 17,
    'sha256': 'a' * 64, 'payload_size_bytes': None, 'payload_sha256': None})
verifier_report = {'status': 'verified', 'envelope_sha256': '1' * 64, 'manifest_body_sha256': '2' * 64, 'candidate_sha256': '3' * 64,
    'qualification_receipt_sha256': '4' * 64, 'qualified_candidate_sha256': '7' * 64,
    'internal_validation_receipt_sha256': '8' * 64,
    'authenticated_source_seal_projection_sha256': 'b' * 64, 'reviewed_cargo_binary_sha256': 'c' * 64,
    'reviewed_rustc_binary_sha256': 'd' * 64, 'promotion_record_sha256': '6' * 64, 'release_policy_sha256': '5' * 64,
    'generator_binary_sha256': 'e' * 64, 'sealed_candidate_build_report_sha256': 'f' * 64,
    'generation': 'self-test', 'generation_memory_limit_bytes': 1, 'generation_memory_enforcement_profile': 'self-test-profile',
    'network_id': 'self-test-network', 'asset_definition_id': 'self-test-asset', 'asset_scale': 2, 'bridge_abi_version': 22,
    'recursive_step_verifier_commitment': '9' * 64, 'artifacts': report_artifacts}
try:
    validate_kagami_verification_report(verifier_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
        policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64,
        internal_validation_receipt_sha256='8' * 64, ios_candidate_sha256='3' * 64)
    invalid_report = dict(verifier_report)
    invalid_report['status'] = 'unverified'
    validate_kagami_verification_report(invalid_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
        policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64,
        internal_validation_receipt_sha256='8' * 64, ios_candidate_sha256='3' * 64)
except ValueError as error:
    if 'did not report one verified' not in str(error):
        errors.append(f'authenticated report self-test failed unexpectedly: {error}')
else:
    errors.append('self-test failed to reject an unverified Kagami report')
for field in (
    'authenticated_source_seal_projection_sha256',
    'reviewed_cargo_binary_sha256',
    'reviewed_rustc_binary_sha256',
    'generator_binary_sha256',
    'sealed_candidate_build_report_sha256',
):
    mismatched_report = dict(verifier_report)
    mismatched_report[field] = '0' * 63 + '1'
    try:
        validate_kagami_verification_report(mismatched_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
            policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64,
            internal_validation_receipt_sha256='8' * 64, ios_candidate_sha256='3' * 64)
    except ValueError as error:
        if 'differs from the manifest' not in str(error):
            errors.append(f'authenticated report {field} self-test failed unexpectedly: {error}')
    else:
        errors.append(f'self-test failed to reject a mismatched Kagami report {field}')
try:
    validate_kagami_verification_report(verifier_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
        policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64,
        internal_validation_receipt_sha256='0' * 64, ios_candidate_sha256='3' * 64)
except ValueError as error:
    if 'different internal-validation receipt' not in str(error):
        errors.append(f'internal-validation report binding self-test failed unexpectedly: {error}')
else:
    errors.append('self-test failed to reject a mismatched internal-validation receipt report')
try:
    with tempfile.TemporaryDirectory(prefix='kagemusha-catalog-pin-self-test-') as temporary:
        catalog_root = Path(temporary).resolve(strict=True)
        release = catalog_root / 'release'
        replacement = catalog_root / 'replacement'
        release.mkdir()
        replacement.mkdir()
        release_file = release / 'artifact'
        release_file.write_bytes(b'pinned release artifact')
        (replacement / 'artifact').write_bytes(b'substituted release artifact')
        pins: list[tuple[Path, int, tuple[int, ...], str]] = []
        try:
            for component in absolute_directory_chain(catalog_root):
                label = f'self-test catalog path component {component}'
                (descriptor, fingerprint) = pin_directory_metadata(component, label)
                pins.append((component, descriptor, fingerprint, label))
            release_label = 'self-test release directory'
            (release_descriptor, release_fingerprint) = pin_directory_metadata(release, release_label)
            pins.append((release, release_descriptor, release_fingerprint, release_label))
            file_label = 'self-test release file'
            (file_descriptor, file_fingerprint) = pin_regular_metadata(release_file, file_label)
            pins.append((release_file, file_descriptor, file_fingerprint, file_label))
            for (path, descriptor, fingerprint, label) in pins:
                revalidate_pinned_metadata(path, descriptor, fingerprint, label)
            displaced = catalog_root / 'displaced'
            release.rename(displaced)
            replacement.rename(release)
            try:
                revalidate_pinned_metadata(release, release_descriptor, release_fingerprint, release_label)
            except (OSError, ValueError):
                pass
            else:
                errors.append('self-test failed to reject a substituted release directory')
        finally:
            for (_, descriptor, _, _) in reversed(pins):
                os.close(descriptor)
except (OSError, ValueError) as error:
    errors.append(f'catalog pin self-test failed unexpectedly: {error}')
# Mutate exact reviewed snippets and require the matching static diagnostic.
baseline = {
    READINESS: read(READINESS, []),
    READINESS_SOURCE_CONTRACT: read(READINESS_SOURCE_CONTRACT, []),
    READINESS_SOURCE_SUPPORT: read(READINESS_SOURCE_SUPPORT, []),
    READINESS_RECURSION_SOURCE_CONTRACT: read(
        READINESS_RECURSION_SOURCE_CONTRACT, []
    ),
    READINESS_LIFECYCLE_SOURCE_CONTRACT: read(
        READINESS_LIFECYCLE_SOURCE_CONTRACT, []
    ),
    READINESS_SELF_TEST: read(READINESS_SELF_TEST, []),
    MODEL: read_reviewed_model([], {}),
    MODEL_COMPONENT: read(MODEL_COMPONENT, []),
    MODEL_VERIFIER_COMPONENT: read(MODEL_VERIFIER_COMPONENT, []),
    MODEL_PROMOTION_RECEIPT_COMPONENT: read(MODEL_PROMOTION_RECEIPT_COMPONENT, []),
    MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT: read(
        MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT, []
    ),
    MODEL_CANARY_EVIDENCE_COMPONENT: read(MODEL_CANARY_EVIDENCE_COMPONENT, []),
    MODEL_CANARY_LIVENESS_COMPONENT: read(MODEL_CANARY_LIVENESS_COMPONENT, []),
    MODEL_ISI_OFFLINE: read(MODEL_ISI_OFFLINE, []),
    MODEL_ISI_MOD: read(MODEL_ISI_MOD, []),
    SCHEMA_GOLDEN: read(SCHEMA_GOLDEN, []),
    PRIVACY: read(PRIVACY, []),
    PRIVACY_PROTOCOL: read(PRIVACY_PROTOCOL, []),
    CATALOG: read_reviewed_catalog([], {}),
    CATALOG_COMPONENT: read(CATALOG_COMPONENT, []),
    CATALOG_VALIDATOR_QUALIFICATION_COMPONENT: read(
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT, []
    ),
    CORE: read_reviewed_core([], {}),
    CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT: read(
        CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, []
    ),
    CORE_KAGEMUSHA_ACTIVATION_COMPONENT: read(
        CORE_KAGEMUSHA_ACTIVATION_COMPONENT, []
    ),
    CORE_KAGEMUSHA_CANARY_COMPONENT: read(CORE_KAGEMUSHA_CANARY_COMPONENT, []),
    CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT: read(CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT, []),
    CORE_TX: read(CORE_TX, []),
    CORE_STATE: read(CORE_STATE, []),
    CORE_STATE_TESTS: read(CORE_STATE_TESTS, []),
    CORE_COMMITTED_TX_CONTEXT: read(
        CORE_COMMITTED_TX_CONTEXT, []
    ),
    CORE_BLOCK: read(CORE_BLOCK, []),
    CORE_EXECUTOR: read(CORE_EXECUTOR, []),
    CORE_ISI_MOD: read(CORE_ISI_MOD, []),
    CORE_ISI_TESTS: read(CORE_ISI_TESTS, []),
    CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS: read(
        CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS, []
    ),
    CORE_AUTONOMOUS_MERGE_TESTS: read(CORE_AUTONOMOUS_MERGE_TESTS, []),
    CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS: read(
        CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS, []
    ),
    RECURSION_ADAPTER: read(RECURSION_ADAPTER, []),
    CONFIG: read(CONFIG, []),
    NODE: read_reviewed_node([], {}),
    NODE_VALIDATOR_QUALIFICATION_COMPONENT: read(
        NODE_VALIDATOR_QUALIFICATION_COMPONENT, []
    ),
    NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT: read(
        NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT, []
    ),
    NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT: read(
        NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT, []
    ),
    NODE_ROOT_OWNED_PUBLICATION_COMPONENT: read(
        NODE_ROOT_OWNED_PUBLICATION_COMPONENT, []
    ),
    KAGAMI: read(KAGAMI, []),
    AUTHENTICATED_TOOL_CONTROLLER: read(AUTHENTICATED_TOOL_CONTROLLER, []),
    KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT: read(
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT, []
    ),
    KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT: read(
        KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT, []
    ),
    CLIENT: read(CLIENT, []),
    HTTP_DEFAULT: read(HTTP_DEFAULT, []),
    CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT: read(
        CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT, []
    ),
    OFFLINE_CLI: read(OFFLINE_CLI, []),
    CLI_MAIN_SHARED: read(CLI_MAIN_SHARED, []),
    KAGEMUSHA_LIFECYCLE_COMPONENT: read(KAGEMUSHA_LIFECYCLE_COMPONENT, []),
    KAGEMUSHA_ROLLOUT_COMPONENT: read(KAGEMUSHA_ROLLOUT_COMPONENT, []),
    KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT: read(
        KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, []
    ),
    BUNDLE: read(BUNDLE, []),
    BUNDLE_SOURCE_SEAL_INPUTS: read(BUNDLE_SOURCE_SEAL_INPUTS, []),
    WORKFLOW: read(WORKFLOW, []),
    PROMOTION_WORKFLOW: read(PROMOTION_WORKFLOW, []),
    PRODUCTION_IOS_EVIDENCE_MODULE: read(PRODUCTION_IOS_EVIDENCE_MODULE, []),
    IOS_EVIDENCE_MODULE: read(IOS_EVIDENCE_MODULE, []),
}
baseline.update({path: read(path, []) for path in DEVICE_ATTESTATION_SOURCE_PATHS})
baseline.update({path: read(path, []) for path in LIFECYCLE_SOURCE_PATHS})
def expect_static_rejection(overrides: dict[str, str], failure: str, *needles: str) -> None:
    mutation_errors = static_errors(overrides)
    if mutation_errors and (not needles or any((needle in error for error in mutation_errors for needle in needles))):
        return
    errors.append(failure)
def expect_static_mutation(relative: str, before: str, after: str, failure: str,
        needles: tuple[str, ...]=(), count: int=1) -> None:
    expect_static_rejection({relative: baseline[relative].replace(before, after, count)},
        f'self-test failed to {failure}', *needles)
conflicted_model = baseline[MODEL] + '\n<<<<<<< HEAD\nreviewed-side\n=======\nincoming-side\n>>>>>>> origin/reviewed\n'
expect_static_rejection({MODEL: conflicted_model}, 'self-test failed to reject a reviewed merge conflict',
    'unresolved Git merge conflict marker')
conflicted_build_inputs = baseline[BUNDLE_SOURCE_SEAL_INPUTS] + '\n<<<<<<< HEAD\nreviewed-side\n=======\nincoming-side\n>>>>>>> origin/reviewed\n'
expect_static_rejection({BUNDLE_SOURCE_SEAL_INPUTS: conflicted_build_inputs},
    'self-test failed to authenticate the reviewed bundle build-input component',
    'unresolved Git merge conflict marker')
ATTESTATION_STATIC_MUTATIONS = (
    (CORE, CORE_ATTESTATION_CERTIFICATE_VALIDATION_INCLUDE,
        '// attestation certificate validator detached',
        'reject a detached attestation certificate validator',
        (CORE_ATTESTATION_CERTIFICATE_VALIDATION_INCLUDE,)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    validate_x509_certificate_signature_algorithm(&certificate)?;',
        '    // signature algorithm profile bypassed',
        'reject an unauthenticated certificate signature algorithm',
        ('bounded DER parsing with strict signature algorithm validation',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '            validate_x509_rsa_pss_signature_algorithm(signature_algorithm)?;',
        '            // RSA-PSS parameter profile bypassed',
        'reject an unauthenticated RSA-PSS parameter profile',
        ('strict RSA-PSS verifier parameter profile',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '        if !seen_extension_oids.insert(extension_oid.clone()) {',
        '        if false {', 'reject duplicate certificate extension OIDs',
        ('strict certificate extension processing',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '        if extension.critical', '        if false',
        'reject an unsupported critical certificate extension',
        ('strict certificate extension processing',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '        if let Err(error) = verify_x509_certificate_signature(tail, &root) {',
        '        if false {', 'reject a same-subject root signature bypass',
        ('order-independent exact-pinned trust-anchor validation',), -1),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    for (index, certificate_der) in certificate_chain.iter().enumerate().rev() {',
        '    for (index, certificate_der) in certificate_chain.iter().enumerate() {',
        'reject leaf-first Android attestation extension selection',
        ('root-nearest Android KeyMint extension selection',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    if certificate_index != 0 {', '    if false {',
        'reject a non-leaf Android attestation extension',
        ('root-nearest Android KeyMint extension selection',)),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    #[test]\n    fn android_keymint_uses_only_a_directly_attested_leaf_extension()',
        '    #[test]\n    #[ignore]\n    fn android_keymint_uses_only_a_directly_attested_leaf_extension()',
        'reject an ignored attacker-extended Android chain regression', ('#[ignore]',)),
    (CORE_ATTESTATION_CERTIFICATE_DER_PROFILE_COMPONENT,
        'inner_algorithm_tag.first_byte != 0x30 || inner_algorithm_raw != outer_algorithm_raw,',
        'inner_algorithm_tag.first_byte != 0x30 || false,',
        'reject different raw inner and outer certificate signature algorithms', D_STRICT_X509),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    let strict_tbs_der = strict_x509_tbs_certificate_der(certificate_der)?;',
        '    let strict_tbs_der = certificate_der;',
        'reject bypassing the strict raw certificate parser', D_STRICT_X509),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '        let certificate_tbs_sha256 = sha256_bytes(certificate.tbs_certificate.as_ref());',
        '        let certificate_tbs_sha256 = sha256_bytes(certificate_der);',
        'reject full-certificate revocation identity', D_STRICT_X509, -1),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    for certificate_der in certificate_chain {',
        '    for certificate_der in &certificate_chain[..1] {',
        'reject Android status checks that skip presented certificates', D_ANDROID_CHAIN, -1),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '    for certificate in parsed_chain.iter().skip(1) {',
        '    for certificate in parsed_chain.iter().skip(2) {',
        'reject an incomplete Android non-target time profile', D_ANDROID_CHAIN),
    (CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
        '&& self.subsecond_millis != 0', '&& self.subsecond_millis == 0',
        'reject second-rounded X.509 expiration checks', D_ANDROID_CHAIN),
    (CORE,
        '                        .checked_mul(128)',
        '                        .wrapping_mul(128)',
        'reject overflowing Android DER high tags', ('.checked_mul(128)',)),
    (CORE, ANDROID_AUTH_INCLUDE, '// Android authorization validator detached',
        'reject a detached Android authorization validator', (ANDROID_AUTH,)),
    (CORE_ISI_TESTS,
        'fn kagemusha_v4_activation_validates_identity_and_policy_before_state_mutation()',
        'fn detached_activation_order_regression()',
        'reject loss of activation anti-rollback ordering coverage',
        ('active Android authorization and activation regressions',)),
    (ANDROID_AUTH,
        '        if !seen_tags.insert(tag.number) {', '        if false {',
        'reject duplicate Android authorization tags',
        ('strict Android application, boot, and authorization identity',)),
    (CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
        '        (Some(_), None) => {',
        '        (Some(_), None) => return Ok(()), // rollback accepted',
        'reject removal of Android status anti-rollback state', D_ANDROID_STATUS),
    (CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
        '    if candidate.response_date_ms <= previous.response_date_ms',
        '    if false', 'reject a stale Android status transition', D_ANDROID_STATUS),
    (CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
        '    if ios_roots != expected_ios_roots {',
        '    if false {', 'reject a substituted production Apple root', D_ANDROID_STATUS),
    (CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
        '    if android_roots != expected_android_roots {',
        '    if false {', 'reject substituted production Android roots', D_ANDROID_STATUS),
    (CORE_ATTESTATION_POLICY_VALIDATION_COMPONENT,
        '        .any(|root| !trusted_root_is_active(root, block_unix_timestamp_ms))',
        '        .any(|_| false)',
        'reject a governance-inactive exact production root', D_ANDROID_STATUS),
    (CORE_ISI_TESTS, POLICY_TESTS_INCLUDE,
        '// release attestation-policy regressions detached',
        'reject a detached release attestation-policy regression component',
        (POLICY_TESTS,)),
    (POLICY_TESTS,
        '.expect_err("activation must pin the exact Apple App Attest root");',
        '.expect("substituted Apple root accepted");',
        'reject loss of the exact Apple/all-root release regression',
        ('active exact built-in release-root policy regressions',)),
    (CORE_KAGEMUSHA_ACTIVATION_COMPONENT,
        '        validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?;',
        '        // governed Android status transition bypassed',
        'reject atomic activation without status anti-rollback',
        ('validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?;',)),
    (CORE_DEVICE_ATTESTATION_REGISTRATION_VALIDATION_COMPONENT,
        '    validate_offline_attestation_policy_status_coverage(\n        &lifetime_policy,\n        registration.expires_at_ms,\n    )?;',
        '    // Android status lifetime coverage bypassed',
        'reject registration beyond Android status freshness',
        ('Android status coverage through registration expiry',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '            .checked_add(1)',
        '            .checked_add(0)',
        'reject qualification beyond Android status freshness',
        ('Android status coverage through validator qualification expiry',)),
    (CATALOG, QUAL_TESTS_INCLUDE,
        '// qualification boundary regressions detached',
        'reject a detached qualification boundary regression component',
        (QUAL_TESTS,)),
    (QUAL_TESTS,
        '        expires - 1,\n    )\n    .expect("status freshness through the inclusive millisecond before expiry is sufficient");',
        '        expires,\n    )\n    .expect("stale status accepted");',
        'reject loss of the qualification status +1ms boundary regression',
        ('active qualification Android-status +1ms freshness boundary regression',)),
    (STATUS_CAPTURE,
        'NON_VALID_STATUSES = frozenset(("REVOKED", "SUSPENDED"))',
        'NON_VALID_STATUSES = frozenset(("REVOKED",))',
        'reject dropping suspended Android certificates',
        ('NON_VALID_STATUSES = frozenset(("REVOKED", "SUSPENDED"))',)),
    (ANDROID_CERT,
        '    for certificate in certificates[1:]:',
        '    for certificate in certificates[2:]:',
        'reject an incomplete physical-lab Android time profile', D_ANDROID_CHAIN),
    (ANDROID_DEVICE_LAB_SLOT,
        'evaluation_time_ms=time.time_ns() // 1_000_000,',
        'evaluation_time_ms=time.time_ns() // 1_000_000_000,',
        'reject second-rounded physical-lab X.509 validation',
        ('evaluation_time_ms=time.time_ns() // 1_000_000,',)),
    (ANDROID_CERT,
        'if len(serial_value) > 20:', 'if len(serial_value) > 21:',
        'reject overlong Android certificate serials',
        ('strict bounded Android certificate DER and serial parsing',)),
    (ANDROID_DEVICE_LAB_SLOT,
        '    _android_x509._validate_android_attestation_certificate_time_profile',
        '    _android_x509._classify_android_attestation_certificate_chain',
        'reject detached Android certificate-profile delegation',
        ('exact Android certificate-profile module delegation',)),
    (ANDROID_CERT_FIX,
        '            elif chain_kind == "unknown":',
        '            elif chain_kind == "rkp":',
        'reject loss of unknown Android certificate-chain fixtures',
        ('bound Factory/RKP Android certificate fixtures',)),
    (ANDROID_CERT_TEST,
        '            self.assertIn("-no_check_time", call.args[0])',
        '            self.assertNotIn("-no_check_time", call.args[0])',
        'reject loss of manual Android certificate-time verification coverage',
        ('self.assertIn("-no_check_time", call.args[0])',)),
    (ANDROID_DEVICE_LAB_SLOT,
        '            if capture_receipt != rebuilt_receipt:', '            if false:',
        'reject a receipt detached from exact status bytes and headers', D_ANDROID_CAPTURE),
    (ANDROID_DEVICE_LAB_RUNNER,
        '  --android-attestation-status-capture-receipt "$AUTHORITY_STATUS_CAPTURE_RECEIPT"',
        '  # governed status receipt forwarding removed',
        'reject a lab run detached from its governed status receipt', D_ANDROID_CAPTURE),
)
static_mutations = (
    *ATTESTATION_STATIC_MUTATIONS,
    (READINESS_LIFECYCLE_SOURCE_CONTRACT,
        'globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_CONTEXT_V1") is not True',
        'False', 'reject a detached lifecycle source-contract provider',
        ('authenticated lifecycle source-provider boundary',), -1),
    (MODEL_LIFECYCLE,
        'require_nonzero(manifest_sha256, "lifecycle.manifest_sha256")?;',
        '// zero manifest lifecycle key accepted',
        'reject a zero manifest-scoped lifecycle key',
        ('manifest-scoped bounded lifecycle model',)),
    (MODEL_KAGEMUSHA_MODEL,
        'pub internal_validation_runner_identity_sha256: [u8; 32]',
        'pub untrusted_internal_validation_runner_identity_sha256: [u8; 32]',
        'reject a release policy without an internal-validation runner trust root',
        ('policy-owned internal-validation runner trust root',)),
    (MODEL_RELEASE_V5,
        '|| self.internal_validation_runner_identity_sha256 == [0; 32]',
        '|| false',
        'reject an all-zero internal-validation runner trust root',
        ('nonzero internal-validation runner trust root',)),
    (MODEL_RELEASE_V5,
        '.is_some_and(|expected| body.validation_runner_identity_sha256 != expected)',
        '.is_some_and(|_| false)',
        'reject an internal-validation receipt detached from the policy runner identity',
        ('receipt runner identity against the policy trust root',)),
    (MODEL_RELEASE_V5,
        'Some(policy.internal_validation_runner_identity_sha256)',
        'None',
        'reject authenticated V4 validation without its runner trust root',
        ('authenticated V4 internal-validation trust-root forwarding',)),
    (MODEL_TAIL_TESTS,
        'fn release_lifecycle_state_enforces_exact_predecessors_and_terminal_phases()',
        'fn unchecked_release_lifecycle_transitions()',
        'reject loss of exact lifecycle predecessor coverage',
        ('active lifecycle transition and retained-policy regressions',)),
    (MODEL_PROMOTION_RECEIPT_TESTS,
        'fn github_promotion_id_derivation_matches_known_vector()',
        'fn unchecked_github_promotion_id_vector()',
        'reject loss of the promotion-identity known vector',
        ('promotion identity and exact reservation-generation regressions',)),
    (MODEL_PROMOTION_RECEIPT_TESTS,
        'fn validator_seals_reject_mixed_exact_reservation_generations()',
        'fn validator_seals_accept_mixed_exact_reservation_generations()',
        'reject loss of exact reservation-generation binding',
        ('promotion identity and exact reservation-generation regressions',)),
    (MODEL_ISI,
        '"iroha.offline.kagemusha.recursive_release.enable.v1"',
        '"iroha.offline.kagemusha.recursive_release.enable.v2"',
        'reject lifecycle enable wire-id drift',
        ('stable canonical lifecycle instruction wires',)),
    (MODEL_ISI,
        '|| self.runtime_effective_config_sha256 == [0; 32]', '|| false',
        'reject a zero activation runtime projection identity',
        ('activation-wire runtime projection identity',)),
    (MODEL_PROMOTION_RECEIPT,
        'if activation.runtime_effective_config_sha256()', 'if unchecked_runtime_digest()',
        'reject an activation digest detached from the validator projection',
        ('activation digest against unanimous validator projection',)),
    (MODEL_PROMOTION_RECEIPT,
        '.all(|member| member.weight() < policy.threshold())',
        '.all(|member| member.weight() <= policy.threshold())',
        'reject a governance policy satisfiable by one weighted member',
        ('distinct-signer governance policy floor',)),
    (MODEL_LIFECYCLE,
        '|| self.runtime_effective_config_sha256 == [0; 32]', '|| false',
        'reject a zero persisted runtime projection identity',
        ('persisted nonzero runtime projection identity',)),
    (MODEL_ISI_REGISTRY,
        'impl_direct_instruction_box!(crate::isi::offline::CancelKagemushaRecursiveReleaseV4);',
        '// lifecycle cancellation boxing detached',
        'reject a detached lifecycle cancellation instruction box',
        ('direct lifecycle instruction boxing',)),
    (CORE_ISI_REGISTRY,
        'dispatch_instruction::<iroha_data_model::isi::offline::DeactivateKagemushaRecursiveIssuanceV4>',
        'dispatch_instruction::<iroha_data_model::isi::offline::EnableKagemushaRecursiveIssuanceV4>',
        'reject detached lifecycle deactivation dispatch',
        ('direct lifecycle executor dispatch',)),
    (CORE_LIFECYCLE,
        'let context = context.take();',
        'let context = context.as_ref().cloned();',
        'reject reusable lifecycle transaction context',
        ('affine direct lifecycle transaction carrier',)),
    (CORE_ACTIVATION,
        'kagemusha_release_lifecycle::require_direct_stage(&self, state_transaction)?;',
        '// direct stage carrier bypassed',
        'reject staging without an exact direct carrier',
        ('stage validation before atomic lifecycle mutation',)),
    (CORE_ACTIVATION,
        'let runtime_effective_config_sha256 = self.runtime_effective_config_sha256;',
        'let runtime_effective_config_sha256 = [0xAA; 32];',
        'reject staging detached from the signed runtime projection identity',
        ('signed activation digest retained by staging',)),
    (CORE_LIFECYCLE,
        'load_lifecycle_by_manifest(world, &binding.manifest_sha256)?',
        'load_lifecycle_by_manifest(world, &binding.promotion_id)?',
        'reject a non-manifest lifecycle lookup',
        ('manifest-addressed fail-closed lifecycle state',)),
    (CORE_LIFECYCLE, "    Ok(record)\n}\n\nfn exact_lifecycle_verifier_records", "    Ok(record.clone())\n}\n\nfn exact_lifecycle_verifier_records", 'reject cloned verifier records on the issuance path', ('cloned verifier records on the issuance/readiness path',)),
    (CORE_LIFECYCLE, 'ensure_release_qualified_kagemusha_v4_verifier_id(id, record, parity, role)?;', '// release qualification bypassed', 'reject cancellation without release-qualified verifier identity', ('borrowed exact release-qualified Eq/Ep verifier authentication',)),
    (CORE_LIFECYCLE, 'record.status != ConfidentialStatus::Active', 'false', 'reject cancellation of an inactive verifier', ('borrowed exact release-qualified Eq/Ep verifier authentication',)),
    (CORE_LIFECYCLE, 'world.verifying_keys_by_circuit().get(&expected_index) != Some(id)', 'false', 'reject cancellation detached from the verifier circuit index', ('borrowed exact release-qualified Eq/Ep verifier authentication',)),
    (CORE_LIFECYCLE, 'if state.step_eq_verifier_key_id != expected_eq || state.step_ep_verifier_key_id != expected_ep', 'if false', 'reject cancellation with substituted lifecycle verifier IDs', ('borrowed exact release-qualified Eq/Ep verifier authentication',)),
    (CORE, 'kagemusha_terminal_registry_v4::verifier_owner_manifest_sha256(record, role)?;', '[0; 32];', 'reject verifier qualification detached from release ownership', ('release owner/circuit-qualified verifier identity',)), (CORE, 'kagemusha_release_verifier_id_has_exact_digest(id, "")\n        || kagemusha_v4_parity_for_circuit(circuit_id).is_some()', 'id.backend.as_str() == KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4', 'reject widening native hydration to same-backend lookalikes', ('narrow V4 candidate and V5 rejection',)), (CORE, 'if kagemusha_release_verifier_id_has_exact_digest(id, "v5-") {', 'if false {', 'reject exact V5 verifier identities entering generic hydration', ('narrow V4 candidate and V5 rejection', 'atomic native Eq/Ep hydration')), (CORE, 'ensure_exact_kagemusha_v4_native_verifier_storage_shape(record, identity.parity)?;', '// storage shape bypassed', 'reject malformed native verifier storage exclusion', ('atomic native Eq/Ep hydration',)),
    (CORE, 'let parity = kagemusha_v4_parity_for_circuit(&record.circuit_id).ok_or_else', 'let parity = Some(KagemushaPastaCycleParityV1::StepEq).ok_or_else', 'reject verifier qualification detached from its circuit', ('release owner/circuit-qualified verifier identity',)), (CORE, '|| step_eq.status != step_ep.status', '|| false', 'reject non-atomic native verifier pair exclusion', ('atomic native Eq/Ep hydration',)), (CORE_IVM_HOST, 'if native_kagemusha_v4_ids.contains(id) {', 'if false {', 'reject native Kagemusha verifiers entering strict generic hydration', ('native VK exclusion before generic hydration',)),
    (CORE_LIFECYCLE, '        commit_transition(marker, loaded, next, state_transaction)?;\n        withdrawal.apply(state_transaction);', '        withdrawal.apply(state_transaction);\n        commit_transition(marker, loaded, next, state_transaction)?;', 'reject verifier tombstones applied before lifecycle/replay commit', ('full verifier validation and planning before lifecycle/replay commit and tombstone apply',)),
    (CORE_LIFECYCLE, '(state.step_ep_verifier_key_id.clone(), step_ep.clone()),', '(state.step_eq_verifier_key_id.clone(), step_eq.clone()),', 'reject a cancellation plan detached from the Ep verifier', ('atomic two-verifier cancellation withdrawal plan',)), (CORE_LIFECYCLE, 'record.activation_height = None;', 'record.activation_height = Some(current_height.saturating_add(1));', 'reject a cancelled pre-activation verifier retaining an inverted activation boundary', ('pre-activation cancellation clears the never-reached boundary before withdrawal',)),
    (CORE_LIFECYCLE, 'let key_bytes = vec![key_byte; 32];', 'let key_bytes = Vec::new();', 'reject empty verifier fixtures masking key scrubbing', ('two owned active indexed nonempty lifecycle verifier records',)),
    (CORE_LIFECYCLE, '            Some(&lifecycle_before),', '            None,', 'reject a corruption regression without lifecycle atomicity', ('atomic missing/substituted/owner/version/status/index cancellation verifier regressions',)),
    (CORE_LIFECYCLE,
        '|| !liveness_terminal_is_current_parent(',
        '|| !liveness_terminal_is_accepted_ancestor(',
        'reject enablement from a non-parent liveness terminal',
        ('evidence-backed enable transition',)),
    (CORE_LIFECYCLE,
        'if local_runtime_effective_config_sha256 != Some(expected)', 'if false',
        'reject an active lifecycle without the exact local runtime projection',
        ('active lifecycle runtime lock',)),
    (CORE_LIFECYCLE,
        '.range(range_start..)', '.iter()',
        'reject a whole-WSV active lifecycle scan',
        ('prefix-bounded active lifecycle scan', 'whole-WSV active lifecycle scan')),
    (CORE_LIFECYCLE,
        '            break;', '            continue;',
        'reject a lifecycle-prefix scan without an early break',
        ('prefix-bounded active lifecycle scan',)),
    (CORE_LIFECYCLE,
        'transaction.verify_signature()', 'Ok(())',
        'reject a lifecycle carrier without canonical signature verification',
        ('verified canonical distinct transaction-signature floor',)),
    (CORE_LIFECYCLE,
        'if signer_count < KAGEMUSHA_V4_ACTIVATION_GOVERNANCE_MIN_SIGNERS', 'if false',
        'reject a lifecycle carrier with fewer than two distinct signers',
        ('verified canonical distinct transaction-signature floor',)),
    (CORE_LIFECYCLE,
        'for kind in [\n            LifecycleEntrypointKind::Stage,',
        'for kind in [\n            LifecycleEntrypointKind::Enable,',
        'reject loss of the Stage signature-floor regression',
        ('all-four-kind distinct-signer regressions',)),
    (CORE_LIFECYCLE,
        'LifecycleEntrypointKind::Stage,\n            LifecycleEntrypointKind::Enable,',
        'LifecycleEntrypointKind::Stage,\n            LifecycleEntrypointKind::Stage,',
        'reject loss of the Enable signature-floor regression',
        ('all-four-kind distinct-signer regressions',)),
    (CORE_LIFECYCLE,
        'LifecycleEntrypointKind::Enable,\n            LifecycleEntrypointKind::Cancel,',
        'LifecycleEntrypointKind::Enable,\n            LifecycleEntrypointKind::Enable,',
        'reject loss of the Cancel signature-floor regression',
        ('all-four-kind distinct-signer regressions',)),
    (CORE_LIFECYCLE,
        'LifecycleEntrypointKind::Cancel,\n            LifecycleEntrypointKind::Deactivate,',
        'LifecycleEntrypointKind::Cancel,\n            LifecycleEntrypointKind::Cancel,',
        'reject loss of the Deactivate signature-floor regression',
        ('all-four-kind distinct-signer regressions',)),
    (CORE_LIFECYCLE,
        'fn lifecycle_state_rejects_a_policy_with_one_threshold_weight_member()',
        'fn lifecycle_state_accepts_a_policy_with_one_threshold_weight_member()',
        'reject loss of the single-member governance-policy regression',
        ('all-four-kind distinct-signer regressions',)),
    (CORE_LIFECYCLE, '&lifecycle.step_ep_verifier_key_id,\n                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,', '&lifecycle.step_eq_verifier_key_id,\n                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,', 'reject a fixture without both active verifier records', ('two owned active indexed nonempty lifecycle verifier records',)),
    (CORE_LIFECYCLE, 'record.status = ConfidentialStatus::Active;', 'record.status = ConfidentialStatus::Withdrawn;', 'reject an inactive lifecycle verifier fixture', ('two owned active indexed nonempty lifecycle verifier records',)),
    (CORE_LIFECYCLE, 'assert_eq!(verifier.status, ConfidentialStatus::Withdrawn);', 'assert_eq!(verifier.status, ConfidentialStatus::Active);', 'reject cancellation without withdrawn verifier tombstones', ('Cancel verifier tombstones',)), (CORE_LIFECYCLE, 'record.activation_height = Some(', 'record.activation_height = None; //', 'reject a fixture that never exercises pre-activation cancellation', ('two owned active indexed nonempty lifecycle verifier records',)), (CORE_LIFECYCLE, 'assert_eq!(verifier.activation_height, None);', 'assert!(verifier.activation_height.is_some());', 'reject a cancellation regression retaining the future activation boundary', ('Cancel verifier tombstones',)), (CORE_LIFECYCLE, 'Some(id),\n                "cancellation must retain the original release verifier index"', 'None,\n                "cancellation must retain the original release verifier index"', 'reject cancellation detached from retained Eq/Ep indexes', ('Cancel verifier tombstones',)),
    (CORE_LIFECYCLE, 'assert_eq!(verifier.withdraw_height, Some(3));', 'assert_eq!(verifier.withdraw_height, None);', 'reject cancellation without the exact withdrawal height', ('Cancel verifier tombstones',)),
    (CORE_LIFECYCLE, 'assert!(verifier.key.is_none());', 'assert!(verifier.key.is_some());', 'reject cancellation retaining verifier key bytes', ('Cancel verifier tombstones',)),
    (CORE_LIFECYCLE, 'assert_eq!(verifier.vk_len, 0);', 'assert_ne!(verifier.vk_len, 0);', 'reject cancellation retaining verifier length', ('Cancel verifier tombstones',)),
    (CORE_LIFECYCLE, 'transaction.world.smart_contract_state.get(&lifecycle_key),\n            Some(&lifecycle_before)', 'transaction.world.smart_contract_state.get(&lifecycle_key),\n            None', 'reject a repeated terminal transition with state effects', ('terminal repeat and cross-terminal no-effects rejection',), -1),
    (CORE_LIFECYCLE, '.get(&cancellation_marker)\n                .is_none()', '.get(&cancellation_marker)\n                .is_some()', 'reject a failed repeated Cancel that commits replay', ('terminal repeat and cross-terminal no-effects rejection',)),
    (CORE_LIFECYCLE, '.get(&deactivation_marker)\n                .is_none()', '.get(&deactivation_marker)\n                .is_some()', 'reject a failed cross-terminal Deactivate that commits replay', ('terminal repeat and cross-terminal no-effects rejection',)),
    (CORE_TX,
        'let allows_direct_kagemusha_lifecycle_authority = lifecycle_entrypoint.is_some()',
        'let allows_direct_kagemusha_lifecycle_authority = tx.multisig_signatures().is_some()',
        'reject a lifecycle admission exception outside the verified carrier classifier',
        ('narrow verified-multisig lifecycle admission wiring',)),
    (CORE_LIFECYCLE, '    require_no_proof_attachments(transaction)?;',
        '    // proof attachments accepted',
        'reject a lifecycle carrier with proof attachments',
        ('affine direct lifecycle transaction carrier',)),
    (CORE_TX_AUTHORITY_ADMISSION,
        'crate::smartcontracts::isi::offline::direct_lifecycle_entrypoint_kind(instruction).is_some()',
        'true',
        'reject a lifecycle authority exception without one exact lifecycle instruction',
        ('one-exact-instruction lifecycle authority classifier',)),
    (CORE_TX_LIFECYCLE_TESTS,
        'fn exact_kagemusha_lifecycle_rejects_one_threshold_weight_signer_at_stateful_admission()',
        'fn unchecked_weighted_kagemusha_lifecycle_admission()',
        'reject loss of the stateful distinct-signer admission regression',
        ('stateful-admission distinct-signer regression',)),
    (CORE_TX_LIFECYCLE_TESTS,
        'fn kagemusha_v4_non_lifecycle_proof_attachments_remain_outside_the_lifecycle_gate()',
        'fn unchecked_non_lifecycle_attachment_routing()',
        'reject loss of non-lifecycle attachment routing coverage',
        ('narrow verified-multisig lifecycle admission regressions',)),
    (CORE_TX_LIFECYCLE_TESTS,
        'fn kagemusha_v4_lifecycle_proof_attachments_fail_closed_at_stateful_admission()',
        'fn unchecked_lifecycle_attachment_admission()',
        'reject loss of lifecycle attachment fail-closed coverage',
        ('narrow verified-multisig lifecycle admission regressions',)),
    (CORE_STATE,
        'pub(crate) kagemusha_release_lifecycle_entrypoint: Option<LifecycleEntrypointContext>',
        'pub(crate) kagemusha_release_lifecycle_entrypoint: bool',
        'reject an unbound lifecycle state carrier',
        ('one-shot lifecycle state carrier',)),
    (CORE_STATE_RUNTIME_CONFIG,
        'pub(crate) fn require_kagemusha_runtime_effective_config_for_world(',
        'pub(crate) fn unchecked_kagemusha_runtime_effective_config_for_world(',
        'reject prospective worlds detached from the local runtime projection',
        ('immutable process-local runtime projection',)),
    (CORE_STATE_RUNTIME_CONFIG,
        '.get() == Some(&digest)',
        '.get().is_some()',
        'reject a concurrent different runtime projection digest',
        ('atomic install-once runtime projection identity',)),
    (CORE_STATE_RUNTIME_CONFIG_TESTS,
        'fn concurrent_runtime_projection_install_accepts_only_one_distinct_digest()',
        'fn unchecked_concurrent_runtime_projection_install()',
        'reject loss of concurrent runtime projection install coverage',
        ('sequential and concurrent runtime projection install regressions',)),
    (CORE_RUNTIME_CONFIG,
        'signed_genesis_context: SumeragiV2GenesisContextParameters',
        '_caller_genesis_context: SumeragiV2GenesisContextParameters',
        'reject snapshot runtime derivation detached from the signed seal context',
        ('authenticated complete local runtime derivation',)),
    (IROHAD_VALIDATOR_SEAL_READER,
        'seal.verify()\n        .map_err(|error| format!("invalid Kagemusha validator qualification seal: {error}"))?;',
        'Ok::<(), String>(())?;',
        'reject an unverified configured validator qualification seal',
        ('bounded canonical signature-verified validator seal',)),
    (IROHAD_STARTUP,
        'let Some(_) = config',
        'let Some(_) = unchecked_config',
        'reject snapshot startup without fail-closed absent-seal handling',
        ('startup-authenticated local runtime projection installation',)),
    (IROHAD_STARTUP,
        'if derived.projection() != &seal.body.runtime_effective_config', 'if false',
        'reject snapshot startup detached from the exact signed validator projection',
        ('startup-authenticated local runtime projection installation',)),
    (CORE_LIFECYCLE,
        'if catalog_configured || lifecycle_frozen', 'if false',
        'reject consensus-parameter mutation after catalog authentication or lifecycle staging',
        ('catalog-or-lifecycle consensus-parameter freeze',)),
    (CORE_WORLD,
        'crate::smartcontracts::isi::offline::validate_runtime_consensus_parameter_update(',
        'crate::smartcontracts::isi::offline::unchecked_runtime_consensus_parameter_update(',
        'reject a world parameter path detached from the runtime lock',
        ('world parameter execution runtime lock',)),
    (CORE_IVM_HOST,
        '    "kagemusha",',
        '    "kagemusha_online_registration_",',
        'reject a native Kagemusha namespace narrowed to one record family',
        ('exact delimiter-aware native Kagemusha namespace root',)),
    (CORE_IVM_HOST,
        '            "kagemushax",',
        '            "kagemusha_x",',
        'reject a Kagemusha namespace classifier without its delimiter false-positive regression',
        ('Kagemusha namespace ownership and delimiter regression',)),
    (CORE_SUMERAGI_APPLY,
        '.require_kagemusha_runtime_effective_config_for_world(state_block.world())',
        '.unchecked_kagemusha_runtime_effective_config_for_world(state_block.world())',
        'reject prospective block validation detached from the runtime projection',
        ('prospective state runtime check before witness/Kura effects',)),
    (CORE_SUMERAGI_WORKER,
        '.require_committed_kagemusha_runtime_effective_config(',
        '.unchecked_committed_kagemusha_runtime_effective_config(',
        'reject consensus signing detached from committed runtime state',
        ('ordinary and recovered pre-sign runtime recheck',)),
    (CORE_SUMERAGI_RUNNER,
        '.require_committed_kagemusha_runtime_effective_config()',
        '.unchecked_committed_kagemusha_runtime_effective_config()',
        'reject normal replay detached from reconstructed runtime state',
        ('normal replay runtime check after startup reconstruction',)),
    (CORE_SUMERAGI_PENDING_KURA,
        '.require_committed_kagemusha_runtime_effective_config()',
        '.unchecked_committed_kagemusha_runtime_effective_config()',
        'reject pending-tip replay detached from reconstructed runtime state',
        ('pending-tip runtime check after reconstruction',)),
    (CORE_COMMITTED_CONTEXT,
        'signed_lifecycle_entrypoint_context(transaction)',
        'Ok(None)',
        'reject committed replay detached from lifecycle context',
        ('committed lifecycle context reset and External-only derivation',)),
    (CORE_BLOCK,
        'signed_lifecycle_entrypoint_context(tx)',
        'Ok(None)',
        'reject block admission detached from lifecycle context',
        ('block-admission lifecycle context derivation',)),
    (CORE_EXECUTOR,
        'signed_lifecycle_entrypoint_context(&transaction)?',
        'None',
        'reject execution detached from lifecycle context',
        ('executor lifecycle reset and direct-carrier derivation',)),
    (CORE_LIFECYCLE,
        'let loaded = load_lifecycle(world, binding)?',
        'let loaded = issuance_enabled(world, binding)?.then_some(load_lifecycle(world, binding)?).flatten()',
        'reject issuance-phase gating of full redemption',
        ('terminal release-scoped redemption policy', 'issuance-phase gating of full redemption')),
    (CORE_REDEMPTION_POLICY,
        'let current_policy = effective_offline_device_attestation_policy(state_transaction)?;',
        'let current_policy = release_policy.clone();',
        'reject redemption detached from live emergency trust',
        ('release-scoped redemption with live emergency trust compatibility',)),
    (CORE,
        'kagemusha_release_lifecycle::redemption_policy(',
        'unchecked_release_redemption_policy(',
        'reject redemption before a release-policy lookup',
        ('release-policy lookup before redemption authentication and replay',)),
    (CORE_ISI_TESTS, CORE_REDEMPTION_POLICY_TESTS_INCLUDE,
        '// release-scoped redemption regressions detached',
        'reject a detached release-scoped redemption regression component',
        ('exactly one authenticated isi_kagemusha_redemption_policy_tests.rs attachment',)),
    (CORE_REDEMPTION_POLICY_TESTS,
        'fn release_scoped_registration_rejects_incompatible_live_trust_rotation()',
        'fn release_scoped_registration_accepts_incompatible_live_trust_rotation()',
        'reject loss of live emergency-trust rejection coverage',
        ('active release-scoped redemption policy regressions',)),
    (READINESS_SOURCE_SUPPORT,
        'globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1") is not True',
        'False', 'reject a detached readiness source-support provider',
        ('_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1',), -1),
    (READINESS_SOURCE_CONTRACT,
        'globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1") is not True',
        'False', 'reject a detached readiness source-contract provider',
        ('_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1',), -1),
    (READINESS_RECURSION_SOURCE_CONTRACT,
        'def recursion_source_contract_errors(',
        'def detached_recursion_source_contract_errors(',
        'reject a detached recursion source-contract provider',
        ('recursion_source_contract_errors',), -1),
    (READINESS_SELF_TEST, 'globals().get("_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1") is not True',
        'False', 'reject a detached readiness self-test helper', ('_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1',), -1),
    (RECURSION_ADAPTER, 'compact.len() != expected_compact_len',
        'compact.len() != 64', 'reject a fixed compact-header length',
        ('compact.len() != expected_compact_len',)),
    (READINESS, 'sys.version_info >= (3, 10)', 'True', 'reject a missing Python version preflight', ('sys.version_info >= (3, 10)',)),
    (READINESS, '--diff-filter=U', '--diff-filter=M', 'reject a missing unresolved-index preflight', ('--diff-filter=U',)),
    (READINESS, 'diff --cached --quiet --no-ext-diff --no-textconv --diff-filter=U',
        'diff --quiet --diff-filter=U', 'reject a worktree-refreshing index preflight', ('--cached',)),
    (READINESS, '-c core.fsmonitor=false', '-c core.fsmonitor=/tmp/hostile-hook',
        'reject an executable Git fsmonitor hook', ('core.fsmonitor=false',)),
    (READINESS, 'config --get-all core.worktree', 'config --local --get-all core.worktree',
        'reject a per-worktree core.worktree bypass', ('config --get-all core.worktree',)),
    (READINESS, '--work-tree="${ROOT_DIR}"', '--work-tree="/tmp/substituted-worktree"',
        'reject a substituted Git worktree pin', ('--work-tree="${ROOT_DIR}"',)),
    (READINESS, 'if [[ "${CONFIGURED_CORE_WORKTREE}" != "${ROOT_DIR}" ]]; then', 'if false; then',
        'reject an external configured core.worktree', ('CONFIGURED_CORE_WORKTREE',)),
    (READINESS, 'if [[ "${CONFIGURED_CORE_WORKTREE_STATUS}" -ne 1 ]]; then', 'if false; then',
        'reject an unreadable configured core.worktree state', ('CONFIGURED_CORE_WORKTREE_STATUS',)),
    (READINESS,
        '        require_production_root_custody(descriptor, label)\n        payload = read_pinned_descriptor(',
        '        # reviewed-file root custody removed\n        payload = read_pinned_descriptor(',
        'reject a reviewed source provider without root custody',
        ('generic root-custodied source-closure-authenticated reviewed-file loader',)),
    (READINESS,
        '    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_LIFECYCLE_SOURCE_CONTRACT,',
        '    # recursion provider omitted from the authenticated set\n    READINESS_LIFECYCLE_SOURCE_CONTRACT,',
        'reject an unauthenticated recursion source-contract provider',
        ('exact authenticated source-provider set',)),
    (READINESS,
        'READINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_LIFECYCLE_SOURCE_CONTRACT,\n    READINESS_SOURCE_CONTRACT,\n)',
        'READINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_LIFECYCLE_SOURCE_CONTRACT,\n    READINESS_SOURCE_CONTRACT,\n)\nREADINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n)',
        'reject a later authenticated source-provider tuple rebind',
        ('exactly one immutable authenticated source-provider tuple',)),
    (READINESS,
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)',
        'support_bytes = (root / READINESS_SOURCE_SUPPORT).read_bytes()',
        'reject execution of reopened source-support provider bytes',
        ('authenticated byte-only support, recursion, lifecycle, and readiness source-contract dispatch',)),
    (READINESS,
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)',
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)\nsupport_bytes = Path("/tmp/hostile-support").read_bytes()',
        'reject a shadowed source-support provider byte assignment',
        ('support_bytes must have exactly one authenticated provider-map assignment',)),
    (READINESS,
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)',
        'recursion_bytes = (root / READINESS_RECURSION_SOURCE_CONTRACT).read_bytes()',
        'reject execution of reopened recursion source-contract provider bytes',
        ('authenticated byte-only support, recursion, lifecycle, and readiness source-contract dispatch',
         'candidate-only path read')),
    (READINESS,
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)',
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)\nrecursion_bytes = open("/tmp/hostile-recursion", "rb").read()',
        'reject a shadowed recursion provider byte assignment',
        ('recursion_bytes must have exactly one authenticated provider-map assignment',)),
    (READINESS,
        'lifecycle_bytes = source_contract_bytes.get(READINESS_LIFECYCLE_SOURCE_CONTRACT)',
        'lifecycle_bytes = (root / READINESS_LIFECYCLE_SOURCE_CONTRACT).read_bytes()',
        'reject execution of reopened lifecycle source-contract provider bytes',
        ('authenticated byte-only support, recursion, lifecycle, and readiness source-contract dispatch',
         'candidate-only path read')),
    (READINESS,
        'lifecycle_bytes = source_contract_bytes.get(READINESS_LIFECYCLE_SOURCE_CONTRACT)',
        'lifecycle_bytes = source_contract_bytes.get(READINESS_LIFECYCLE_SOURCE_CONTRACT)\nlifecycle_bytes = open("/tmp/hostile-lifecycle", "rb").read()',
        'reject a shadowed lifecycle provider byte assignment',
        ('lifecycle_bytes must have exactly one authenticated provider-map assignment',)),
    (READINESS,
        'primary_bytes = source_contract_bytes.get(READINESS_SOURCE_CONTRACT)',
        'primary_bytes = source_contract_bytes.get(READINESS_SOURCE_CONTRACT)\nprimary_bytes = read("/tmp/hostile-primary", []).encode()',
        'reject a shadowed primary provider byte assignment',
        ('primary_bytes must have exactly one authenticated provider-map assignment',)),
    (PRODUCTION_IOS_EVIDENCE_MODULE, 'def _validate_online_freshness_receipt(\n', 'def _unchecked_online_freshness_receipt(\n',
        'reject removal of online App Attest freshness validation', ('def _validate_online_freshness_receipt(',)),
    (PRODUCTION_IOS_EVIDENCE_MODULE, 'def build_production_signed_evidence(\n', 'def build_unchecked_production_signed_evidence(\n',
        'reject removal of production App Attest envelope construction', ('def build_production_signed_evidence(',)),
    (PRODUCTION_IOS_EVIDENCE_MODULE, '        require_current_freshness_receipt=True,\n',
        '        require_current_freshness_receipt=False,\n',
        'reject stale receipts in ordinary production evidence validation',
        ('separate current and historical freshness validator wrappers',)),
    (MODEL, 'KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22',
        'KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21', 'reject ABI-21 substitution'),
    (MODEL_COMPONENT, 'pub enum KagemushaPastaCycleArtifactKindV4', 'pub enum DetachedKagemushaPastaCycleArtifactKindV4', 'authenticate the split model component'),
    (MODEL_VERIFIER_COMPONENT, 'const VERIFIER_IDENTITY_SCHEMA_V4', 'const DETACHED_VERIFIER_IDENTITY_SCHEMA_V4', 'authenticate the release-verifier component'),
    (MODEL_PROMOTION_RECEIPT_COMPONENT, '        check_artifact_input_size(\n            bytes,\n            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,',
        '        unchecked_artifact_input_size(\n            bytes,\n            KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,',
        'reject an unbounded promotion reservation decoder',
        ('reservation decode',)),
    (MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        '        self.body.validate()?;\n        self.signature',
        '        self.signature',
        'reject an internal-validation receipt without signed-body validation',
        ('internal-validation receipt canonical signature/body validation',)),
    (MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        '            "final_release_inventory_is_exact_and_includes_both_receipts",',
        '            "retired_final_release_inventory_test",',
        'reject a stale internal-validation final-inventory command',
        ('exact internal-validation final-inventory command',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        'verify_authorization_signature(\n            &self.signature,',
        'unchecked_authorization_signature(\n            &self.signature,',
        'reject an unsigned embedded controller permit',
        D_PRECOMMIT),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            || &self.body.canary_authority != canary_authority',
        '            || false',
        'reject a canary permit detached from the actual transaction authority',
        D_PRECOMMIT),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            canary_transaction_intent: canary_transaction.hash(),',
        '            canary_transaction_intent: HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),',
        'reject a reservation detached from the exact transaction intent',
        D_RESERVATION),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            canary_entrypoint_hash: Hash::from(canary_transaction.hash_as_entrypoint()),',
        '            canary_entrypoint_hash: Hash::prehashed([0; 32]),',
        'reject a reservation detached from the external entrypoint hash',
        D_RESERVATION),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        verify_reservation_signature(\n            &self.signature,',
        '        unchecked_reservation_signature(\n            &self.signature,',
        'reject an unsigned minimal on-chain canary reservation',
        D_RESERVATION),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        verify_authorization_package_signature(\n            &self.signature,',
        '        unchecked_authorization_package_signature(\n            &self.signature,',
        'reject an unsigned full private canary package',
        ('full private authorization with exact reservation transaction and outer signature',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        'let Some(authority) = origin.strip_prefix("https://") else {',
        'let Some(authority) = origin.strip_prefix("http://") else {',
        'reject a non-HTTPS canary origin',
        ('canonical lower-case HTTPS DNS canary origin',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        || transaction.nonce().is_none()', '        || false',
        'reject a nonce-free canary transaction',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        || transaction.admission_intent() != TransactionAdmissionIntent::Ordinary',
        '        || false', 'reject a privileged canary transaction',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let time_to_live_ms = transaction\n        .time_to_live()',
        '    let time_to_live_ms = transaction\n        .unchecked_time_to_live()',
        'reject a canary without exact TTL validation',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let expires_at_height = transaction\n        .expires_at_height()',
        '    let expires_at_height = transaction\n        .unchecked_expires_at_height()',
        'reject a canary without height-expiry validation',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let [instruction] = instructions.as_ref() else {',
        '    let [instruction, ..] = instructions.as_ref() else {',
        'reject a multi-instruction canary transaction',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        .downcast_ref::<RecordKagemushaTairaCanaryV4>()',
        '        .downcast_ref::<AuthorizeKagemushaTairaCanaryV4>()',
        'reject a non-Record canary instruction',
        D_RECORD_CANARY),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if body.binding != *expectations.binding()', '    if false',
        'reject authorization detached from the activation network',
        D_POST_RECEIPT),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            .activation_finality_receipt\n            .matches_bytes(exact_receipt_bytes)',
        '            .activation_finality_receipt\n            .matches_bytes(b"")',
        'reject authorization detached from exact receipt bytes',
        D_POST_RECEIPT),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        .and_then(|height| height.checked_add(1))',
        '        .and_then(|height| height.checked_add(2))',
        'reject drift in the exclusive maximum canary height',
        D_POST_RECEIPT),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        'verify_evidence_signature(&self.signature, &self.body.issuer, self.body.signing_hash())?;',
        'unchecked_evidence_signature(&self.signature)?;',
        'reject unsigned canary evidence',
        ('exact issuer-signed canary evidence entrypoint',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if committed_wire != authorized_wire', '    if false',
        'reject committed canary wire detached from authorization',
        D_PROOF_CHAIN),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        activation_expectations_artifact: body.activation_expectations_artifact,',
        '        activation_expectations_artifact: body.activation_finality_receipt,',
        'reject a verified canary detached from its exact expectations artifact',
        D_EXPECTATIONS),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if receipt_terminal.finality_artifact.height.checked_add(1)',
        '    if receipt_terminal.finality_artifact.height.checked_add(2)',
        'reject a non-contiguous post-receipt finality extension',
        D_PROOF_CHAIN),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            || self.pipeline_transaction_intent != canary_transaction_intent',
        '            || false', 'reject activation-status substitution for canary status',
        ('canary-specific global Applied query observation',)),
    (MODEL_PROMOTION_RECEIPT_COMPONENT,
        '        || context.da_layout != runtime.genesis_context.da_layout',
        '        || false', 'reject canary finality with a different DA layout',
        D_FINALITY_CORRIDOR),
    (MODEL_PROMOTION_RECEIPT_COMPONENT,
        '            .any(|(actual, expected)| actual != &expected.bls_pop)',
        '            .any(|_| false)', 'reject canary finality without exact validator PoPs',
        D_FINALITY_CORRIDOR),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'if canonical_json_bytes_v1(&value)? != exact_receipt_json {', 'if false {',
        'reject a non-canonical catalog-revalidation receipt',
        ('catalog receipt decode',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'if json_required_string_v1(object, "signer_key_id")? != trusted_authority_key_id {',
        'if false {', 'reject a receipt without exact authority key-id binding',
        D_CATALOG_SIGNATURE),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'hasher.update(ED25519_SUBJECT_PUBLIC_KEY_INFO_DER_PREFIX_V1);',
        'hasher.update([]);', 'reject an authority digest without the Ed25519 SPKI prefix',
        ('authority id/SPKI digest',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,',
        '    0x30, 0x2b, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,',
        'reject a non-canonical Ed25519 SPKI digest prefix',
        ('receipt fields/SPKI',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '    unsigned.remove("signature_payload_sha256");',
        '    // signature payload digest retained in signed payload',
        'reject a non-Python-compatible receipt signature payload',
        ('canonical receipt payload',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '.verify(trusted_authority_public_key, signature_payload)',
        '.verify(trusted_authority_public_key, b"")',
        'reject a signature detached from the canonical receipt payload',
        D_CATALOG_SIGNATURE),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '            verify_kagemusha_catalog_sealed_paths_v1(&self.seal.paths, 0)?;\n            let current_time_ms = current_unix_time_ms_v1()?;',
        '            // final sealed-path recheck removed\n            let current_time_ms = current_unix_time_ms_v1()?;',
        'reject signing without a final sealed-path recheck',
        D_FINAL_RECHECK),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '        validate_validator_qualification_freshness_at_v1(subject, current_time_ms)?;',
        '        // final wall-clock freshness recheck removed',
        'reject signing without a final wall-clock freshness recheck',
        D_FINAL_RECHECK),
    (CONFIG, 'validator_qualification_inputs != 5',
        'validator_qualification_inputs != 4',
        'reject partial validator qualification configuration',
        ('qualification config completeness',)),
    (NODE_VALIDATOR_QUALIFICATION_COMPONENT,
        '            promotion.catalog_revalidation_authority_key_id,',
        '            "detached-authority",',
        'reject daemon authority-key detachment before Core',
        ('trusted promotion forwarding',)),
    (NODE_VALIDATOR_QUALIFICATION_COMPONENT,
        '        reason => Err(format!(\n'
        '            "stock launcher returned an unexpected Kagemusha qualification outcome: {reason:?}"\n'
        '        )),',
        '        _ => Ok(()),',
        'reject a stock launcher that accepts an unexpected unavailable outcome',
        ('stock-launcher fail-closed qualification outcome',)),
    (NODE_VALIDATOR_QUALIFICATION_COMPONENT,
        '        KagemushaValidatorQualificationOutcomeV1::Signed(_) => Err(\n'
        '            "stock launcher unexpectedly signed a Kagemusha validator qualification without trusted promotion inputs"\n'
        '                .to_owned(),\n'
        '        ),',
        '        KagemushaValidatorQualificationOutcomeV1::Signed(_) => Ok(()),',
        'reject a stock launcher that accepts an unexpected signed outcome',
        ('stock-launcher fail-closed qualification outcome',)),
    (NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT,
        '"/Library/SORA/Kagemusha/catalog-revalidation";',
        '"/tmp/kagemusha-catalog-revalidation";',
        'reject a non-fixed macOS catalog-revalidation path',
        D_RECEIPT_PATH),
    (NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT,
        '#[cfg(not(target_os = "macos"))]\npub fn read_configured_kagemusha_promotion_reservation(',
        '#[cfg(any())]\npub(super) fn read_configured_kagemusha_promotion_reservation(',
        'reject a non-macOS validator-qualification bypass',
        D_RECEIPT_PATH),
    (NODE,
        'kagemusha_validator_qualification_command::KagemushaValidatorSealPublicationTarget::prepare(',
        'kagemusha_validator_qualification_command::KagemushaValidatorSealPublicationTarget::unprepared(',
        'reject publication without a prepared validator-seal destination',
        ('seal action ordering',)),
    (NODE, 'continue_after_full_kagemusha_check(\n        full_validation,',
        'continue_after_full_kagemusha_check(\n        Ok((validated_genesis, block_cadence_ms)),',
        'reject validator signing detached from full genesis validation',
        D_VALIDATION_SIGN),
    (NODE, 'action(full_validation?)', 'action(unvalidated)',
        'reject a value-carrying validation-gate bypass', ('validation result gate',)),
    (NODE, 'Some(&runtime_effective_config),', 'None,',
        'reject qualification without the runtime projection', D_VALIDATION_SIGN),
    (MODEL, 'pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;',
        'pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 5;',
        'reject a non-four-validator projection', ('ACTIVATION_VALIDATOR_COUNT',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'config.sumeragi.role != NodeRole::Validator', 'false',
        'reject a non-validator projection', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'context.mode != ConsensusMode::Permissioned', 'false',
        'reject a non-permissioned projection', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'context.roster.iter().any(|member| member.power != 1)', 'false',
        'reject a weighted projection roster', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'trusted.pops.get(validator_id.public_key()) == Some(pop)', 'true',
        'reject projection without the exact configured PoP map', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'config.network.public_address.value().clone()', 'trusted.myself.address().clone()',
        'reject projection without the advertised local public address', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'Duration::from_millis(metadata.block_cadence_ms.get())', 'Duration::from_secs(1)',
        'reject projection without signed-metadata cadence', (), -1),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'projection.validate().map_err(|error| error.to_string())?;', 'let _ = &projection;',
        'reject an unvalidated runtime projection', ()),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        '    projection: KagemushaV4RuntimeEffectiveConfigProjectionV1,',
        '    pub projection: KagemushaV4RuntimeEffectiveConfigProjectionV1,',
        'reject a caller-forgeable verified projection', ('opaque Core',)),
    (NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT,
        'VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive(config, genesis, bootstrap)',
        'VerifiedKagemushaV4RuntimeEffectiveConfigV1::derive(config, genesis, unvalidated)',
        'reject a daemon wrapper detached from validated bootstrap', ('thin verified',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1',
        'runtime_effective_config: &KagemushaV4RuntimeEffectiveConfigProjectionV1',
        'reject raw runtime projections at the production signer', ('verified runtime config',)),
    (NODE_ROOT_OWNED_PUBLICATION_COMPONENT, 'rustix::fs::RenameFlags::NOREPLACE,',
        'rustix::fs::RenameFlags::empty(),',
        'reject replace-capable validator-seal publication',
        ('no-replace commit protocol',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'if metadata.mode != SumeragiConsensusMode::Permissioned {',
        'if false {',
        'reject NPoS-mode validator qualification',
        ('metadata.mode != SumeragiConsensusMode::Permissioned',)),
    (NODE_ROOT_OWNED_PUBLICATION_COMPONENT,
        '            MACOS_XATTR_SHOWCOMPRESSION,',
        '            0,',
        'reject hidden macOS compression xattrs',
        ('macOS hidden-xattr query',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        'let promotion_marker = plan_v4_promotion_id(binding.promotion_id, state_transaction)?;',
        'let promotion_marker = unchecked_v4_promotion_id(binding.promotion_id, state_transaction)?;',
        'reject unconsumed promotion ids',
        D_CANARY_MARKER),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '    let wire = transaction.encode_wire_v1().map_err(|error| {',
        '    let wire = unchecked_canary_wire(transaction).map_err(|error| {',
        'reject canary identity detached from the complete signed wire',
        D_WIRE_BOUNDARY),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '    if transaction.admission_intent() != TransactionAdmissionIntent::Ordinary {',
        '    if false {',
        'reject a non-Ordinary signed canary wire identity',
        D_WIRE_BOUNDARY),
    (CORE_TX,
        '        state_transaction.kagemusha_taira_canary_external_entrypoint =\n            matches!(tx.entrypoint(), TransactionEntrypoint::External(_));',
        '        state_transaction.kagemusha_taira_canary_external_entrypoint = true;',
        'reject unconditional live canary provenance',
        ('External-only live transaction canary provenance',)),
    (CORE_COMMITTED_TX_CONTEXT,
        'signed_kagemusha_taira_canary_wire_identity_v1(transaction)',
        'unchecked_kagemusha_taira_canary_wire_identity_v1(transaction)',
        'reject replay without the exact committed canary wire',
        D_WIRE_BOUNDARY),
    (CORE_STATE,
        '                crate::state::seed_committed_transaction_context(',
        '                crate::state::unchecked_committed_transaction_context(',
        'reject a detached committed replay context call',
        D_WIRE_BOUNDARY),
    (CORE_BLOCK,
        'crate::smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1(tx)',
        'crate::smartcontracts::isi::offline::unchecked_kagemusha_taira_canary_wire_identity_v1(tx)',
        'reject block admission without the exact canary wire',
        D_WIRE_BOUNDARY),
    (CORE_BLOCK,
        '                .any(|entrypoint| !matches!(entrypoint, TransactionEntrypoint::External(_)));',
        '                .any(|_| false);',
        'reject parallel live admission of a non-External block',
        ('non-External live block sequential-execution selector',)),
    (CORE_EXECUTOR,
        '            signed_kagemusha_taira_canary_wire_identity_v1(&transaction)?;',
        '            unchecked_kagemusha_taira_canary_wire_identity_v1(&transaction)?;',
        'reject sequential execution without the exact canary wire',
        D_WIRE_BOUNDARY),
    (CORE_EXECUTOR,
        '        state_transaction.kagemusha_taira_canary_wire_identity = None;',
        '        // stale Kagemusha signed-wire identity retained',
        'reject stale signed-boundary context before executor validation',
        D_WIRE_BOUNDARY),
    (CORE_EXECUTOR,
        '        if state_transaction.kagemusha_taira_canary_external_entrypoint {',
        '        if true {', 'reject sealed reveal upgraded by executor', D_WIRE_BOUNDARY),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        if !state_transaction.kagemusha_taira_canary_external_entrypoint {',
        '        if false {', 'reject canary without External carrier', D_CANARY_EXEC),
    (MODEL_ISI_OFFLINE,
        'pub reservation: KagemushaV4TairaCanaryReservationV1,',
        'pub reservation: KagemushaV4TairaCanaryAuthorizationV1,',
        'reject disclosure of the full canary authorization in Authorize',
        ('canonical minimal canary Record and Authorize instruction wires',
         'AuthorizeKagemushaTairaCanaryV4 payload')),
    (MODEL_ISI_MOD,
        'impl_direct_instruction_box!(crate::isi::offline::AuthorizeKagemushaTairaCanaryV4);',
        '// AuthorizeKagemushaTairaCanaryV4 boxing removed',
        'reject a detached canary authorization instruction box',
        ('AuthorizeKagemushaTairaCanaryV4',)),
    (CORE_ISI_MOD,
        'dispatch_instruction::<iroha_data_model::isi::offline::RecordKagemushaTairaCanaryV4>',
        'dispatch_instruction::<iroha_data_model::isi::offline::RetiredKagemushaTairaCanaryV4>',
        'reject detached Core dispatch for the one-shot canary',
        ('RecordKagemushaTairaCanaryV4',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '                state_transaction.network_id(),\n                authority,',
        '                state_transaction.network_id(),\n                &self.reservation.body.permit.body.canary_authority,',
        'reject canary authorization detached from the actual authorizer',
        D_CANARY_EXEC, 2),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        (true, true, true, true) => Ok(None),',
        '        (true, true, true, true) => Err(labeled_invariant("unchecked", "unchecked").into()),',
        'reject loss of same-exact reservation idempotence',
        D_CANARY_MARKER),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '    let reservation_bytes = norito::encode_canonical(reservation).map_err(|error| {',
        '    let reservation_bytes = norito::encode_canonical(reservation.permit()).map_err(|error| {',
        'reject idempotence detached from the complete signed reservation bytes',
        D_CANARY_MARKER),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        .insert(exact_wire, ());', '        .insert(exact_call, ());',
        'reject authorization detached from the complete signed-wire marker',
        D_CANARY_MARKER),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '            .kagemusha_taira_canary_wire_identity',
        '            .unchecked_kagemusha_taira_canary_wire_identity',
        'reject canary execution without its admitted complete wire',
        D_CANARY_EXEC),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '            .kagemusha_taira_canary_wire_identity\n            .take()\n            .ok_or_else',
        '            .kagemusha_taira_canary_wire_identity\n            .ok_or_else',
        'reject reuse of the exact signed-wire capability by a nested canary',
        D_CANARY_EXEC),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        commit_v4_taira_canary(marker, state_transaction);',
        '        // promotion one-shot commit removed',
        'reject a canary that does not consume the promotion one-shot',
        D_CANARY_EXEC),
    (AUTHENTICATED_TOOL_CONTROLLER, KAGEMUSHA_PROMOTION_PUBLISHER_MODULE,
        '// promotion publisher module detached',
        'authenticate the promotion-publisher module wiring',
        ('expected exactly one reviewed kagemusha_promotion_publisher.rs module',)),
    (AUTHENTICATED_TOOL_CONTROLLER, KAGEMUSHA_PYTHON_LAUNCHER_MODULE,
        '// Python launcher module detached',
        'authenticate the promotion Python-launcher module wiring',
        ('expected exactly one reviewed kagemusha_python_launcher.rs module',)),
    (AUTHENTICATED_TOOL_CONTROLLER,
        'Some("promote-kagemusha-release-v4") => {',
        'Some("unchecked-promote-kagemusha-release-v4") => {',
        'reject a substituted authenticated promotion subcommand',
        ('exact authenticated promotion-publisher controller dispatch',)),
    (KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        '        pub(crate) fn file_mut(&mut self) -> &mut File {',
        '        pub(crate) fn exposed_file_mut(&mut self) -> &mut File {',
        'reject an altered pinned-file descriptor accessor',
        ('private pinned-file state with the sole mutable descriptor accessor',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        let source_size = source\n            .file_mut()',
        '        let source_size = source\n            .unbounded_file_mut()',
        'reject a snapshot copy outside the pinned-file accessor',
        ('accessor-confined bounded Kagami snapshot copy',)),
    (OFFLINE_CLI, KAGEMUSHA_ROLLOUT_MODULE,
        '// rollout module detached',
        'authenticate the rollout module wiring',
        D_ROLLOUT_CLI),
    (KAGEMUSHA_ROLLOUT_COMPONENT, KAGEMUSHA_ROLLOUT_LIVENESS_MODULE,
        '// validator-liveness module detached',
        'authenticate the validator-liveness module wiring',
        ('mod liveness;',)),
    (OFFLINE_CLI, '#[command(name = "rollout-v4")]',
        '#[command(name = "unchecked-rollout-v4")]',
        'reject substituted rollout-v4 CLI wiring',
        D_ROLLOUT_CLI),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'matches!(&self.command, Command::CreateExpectations(_))',
        'matches!(&self.command, Command::Submit(_))',
        'reject fallback credentials outside expectations creation',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::Submit(args) => args.run(context),',
        '            Command::Submit(args) => Command::CreateExpectations(args).run(context),',
        'reject collapsed rollout phase dispatch',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::SubmitCanaryAuthorization(args) => args.run(context),',
        '            Command::SubmitCanaryAuthorization(args) => Command::SubmitCanary(args).run(context),',
        'reject detached canary-reservation dispatch',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::SubmitCanary(args) => args.run(context),',
        '            Command::SubmitCanary(args) => Command::Submit(args).run(context),',
        'reject detached canary submission dispatch',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::FinalizeCanaryEvidence(args) => args.run(context),',
        '            Command::FinalizeCanaryEvidence(args) => Command::FinalizeReceipt(args).run(context),',
        'reject detached canary-finalization dispatch',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::FinalizeValidatorLiveness(args) => args.run(context),',
        '            Command::FinalizeValidatorLiveness(args) => Command::FinalizeCanaryEvidence(args).run(context),',
        'reject detached validator-liveness dispatch',
        D_ROLLOUT_PHASES),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'const ROLLOUT_STATE_ROOT: &str = "/var/lib/iroha/kagemusha-rollout-v1";',
        'const ROLLOUT_STATE_ROOT: &str = "/tmp/iroha-kagemusha-rollout-v1";',
        'reject an ambient rollout state root',
        ('/var/lib/iroha/kagemusha-rollout-v1',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'const SUBMISSION_JOURNAL_FILE_NAME: &str = "activation-submission-journal-v1.norito";',
        'const SUBMISSION_JOURNAL_FILE_NAME: &str = "activation-submission-digest-v1.norito";',
        'reject a substituted fixed submission-journal leaf',
        ('activation-submission-journal-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'const CANARY_AUTHORIZATION_FILE_NAME: &str = "canary-authorization-v1.norito";',
        'const CANARY_AUTHORIZATION_FILE_NAME: &str = "canary-authorization-v0.norito";',
        'reject a substituted canary-authorization leaf',
        ('canary-authorization-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    "canary-authorization-submission-journal-v1.norito";',
        '    "canary-authorization-submission-digest-v1.norito";',
        'reject a substituted canary-authorization journal leaf',
        ('canary-authorization-submission-journal-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'const CANARY_SUBMISSION_JOURNAL_FILE_NAME: &str = "canary-submission-journal-v1.norito";',
        'const CANARY_SUBMISSION_JOURNAL_FILE_NAME: &str = "canary-submission-digest-v1.norito";',
        'reject a substituted canary-journal leaf',
        ('canary-submission-journal-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'const CANARY_EVIDENCE_FILE_NAME: &str = "canary-evidence-v1.norito";',
        'const CANARY_EVIDENCE_FILE_NAME: &str = "activation-evidence-v1.norito";',
        'reject a substituted canary-evidence leaf',
        ('canary-evidence-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    "post-canary-validator-liveness-challenge-v1.norito";',
        '    "post-canary-validator-liveness-challenge-v0.norito";',
        'reject a substituted validator-liveness challenge leaf',
        ('post-canary-validator-liveness-challenge-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    "post-canary-validator-liveness-evidence-v1.norito";',
        '    "post-canary-validator-liveness-evidence-v0.norito";',
        'reject a substituted validator-liveness evidence leaf',
        ('post-canary-validator-liveness-evidence-v1.norito',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '#[arg(long, required = true, action = clap::ArgAction::SetTrue)]',
        '#[arg(long, action = clap::ArgAction::SetTrue)]',
        'reject an optional --write-authorized flag',
        ('required activation --write-authorized CLI flag',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        if !self.write_authorized {', '        if false {',
        'reject submission without runtime write authorization',
        ('explicit write authorization before rollout submission state access',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            && metadata.mode() & 0o7777 == 0o600',
        '            && metadata.mode() & 0o7777 == 0o644',
        'reject a non-private rollout signing key',
        ('stable root-owned mode-0600 private-key custody',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW',
        'rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC',
        'reject a follow-capable trusted rollout read',
        D_ROOT_READ),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    require_no_xattrs(&file, label)?;\n    require_no_macos_acl(&file, label)?;\n    let mut bytes',
        '    require_no_xattrs(&file, label)?;\n    // ACL check removed\n    let mut bytes',
        'reject a trusted rollout read without its ACL check',
        D_ROOT_READ),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        rustix::fs::RenameFlags::NOREPLACE,',
        '        rustix::fs::RenameFlags::empty(),',
        'reject replace-capable rollout publication',
        D_ROOT_PUBLISH),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        staging.sync_all().map_err(|error| error.to_string())?;',
        '        // pre-rename staging fsync removed',
        'reject rollout publication without a pre-rename fsync',
        D_ROOT_PUBLISH),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'KagemushaV4ActivationReceiptExpectationsArtifactV1::try_sign(body, &controller_key)',
        'KagemushaV4ActivationReceiptExpectationsArtifactV1::new_unchecked(body, &controller_key)',
        'reject unsigned activation expectations',
        D_DEFERRED_EXPECTATIONS),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            .verify_exact(&bytes, &controller, &reservation_bytes)',
        '            .verify(&controller)',
        'reject expectations publication without exact reverification',
        D_DEFERRED_EXPECTATIONS),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published expectations report failed: {error}"),',
        '                detail: format!("ordinary expectations output failure: {error}"),',
        'reject ordinary-error reporting after expectations publication',
        ('commit-uncertain expectations publication reporting',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if bytes != loaded.exact_bytes {', '    if false {',
        'reject a digest-only or mismatched submission journal',
        D_EXPECTATIONS_JOURNAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    publish_root_owned(path, &loaded.exact_bytes, |published| {',
        '    publish_root_owned(path, b"digest-only", |published| {',
        'reject a journal that does not durably publish exact expectations',
        D_EXPECTATIONS_JOURNAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        (SubmissionJournalObservation::Absent, true) => {',
        '        (SubmissionJournalObservation::Absent, false) => {',
        'reject retrospective submission without a journal',
        D_SAFE_RESUME),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        (SubmissionJournalObservation::Matching, _) => Ok(SubmissionJournalAction::Resume),',
        '        (SubmissionJournalObservation::Matching, _) => Ok(SubmissionJournalAction::Publish),',
        'reject destructive publication on matching-journal resume',
        D_SAFE_RESUME),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            publish_submission_journal(&journal_path, &loaded)?;',
        '            // journal publication removed',
        'reject POST reachability before durable journal publication',
        ('durable exact journal and status reconciliation before every POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            require_journal_bound_status_response(\n                status,\n                transaction,\n                &journal_path,\n                "pre-submit status identity reconciliation",\n            )?;',
        '            require_status_response_hash(status, transaction)?;',
        'reject an unbound matching-journal status identity failure',
        ('every submit status identity path must be journal-bound',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    require_status_response_hash(status, transaction).map_err(|error| {',
        '    require_status_response_hash(status, transaction).map_err(eyre::Report::from)?;',
        'reject ordinary status mismatch errors after journal publication',
        D_STATUS_IDENTITY),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            timeout: client.transaction_status_timeout,',
        '            timeout: TransactionWaitOptions::default().timeout,',
        'reject a configured status timeout substitution',
        ('configured status timeout and exact terminal status set',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                    stage: "proof-anchored Applied result reporting",',
        '                    stage: "ordinary output failure",',
        'reject ordinary-error reporting after proof-anchored Applied',
        ('every activation and canary Applied reporting path must be submission-uncertain',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            stage: "configured terminal wait",',
        '            stage: "ordinary wait failure",',
        'reject ordinary failure after configured wait ambiguity',
        D_SUBMIT_UNCERTAIN, -1),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    require_journal_bound_wait_outcome(&outcome, transaction, journal_path)?;',
        '    require_unbound_wait_outcome(&outcome, transaction)?;',
        'reject an unbound terminal wait outcome',
        D_SUBMIT_UNCERTAIN),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if outcome.terminal_kind != outcome.r#final.status.kind',
        '    if false',
        'reject inconsistent terminal wait summary and final status',
        D_STATUS_IDENTITY),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            require_journal_bound_status_response(\n                &status,\n                transaction,\n                journal_path,\n                "failed wait status identity reconciliation",\n            )?;',
        '            require_status_response_hash(&status, transaction)?;',
        'reject an unbound failed-wait status hash',
        ('proof or explicit submission uncertainty after failed wait reconciliation',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    let proof_anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(',
        '    let proof_anchor = UntrustedBlockHint::from_untrusted_finality_artifact(',
        'reject Applied acceptance without a trusted block proof anchor',
        D_BLOCK_ENTRYPOINT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        || decode_framed_signed_block(&block_bytes),',
        '        || decode_unframed_block(&block_bytes),',
        'reject noncanonical finalized SignedBlock wire',
        D_BLOCK_ENTRYPOINT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        || context.execution_policy_hash != expectations.binding().execution_policy_hash',
        '        || false',
        'reject a finality chain detached from the execution policy',
        ('exact four-validator DA Nexus PoP and execution-policy corridor',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if committed_wire != exact_wire {', '    if false {',
        'reject a hash-equivalent authorization-wire splice',
        ('exact authorization-bearing external transaction wire comparison',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            load_root_custodied_key(&self.issuer_private_key_file, "receipt-issuer key")?;',
        '            load_operator_key_pair(&self.issuer_private_key_file)?;',
        'reject an issuer key outside deferred root custody',
        D_FINAL_RECEIPT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        require_matching_submission_journal(inspect_submission_journal(&journal_path, &loaded)?)?;',
        '        // exact matching submission journal check removed',
        'reject finalization without the exact submission journal',
        D_FINAL_RECEIPT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        require_journal_bound_status_response(\n            &status,\n            transaction,\n            &journal_path,\n            "finalize status identity reconciliation",\n        )?;',
        '        require_status_response_hash(&status, transaction)?;',
        'reject unbound status identity during receipt finalization',
        D_FINAL_RECEIPT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(published)',
        '            let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_unchecked(published)',
        'reject a final receipt without canonical publication readback',
        D_FINAL_RECEIPT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published receipt report failed: {error}"),',
        '                detail: format!("ordinary receipt output failure: {error}"),',
        'reject ordinary-error reporting after final receipt publication',
        ('commit-uncertain final-receipt publication reporting',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    /// Explicit authorization for this production canary network write.\n    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]',
        '    /// Explicit authorization for this production canary network write.\n    #[arg(long, action = clap::ArgAction::SetTrue)]',
        'reject an optional production-canary write authorization',
        D_WRITE_AUTH),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        if !self.write_authorized {\n            bail!("--write-authorized is required for production canary submission");',
        '        if false {\n            bail!("--write-authorized is required for production canary submission");',
        'reject production canary submission without runtime write authorization',
        D_WRITE_AUTH),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        preflight_root_owned_output(&self.output)?;\n\n        let ttl_ms = self.canary_ttl_ms.get();',
        '        // authorization destination preflight removed\n\n        let ttl_ms = self.canary_ttl_ms.get();',
        'reject sensitive authorization work before output preflight',
        D_EXACT_PERMIT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        client.add_transaction_nonce = true;',
        '        client.add_transaction_nonce = false;',
        'reject canary authorization without a transaction nonce',
        D_EXACT_PERMIT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let canonical_torii_origin = canonical_torii_origin(&client.torii_url)?;',
        '        let canonical_torii_origin = client.torii_url.to_string();',
        'reject a controller authorization with an uncanonical Torii origin',
        D_EXACT_PERMIT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let controller_key = load_root_custodied_key(\n            &self.controller_private_key_file,\n            "promotion-controller key",',
        '        let controller_key = load_operator_key_pair(\n            &self.controller_private_key_file,\n            "promotion-controller key",',
        'reject controller signing outside deferred root custody',
        D_EXACT_PERMIT),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    let verification_time_unix_ms = artifact.permit().body.authorized_at_unix_ms;',
        '    let verification_time_unix_ms = current_unix_ms()?;',
        'reject structural authorization reconciliation after wall expiry',
        D_STRUCTURAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if bytes != authorization.exact_bytes {', '    if false {',
        'reject a digest-only canary submission journal',
        D_STRUCTURAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                    .get_transaction_status_response_auto(transaction.hash())\n                    .wrap_err(',
        '                    .get_transaction_status_response_auto(authorization.verified.canary_transaction().hash())\n                    .wrap_err(',
        'reject canary-journal publication without proof that the reservation is absent',
        D_CANARY_JOURNAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                    .get_transaction_status_response_auto(\n                        authorization.verified.canary_transaction().hash(),\n                    )',
        '                    .get_transaction_status_response_auto(transaction.hash())',
        'reject canary-journal publication without proof that the canary is absent',
        D_CANARY_JOURNAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let verification_time_unix_ms = current_unix_ms().map_err(|error| error.to_string())?;',
        '        let verification_time_unix_ms = authorization.verified.authorized_at_unix_ms();',
        'reject stale authorization at journal publication',
        D_STRUCTURAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            let fresh_time = current_unix_ms()?;\n            authorization',
        '            let fresh_time = authorization.verified.authorized_at_unix_ms();\n            authorization',
        'reject stale authorization immediately before POST',
        ('precommitted exact journal and fresh verification before canary POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                publish_canary_submission_journal(\n                    &canary_journal_path,',
        '                // durable canary journal publication removed\n                let _ = &canary_journal_path;',
        'reject canary POST reachability before durable exact journal publication',
        D_CANARY_JOURNAL),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        || canonical_torii_origin(&client.torii_url)?\n            != authorization.verified.canonical_torii_origin()',
        '        || false',
        'reject canary submission through a different origin',
        ('exact canary network authority and HTTPS origin client binding',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            expectations.binding().promotion_id,\n            CANARY_EVIDENCE_FILE_NAME,',
        '            [0; 32],\n            CANARY_EVIDENCE_FILE_NAME,',
        'reject a canary evidence leaf detached from its promotion',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            CANARY_EVIDENCE_FILE_NAME,\n        )?;\n        preflight_root_owned_output(&self.output)?;\n        let journal_path = rollout_state_path(',
        '            CANARY_EVIDENCE_FILE_NAME,\n        )?;\n        // evidence destination preflight removed\n        let journal_path = rollout_state_path(',
        'reject live canary queries before evidence output preflight',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let pipeline_status = client\n            .get_transaction_status_response_auto(transaction.hash())',
        '        let pipeline_status = client\n            .get_transaction_status_response_auto(expectations.activation_transaction_intent())',
        'reject activation-status substitution for canary status',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        if pipeline_status.status.kind != "Applied"\n            || pipeline_status.scope != "global"',
        '        if pipeline_status.status.kind != "Applied"\n            || false',
        'reject local canary pipeline status',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            committed_transaction: fresh.committed.clone(),',
        '            committed_transaction: activation_committed.clone(),',
        'reject digest-only evidence without the full committed canary',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            finality_proof_chain: finality_proof_chain.clone(),',
        '            finality_proof_chain: receipt.body.finality_proof_chain.clone(),',
        'reject canary evidence without its post-receipt proof extension',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let evidence = KagemushaV4TairaCanaryEvidenceV1::try_sign(',
        '        let evidence = KagemushaV4TairaCanaryEvidenceV1::new_unchecked(',
        'reject unsigned full canary evidence',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        publish_root_owned(&self.output, &bytes, |published| {\n            let evidence = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(published)',
        '        fs::write(&self.output, &bytes)?;\n        {\n            let evidence = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(&bytes)',
        'reject replace-capable canary publication',
        D_BLOCK_PROOF),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published canary-authorization report failed: {error}"),',
        '                detail: format!("ordinary authorization output failure: {error}"),',
        'reject ordinary-error reporting after authorization publication',
        ('canary authorization no-replace commit-uncertain reporting',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published canary-evidence report failed: {error}"),',
        '                detail: format!("ordinary canary output failure: {error}"),',
        'reject ordinary-error reporting after canary publication',
        ('canary evidence no-replace commit-uncertain reporting',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || verified_canary.activation_expectations_artifact()\n                != expectations.activation_expectations_artifact()',
        '            || false',
        'reject liveness under a different activation-expectations artifact',
        D_EXPECTATIONS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || canary_anchor.activation_finality_receipt\n                != verified_canary.activation_finality_receipt()',
        '            || false',
        'reject a liveness anchor detached from the exact activation receipt',
        D_EXPECTATIONS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || canary_anchor.canary_transaction_wire != verified_canary.canary_transaction_wire()',
        '            || false',
        'reject a liveness anchor detached from the exact canary wire',
        D_EXPECTATIONS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        'previous.is_some_and(|id: &PeerId| id >= &target.validator_id)',
        'previous.is_some_and(|id: &PeerId| id > &target.validator_id)',
        'reject duplicate validator identities in the liveness challenge',
        ('issuer-signed fresh canary challenge with four distinct qualified targets',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '        attestation\n            .verify()',
        '        attestation\n            .verify_structure_only()',
        'reject unsigned validator liveness attestations',
        D_VALIDATOR_TIPS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || attestation_body.node_id != trust.validator_ids[index]',
        '            || false',
        'reject a validator attestation under the wrong node identity',
        D_VALIDATOR_TIPS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '        if tip != expected {', '        if false {',
        'reject validator tips detached from the shared verified chain',
        D_VALIDATOR_TIPS),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '        || context.da_layout != runtime.genesis_context.da_layout',
        '        || false',
        'reject liveness finality under a different DA layout',
        ('liveness canary anchor and exact four-validator DA Nexus PoP corridor',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        activation_finality_receipt: KagemushaExactBytesDigestV1::from_bytes(&receipt.exact_bytes)?,',
        '        activation_finality_receipt: KagemushaExactBytesDigestV1::from_bytes(&authorization.exact_bytes)?,',
        'reject a CLI liveness anchor detached from the activation receipt',
        ('liveness anchor derived only from exact verified canary evidence',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        'fn collect_validator_observations(\n    http: &DirectLivenessHttp,',
        'fn collect_validator_observations(\n    _ambient_client: &Client,\n    http: &DirectLivenessHttp,',
        'reject an ambient primary Client in direct validator collection',
        ('ambient Client enters direct validator collection',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        ') -> Result<KagemushaV4PostCanaryValidatorLivenessObservationV1> {\n    loop {',
        ') -> Result<KagemushaV4PostCanaryValidatorLivenessObservationV1> {\n    let _ambient_status = http.client.get_status();\n    loop {',
        'reject ambient primary-client status access in direct validator collection',
        ('direct validator collection transport isolation',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '    let status_timeout = timeout.min(STATUS_HINT_TIMEOUT);',
        '    let status_timeout = STATUS_HINT_TIMEOUT;',
        'reject expansion of a smaller configured validator status timeout',
        ('configured-or-60s direct client with non-expanding status timeout',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        .header(ACCEPT, APPLICATION_JSON)',
        '        .header(ACCEPT, APPLICATION_JSON)\n        .header("authorization", "ambient")',
        'reject extra credentials on the direct validator status request',
        ('direct validator status requires exact two protocol headers',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        'const STATUS_HINT_MAX_BYTES: usize = 32;',
        'const STATUS_HINT_MAX_BYTES: usize = 33;',
        'reject a widened validator status response ceiling',
        ('bounded identity-encoded direct validator status response',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '    if canonical.as_bytes() != exact_bytes {', '    if false {',
        'reject a noncanonical direct validator status scalar',
        ('direct validator status bounded exact canonical scalar',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '    http.get(url)\n        .header(ACCEPT, APPLICATION_NORITO)',
        '    http.get(url)\n        .headers(HeaderMap::new())\n        .header(ACCEPT, APPLICATION_NORITO)',
        'reject inherited primary-client credentials at direct validator attestation origins',
        ('direct validator attestation credential isolation',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        .header(FINALITY_CHALLENGE_HEADER, hex::encode(challenge))',
        '        .header(FINALITY_CHALLENGE_HEADER, hex::encode(challenge))\n        .header("authorization", "ambient")',
        'reject extra credentials on the direct validator attestation request',
        ('direct validator attestation requires exact three protocol headers',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        .no_proxy()', '        .use_preconfigured_proxy()',
        'reject proxy-routed validator-liveness collection',
        ('direct common-challenge collection with exact canonical attestations',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        .header(FINALITY_CHALLENGE_HEADER, hex::encode(challenge))',
        '        .header(FINALITY_CHALLENGE_HEADER, "detached")',
        'reject a validator request detached from the shared challenge',
        ('direct validator attestation exact three protocol headers',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        .take(u64::try_from(maximum)?.saturating_add(1))',
        '        .take(u64::try_from(maximum)?)',
        'reject an attestation reader that cannot detect one-byte overflow',
        ('bounded identity-encoded no-store attestation response',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        let proof = client.get_next_bridge_finality_proof(height, &mut verifier)?;',
        '        let proof = client.get_bridge_finality_proof(height)?;',
        'reject a non-contiguous unverified shared finality chain',
        ('canary-anchored contiguous shared finality collection',)),
    (KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
        '        let evidence = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::try_sign(',
        '        let evidence = KagemushaV4PostCanaryValidatorLivenessEvidenceV1::new_unchecked(',
        'reject unsigned validator-liveness evidence',
        ('post-canary four-validator liveness phase and no-replace evidence',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        "--kagami",\n        "--kagami-sha256",',
        '        "--kagami-sha256",\n        "--kagami",',
        'reject reordered publisher controller arguments',
        ('exact ordered promotion-controller argument contract',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        .arg("promote-release-v4")',
        '        .arg("verify-release-v4")',
        'reject a substituted Kagami promotion command',
        ('exact sandboxed Kagami promote-release-v4 argument order',)),
    (KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        '        if effective_uid() != 0', '        if false',
        'reject removal of effective-root authentication',
        ('real and effective non-set-id root identity',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '    kagemusha_python_launcher::require_macos_tcb(&request.expected_macos_build)?;',
        '    // macOS build and OS TCB pin removed',
        'reject a promotion without its macOS TCB pin',
        D_ROOT_TCB),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        .and_then(fs::canonicalize)',
        '        .map(PathBuf::from)',
        'reject an uncanonicalized controller executable identity',
        D_ROOT_TCB),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '    let mut kagami_pin = kagemusha_python_launcher::pin_regular(&kagami, request.kagami_sha256)?;',
        '    let mut kagami_pin = kagemusha_python_launcher::pin_regular(&kagami, [0; 32])?;',
        'reject Kagami detached from its requested digest pin',
        D_ROOT_TCB),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        'const SNAPSHOT_PARENT: &str = "/private/var/db/iroha-kagemusha-promotion-v1";',
        'const SNAPSHOT_PARENT: &str = "/tmp/iroha-kagemusha-promotion-v1";',
        'reject an ambient promotion executable snapshot',
        ('SNAPSHOT_PARENT',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '(allow file-write* (require-all (vnode-type REGULAR-FILE) (regex {temporary})))',
        '(allow file-write* (regex {temporary}))',
        'reject Seatbelt special-file creation',
        ('vnode-type REGULAR-FILE',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        'OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC',
        'OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC',
        'reject blocking promotion inventory opens',
        ('nonblocking regular-only descriptor open before bounded inspection',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        validate_bounded_identity(name, &before_identity, bounds)?;',
        '        // pre-read descriptor bounds check removed',
        'reject hashing before metadata and size validation',
        D_BOUNDED_DESCRIPTOR),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        validate_bounded_identity(name, &after, bounds)?;',
        '        // post-read descriptor bounds check removed',
        'reject missing post-hash metadata and size validation',
        D_BOUNDED_DESCRIPTOR),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        'const CANDIDATE_FILES: [CandidateFileSpec; 17] = [',
        'const CANDIDATE_FILES: [CandidateFileSpec; 16] = [',
        'reject a sixteen-file promotion candidate',
        ('CANDIDATE_FILES', 'candidate inventory declaration is not exact seventeen')),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        name: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,',
        '        name: "detached-internal-validation-receipt.norito",',
        'reject a candidate inventory without the internal-validation receipt',
        ('exact candidate inventory entry changed',)),
    (READINESS,
        '    "internal-validation-receipt-v1.norito",\n',
        '',
        'reject a readiness inventory without the internal-validation receipt',
        ('18-file readiness inventory with bounded internal-validation receipt',)),
    (READINESS,
        '    (\n        "internal-validation-receipt-v1.norito",\n        MAX_INTERNAL_VALIDATION_RECEIPT_BYTES,\n    ),\n',
        '',
        'reject an unbounded internal-validation receipt',
        ('bounded opaque internal-validation receipt staging',)),
    (READINESS,
        '        raise ValueError("Kagami verified a different internal-validation receipt")',
        '        pass',
        'reject a verifier report that ignores the staged internal-validation receipt',
        ('internal-validation report digest binding',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '[(name, identity)] if valid_temp_name(name) => {',
        '[(name, identity)] if name.starts_with(TEMP_PREFIX) => {',
        'reject a non-exact temporary publication leaf',
        ('exact candidate, one-temporary, or one-final inventory state machine',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '            let (_, identity) = open_member(&self.directory, &name, bounds, hash_contents)?;',
        '            let identity = self.initial_identities()[&name].clone();',
        'reject an uninspected temporary or final inventory member',
        ('regular-only bounded temporary and final inventory inspection',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '    if payload != canonical.as_bytes() {', '    if false {',
        'reject a noncanonical or differently typed promotion report',
        ('single exact canonical typed promotion-report JSON line',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        if manifest_identity.sha256 != envelope_sha256',
        '        if false',
        'reject a report detached from the canonical manifest envelope',
        ('canonical manifest envelope, sidecar, and typed JSON cross-binding',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '            generation: manifest.generation.clone(),',
        '            generation: "detached-generation".to_owned(),',
        'reject a report scalar detached from the canonical manifest',
        ('fully candidate-bound canonical report field projection',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '                payload_sha256: Some(hex(&descriptor.payload_sha256)),',
        '                payload_sha256: Some(hex(&descriptor.sha256)),',
        'reject an artifact payload digest detached from the manifest descriptor',
        ('exact ordered manifest artifact and payload projection',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        'const COMMIT_UNCERTAIN_EXIT: u8 = 75;',
        'const COMMIT_UNCERTAIN_EXIT: u8 = 74;',
        'reject a nonstandard controller commit-uncertain exit',
        ('COMMIT_UNCERTAIN_EXIT',)),
    (KAGAMI, 'RenameFlags::NOREPLACE,', 'RenameFlags::empty(),',
        'reject replace-capable promotion-record publication',
        D_PROMOTION_PUBLISH),
    (KAGAMI, '.is_some_and(|delta| delta <= 1)', '.is_some_and(|_| true)',
        'reject an unbounded staged-parent link transition',
        D_PROMOTION_PUBLISH),
    (KAGAMI, '.is_some_and(|delta| delta <= 2)', '.is_some_and(|_| true)',
        'reject an unbounded two-leaf staging transition', D_PROMOTION_PUBLISH),
    (KAGAMI, '.checked_sub(staging_snapshot.links)',
        '.checked_sub(complete_staging_snapshot.links)',
        'reject a decreasing two-leaf staging transition', D_PROMOTION_PUBLISH),
    (KAGAMI,
        '            complete_staging_snapshot,\n            &linked,',
        '            staging_snapshot,\n            &linked,',
        'reject a detached complete-staging binding', D_PROMOTION_PUBLISH),
    (KAGAMI, 'original_expected.matches_except_links(current) && current.links > 0',
        'current.links > 0', 'reject an ancestor identity bypass',
        D_PROMOTION_PUBLISH),
    (KAGAMI, 'match parent.snapshot_after_one_staged_entry("a promotion-record file") {',
        'match Ok(parent.snapshot) {',
        'reject a promotion-record file detached from its staged-parent snapshot',
        D_PROMOTION_PUBLISH),
    (KAGAMI, 'match parent.snapshot_after_one_staged_entry("a circuit-parameter directory") {',
        'match Ok(parent.snapshot) {',
        'reject a circuit directory detached from its staged-parent snapshot',
        D_PROMOTION_PUBLISH),
    (KAGAMI, '        RenameFlags::NOREPLACE,\n    ) {\n        let rename_error =\n            eyre!(error).wrap_err("failed to atomically publish circuit-parameter directory");',
        '        RenameFlags::empty(),\n    ) {\n        let rename_error =\n            eyre!(error).wrap_err("failed to atomically publish circuit-parameter directory");',
        'reject replace-capable circuit-directory publication', D_PROMOTION_PUBLISH),
    (KAGAMI,
        'write_new_durable_file_with_hooks_v1(path, bytes, || Ok(()), File::sync_all)',
        'write_new_durable_file_with_hooks_v1(path, bytes, || Ok(()), |_| Ok(()))',
        'reject a promotion-record production fsync bypass', D_PROMOTION_PUBLISH),
    (KAGAMI,
        'write_release_circuit_params_directory_with_hooks_v1(path, bytes, || Ok(()), File::sync_all)',
        'write_release_circuit_params_directory_with_hooks_v1(path, bytes, || Ok(()), |_| Ok(()))',
        'reject a circuit-directory production fsync bypass', D_PROMOTION_PUBLISH),
    (KAGAMI,
        '    file.sync_all()\n        .wrap_err_with(|| format!("failed to sync staged `{file_name}`"))?;',
        '    // staged circuit leaf fsync removed',
        'reject a circuit leaf without pre-rename durability', D_PROMOTION_PUBLISH),
    (KAGAMI,
        '        staging\n            .sync_all()\n            .wrap_err("failed to sync complete circuit-parameter staging directory")?;',
        '        // staged circuit directory fsync removed',
        'reject a circuit directory without pre-rename durability', D_PROMOTION_PUBLISH),
    (KAGAMI, '        sync_parent(&parent.file).wrap_err("failed to durably sync promotion-record parent")?;',
        '        // promotion-record parent durability removed',
        'reject promotion-record publication without parent durability',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        '        sync_parent(&parent.file)\n            .wrap_err("failed to durably sync circuit-parameter parent directory")?;',
        '        // circuit-parameter parent durability removed',
        'reject circuit-directory publication without parent durability',
        D_PROMOTION_PUBLISH),
    (KAGAMI, 'fn release_circuit_params_file_snapshot_matches_stat_v1(\n    snapshot: PromotionFileSnapshotV1,\n    stat: &rustix::fs::Stat,\n) -> bool {\n    stat_field_matches_v1(stat.st_dev, snapshot.device)\n        && stat_field_matches_v1(stat.st_ino, snapshot.inode)',
        'fn release_circuit_params_file_snapshot_matches_stat_v1(\n    snapshot: PromotionFileSnapshotV1,\n    stat: &rustix::fs::Stat,\n) -> bool {\n    stat_field_matches_v1(stat.st_dev, snapshot.device)\n        && true // inode identity bypass',
        'reject a published-file inode identity bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, '&& stat_field_matches_v1(stat.st_mode, snapshot.mode)',
        '&& true // custody mode bypass',
        'reject a published-file custody bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, '&& stat_field_matches_v1(stat.st_size, snapshot.length)',
        '&& true // length bypass',
        'reject a published-file length bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, 'OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC',
        'OFlags::RDONLY | OFlags::CLOEXEC',
        'reject a follow-capable published-file verification', D_PROMOTION_PUBLISH),
    (KAGAMI, 'if opened_snapshot != Some(snapshot) {', 'if false {',
        'reject a reopened-file identity bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, 'if actual != expected_bytes {', 'if false {',
        'reject a published-file content bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, 'if opened_after != Some(snapshot)', 'if false',
        'reject a post-read file identity bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, 'if opened != Some(directory_snapshot)', 'if false',
        'reject a circuit-directory identity bypass', D_PROMOTION_PUBLISH),
    (KAGAMI, 'if opened_after != Some(directory_snapshot)', 'if false',
        'reject a post-read circuit-directory identity bypass', D_PROMOTION_PUBLISH),
    (KAGAMI,
        '                snapshot,\n                bytes,\n                "staged promotion record",',
        '                snapshot,\n                &[],\n                "staged promotion record",',
        'reject staged promotion content verification against detached bytes',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        '            snapshot,\n            bytes,\n            "durably published promotion record",',
        '            snapshot,\n            &[],\n            "durably published promotion record",',
        'reject durable promotion content verification against detached bytes',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        '            ep_snapshot,\n            bytes,\n            "staged circuit-parameter",',
        '            ep_snapshot,\n            &[],\n            "staged circuit-parameter",',
        'reject staged circuit content verification against detached bytes',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        '            ep_snapshot,\n            bytes,\n            "published circuit-parameter",',
        '            ep_snapshot,\n            &[],\n            "published circuit-parameter",',
        'reject published circuit content verification against detached bytes',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        '            ep_snapshot,\n            bytes,\n            "durably published circuit-parameter",',
        '            ep_snapshot,\n            &[],\n            "durably published circuit-parameter",',
        'reject durable circuit content verification against detached bytes',
        D_PROMOTION_PUBLISH),
    (KAGAMI,
        'Err(error) => DurableFilePublicationOutcomeV1::CommitUncertain {\n            final_path,\n            reason: format!("{error:#}"),',
        'Err(error) => DurableFilePublicationOutcomeV1::CommitUncertain {\n            final_path,\n            reason: error.to_string(),',
        'reject flattened promotion commit-uncertain diagnostics', D_PROMOTION_PUBLISH),
    (KAGAMI,
        'Err(error) => ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain {\n            final_path,\n            reason: format!("{error:#}"),',
        'Err(error) => ReleaseCircuitParamsPublicationOutcomeV1::CommitUncertain {\n            final_path,\n            reason: error.to_string(),',
        'reject flattened circuit commit-uncertain diagnostics', D_PROMOTION_PUBLISH),
    (KAGAMI, '    ) && staging == Some(NoReplaceRenameNameStateV1::Owned)', '    ) || staging == Some(NoReplaceRenameNameStateV1::Owned)', 'reject an unsafe failed-rename pre-commit classification', ('failed no-replace publication reconciliation',)),
    (KAGAMI, '    if disposition == FailedNoReplaceRenameDispositionV1::PreCommit {', '    if true {', 'reject cleanup after commit-uncertain rename evidence', ('failed no-replace publication reconciliation',)),
    (KAGAMI, '        return match cleanup_exact_owned_staging() {', '        return match Ok(()) {', 'reject pre-commit without exact staging cleanup', ('failed no-replace publication reconciliation',)),
    (KAGAMI, 'statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)', 'statat(parent, name, rustix::fs::AtFlags::empty())', 'reject follow-capable failed-rename inspection', ('failed no-replace publication reconciliation',)),
    (KAGAMI, '|stat| promotion_file_snapshot_has_stat_identity_v1(snapshot, stat),', '|_| true,', 'reject detached promotion destination reconciliation', ('promotion-record rename-error reconciliation',)),
    (KAGAMI, '|stat| release_circuit_params_file_snapshot_matches_stat_v1(snapshot, stat)', '|stat| promotion_file_snapshot_has_stat_identity_v1(snapshot, stat)', 'reject promotion staging reconciliation without full custody', ('promotion-record rename-error reconciliation',)),
    (KAGAMI, '|stat| {\n                promotion_directory_snapshot_has_stat_identity_v1(complete_staging_snapshot, stat)\n            },', '|_| true,', 'reject detached circuit destination reconciliation', ('circuit-directory rename-error reconciliation',)),
    (KAGAMI, '                release_circuit_params_directory_snapshot_matches_stat_v1(\n                    complete_staging_snapshot,\n                    stat,\n                )', '                true', 'reject circuit staging reconciliation without full custody', ('circuit-directory rename-error reconciliation',)),
    (KAGAMI, '                parent.verify_path_identity_against(publication_parent_snapshot)?;\n                verify_pinned_file_contents_v1(', '                verify_pinned_file_contents_v1(', 'reject promotion cleanup through a substituted parent', ('promotion-record rename-error reconciliation',)),
    (KAGAMI, '                parent.verify_path_identity_against(publication_parent_snapshot)?;\n                verify_release_circuit_params_directory_contents_v1(', '                verify_release_circuit_params_directory_contents_v1(', 'reject circuit cleanup through a substituted parent', ('circuit-directory rename-error reconciliation',)),
    (KAGAMI, 'const DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE: u8 = 75;',
        'const DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE: u8 = 74;',
        'reject a nonstandard Kagami commit-uncertain exit',
        ('durability-uncertain Kagami exit 75',)),
    (KAGAMI, '                            &mut std::io::sink(),',
        '                            writer,',
        'reject a preliminary committed-record stdout line',
        ('verify-publish-verify promotion with one final canonical stdout JSON',)),
    (PROMOTION_WORKFLOW,
        'name: Verify Kagemusha V4 production readiness (publication blocked)',
        'name: Verify Kagemusha V4 production readiness',
        'reject an unblocked Kagemusha promotion workflow',
        ('name: Verify Kagemusha V4 production readiness (publication blocked)',)),
    (KAGAMI,
        'if inventory_state.includes_promotion_record() && expected.len() != 18',
        'if inventory_state.includes_promotion_record() && expected.len() != 17',
        'reject a seventeen-file final release verifier'),
    (KAGAMI, '        (\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n            "qualification receipt",\n        ),\n',
        '', 'reject a verifier inventory without the qualification receipt', ('18-file verifier inventory',)),
    (KAGAMI, '        (\n            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,\n            "internal-validation receipt",\n        ),\n',
        '', 'reject a verifier inventory without the internal-validation receipt', ('18-file verifier inventory',)),
    (BUNDLE, 'const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 18;', 'const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 17;', 'reject a seventeen-file final release producer'),
    (BUNDLE, '            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n            PROMOTION_RECORD_FILE_NAME_V4,\n',
        '            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,\n            PROMOTION_RECORD_FILE_NAME_V4,\n',
        'reject a producer inventory without the qualification receipt', ('function-scoped 18-file producer inventory',)),
    (BUNDLE, '            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,\n            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n',
        '            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n',
        'reject a producer inventory without the internal-validation receipt', ('function-scoped 18-file producer inventory',)),
    (BUNDLE, 'fn final_release_inventory_is_exact_and_includes_both_receipts()',
        'fn retired_final_release_inventory_test()', 'reject a missing producer inventory test',
        ('fn final_release_inventory_is_exact_and_includes_both_receipts()',)),
    (BUNDLE, 'include!("kagemusha_recursive_spend_v4_bundle/source_seal_build_inputs.rs");',
        '// reviewed source-seal build inputs removed',
        'reject a detached bundle build-input component',
        ('expected exactly one reviewed source-seal input include',)),
    (MODEL, 'KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 384 * 1024;',
        'KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 385 * 1024;',
        'reject qualification receipt bound drift', ('384 KiB absolute V4 proof-pair bound',)),
    (READINESS, '    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),\n)',
        '    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),\n    ("recursive-step-two-qualification-v4.norito", MAX_QUALIFICATION_RECEIPT_BYTES),\n)',
        'reject textual scanning of an opaque receipt', ('opaque qualification receipt is routed through textual evidence scanning',)),
    (PRIVACY_PROTOCOL, 'pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;', 'pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 21;', 'reject shared bridge ABI-21 substitution'),
    (PRIVACY, 'include!("privacy/protocol.rs");', '// protocol include removed', 'reject detached privacy protocol surface'),
    (MODEL, 'cfg!(feature = "kagemusha-production-enabled")', 'true', 'reject an invalid availability state'),
    (CATALOG, 'KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len();', '7;', 'reject a seven-artifact manifest check',
        ('exact-eight manifest inventory check',)),
    (CORE, '.is_some_and(|release| !release.issuance_active)', '.is_none_or(|release| !release.issuance_active)',
        'reject an unguarded offline-change issuance path', ('offline-change issuance window',)),
    (CORE, 'pub(crate) use isi::signed_kagemusha_taira_canary_wire_identity_v1;',
        'pub(crate) use isi::unchecked_kagemusha_taira_canary_wire_identity_v1;',
        'reject an unreachable signed-canary wire helper',
        ('pub(crate) use isi::signed_kagemusha_taira_canary_wire_identity_v1;',)),
    (CORE, CORE_ISI_TESTS_PARENT_INCLUDE,
        '// Kagemusha ISI test parent include removed',
        'reject a detached Kagemusha ISI test parent',
        (CORE_ISI_TESTS_PARENT_INCLUDE,)),
    (CORE_STATE_TESTS, CORE_AUTONOMOUS_MERGE_TESTS_PARENT_INCLUDE,
        '// autonomous merge test parent include removed',
        'reject a detached autonomous merge test parent',
        (CORE_AUTONOMOUS_MERGE_TESTS_PARENT_INCLUDE,)),
    (SCHEMA_GOLDEN, '"6ac84133729450e2392f324aae6d4e98".to_owned()',
        '"pending-exact-bytes-schema".to_owned()', 'reject a pending public schema golden',
        ('public schema golden contains pending placeholder',)),
    (CORE_ISI_TESTS, CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE,
        '// exact-wire context regressions detached',
        'reject a detached exact-wire boundary regression component',
        (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS_INCLUDE,)),
    (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        '        let error = ExecuteTrigger::new(trigger_id)',
        '        let error = unchecked_nested_trigger(trigger_id)',
        'reject loss of the hostile nested-trigger boundary regression',
        ('nested-trigger affine signed-wire rejection',)),
    (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        'fn taira_canary_sealed_reveal_validation_cannot_gain_external_provenance()',
        'fn unchecked_sealed_reveal_validation()',
        'reject loss of sealed-reveal validation regression',
        ('sealed-reveal validation rejects External canary provenance',)),
    (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        '    #[test]\n    fn taira_canary_committed_replay_seeds_only_one_direct_wire()',
        '    fn taira_canary_committed_replay_seeds_only_one_direct_wire()',
        'reject a non-test committed replay regression',
        ('External-only committed-replay exact-wire seeding boundaries',)),
    (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        '    #[test]\n    fn taira_canary_committed_replay_seeds_only_one_direct_wire()',
        '    #[test]\n    #[ignore]\n    fn taira_canary_committed_replay_seeds_only_one_direct_wire()',
        'reject an ignored committed replay regression',
        ('#[ignore]',)),
    (CORE_KAGEMUSHA_CANARY_CONTEXT_TESTS,
        '.with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)',
        '.with_admission_intent(TransactionAdmissionIntent::Ordinary)',
        'reject loss of non-Ordinary canary replay coverage',
        ('External-only committed-replay exact-wire seeding boundaries',)),
    (CORE_AUTONOMOUS_MERGE_TESTS,
        CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS_INCLUDE,
        '// autonomous merge admission-intent regressions detached',
        'reject a detached autonomous merge admission-intent regression component',
        (CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS_INCLUDE,)),
    (CORE_STATE,
        '            if source.input.entrypoints.iter().any(|entrypoint| {',
        '            if source.input.entrypoints.iter().all(|entrypoint| {',
        'reject Ordinary autonomous merge producer',
        ('QueuePlanSynced-only authenticated autonomous merge producer boundary',)),
    (CORE_STATE,
        '        if batch.lanes.iter().any(|execution| {',
        '        if false && batch.lanes.iter().any(|execution| {',
        'reject an Ordinary autonomous merge follower batch',
        ('commitment-checked QueuePlanSynced autonomous merge follower boundary',)),
    (CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS,
        'fn autonomous_merge_admission_intent_producer_rejects_ordinary_external_before_effects()',
        'fn unchecked_autonomous_merge_producer()',
        'reject loss of the autonomous merge producer no-effects regression',
        ('Ordinary autonomous merge producer no-effects regression',)),
    (CORE_AUTONOMOUS_MERGE_ADMISSION_INTENT_TESTS,
        '#[test]\nfn autonomous_merge_admission_intent_producer_rejects_ordinary_external_before_effects()',
        '#[cfg(any())]\n#[test]\nfn autonomous_merge_admission_intent_producer_rejects_ordinary_external_before_effects()',
        'reject a cfg-disabled autonomous merge regression',
        ('#[cfg',)),
    (IOS_EVIDENCE_MODULE,
        'CANDIDATE_XCODE_VERSION = "Xcode 26.6"',
        'CANDIDATE_XCODE_VERSION = "Xcode 26.2"',
        'reject candidate evidence from a different Xcode',
        ('CANDIDATE_XCODE_VERSION = "Xcode 26.6"',)),
    (WORKFLOW, 'ci/check_kagemusha_production_readiness_source_support.py',
        'ci/retired_kagemusha_production_readiness_source_support.py',
        'reject a missing readiness source-support workflow filter',
        ('ci/check_kagemusha_production_readiness_source_support.py',)),
    (WORKFLOW, 'ci/check_kagemusha_recursion_source_contract.py',
        'ci/retired_kagemusha_recursion_source_contract.py',
        'reject a missing recursion source-contract workflow filter',
        ('ci/check_kagemusha_recursion_source_contract.py',)),
    (WORKFLOW, 'ci/check_kagemusha_lifecycle_source_contract.py',
        'ci/retired_kagemusha_lifecycle_source_contract.py',
        'reject a missing lifecycle source-contract workflow filter',
        ('ci/check_kagemusha_lifecycle_source_contract.py',)),
    (WORKFLOW, 'cargo test -p iroha_core output_membership --lib', 'cargo test -p iroha_core retired_output_membership_filter --lib',
        'reject a missing frontier-test workflow filter', ('cargo test -p iroha_core output_membership --lib',)),
)
for mutation in static_mutations:
    expect_static_mutation(*mutation)
for (relative, before, after, failure, diagnostic) in (
    (KAGAMI, '#[command(name = "prepare-enable-issuance-v4")]',
        '#[command(name = "retired-enable-issuance-v4")]',
        'reject loss of the Kagami enable-preparation command',
        '#[command(name = "prepare-enable-issuance-v4")]'),
    (KAGAMI, '#[command(name = "prepare-cancel-release-v4")]',
        '#[command(name = "retired-cancel-release-v4")]',
        'reject loss of the Kagami cancellation-preparation command',
        '#[command(name = "prepare-cancel-release-v4")]'),
    (KAGAMI, '#[command(name = "prepare-deactivate-issuance-v4")]',
        '#[command(name = "retired-deactivate-issuance-v4")]',
        'reject loss of the Kagami deactivation-preparation command',
        '#[command(name = "prepare-deactivate-issuance-v4")]'),
    (KAGAMI, 'KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&bytes)',
        'KagemushaV4IssuanceEnableWitnessV1::decode_unchecked(&bytes)',
        'reject noncanonical Kagami lifecycle input decoding',
        'bounded canonical PrepareEnableIssuanceV4 lifecycle preparation'),
    (KAGAMI, '                    KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1,',
        '                    usize::MAX,',
        'reject unbounded Kagami lifecycle input decoding',
        'bounded canonical PrepareEnableIssuanceV4 lifecycle preparation'),
    (KAGAMI, 'let instructions = vec![instruction];',
        'let instructions = vec![instruction.clone(), instruction];',
        'reject a multi-instruction Kagami lifecycle output',
        'one-instruction no-replace lifecycle publication'),
    (KAGAMI, 'fn lifecycle_terminal_commands_publish_exact_typed_instructions_and_reports()',
        'fn unchecked_lifecycle_terminal_commands()',
        'reject loss of exact typed Kagami lifecycle output coverage',
        'fn lifecycle_terminal_commands_publish_exact_typed_instructions_and_reports()'),
    (KAGAMI, 'fn lifecycle_commands_reject_tampered_noncanonical_oversized_and_malformed_inputs()',
        'fn unchecked_lifecycle_command_inputs()',
        'reject loss of hostile Kagami lifecycle input coverage',
        'fn lifecycle_commands_reject_tampered_noncanonical_oversized_and_malformed_inputs()'),
    (KAGAMI, 'fn lifecycle_command_refuses_to_replace_existing_output()',
        'fn lifecycle_command_replaces_existing_output()',
        'reject loss of Kagami lifecycle no-replace coverage',
        'fn lifecycle_command_refuses_to_replace_existing_output()'),
    (IROHAD_STARTUP, 'pub(super) fn install_runtime_effective_config_with_validator_seal_reader(',
        'pub(super) fn install_runtime_effective_config_with_unchecked_reader(',
        'reject loss of the injectable validator-seal reader seam',
        'injectable exact validator-seal reader seam'),
    (IROHAD, IROHAD_STARTUP_TESTS_INCLUDE,
        '// authenticated snapshot startup tests detached',
        'reject detached authenticated snapshot startup regressions',
        'kagemusha_runtime_effective_config_projection_tests.rs'),
    (CORE_LIFECYCLE, '.with_admission_intent(TransactionAdmissionIntent::Ordinary)',
        '.with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)',
        'reject a non-Ordinary direct lifecycle execution fixture',
        'direct ordinary multisig lifecycle execution fixture'),
    (CORE_SUMERAGI_APPLY_TESTS, CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS_INCLUDE,
        '// production Kagemusha apply gate tests detached',
        'reject detached production Kagemusha apply regressions',
        'v2_apply_kagemusha_runtime_gate.rs'),
    (CORE_SUMERAGI_WORKER_ROOT, CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS_INCLUDE,
        '// production Kagemusha worker gate tests detached',
        'reject detached production Kagemusha signing regressions',
        'v2_worker_kagemusha_runtime_gate.rs'),
    (OFFLINE_CLI, KAGEMUSHA_LIFECYCLE_MODULE, '// lifecycle module detached',
        'reject detached lifecycle CLI routing', 'lifecycle route'),
    (OFFLINE_CLI, '#[command(name = "lifecycle-v4")]',
        '#[command(name = "retired-lifecycle-v4")]',
        'reject a renamed lifecycle CLI route', 'lifecycle route'),
    (CLI_MAIN_SHARED, 'let _chain_guard = ChainDiscriminantGuard::enter(config.account_chain_discriminant);', '// configured chain guard removed', 'reject key-free signing preflight outside the configured I105 guard', 'guarded key-free signing preflight'),
    (CLI_MAIN_SHARED, '    let _chain_guard = ChainDiscriminantGuard::enter(config.account_chain_discriminant);\n    if let Command::Offline(command) = &args.command {\n        command\n            .preflight_before_operator_key_load()\n            .map_err(|error| Report::new(MainError::Command(error.to_string())))?;\n    }', '    if let Command::Offline(command) = &args.command {\n        command\n            .preflight_before_operator_key_load()\n            .map_err(|error| Report::new(MainError::Command(error.to_string())))?;\n    }\n    let _chain_guard = ChainDiscriminantGuard::enter(config.account_chain_discriminant);', 'reject operator-key preflight before the configured I105 guard', 'guarded key-free signing preflight'),
    (OFFLINE_CLI, 'Self::Kagemusha(command) => command.preflight_before_operator_key_load(),', 'Self::Kagemusha(_) => Ok(()),', 'reject root offline dispatch detached from key-free preflight', 'narrow key-free signing preflight'),
    (OFFLINE_CLI, 'Self::LifecycleV4(args) => args.preflight_before_operator_key_load(),', 'Self::LifecycleV4(_) => Ok(()),', 'reject lifecycle route detached from key-free preflight', 'narrow key-free signing preflight'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Command::SignFeeQuote(args) => args.validated_signing_input().map(drop),', 'Command::SignFeeQuote(_) => Ok(()),', 'reject fee-quote signing outside key-free preflight', 'narrow key-free signing preflight'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Command::SignTransaction(args) => args.validated_signing_input().map(drop),', 'Command::SignTransaction(_) => Ok(()),', 'reject transaction signing outside key-free preflight', 'narrow key-free signing preflight'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'crate::resolve_account_id_with(literal)', 'payload.authority.clone()', 'reject a governance authority derived from the signed payload', 'guarded key-free signing preflight'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'FinalizeFeeQuote(FinalizeFeeQuote)',
        'RetiredPhase(RetiredPhase)',
        'reject loss of a lifecycle CLI phase', 'six phases'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Some(LifecycleKind::Cancel)',
        'Some(LifecycleKind::Stage)', 'reject a misclassified Cancel instruction',
        'kinds'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        '.with_admission_intent(TransactionAdmissionIntent::Ordinary)',
        '.with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)',
        'reject a non-Ordinary lifecycle preparation',
        '.with_admission_intent(TransactionAdmissionIntent::Ordinary)'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'let [instruction] = instructions.as_slice() else',
        'let Some(instruction) = instructions.first() else',
        'reject a multi-instruction lifecycle preparation',
        'let [instruction] = instructions.as_slice() else'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'const LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS: u64 = 60_000;',
        'const LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS: u64 = 120_000;',
        'reject drift from the 60-second request binding',
        'LIFECYCLE_FEE_QUOTE_MAX_CLOCK_SKEW_MS: u64 = 60_000'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Command::SignFeeQuote(_)\n                | Command::SignTransaction(_)',
        'Command::Prepare(_)\n                | Command::SignTransaction(_)', 'reject fallback config for a non-signing phase', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '&self.expected_draft_sha256,', '&hex::encode(Sha256::digest(&draft_bytes)),',
        'reject a draft signer without an independent digest pin', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '&self.expected_payload_sha256,', '&hex::encode(Sha256::digest(&payload_bytes)),',
        'reject a transaction signer without an independent digest pin', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '&self.expected_network_id,\n            &draft.payload,',
        '&draft.payload.network_id().unwrap().to_string(),\n            &draft.payload,', 'reject a draft-derived signing network pin', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '&self.expected_network_id,\n            &payload,',
        '&payload.network_id().unwrap().to_string(),\n            &payload,', 'reject a payload-derived signing network pin', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '        )?;\n        validate_fee_quote_draft(&draft, self.kind)?;\n        require_expected_authority(&self.governance_authority, &draft.payload)?;',
        '        )?;\n        Ok(())?;\n        require_expected_authority(&self.governance_authority, &draft.payload)?;',
        'reject signing before lifecycle-draft validation', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if payload.network_id() != Some(&expected) {', 'if false {',
        'reject a signing NetworkId pin bypass', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'let expected: [u8; 32] = decoded', 'let expected = decoded',
        'reject an ambiguously sized signing digest pin', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Sha256::digest(bytes).into()', 'Sha256::digest([]).into()',
        'reject a digest detached from signed bytes', 'pinned signing'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'domain == "localhost"', 'domain.ends_with("localhost")',
        'reject a suffix-trusted HTTP origin', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Some(Host::Ipv4(address)) => address.is_loopback(),',
        'Some(Host::Ipv4(_)) => true,', 'reject a non-loopback IPv4 HTTP origin', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Some(Host::Ipv6(address)) => address.is_loopback(),',
        'Some(Host::Ipv6(_)) => true,', 'reject a non-loopback IPv6 HTTP origin', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'None => false,', 'None => true,',
        'reject an unhosted plaintext origin', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'url.scheme() != "https" && (url.scheme() != "http" || !loopback)',
        'url.scheme() != "https" && url.scheme() != "http"', 'reject remote plaintext fee-quote transport', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'let url = Url::parse(&draft.fee_quote_url).wrap_err("invalid lifecycle fee-quote URL")?;\n    require_secure_fee_quote_origin(&url)?;',
        'let url = Url::parse(&draft.fee_quote_url).wrap_err("invalid lifecycle fee-quote URL")?;', 'reject a draft detached from secure-origin validation', 'secure fee-quote transport'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, '.wrap_err("failed to construct exact Torii fee-quote URL")?;\n    require_secure_fee_quote_origin(&url)?;',
        '.wrap_err("failed to construct exact Torii fee-quote URL")?;', 'reject configured transport detached from secure-origin validation', 'secure fee-quote transport'),
    (CLIENT, '        let request = if cleartext {\n            request.direct_loopback()\n        } else {\n            request\n        };', '        let request = request;', 'reject loopback fee quotes detached from direct transport', 'proxy-free loopback fee-quote dispatch'),
    (HTTP_DEFAULT, 'blocking_http_client_builder().no_proxy()', 'blocking_http_client_builder()', 'reject a proxy-capable cleartext loopback client', 'proxy-safe loopback client'),
    (HTTP_DEFAULT, 'builder = builder.resolve_to_addrs("localhost", &addresses);', '// localhost resolver pin removed', 'reject proxy-free localhost transport without a resolver pin', 'proxy-safe loopback client'),
    (HTTP_DEFAULT, '            Some(client) => client,\n            None => http_client(),', '            Some(_) => http_client(),\n            None => http_client(),', 'reject direct loopback selection after building its client', 'proxy-free loopback HTTP selection before dispatch'),
    (HTTP_DEFAULT, 'if pending.url.scheme() != "http" || !loopback {', 'if !loopback {', 'reject HTTPS admitted to direct loopback mode', 'proxy-free loopback HTTP selection before dispatch'),
    (HTTP_DEFAULT, 'Some(url::Host::Domain(domain)) => domain == "localhost",', 'Some(url::Host::Domain(_)) => true,', 'reject arbitrary domains in direct loopback mode', 'proxy-free loopback HTTP selection before dispatch'),
    (HTTP_DEFAULT, '    blocking_http_client_builder()\n        .build()', '    blocking_http_client_builder()\n        .no_proxy()\n        .build()', 'reject proxy-disabled ordinary HTTPS transport', 'sole proxy-disabled client'),
    (HTTP_DEFAULT, 'direct_loopback: false,', 'direct_loopback: true,', 'reject proxy-disabled HTTPS by default', 'direct_loopback: false'),
    (CLIENT, 'witness.signatures.len() < 2', 'witness.signatures.len() < 1',
        'reject loss of the two-distinct-signer floor',
        'witness.signatures.len() < 2'),
    (CLIENT, 'pair[0].signer >= pair[1].signer',
        'pair[0].signer > pair[1].signer',
        'reject non-distinct fee-witness signers',
        'pair[0].signer >= pair[1].signer'),
    (CLIENT, 'member.public_key() == &entry.signer',
        'member.public_key() != &entry.signer',
        'reject fee-witness nonmember acceptance',
        'member.public_key() == &entry.signer'),
    (CLIENT, 'iroha_crypto::verify_signature_for_admission(',
        'iroha_crypto::unchecked_signature(',
        'reject unverified fee-witness signatures',
        'verify_signature_for_admission('),
    (CLIENT, 'checked_add(u32::from(member.weight()))', 'checked_add(1)',
        'reject fee-witness policy-weight bypass',
        'checked_add(u32::from(member.weight()))'),
    (CLIENT, 'total_weight < u32::from(policy.threshold())',
        'total_weight < 1', 'reject fee-witness threshold bypass',
        'total_weight < u32::from(policy.threshold())'),
    (CLIENT, 'witness.subject_account != payload.authority',
        'false', 'reject an unbound fee-witness subject',
        'witness.subject_account != payload.authority'),
    (CLIENT, 'witness.canonical_request_hash != expected_hash',
        'false', 'reject an unbound fee-quote request hash',
        'witness.canonical_request_hash != expected_hash'),
    (CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT,
        'pub fn canonical_request_witness_message(',
        'pub fn unchecked_request_witness_message(',
        'reject loss of the canonical fee-witness message',
        'pub fn canonical_request_witness_message('),
    (CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT,
        '        canonical_request_hash: Hash,\n    }',
        '        canonical_request_hash: Hash,\n        signatures: Vec<u8>,\n    }',
        'reject signatures inside the canonical witness message', 'signatures:'),
    (CLIENT_CANONICAL_REQUEST_AUTH_COMPONENT,
        'norito::core::to_bytes_bounded(\n        &payload,',
        'norito::core::to_bytes(\n        &payload,',
        'reject unbounded canonical witness-message encoding',
        'to_bytes_bounded('),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'rustix::fs::RenameFlags::NOREPLACE,',
        'rustix::fs::RenameFlags::empty(),', 'reject replace-capable lifecycle archives',
        'bounded archive no-replace rename wrapper'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Ok(binding) if staged.same_inode(&binding) => {\n            return lifecycle_publication_commit_uncertain(', 'Ok(binding) if staged.same_inode(&binding) => {\n            return lifecycle_publication_precommit(', 'reject an owned destination reported pre-commit', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Ok(binding) if lifecycle_file_snapshot_matches_stat(staged, &binding) => binding,', 'Ok(binding) => binding,', 'reject rename reconciliation without exact staging custody', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Ok(LifecycleStagingCleanupOutcome::AlreadyAbsent) => {', 'Ok(LifecycleStagingCleanupOutcome::Removed) => {', 'reject disappeared staging evidence reported pre-commit', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'Ok(binding) => !staged.same_inode(&binding),', 'Ok(_) => true,', 'reject detached destination reinspection', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if let Err(error) = parent.verify_path_identity_against(publication_parent) {', 'if false {', 'reject rename reconciliation with a substituted parent', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'unlinkat(&parent.file, staging_name, AtFlags::empty())', 'unlinkat(&parent.file, target_name, AtFlags::empty())', 'reject cleanup of a non-staging namespace name', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'parent\n        .file\n        .sync_all()\n        .wrap_err("sync lifecycle staging cleanup")?;', 'Ok(())?;', 'reject pre-commit classification without durable staging cleanup', 'rename-error reconciliation'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'drop(staging);\n        return Err(reconcile_lifecycle_rename_error(', 'return Err(reconcile_lifecycle_rename_error(', 'reject rename reconciliation before releasing the staging descriptor', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'if !lifecycle_file_snapshot_matches_stat(expected, &before) {',
        'if false {', 'reject a detached lifecycle archive binding', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if observed != bytes {', 'if false {',
        'reject lifecycle archive content drift', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'if after != Some(expected) || !lifecycle_file_snapshot_matches_stat(expected, &linked_after) {',
        'if false {', 'reject a post-read lifecycle archive identity bypass', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        '        sync_parent(&parent.file).wrap_err_with(|| format!("sync published {label} parent"))',
        '        Ok(())', 'reject lifecycle archive publication without parent durability',
        'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        '            verify_lifecycle_artifact_file(\n                &parent,\n                target_name,\n                staged,\n                bytes,\n                &format!("durably published {label}"),\n            )',
        '            Ok(())', 'reject lifecycle archive publication without a durable final recheck',
        'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'File::sync_all,\n        )\n        .map_err(eyre::Report::new)',
        'File::sync_all,\n        )\n        .map_err(|_| eyre!("lifecycle publication failed"))',
        'reject erased lifecycle publication outcomes', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'lifecycle_publication_commit_uncertain(&final_path, label, error)',
        'lifecycle_publication_precommit(&final_path, label, error)',
        'reject post-commit failures reported as pre-commit', 'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT,
        'production lifecycle artifact publication requires Unix descriptor-relative no-replace APIs',
        'non-Unix lifecycle artifact publication is unsupported',
        'reject a detached non-Unix fail-closed publisher', KAGEMUSHA_LIFECYCLE_COMPONENT),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if length == 0 || length > maximum',
        'if length == 0', 'reject unbounded lifecycle archive reads',
        'bounded archive'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if prepared.as_bytes() != bytes',
        'if false', 'reject prepared lifecycle byte drift',
        'exact raw assembly'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'if !self.write_authorized',
        'if false', 'reject a lifecycle submit without --write-authorized',
        KAGEMUSHA_LIFECYCLE_COMPONENT),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'submit_prepared_kagemusha_lifecycle_payload(&transaction, &prepared, &expected_receipt_signer)', 'submit_prepared_transaction_payload(&prepared)', 'reject lifecycle submission outside the authenticated dedicated route', 'authenticated dedicated lifecycle submit'), (CLIENT, 'let direct_loopback = secure_transaction_submission_uses_direct_loopback(&self.torii_url)?;', 'let direct_loopback = false;', 'reject lifecycle submission detached from the validated origin', 'fresh direct exact-byte lifecycle submit with authenticated receipt'), (CLIENT, 'ensure_transaction_submit_compatibility_with_transport(direct_loopback, true)?;', 'ensure_transaction_submit_compatibility_with_transport(direct_loopback, false)?;', 'reject a lifecycle submit trusting a stale capability cache', 'fresh direct exact-byte lifecycle submit with authenticated receipt'), (CLIENT, 'join_torii_url(&self.torii_url, torii_uri::KAGEMUSHA_LIFECYCLE_TRANSACTION)', 'join_torii_url(&self.torii_url, torii_uri::TRANSACTION)', 'reject lifecycle bytes sent through the generic route', 'fresh direct exact-byte lifecycle submit with authenticated receipt'), (CLIENT, 'VerifiedTransactionResponseHandler::handle(&response, &identity, expected_receipt_signer)', 'TransactionResponseHandler::handle(&response)', 'reject lifecycle success without authenticated identity evidence', 'fresh direct exact-byte lifecycle submit with authenticated receipt'), (CLIENT, '.wrap_err_with(|| uncertain_context)', '.map_err(eyre::Report::new)', 'reject acknowledgement failures reported as safely retryable', 'fresh direct exact-byte lifecycle submit with authenticated receipt'), (CLIENT, 'DataModelCompatibility::Incompatible(err) if !require_fresh_probe', 'DataModelCompatibility::Incompatible(err)', 'reject fresh probes wedged by cached model incompatibility', CLIENT), (CLIENT, 'DataModelCompatibility::SchemaIncompatible(err) if !require_fresh_probe', 'DataModelCompatibility::SchemaIncompatible(err)', 'reject fresh probes wedged by cached schema incompatibility', CLIENT), (CLIENT, '.field("headers", &"<redacted>")', '.field("headers", &self.headers)', 'reject Client Debug leaking runtime headers', CLIENT), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'encode_and_publish_verified_lifecycle_receipt(&receipt, transaction_hash.clone(), &self.receipt_output)', 'publish_no_replace(&self.receipt_output, b"", "receipt")', 'reject post-acknowledgement receipt failures presented as safely retryable', 'authenticated dedicated lifecycle submit'),
    (CORE_PROXY, 'signed_lifecycle_entrypoint_context(transaction)', 'unchecked_lifecycle_entrypoint_context(transaction)', 'reject proxy classification detached from the exact Core carrier', CORE_PROXY), (CORE_PROXY, 'if durable.global_admission_identity.is_some()', 'if false', 'reject a globally bound Ordinary lifecycle durable claim', CORE_PROXY), (CORE_PROXY, 'durable.enqueue_timestamp_ms,\n            None,', 'durable.enqueue_timestamp_ms,\n            durable.global_admission_identity.clone(),', 'reject an Ordinary lifecycle journal digest with a QueuePlan identity', CORE_PROXY), (CORE_PROXY, 'attestation_count != durability_threshold', 'attestation_count < durability_threshold', 'reject a non-exact lifecycle durability quorum', CORE_PROXY), (CORE_PROXY, 'previous >= attestation.validator_index', 'previous > attestation.validator_index', 'reject duplicate lifecycle validator attestations', CORE_PROXY), (CORE_PROXY, '.verify(validator.public_key(), &signing_bytes)', '.map(|_| ())', 'reject unsigned lifecycle durability attestations', CORE_PROXY),
    (TORII_ROUTING, '.push_with_lane_with_state_and_routing_plan_strict_durable_claim(', '.push_with_lane_with_state_and_routing_plan(', 'reject lifecycle enqueue outside the strict durable journal', TORII_ROUTING), (TORII_ROUTING, 'validate_ordinary_kagemusha_lifecycle_entrypoint(', 'unchecked_ordinary_kagemusha_lifecycle_entrypoint(', 'reject the generic transaction-batch lifecycle bypass', TORII_ROUTING), (TORII, 'if queue_plan_binding.is_some()', 'if false', 'reject a lifecycle proxy carrying QueuePlan state', TORII), (TORII, 'if ordinary_lifecycle_attestations.len()\n                                >= expected.durability_threshold', 'if ordinary_lifecycle_attestations.len()\n                                > expected.durability_threshold', 'reject a lifecycle proxy requiring more than exact f+1', TORII), (TORII, '.take(expected.durability_threshold)', '.take(1)', 'reject lifecycle quorum truncation', TORII), (TORII, 'OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Partial', 'OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Quorum', 'reject loss of authenticated partial authority evidence', TORII), (TORII, 'OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Quorum', 'OrdinaryKagemushaLifecycleAdmissionCertificateStrengthV1::Partial', 'reject downgrade of final lifecycle quorum validation', TORII), (TORII, 'admission_binding: None', 'admission_binding: Some(binding.clone())', 'reject lifecycle admission coupled to QueuePlan publication', TORII), (TORII, '!= Some(submitted_signed_transaction_hash)', '== Some(submitted_signed_transaction_hash)', 'reject post-admission lifecycle identity drift', TORII), (TORII, '.entry(pending_key)', '.insert(pending_key', 'reject identical lifecycle waiters replacing each other', TORII), (TORII, 'Arc::ptr_eq(&waiter.waiter_token, waiter_token)', 'Arc::strong_count(&waiter.waiter_token) == 0', 'reject lifecycle waiter cleanup crossing callers', TORII), (TORII, 'for pending in pending_waiters', 'for pending in pending_waiters.into_iter().take(1)', 'reject lifecycle response fanout dropping identical callers', TORII), (CORE_IVM_HOST, 'Self::validate_zk_elections_snapshot(&elections)?', 'let _ = &elections;', 'reject ZK snapshot mutation before selector validation', CORE_IVM_HOST),
    (CORE, 'for ((circuit_id, version), id) in world.verifying_keys_by_circuit().iter() {\n        if kagemusha_release_verifier_id_has_exact_digest(id, "v5-") {', 'for ((circuit_id, version), id) in world.verifying_keys_by_circuit().iter() {\n        if false {', 'reject V5 verifier indexes entering generic hydration', CORE), (TORII_CATALOG, '"/v1/offline/kagemusha/lifecycle-v4/transactions"', '"/v1/pipeline/transactions"', 'reject lifecycle route aliasing the generic pipeline', TORII_CATALOG), (TORII, 'KAGEMUSHA_LIFECYCLE_TRANSACTION => limited_canonical_signed_post(handler_post_kagemusha_lifecycle_transaction, transaction_max_content_len)', 'KAGEMUSHA_LIFECYCLE_TRANSACTION => limited_public_post(handler_post_kagemusha_lifecycle_transaction, transaction_max_content_len)', 'reject mounting lifecycle submission without canonical signed-body authentication', TORII),
):
    expect_static_mutation(relative, before, after, failure, (diagnostic,))
for relative, test_name, diagnostic in (
    (IROHAD_STARTUP_TESTS, 'authenticated_snapshot_with_valid_local_seal_installs_runtime_digest', 'authenticated snapshot startup install and fail-closed regressions'), (IROHAD_STARTUP_TESTS, 'authenticated_snapshot_rejects_wrong_local_peer_without_installing', 'authenticated snapshot startup install and fail-closed regressions'),
    (IROHAD_STARTUP_TESTS, 'authenticated_snapshot_rejects_projection_mismatch_without_installing', 'authenticated snapshot startup install and fail-closed regressions'), (IROHAD_STARTUP_TESTS, 'authenticated_snapshot_without_configured_seal_does_not_install', 'authenticated snapshot startup install and fail-closed regressions'),
    (CORE_LIFECYCLE, 'direct_ordinary_multisig_cancel_executes_exact_staged_transition', 'direct signed ordinary Cancel and Deactivate transition regressions'), (CORE_LIFECYCLE, 'direct_ordinary_multisig_deactivate_executes_exact_enabled_transition', 'direct signed ordinary Cancel and Deactivate transition regressions'), (CORE_LIFECYCLE, 'terminal_lifecycle_rejects_repeated_and_cross_terminal_transitions', 'fn terminal_lifecycle_rejects_repeated_and_cross_terminal_transitions('),
    (CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS, 'production_proposal_validation_enforces_kagemusha_runtime_projection', 'production proposal and Commit runtime-projection regressions'), (CORE_SUMERAGI_APPLY_RUNTIME_GATE_TESTS, 'production_commit_apply_enforces_kagemusha_runtime_projection', 'production proposal and Commit runtime-projection regressions'),
    (CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS, 'production_vote_worker_rejects_missing_and_mismatched_kagemusha_projection', 'production Prepare and Commit signing runtime-projection regressions'), (CORE_SUMERAGI_WORKER_RUNTIME_GATE_TESTS, 'production_vote_worker_signs_prepare_and_commit_for_exact_kagemusha_projection', 'production Prepare and Commit signing runtime-projection regressions'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'classifier_covers_stage_enable_cancel_and_deactivate', 'fn classifier_covers_stage_enable'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'fee_quote_witness_enforces_distinct_floor_membership_order_and_weight', 'fn fee_quote_witness_enforces_distinct_floor'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'nonordinary_and_multi_instruction_carriers_are_rejected', 'fn nonordinary_and_multi_instruction'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'fee_quote_timestamp_rejects_stale_and_future_material', 'fn fee_quote_timestamp_rejects_stale_and_future_material()'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'raw_submission_preserves_authorized_wire_before_io', 'fn raw_submission_preserves_authorized_wire'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_signing_pins_reject_wrong_network_and_artifact_digest', 'fn lifecycle_signing_pins_reject_wrong_network_and_artifact_digest'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_fee_quote_transport_requires_https_except_exact_loopback', 'fn lifecycle_fee_quote_transport_requires_https_except_exact_loopback'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_operator_key_preflight_rejects_bad_digest_and_network', 'fn lifecycle_operator_key_preflight_rejects_bad_digest_and_network'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_operator_key_preflight_precedes_key_file_load_in_root', 'fn lifecycle_operator_key_preflight_precedes_key_file_load_in_root'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'archive_publication_is_no_replace_and_read_only', 'fn archive_publication_is_no_replace'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_rejects_partial_staged_write_before_commit', 'fn lifecycle_publication_rejects_partial_staged_write'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_rejects_parent_substitution_before_commit', 'fn lifecycle_publication_rejects_parent_substitution'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_noreplace_race_is_precommit_and_preserves_destination', 'fn lifecycle_publication_noreplace_race'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_rename_error_with_intact_staging_is_precommit', 'fn lifecycle_publication_rename_error_with_intact_staging_is_precommit'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_lost_rename_ack_is_commit_uncertain', 'fn lifecycle_publication_lost_rename_ack_is_commit_uncertain'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_missing_names_after_rename_error_is_commit_uncertain', 'fn lifecycle_publication_missing_names_after_rename_error_is_commit_uncertain'),
    (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_parent_drift_preserves_staging_during_rename_reconciliation', 'fn lifecycle_publication_parent_drift_preserves_staging_during_rename_reconciliation'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_reports_post_rename_replacement_as_commit_uncertain', 'fn lifecycle_publication_reports_post_rename_replacement'), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'lifecycle_publication_reports_parent_sync_failure_as_commit_uncertain', 'fn lifecycle_publication_reports_parent_sync_failure'),
    (HTTP_DEFAULT, 'direct_loopback_builder_is_fail_closed_and_leaves_https_proxy_capable', 'fn direct_loopback_builder_is_fail_closed'), (HTTP_DEFAULT, 'kagemusha_lifecycle_loopback_transport_ignores_proxy_environment', 'fn kagemusha_lifecycle_loopback_transport_ignores_proxy'), (CLIENT, 'verified_lifecycle_submit_pins_direct_transport_body_and_receipt', 'fn verified_lifecycle_submit_pins_direct_transport_body_and_receipt'), (CLIENT, 'verified_lifecycle_submit_rejects_unsafe_origins_before_io', 'fn verified_lifecycle_submit_rejects_unsafe_origins_before_io'), (CLIENT, 'verified_transaction_response_rejects_missing_or_untrusted_evidence', 'fn verified_transaction_response_rejects_missing_or_untrusted_evidence'), (CLIENT, 'client_debug_redacts_runtime_headers_and_torii_url_secrets', CLIENT), (CLIENT, 'verified_lifecycle_submit_marks_acknowledgement_validation_outcome_uncertain', CLIENT), (CLIENT, 'fresh_submit_compatibility_probe_replaces_cached_failures', CLIENT), (CLIENT, 'ordinary_submit_compatibility_preserves_cached_failures_without_io', CLIENT), (KAGEMUSHA_LIFECYCLE_COMPONENT, 'verified_receipt_publication_race_is_post_acknowledgement_and_no_retry', KAGEMUSHA_LIFECYCLE_COMPONENT), (CORE_IVM_HOST, 'zk_snapshot_hydration_separates_exact_native_kagemusha_v4_pairs_from_open_verify', 'native/generic hydration tests'), (CORE_IVM_HOST, 'zk_snapshot_hydration_rejects_malformed_kagemusha_v4_near_matches', 'native/generic hydration tests'), (CORE_IVM_HOST, 'zk_snapshot_hydration_does_not_exempt_allowed_same_backend_lookalike', 'native/generic hydration tests'), (CORE_IVM_HOST, 'zk_snapshot_hydration_rejects_v5_release_records_before_generic_hydration', 'native/generic hydration tests'), (KAGAMI, 'failed_no_replace_rename_is_precommit_only_with_unpublished_target_and_owned_staging', 'fn failed_no_replace_rename_is_precommit_only'), (KAGAMI, 'failed_no_replace_rename_cleanup_uncertainty_retains_the_full_error_chain', 'fn failed_no_replace_rename_cleanup_uncertainty'), (KAGAMI, 'failed_no_replace_rename_commit_uncertainty_never_runs_staging_cleanup', 'fn failed_no_replace_rename_commit_uncertainty'),
    (KAGAMI, 'failed_no_replace_rename_for_release_circuit_params_preserves_foreign_target', 'fn failed_no_replace_rename_for_release_circuit_params'), (KAGAMI, 'failed_no_replace_rename_for_promotion_preserves_foreign_target', 'fn failed_no_replace_rename_for_promotion'), (KAGAMI, 'promotion_publication_rejects_same_length_staged_content_mutation', 'fn promotion_publication_rejects_same_length_staged_content_mutation'), (KAGAMI, 'release_circuit_params_publication_rejects_staged_leaf_substitution', 'fn release_circuit_params_publication_rejects_staged_leaf_substitution'), (KAGAMI, 'release_circuit_params_post_rename_mutation_is_commit_uncertain', 'fn release_circuit_params_post_rename_mutation_is_commit_uncertain'),
    (CORE_LIFECYCLE, 'cancellation_rejects_missing_verifier_record_atomically', 'fn cancellation_rejects_missing_verifier_record_atomically()'), (CORE_LIFECYCLE, 'cancellation_rejects_substituted_lifecycle_verifier_id_atomically', 'fn cancellation_rejects_substituted_lifecycle_verifier_id_atomically()'), (CORE_LIFECYCLE, 'cancellation_rejects_verifier_owner_mismatch_atomically', 'fn cancellation_rejects_verifier_owner_mismatch_atomically()'), (CORE_LIFECYCLE, 'cancellation_rejects_verifier_version_mismatch_atomically', 'fn cancellation_rejects_verifier_version_mismatch_atomically()'), (CORE_LIFECYCLE, 'cancellation_rejects_inactive_verifier_atomically', 'fn cancellation_rejects_inactive_verifier_atomically()'), (CORE_LIFECYCLE, 'cancellation_rejects_verifier_index_mismatch_atomically', 'fn cancellation_rejects_verifier_index_mismatch_atomically()'),
    (CORE_PROXY, 'ordinary_kagemusha_lifecycle_scope_accepts_exact_and_rejects_near_matches', CORE_PROXY), (CORE_PROXY, 'ordinary_kagemusha_lifecycle_certificate_requires_two_distinct_of_four', CORE_PROXY), (CORE_PROXY, 'ordinary_kagemusha_lifecycle_certificate_rejects_binding_roster_route_and_journal_drift', CORE_PROXY), (CORE_PROXY, 'ordinary_kagemusha_lifecycle_durable_claim_must_remain_globally_unbound', CORE_PROXY), (TORII_TESTS, 'ordinary_kagemusha_lifecycle_proxy_requires_exact_f_plus_one_unbound_certificate', TORII_TESTS), (TORII_TESTS, 'generic_transaction_proxy_rejects_ordinary_lifecycle_and_dedicated_route_rejects_bare_202', TORII_TESTS), (TORII_TESTS, 'generic_transaction_batch_rejects_ordinary_lifecycle_without_enqueuing', TORII_TESTS), (TORII_TESTS, 'ordinary_kagemusha_lifecycle_receiver_rolls_durable_retry_without_queue_plan_publication', TORII_TESTS), (TORII_PENDING_TESTS, 'identical_torii_proxy_submissions_keep_all_pending_waiters', TORII_PENDING_TESTS), (TORII_PENDING_TESTS, 'identical_torii_proxy_attempt_cleanup_is_waiter_scoped', TORII_PENDING_TESTS), (CORE_IVM_HOST, 'zk_snapshot_hydration_rejects_invalid_election_selector_without_partial_replacement', CORE_IVM_HOST), (CORE_LIFECYCLE, 'sealed_reveal_cannot_gain_direct_external_lifecycle_provenance', CORE_LIFECYCLE), (TORII_CATALOG_TESTS, 'ordinary_kagemusha_lifecycle_has_one_dedicated_canonical_signed_route', TORII_CATALOG_TESTS),
):
    expect_static_mutation(relative, test_name, f'retired_{test_name}',
        f'reject loss of {test_name}', (diagnostic,))
for (required_filter, retired_filter, label) in (('cargo test -p iroha_data_model receiver_snapshot --lib',
    'cargo test -p iroha_data_model retired_receiver_snapshot_filter --lib', 'receiver-snapshot data-model'),
    ('cargo test -p iroha_core kagemusha_online_registration_ --lib', 'cargo test -p iroha_core retired_registration_filter --lib',
    'compact registration'), ('cargo test -p iroha_core active_receiver_snapshot_ --lib',
    'cargo test -p iroha_core retired_active_receiver_filter --lib', 'active-receiver resolver'),
    ('cargo test -p iroha_kagami --bin kagami atomic_activation_', 'cargo test -p iroha_kagami --bin kagami retired_activation_filter',
    'activation-policy parity'), ('cargo test -p iroha_kagami --bin kagami backing_',
    'cargo test -p iroha_kagami --bin kagami retired_backing_filter', 'ordered Taira backing')):
    expect_static_mutation(WORKFLOW, required_filter, retired_filter, f'reject a missing {label} workflow filter', (required_filter,))
folded_acceptance_filter = KAGEMUSHA_RELEASE_RUST_TEST_FILTERS[0]
folded_acceptance_inline = f'- run: {folded_acceptance_filter}'
folded_acceptance_step = f'- run: >-\n          {folded_acceptance_filter}'
if folded_acceptance_inline not in baseline[WORKFLOW]:
    errors.append('self-test could not locate an inline Kagemusha release Rust filter')
elif static_errors({WORKFLOW: baseline[WORKFLOW].replace(
        folded_acceptance_inline, folded_acceptance_step, 1)}):
    errors.append('self-test failed to accept an exact single-command folded Kagemusha Rust filter')
for required_filter in KAGEMUSHA_RELEASE_RUST_TEST_FILTERS:
    inline_step = f'- run: {required_filter}'
    folded_step = f'- run: >-\n          {required_filter}'
    if folded_step in baseline[WORKFLOW]:
        expect_static_mutation(WORKFLOW, folded_step,
            f'- run: >-\n          retired {required_filter}',
            'reject a missing folded Kagemusha release Rust filter', (required_filter,))
    else:
        expect_static_mutation(WORKFLOW, inline_step, f'# {inline_step}',
            'reject a missing Kagemusha release Rust filter', (required_filter,))
folded_continuation_filter = next(
    command for command in KAGEMUSHA_RELEASE_RUST_TEST_FILTERS
    if 'kagemusha::tests::' in command
)
folded_continuation_step = f'- run: >-\n          {folded_continuation_filter}'
expect_static_mutation(WORKFLOW, folded_continuation_step,
    f'{folded_continuation_step}\n          echo unexpected continuation',
    'reject an extra folded Kagemusha Rust filter continuation',
    (folded_continuation_filter,))
for required_path in KAGEMUSHA_RELEASE_PYTHON_TEST_PATHS:
    expect_static_mutation(WORKFLOW, required_path, f'# {required_path}',
        'reject a missing Kagemusha release Python test', (required_path,), -1)
boundary_artifacts = {name: MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES // len(ARTIFACTS) for name in ARTIFACTS}
if checked_declared_artifact_total(boundary_artifacts) != MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES:
    errors.append('self-test failed to accept the exact artifact aggregate limit')
exact_file_artifacts = {name: 1 for name in ARTIFACTS}
exact_file_artifacts[ARTIFACTS[0]] = MAX_DECLARED_ARTIFACT_FILE_BYTES
if checked_declared_artifact_total(exact_file_artifacts) != MAX_DECLARED_ARTIFACT_FILE_BYTES + len(ARTIFACTS) - 1:
    errors.append('self-test failed to accept the exact artifact file limit')
oversized_cases = ((MAX_DECLARED_ARTIFACT_FILE_BYTES + 1, 'self-test failed to reject an oversized artifact file'),
    (boundary_artifacts[ARTIFACTS[0]] + 1, 'self-test failed to reject an oversized artifact aggregate'))
for (first_artifact_bytes, failure) in oversized_cases:
    oversized_artifacts = dict(boundary_artifacts)
    oversized_artifacts[ARTIFACTS[0]] = first_artifact_bytes
    try:
        checked_declared_artifact_total(oversized_artifacts)
    except ValueError:
        continue
    errors.append(failure)
# Exercise exact output bounds, timeouts, and descendant cleanup.
bounded_test_environment = {'LANG': 'C', 'LC_ALL': 'C', 'PATH': '/usr/bin:/bin'}
try:
    exact_capture = run_bounded_authenticated_process([sys.executable, '-I', '-c',
        "import os; os.write(1, b'x' * 32); os.write(2, b'y' * 16)"], timeout_seconds=2.0, stdout_limit=32, stderr_limit=16,
        environment=bounded_test_environment)
    if exact_capture.returncode != 0 or exact_capture.stdout != b'x' * 32 or exact_capture.stderr != b'y' * 16:
        errors.append('self-test failed to preserve exact-limit authenticated verifier output')
    for (stream_number, stream_name) in ((1, 'stdout'), (2, 'stderr')):
        try:
            run_bounded_authenticated_process([sys.executable, '-I', '-c', f"import os; os.write({stream_number}, b'z' * 33)"],
                timeout_seconds=2.0, stdout_limit=32, stderr_limit=32, environment=bounded_test_environment)
        except ValueError as error:
            if str(error) != f'authenticated verifier {stream_name} exceeded its 32-byte limit':
                errors.append(f'self-test produced a nondeterministic verifier {stream_name} limit rejection: {error}')
        else:
            errors.append(f'self-test failed to reject oversized verifier {stream_name}')
    with tempfile.TemporaryDirectory(prefix='kagemusha-verifier-timeout-self-test-') as temporary:
        descendant_pid_path = Path(temporary) / 'descendant.pid'
        if os.geteuid() == PRODUCTION_TRUSTED_UID:
            timeout_source = (
                "import pathlib,subprocess,sys,time; "
                "child=subprocess.Popen([sys.executable, '-I', '-c', "
                "'import time; time.sleep(30)']); "
                "pathlib.Path(sys.argv[1]).write_text(str(child.pid), "
                "encoding='ascii'); time.sleep(30)"
            )
        else:
            timeout_source = 'import time; time.sleep(30)'
        try:
            run_bounded_authenticated_process([sys.executable, '-I', '-c', timeout_source, str(descendant_pid_path)],
                timeout_seconds=0.1, stdout_limit=32, stderr_limit=32, environment=bounded_test_environment)
        except ValueError as error:
            if str(error) != 'authenticated verifier exceeded its 0.1-second timeout':
                errors.append(f'self-test produced a nondeterministic verifier timeout rejection: {error}')
        else:
            errors.append('self-test failed to time out a stalled verifier')
        if os.geteuid() == PRODUCTION_TRUSTED_UID and (not descendant_pid_path.is_file()):
            errors.append('self-test verifier did not launch its process-group descendant')
        elif descendant_pid_path.is_file():
            descendant_pid = int(descendant_pid_path.read_text(encoding='ascii'))
            descendant_deadline = time.monotonic() + 1.0
            while True:
                try:
                    os.kill(descendant_pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= descendant_deadline:
                    errors.append('self-test failed to clean up the verifier process group')
                    break
                time.sleep(0.01)
    with tempfile.TemporaryDirectory(prefix='kagemusha-verifier-success-descendant-self-test-') as temporary:
        descendant_pid_path = Path(temporary) / 'descendant.pid'
        success_source = (
            "import pathlib,subprocess,sys; "
            "child=subprocess.Popen([sys.executable, '-I', '-c', "
            "'import time; time.sleep(30)'], stdin=subprocess.DEVNULL, "
            "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, "
            "close_fds=True); "
            "pathlib.Path(sys.argv[1]).write_text(str(child.pid), encoding='ascii')"
        )
        successful = run_bounded_authenticated_process([sys.executable, '-I', '-c', success_source, str(descendant_pid_path)],
            timeout_seconds=2.0, stdout_limit=32, stderr_limit=32, environment=bounded_test_environment)
        if successful.returncode != 0:
            errors.append('self-test verifier leader did not exit successfully')
        if not descendant_pid_path.is_file():
            errors.append('self-test successful verifier did not launch its descendant')
        else:
            descendant_pid = int(descendant_pid_path.read_text(encoding='ascii'))
            descendant_deadline = time.monotonic() + 1.0
            while True:
                try:
                    os.kill(descendant_pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= descendant_deadline:
                    errors.append('self-test failed to sweep a successful verifier descendant')
                    break
                time.sleep(0.01)
    if authenticated_verifier_exit_diagnostic(7) != 'exited with status 7':
        errors.append('self-test verifier exit-status diagnostic is not deterministic')
    if authenticated_verifier_exit_diagnostic(-9) != 'terminated by signal 9':
        errors.append('self-test verifier signal diagnostic is not deterministic')
except (OSError, ValueError) as error:
    errors.append(f'bounded verifier self-test failed unexpectedly: {error}')
verifier_command = release_verifier_command(Path('/trusted/kagami'), Path('/release'), Path('/policy.norito'))
if verifier_command[:3] != ['/trusted/kagami', 'kagemusha', 'verify-release-v4']:
    errors.append('self-test failed to pin the explicit Kagami release verifier')
controlled_verifier_command = authenticated_verifier_controller_command(Path('/trusted/isolation-controller'), verifier_command)
controlled_separator = controlled_verifier_command.index('--')
if (
    controlled_verifier_command[:4]
    != [
        '/trusted/isolation-controller',
        AUTHENTICATED_TOOL_CONTROLLER_SUBCOMMAND,
        '--contract',
        AUTHENTICATED_TOOL_CONTROLLER_CONTRACT,
    ]
    or controlled_verifier_command[controlled_separator + 1:]
    != verifier_command
    or not {
        '--use-attested-runtime-identity',
        '--expected-runtime-uid',
        '--expected-runtime-gid',
        '--no-new-privileges',
        '--close-inherited-fds',
        '--forward-tool-exit-status',
        '--exact-tool-stdio',
        '--deny-network',
        '--deny-tool-process-spawn',
        '--deny-read-outside-allowlist',
        '--deny-all-writes',
        '--account-unlinked-write-bytes',
        '--require-empty-process-tree',
        '--cumulative-write-limit-bytes',
        '--maximum-live-write-root-bytes',
    }
    <= set(controlled_verifier_command[:controlled_separator])
):
    errors.append('self-test failed to bind Kagami to the authenticated OS-isolation controller')
controlled_options = controlled_verifier_command[:controlled_separator]
def controlled_values(option: str) -> list[str]: return [controlled_options[index + 1] for index, value in enumerate(controlled_options) if value == option]
expected_readable_files = ['/policy.norito', *(f'/release/{name}' for name in ARTIFACTS + FINAL_METADATA)]
if (controlled_values('--readable-directory') != ['/release']
        or controlled_values('--readable-file') != expected_readable_files):
    errors.append('self-test failed to close the verifier filesystem read allowlist')
pinned_ios_diagnostic = 'same pinned evidence, trusted key, and production policy snapshots'
readiness_mutations = (('        str(verifier),\n        "kagemusha",', '        "cargo",\n        "run",',
    'reject a PATH-resolved Cargo verifier', ('promotion verifier command',)),
    ('    ios_root, key_id, _, _, freshness_key_id, _, _, _ = ios_configuration',
    '    ios_root, key_id, _, _, freshness_key_id, _ = ios_configuration',
    'reject a truncated physical-iOS configuration unpack',
    ('exact eight-field physical-iOS configuration unpack',)),
    ('                verified = run_authenticated_verifier(command, tool_controller_exec)',
    '                verified = subprocess.run(command, capture_output=True)',
    'reject a Kagami verifier outside the authenticated isolation controller',
    ('bounded authenticated verifier execution and deterministic diagnostics',)), ('            preexec_fn=os.setpgrp,',
    '            # verifier process-group isolation removed',
    'reject a verifier sharing the readiness process group', D_KAGAMI_EXEC),
    ('        "--deny-all-writes",', '        "--allow-ambient-writes",',
    'reject a verifier controller with ambient filesystem writes', D_KAGAMI_EXEC),
    ('        "--deny-read-outside-allowlist",', '        "--allow-ambient-reads",',
    'reject a verifier controller with ambient filesystem reads', D_KAGAMI_EXEC),
    ('        terminate_authenticated_verifier_process_group(\n            process,\n            leader_exit_observed=True,\n        )\n        returncode = process.returncode',
    '        process.wait(timeout=KAGAMI_VERIFIER_REAP_TIMEOUT_SECONDS)\n        returncode = process.returncode',
    'reject reaping a successful verifier before its process-group sweep',
    ('unconditional success-path verifier process-group sweep before leader reap',)),
    ('    require_no_macos_extended_acl(descriptor, label)\n\n\ndef snapshot_private_bytes',
    '    # descriptor ACL custody removed\n\n\ndef snapshot_private_bytes',
    'reject missing descriptor ACL custody', ('descriptor-exact macOS ACL rejection',)),
    ('    promotion_assert_no_extended_acl "${current}" "${label} path component" || return 1', '    # component ACL custody removed',
    'reject missing shell path-component ACL custody', ('root and every path component',)),
    ('  if [[ "${mode_marker}" == *+ ]]; then', '  if [[ "${mode_marker}" == "never-an-acl" ]]; then',
    'reject bypassed shell ACL detection', ('fail-closed macOS extended-ACL inspection',)),
    ('                    evidence_snapshot_path,\n                    raw_root,',
    '                    directory / "physical-device-benchmark.evidence",\n                    raw_root,',
    'reject reopening physical-iOS evidence for validation', (pinned_ios_diagnostic,)),
    ('                    trusted_public_key_snapshot,\n                    trusted_production_policy_snapshot,\n',
    '                    ios_configuration[2],\n                    trusted_production_policy_snapshot,\n',
    'reject reopening the physical-iOS trust key', (pinned_ios_diagnostic,)),
    ('                    trusted_production_policy_snapshot,\n', '                    ios_configuration[3],\n',
    'reject reopening the physical-iOS production policy', (pinned_ios_diagnostic,)),
    ('                    freshness_snapshot_path,\n', '                    freshness_receipt,\n',
    'reject reopening the physical-iOS freshness receipt', (pinned_ios_diagnostic,)),
    ('                    trusted_freshness_public_key_snapshot,\n', '                    ios_configuration[5],\n',
    'reject reopening the physical-iOS freshness authority key', (pinned_ios_diagnostic,)),
    ('production_module.__dict__.get(\n            "validate_production_signed_evidence"\n        )',
    'production_module.__dict__.get("validate_signed_evidence")',
    'reject the testnet-only iOS validator in promotion', ('production-only iOS evidence validator entrypoint',)),
    ('            "validate_historical_production_evidence_for_catalog_revalidation"',
    '            "validate_production_signed_evidence"',
    'reject treating historical consumption as current catalog status',
    D_CATALOG_HISTORY),
    ('            "validate_catalog_revalidation_receipt"',
    '            "validate_catalog_without_fresh_status"',
    'reject removing current catalog revalidation',
    D_CATALOG_HISTORY),
    ('                ios_catalog_bindings.append(ios_catalog_binding)',
    '                # exact catalog binding removed',
    'reject an unbound multi-release App Attest status receipt',
    ('current promotion-scoped exact-catalog App Attest revalidation',)),
    ('                        require_production_root_custody(descriptor, label)',
    '                        # production root custody removed', 'reject a missing production custody check',
    ('root-custody the complete production path-component set',)), ('            != trusted_python_sha256',
    '            != hash_pinned_descriptor(\n                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label\n            )',
    'reject an unpinned running Python runtime', ('running promotion interpreter digest revalidation',)),
    ('"/dev/fd/${PYTHON_PIN_FD}"', '"${PYTHON_BIN}"', 'reject path-reopened pre-exec Python',
    ('pre-exec Python descriptor custody',), -1), ('prefix="kagemusha-pinned-input-", dir=staging_parent',
    'prefix="kagemusha-pinned-input-"', 'reject ambient-TMPDIR promotion staging', ('explicit staging parent',)),
    ('"TMPDIR": str(PROMOTION_STAGING_PARENT),', '"TMPDIR": "/tmp",',
    'reject ambient-TMPDIR promotion subprocess staging', ('fixed staging parent',)),
    ('                str(trusted_source_helper_snapshot),', '                str(source_helper_path),',
    'reject a path-executed source helper', ('source-closure-authenticated source-tree helper snapshot',)),
    ('                validator_bytes,\n                trusted_ios_validator_snapshot,',
    '                validator_bytes,\n                ios_validator_path,', 'reject a path-loaded iOS validator',
    ('source-closure-authenticated candidate and production iOS validator snapshots',)),
    ('"HOME": str(trusted_source_trust_home),', '"HOME": "/var/empty",', 'reject an unconfigured source trust HOME',
    ('source SSH trust', 'closure-bound snapshotted')), ('        source_projection_identity = validate_source_trust_projection(\n            source_projection_bytes,',
    '        source_projection_identity = bypass_source_trust_projection(\n            source_projection_bytes,',
    'reject unbound source SSH trust policies', ('closure-bound snapshotted source SSH trust policies',)),
    ('  promotion_assert_root_custody "${DERIVED_ROOT_DIR}" "promotion readiness checkout" || exit 2',
    '  # promotion checkout custody removed', 'reject an untrusted gate checkout',
    ('root-custodied gate bootstrap', 'promotion_assert_root_custody')),
    ('  if [[ "${OBSERVED_GATE_SHA256}" != "${GATE_SHA256}" ]]; then', '  if false; then',
    'reject a bypassed reviewed gate digest', ('root-custodied gate bootstrap',)),
    ('        if sealed_build_report is not None:\n            production_roots.append(sealed_build_report.parent)',
    '        # sealed-build-report ancestor custody removed',
    'reject a sealed-build report below an uncustodied ancestor',
    ('sealed-build-report ancestor root custody',)),
    ('    if stat.S_IMODE(metadata.st_mode) & 0o022:',
    '    if stat.S_IMODE(metadata.st_mode) & 0o002:',
    'reject group-writable production custody',
    ('root-owned non-group/world-writable production custody',)),
    ('SCRIPT_DIRECTORY_LEXICAL="${SCRIPT_PATH_LEXICAL%/*}"', 'SCRIPT_DIRECTORY_LEXICAL="$(dirname "${SCRIPT_PATH_LEXICAL}")"',
    'reject PATH-resolved root derivation', ('promotion shell bootstrap',)))
for mutation in readiness_mutations:
    expect_static_mutation(READINESS, *mutation)
