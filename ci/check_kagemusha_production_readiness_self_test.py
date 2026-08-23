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
                [str(SOURCE_GIT), '-C', str(repository), '-c', 'core.hooksPath=/dev/null', *arguments],
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
        ordinary_git('-c', 'user.name=Kagemusha Self Test', '-c',
                     'user.email=kagemusha-self-test.invalid', 'commit', '-qm', 'base')
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
        'untracked_path_mode_blob_oid_manifest_sha256': '4' * 64, 'ignored_cargo_lock_size_bytes': 1,
        'ignored_cargo_lock_sha256': '5' * 64, 'combined_source_fingerprint_sha256': '6' * 64}
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
if 'recursive-step-two-qualification-v4.norito' not in FINAL_METADATA or MAX_RELEASE_INVENTORY_ENTRIES != 17 or MAX_QUALIFICATION_RECEIPT_BYTES != 802_816:
    errors.append('self-test failed to pin the final recursive qualification receipt inventory')
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
        with tempfile.TemporaryDirectory(prefix='kagemusha-acl-custody-self-test-') as temporary:
            acl_path = Path(temporary) / 'acl-input'
            acl_path.write_bytes(b'root-custody-acl-test')
            acl_path.chmod(384)
            (descriptor, _) = pin_regular_metadata(acl_path, 'self-test macOS ACL input')
            try:
                require_no_macos_extended_acl(descriptor, 'self-test ACL-free macOS input')
            finally:
                os.close(descriptor)
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
    'authenticated_source_seal_projection_sha256': 'b' * 64, 'reviewed_cargo_binary_sha256': 'c' * 64,
    'reviewed_rustc_binary_sha256': 'd' * 64, 'promotion_record_sha256': '6' * 64, 'release_policy_sha256': '5' * 64,
    'generator_binary_sha256': 'e' * 64, 'sealed_candidate_build_report_sha256': 'f' * 64,
    'generation': 'self-test', 'generation_memory_limit_bytes': 1, 'generation_memory_enforcement_profile': 'self-test-profile',
    'network_id': 'self-test-network', 'asset_definition_id': 'self-test-asset', 'asset_scale': 2, 'bridge_abi_version': 22,
    'recursive_step_verifier_commitment': '9' * 64, 'artifacts': report_artifacts}
try:
    validate_kagami_verification_report(verifier_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
        policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64, ios_candidate_sha256='3' * 64)
    invalid_report = dict(verifier_report)
    invalid_report['status'] = 'unverified'
    validate_kagami_verification_report(invalid_report, directory=Path('/release') / ('1' * 64), manifest=report_manifest,
        policy_sha256='5' * 64, promotion_record_sha256='6' * 64, qualification_receipt_sha256='4' * 64, ios_candidate_sha256='3' * 64)
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
            ios_candidate_sha256='3' * 64)
    except ValueError as error:
        if 'differs from the manifest' not in str(error):
            errors.append(f'authenticated report {field} self-test failed unexpectedly: {error}')
    else:
        errors.append(f'self-test failed to reject a mismatched Kagami report {field}')
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
    READINESS_SELF_TEST: read(READINESS_SELF_TEST, []),
    MODEL: read_reviewed_model([], {}),
    MODEL_COMPONENT: read(MODEL_COMPONENT, []),
    MODEL_VERIFIER_COMPONENT: read(MODEL_VERIFIER_COMPONENT, []),
    MODEL_PROMOTION_RECEIPT_COMPONENT: read(MODEL_PROMOTION_RECEIPT_COMPONENT, []),
    MODEL_CANARY_EVIDENCE_COMPONENT: read(MODEL_CANARY_EVIDENCE_COMPONENT, []),
    MODEL_CANARY_LIVENESS_COMPONENT: read(MODEL_CANARY_LIVENESS_COMPONENT, []),
    MODEL_ISI_OFFLINE: read(MODEL_ISI_OFFLINE, []),
    MODEL_ISI_MOD: read(MODEL_ISI_MOD, []),
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
    CORE_STATE: read(CORE_STATE, []),
    CORE_COMMITTED_TX_CONTEXT: read(
        CORE_COMMITTED_TX_CONTEXT, []
    ),
    CORE_BLOCK: read(CORE_BLOCK, []),
    CORE_EXECUTOR: read(CORE_EXECUTOR, []),
    CORE_ISI_MOD: read(CORE_ISI_MOD, []),
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
    OFFLINE_CLI: read(OFFLINE_CLI, []),
    KAGEMUSHA_ROLLOUT_COMPONENT: read(KAGEMUSHA_ROLLOUT_COMPONENT, []),
    KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT: read(
        KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT, []
    ),
    BUNDLE: read(BUNDLE, []),
    BUNDLE_SOURCE_SEAL_INPUTS: read(BUNDLE_SOURCE_SEAL_INPUTS, []),
    WORKFLOW: read(WORKFLOW, []),
    PROMOTION_WORKFLOW: read(PROMOTION_WORKFLOW, []),
    PRODUCTION_IOS_EVIDENCE_MODULE: read(PRODUCTION_IOS_EVIDENCE_MODULE, []),
}
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
static_mutations = (
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
        '    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_SOURCE_CONTRACT,',
        '    # recursion provider omitted from the authenticated set\n    READINESS_SOURCE_CONTRACT,',
        'reject an unauthenticated recursion source-contract provider',
        ('exact authenticated source-provider set',)),
    (READINESS,
        'READINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_SOURCE_CONTRACT,\n)',
        'READINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n    READINESS_SOURCE_CONTRACT,\n)\nREADINESS_SOURCE_PROVIDERS = (\n    READINESS_SOURCE_SUPPORT,\n    READINESS_RECURSION_SOURCE_CONTRACT,\n)',
        'reject a later authenticated source-provider tuple rebind',
        ('exactly one immutable authenticated source-provider tuple',)),
    (READINESS,
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)',
        'support_bytes = (root / READINESS_SOURCE_SUPPORT).read_bytes()',
        'reject execution of reopened source-support provider bytes',
        ('authenticated byte-only support, recursion, and readiness source-contract dispatch',)),
    (READINESS,
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)',
        'support_bytes = source_contract_bytes.get(READINESS_SOURCE_SUPPORT)\nsupport_bytes = Path("/tmp/hostile-support").read_bytes()',
        'reject a shadowed source-support provider byte assignment',
        ('support_bytes must have exactly one authenticated provider-map assignment',)),
    (READINESS,
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)',
        'recursion_bytes = (root / READINESS_RECURSION_SOURCE_CONTRACT).read_bytes()',
        'reject execution of reopened recursion source-contract provider bytes',
        ('authenticated byte-only support, recursion, and readiness source-contract dispatch',
         'candidate-only path read')),
    (READINESS,
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)',
        'recursion_bytes = source_contract_bytes.get(READINESS_RECURSION_SOURCE_CONTRACT)\nrecursion_bytes = open("/tmp/hostile-recursion", "rb").read()',
        'reject a shadowed recursion provider byte assignment',
        ('recursion_bytes must have exactly one authenticated provider-map assignment',)),
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
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        'verify_authorization_signature(\n            &self.signature,',
        'unchecked_authorization_signature(\n            &self.signature,',
        'reject an unsigned embedded controller permit',
        ('pre-commit controller-signed permit with consensus time authority and height bounds',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            || &self.body.canary_authority != canary_authority',
        '            || false',
        'reject a canary permit detached from the actual transaction authority',
        ('pre-commit controller-signed permit with consensus time authority and height bounds',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            canary_transaction_intent: canary_transaction.hash(),',
        '            canary_transaction_intent: HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),',
        'reject a reservation detached from the exact transaction intent',
        ('minimal controller-signed non-disclosing exact-call reservation',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            canary_entrypoint_hash: Hash::from(canary_transaction.hash_as_entrypoint()),',
        '            canary_entrypoint_hash: Hash::prehashed([0; 32]),',
        'reject a reservation detached from the external entrypoint hash',
        ('minimal controller-signed non-disclosing exact-call reservation',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        verify_reservation_signature(\n            &self.signature,',
        '        unchecked_reservation_signature(\n            &self.signature,',
        'reject an unsigned minimal on-chain canary reservation',
        ('minimal controller-signed non-disclosing exact-call reservation',)),
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
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        || transaction.admission_intent() != TransactionAdmissionIntent::Ordinary',
        '        || false', 'reject a privileged canary transaction',
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let time_to_live_ms = transaction\n        .time_to_live()',
        '    let time_to_live_ms = transaction\n        .unchecked_time_to_live()',
        'reject a canary without exact TTL validation',
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let expires_at_height = transaction\n        .expires_at_height()',
        '    let expires_at_height = transaction\n        .unchecked_expires_at_height()',
        'reject a canary without height-expiry validation',
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    let [instruction] = instructions.as_ref() else {',
        '    let [instruction, ..] = instructions.as_ref() else {',
        'reject a multi-instruction canary transaction',
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        .downcast_ref::<RecordKagemushaTairaCanaryV4>()',
        '        .downcast_ref::<AuthorizeKagemushaTairaCanaryV4>()',
        'reject a non-Record canary instruction',
        ('one exact signed ordinary nonce TTL height-expiry Record canary',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if body.binding != *expectations.binding()', '    if false',
        'reject authorization detached from the activation network',
        ('exact post-receipt promotion network and exclusive proof-corridor binding',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            .activation_finality_receipt\n            .matches_bytes(exact_receipt_bytes)',
        '            .activation_finality_receipt\n            .matches_bytes(b"")',
        'reject authorization detached from exact receipt bytes',
        ('exact post-receipt promotion network and exclusive proof-corridor binding',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        .and_then(|height| height.checked_add(1))',
        '        .and_then(|height| height.checked_add(2))',
        'reject drift in the exclusive maximum canary height',
        ('exact post-receipt promotion network and exclusive proof-corridor binding',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        'verify_evidence_signature(&self.signature, &self.body.issuer, self.body.signing_hash())?;',
        'unchecked_evidence_signature(&self.signature)?;',
        'reject unsigned canary evidence',
        ('exact issuer-signed canary evidence entrypoint',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if committed_wire != authorized_wire', '    if false',
        'reject committed canary wire detached from authorization',
        ('proof-anchored exact committed canary and contiguous post-receipt finality',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '        activation_expectations_artifact: body.activation_expectations_artifact,',
        '        activation_expectations_artifact: body.activation_finality_receipt,',
        'reject a verified canary detached from its exact expectations artifact',
        ('exact expectations provenance and canary anchor binding',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '    if receipt_terminal.finality_artifact.height.checked_add(1)',
        '    if receipt_terminal.finality_artifact.height.checked_add(2)',
        'reject a non-contiguous post-receipt finality extension',
        ('proof-anchored exact committed canary and contiguous post-receipt finality',)),
    (MODEL_CANARY_EVIDENCE_COMPONENT,
        '            || self.pipeline_transaction_intent != canary_transaction_intent',
        '            || false', 'reject activation-status substitution for canary status',
        ('canary-specific global Applied query observation',)),
    (MODEL_PROMOTION_RECEIPT_COMPONENT,
        '        || context.da_layout != runtime.genesis_context.da_layout',
        '        || false', 'reject canary finality with a different DA layout',
        ('exact four-validator DA Nexus and PoP finality corridor',)),
    (MODEL_PROMOTION_RECEIPT_COMPONENT,
        '            .any(|(actual, expected)| actual != &expected.bls_pop)',
        '            .any(|_| false)', 'reject canary finality without exact validator PoPs',
        ('exact four-validator DA Nexus and PoP finality corridor',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'if canonical_json_bytes_v1(&value)? != exact_receipt_json {', 'if false {',
        'reject a non-canonical catalog-revalidation receipt',
        ('catalog receipt decode',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        'if json_required_string_v1(object, "signer_key_id")? != trusted_authority_key_id {',
        'if false {', 'reject a receipt without exact authority key-id binding',
        ('catalog authority signature',)),
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
        ('catalog authority signature',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '            verify_kagemusha_catalog_sealed_paths_v1(&self.seal.paths, 0)?;\n        }\n        let current_time_ms = current_unix_time_ms_v1()?;',
        '            // final sealed-path recheck removed\n        }\n        let current_time_ms = current_unix_time_ms_v1()?;',
        'reject signing without a final sealed-path recheck',
        ('final signing recheck',)),
    (CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        '        validate_validator_qualification_freshness_at_v1(subject, current_time_ms)?;',
        '        // final wall-clock freshness recheck removed',
        'reject signing without a final wall-clock freshness recheck',
        ('final signing recheck',)),
    (CONFIG, 'validator_qualification_inputs != 5',
        'validator_qualification_inputs != 4',
        'reject partial validator qualification configuration',
        ('qualification config completeness',)),
    (NODE_VALIDATOR_QUALIFICATION_COMPONENT,
        '            promotion.catalog_revalidation_authority_key_id,',
        '            "detached-authority",',
        'reject daemon authority-key detachment before Core',
        ('trusted promotion forwarding',)),
    (NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT,
        '"/Library/SORA/Kagemusha/catalog-revalidation";',
        '"/tmp/kagemusha-catalog-revalidation";',
        'reject a non-fixed macOS catalog-revalidation path',
        ('fixed receipt path/platform gate',)),
    (NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT,
        '#[cfg(not(target_os = "macos"))]\npub(super) fn read_configured_kagemusha_promotion_reservation(',
        '#[cfg(any())]\npub(super) fn read_configured_kagemusha_promotion_reservation(',
        'reject a non-macOS validator-qualification bypass',
        ('fixed receipt path/platform gate',)),
    (NODE,
        'kagemusha_validator_qualification_command::KagemushaValidatorSealPublicationTarget::prepare(',
        'kagemusha_validator_qualification_command::KagemushaValidatorSealPublicationTarget::unprepared(',
        'reject publication without a prepared validator-seal destination',
        ('seal action ordering',)),
    (NODE, 'continue_after_full_kagemusha_check(\n        full_validation,',
        'continue_after_full_kagemusha_check(\n        Ok((validated_genesis, block_cadence_ms)),',
        'reject validator signing detached from full genesis validation',
        ('validation before signing',)),
    (NODE, 'action(full_validation?)', 'action(unvalidated)',
        'reject a value-carrying validation-gate bypass', ('validation result gate',)),
    (NODE, 'Some(&runtime_effective_config),', 'None,',
        'reject qualification without the runtime projection', ('validation before signing',)),
    (MODEL, 'pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 4;',
        'pub const KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT: usize = 5;',
        'reject a non-four-validator projection', ('ACTIVATION_VALIDATOR_COUNT',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'config.sumeragi.role != NodeRole::Validator', 'false',
        'reject a non-validator projection', ('permissioned four-unit runtime roster',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'context.mode != ConsensusMode::Permissioned', 'false',
        'reject a non-permissioned projection', ('permissioned four-unit runtime roster',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'context.roster.iter().any(|member| member.power != 1)', 'false',
        'reject a weighted projection roster', ('permissioned four-unit runtime roster',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'trusted.pops.get(signed_id.public_key()) == Some(signed_pop)', 'true',
        'reject projection without the exact configured PoP map', ('PoP map',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'config.network.public_address.value().clone()', 'trusted.myself.address().clone()',
        'reject projection without the advertised local public address', ('advertised endpoint',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'Duration::from_millis(metadata.block_cadence_ms.get())', 'Duration::from_secs(1)',
        'reject projection without signed-metadata cadence', ('signed cadence',)),
    (CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
        'projection.validate().map_err(|error| error.to_string())?;', 'let _ = &projection;',
        'reject an unvalidated runtime projection',
        ('advertised endpoint, signed cadence, and validated projection',)),
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
        ('activation-bound exact reservation and signed-wire marker',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '    let wire = transaction.encode_wire_v1().map_err(|error| {',
        '    let wire = unchecked_canary_wire(transaction).map_err(|error| {',
        'reject canary identity detached from the complete signed wire',
        ('complete signed canary wire bound at all transaction boundaries',)),
    (CORE_COMMITTED_TX_CONTEXT,
        'signed_kagemusha_taira_canary_wire_identity_v1(transaction)',
        'unchecked_kagemusha_taira_canary_wire_identity_v1(transaction)',
        'reject replay without the exact committed canary wire',
        ('complete signed canary wire bound at all transaction boundaries',)),
    (CORE_STATE,
        '                crate::state::seed_committed_transaction_context(',
        '                crate::state::unchecked_committed_transaction_context(',
        'reject a detached committed replay context call',
        ('complete signed canary wire bound at all transaction boundaries',)),
    (CORE_BLOCK,
        'crate::smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1(tx)',
        'crate::smartcontracts::isi::offline::unchecked_kagemusha_taira_canary_wire_identity_v1(tx)',
        'reject block admission without the exact canary wire',
        ('complete signed canary wire bound at all transaction boundaries',)),
    (CORE_EXECUTOR,
        '            signed_kagemusha_taira_canary_wire_identity_v1(&transaction)?;',
        '            unchecked_kagemusha_taira_canary_wire_identity_v1(&transaction)?;',
        'reject sequential execution without the exact canary wire',
        ('complete signed canary wire bound at all transaction boundaries',)),
    (CORE_EXECUTOR,
        '        state_transaction.kagemusha_taira_canary_wire_identity = None;',
        '        // stale Kagemusha signed-wire identity retained',
        'reject stale signed-boundary context before executor validation',
        ('complete signed canary wire bound at all transaction boundaries',)),
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
        ('consensus-authorized exact-wire one-shot canary execution',), 2),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        (true, true, true, true) => Ok(None),',
        '        (true, true, true, true) => Err(labeled_invariant("unchecked", "unchecked").into()),',
        'reject loss of same-exact reservation idempotence',
        ('activation-bound exact reservation and signed-wire marker',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '    let reservation_bytes = norito::encode_canonical(reservation).map_err(|error| {',
        '    let reservation_bytes = norito::encode_canonical(reservation.permit()).map_err(|error| {',
        'reject idempotence detached from the complete signed reservation bytes',
        ('activation-bound exact reservation and signed-wire marker',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        .insert(exact_wire, ());', '        .insert(exact_call, ());',
        'reject authorization detached from the complete signed-wire marker',
        ('activation-bound exact reservation and signed-wire marker',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '            .kagemusha_taira_canary_wire_identity',
        '            .unchecked_kagemusha_taira_canary_wire_identity',
        'reject canary execution without its admitted complete wire',
        ('consensus-authorized exact-wire one-shot canary execution',)),
    (CORE_KAGEMUSHA_CANARY_COMPONENT,
        '        commit_v4_taira_canary(marker, state_transaction);',
        '        // promotion one-shot commit removed',
        'reject a canary that does not consume the promotion one-shot',
        ('consensus-authorized exact-wire one-shot canary execution',)),
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
        ('rollout-v4 offline CLI wiring and credential separation',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT, KAGEMUSHA_ROLLOUT_LIVENESS_MODULE,
        '// validator-liveness module detached',
        'authenticate the validator-liveness module wiring',
        ('mod liveness;',)),
    (OFFLINE_CLI, '#[command(name = "rollout-v4")]',
        '#[command(name = "unchecked-rollout-v4")]',
        'reject substituted rollout-v4 CLI wiring',
        ('rollout-v4 offline CLI wiring and credential separation',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'matches!(&self.command, Command::CreateExpectations(_))',
        'matches!(&self.command, Command::Submit(_))',
        'reject fallback credentials outside expectations creation',
        ('eight phase rollout with isolated create-only fallback credentials',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::Submit(args) => args.run(context),',
        '            Command::Submit(args) => Command::CreateExpectations(args).run(context),',
        'reject collapsed rollout phase dispatch',
        ('eight phase rollout with isolated create-only fallback credentials',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::SubmitCanaryAuthorization(args) => args.run(context),',
        '            Command::SubmitCanaryAuthorization(args) => Command::SubmitCanary(args).run(context),',
        'reject detached canary-reservation dispatch',
        ('eight phase rollout with isolated create-only fallback credentials',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::SubmitCanary(args) => args.run(context),',
        '            Command::SubmitCanary(args) => Command::Submit(args).run(context),',
        'reject detached canary submission dispatch',
        ('eight phase rollout with isolated create-only fallback credentials',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::FinalizeCanaryEvidence(args) => args.run(context),',
        '            Command::FinalizeCanaryEvidence(args) => Command::FinalizeReceipt(args).run(context),',
        'reject detached canary-finalization dispatch',
        ('eight phase rollout with isolated create-only fallback credentials',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            Command::FinalizeValidatorLiveness(args) => args.run(context),',
        '            Command::FinalizeValidatorLiveness(args) => Command::FinalizeCanaryEvidence(args).run(context),',
        'reject detached validator-liveness dispatch',
        ('eight phase rollout with isolated create-only fallback credentials',)),
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
        ('bounded no-follow root-owned read with stable metadata and ACL/xattr checks',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    require_no_xattrs(&file, label)?;\n    require_no_macos_acl(&file, label)?;\n    let mut bytes',
        '    require_no_xattrs(&file, label)?;\n    // ACL check removed\n    let mut bytes',
        'reject a trusted rollout read without its ACL check',
        ('bounded no-follow root-owned read with stable metadata and ACL/xattr checks',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        rustix::fs::RenameFlags::NOREPLACE,',
        '        rustix::fs::RenameFlags::empty(),',
        'reject replace-capable rollout publication',
        ('root-owned no-replace fsync publication with commit uncertainty',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        staging.sync_all().map_err(|error| error.to_string())?;',
        '        // pre-rename staging fsync removed',
        'reject rollout publication without a pre-rename fsync',
        ('root-owned no-replace fsync publication with commit uncertainty',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        'KagemushaV4ActivationReceiptExpectationsArtifactV1::try_sign(body, &controller_key)',
        'KagemushaV4ActivationReceiptExpectationsArtifactV1::new_unchecked(body, &controller_key)',
        'reject unsigned activation expectations',
        ('deferred signing and exact reverification before expectations publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            .verify_exact(&bytes, &controller, &reservation_bytes)',
        '            .verify(&controller)',
        'reject expectations publication without exact reverification',
        ('deferred signing and exact reverification before expectations publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published expectations report failed: {error}"),',
        '                detail: format!("ordinary expectations output failure: {error}"),',
        'reject ordinary-error reporting after expectations publication',
        ('commit-uncertain expectations publication reporting',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if bytes != loaded.exact_bytes {', '    if false {',
        'reject a digest-only or mismatched submission journal',
        ('exact signed expectations journal read, publication, and reverification',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    publish_root_owned(path, &loaded.exact_bytes, |published| {',
        '    publish_root_owned(path, b"digest-only", |published| {',
        'reject a journal that does not durably publish exact expectations',
        ('exact signed expectations journal read, publication, and reverification',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        (SubmissionJournalObservation::Absent, true) => {',
        '        (SubmissionJournalObservation::Absent, false) => {',
        'reject retrospective submission without a journal',
        ('retrospective refusal and matching-journal safe-resume decision',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        (SubmissionJournalObservation::Matching, _) => Ok(SubmissionJournalAction::Resume),',
        '        (SubmissionJournalObservation::Matching, _) => Ok(SubmissionJournalAction::Publish),',
        'reject destructive publication on matching-journal resume',
        ('retrospective refusal and matching-journal safe-resume decision',)),
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
        ('journal-bound status identity uncertainty',)),
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
        ('explicit submission uncertainty after configured wait ambiguity',), -1),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    require_journal_bound_wait_outcome(&outcome, transaction, journal_path)?;',
        '    require_unbound_wait_outcome(&outcome, transaction)?;',
        'reject an unbound terminal wait outcome',
        ('explicit submission uncertainty after configured wait ambiguity',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if outcome.terminal_kind != outcome.r#final.status.kind',
        '    if false',
        'reject inconsistent terminal wait summary and final status',
        ('journal-bound status identity uncertainty',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            require_journal_bound_status_response(\n                &status,\n                transaction,\n                journal_path,\n                "failed wait status identity reconciliation",\n            )?;',
        '            require_status_response_hash(&status, transaction)?;',
        'reject an unbound failed-wait status hash',
        ('proof or explicit submission uncertainty after failed wait reconciliation',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    let proof_anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(',
        '    let proof_anchor = UntrustedBlockHint::from_untrusted_finality_artifact(',
        'reject Applied acceptance without a trusted block proof anchor',
        ('bounded full finality chain and trusted canonical block entrypoint proof',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        || decode_framed_signed_block(&block_bytes),',
        '        || decode_unframed_block(&block_bytes),',
        'reject noncanonical finalized SignedBlock wire',
        ('bounded full finality chain and trusted canonical block entrypoint proof',)),
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
        ('deferred issuer signing and proof-verified no-replace final receipt',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        require_matching_submission_journal(inspect_submission_journal(&journal_path, &loaded)?)?;',
        '        // exact matching submission journal check removed',
        'reject finalization without the exact submission journal',
        ('deferred issuer signing and proof-verified no-replace final receipt',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        require_journal_bound_status_response(\n            &status,\n            transaction,\n            &journal_path,\n            "finalize status identity reconciliation",\n        )?;',
        '        require_status_response_hash(&status, transaction)?;',
        'reject unbound status identity during receipt finalization',
        ('deferred issuer signing and proof-verified no-replace final receipt',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_canonical(published)',
        '            let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_unchecked(published)',
        'reject a final receipt without canonical publication readback',
        ('deferred issuer signing and proof-verified no-replace final receipt',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                detail: format!("published receipt report failed: {error}"),',
        '                detail: format!("ordinary receipt output failure: {error}"),',
        'reject ordinary-error reporting after final receipt publication',
        ('commit-uncertain final-receipt publication reporting',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    /// Explicit authorization for this production canary network write.\n    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]',
        '    /// Explicit authorization for this production canary network write.\n    #[arg(long, action = clap::ArgAction::SetTrue)]',
        'reject an optional production-canary write authorization',
        ('explicit write authorization before both canary network phases',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        if !self.write_authorized {\n            bail!("--write-authorized is required for production canary submission");',
        '        if false {\n            bail!("--write-authorized is required for production canary submission");',
        'reject production canary submission without runtime write authorization',
        ('explicit write authorization before both canary network phases',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        preflight_root_owned_output(&self.output)?;\n\n        let ttl_ms = self.canary_ttl_ms.get();',
        '        // authorization destination preflight removed\n\n        let ttl_ms = self.canary_ttl_ms.get();',
        'reject sensitive authorization work before output preflight',
        ('fresh exact permit Record authorization creation and private no-replace publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        client.add_transaction_nonce = true;',
        '        client.add_transaction_nonce = false;',
        'reject canary authorization without a transaction nonce',
        ('fresh exact permit Record authorization creation and private no-replace publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let canonical_torii_origin = canonical_torii_origin(&client.torii_url)?;',
        '        let canonical_torii_origin = client.torii_url.to_string();',
        'reject a controller authorization with an uncanonical Torii origin',
        ('fresh exact permit Record authorization creation and private no-replace publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let controller_key = load_root_custodied_key(\n            &self.controller_private_key_file,\n            "promotion-controller key",',
        '        let controller_key = load_operator_key_pair(\n            &self.controller_private_key_file,\n            "promotion-controller key",',
        'reject controller signing outside deferred root custody',
        ('fresh exact permit Record authorization creation and private no-replace publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    let verification_time_unix_ms = artifact.permit().body.authorized_at_unix_ms;',
        '    let verification_time_unix_ms = current_unix_ms()?;',
        'reject structural authorization reconciliation after wall expiry',
        ('structural expired-journal reconciliation and fresh precommit publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '    if bytes != authorization.exact_bytes {', '    if false {',
        'reject a digest-only canary submission journal',
        ('structural expired-journal reconciliation and fresh precommit publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                    .get_transaction_status_response_auto(transaction.hash())\n                    .wrap_err(',
        '                    .get_transaction_status_response_auto(authorization.verified.canary_transaction().hash())\n                    .wrap_err(',
        'reject canary-journal publication without proof that the reservation is absent',
        ('private canary journal committed before minimal reservation disclosure and POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                    .get_transaction_status_response_auto(\n                        authorization.verified.canary_transaction().hash(),\n                    )',
        '                    .get_transaction_status_response_auto(transaction.hash())',
        'reject canary-journal publication without proof that the canary is absent',
        ('private canary journal committed before minimal reservation disclosure and POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let verification_time_unix_ms = current_unix_ms().map_err(|error| error.to_string())?;',
        '        let verification_time_unix_ms = authorization.verified.authorized_at_unix_ms();',
        'reject stale authorization at journal publication',
        ('structural expired-journal reconciliation and fresh precommit publication',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            let fresh_time = current_unix_ms()?;\n            authorization',
        '            let fresh_time = authorization.verified.authorized_at_unix_ms();\n            authorization',
        'reject stale authorization immediately before POST',
        ('precommitted exact journal and fresh verification before canary POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '                publish_canary_submission_journal(\n                    &canary_journal_path,',
        '                // durable canary journal publication removed\n                let _ = &canary_journal_path;',
        'reject canary POST reachability before durable exact journal publication',
        ('private canary journal committed before minimal reservation disclosure and POST',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        || canonical_torii_origin(&client.torii_url)?\n            != authorization.verified.canonical_torii_origin()',
        '        || false',
        'reject canary submission through a different origin',
        ('exact canary network authority and HTTPS origin client binding',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            expectations.binding().promotion_id,\n            CANARY_EVIDENCE_FILE_NAME,',
        '            [0; 32],\n            CANARY_EVIDENCE_FILE_NAME,',
        'reject a canary evidence leaf detached from its promotion',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            CANARY_EVIDENCE_FILE_NAME,\n        )?;\n        preflight_root_owned_output(&self.output)?;\n        let journal_path = rollout_state_path(',
        '            CANARY_EVIDENCE_FILE_NAME,\n        )?;\n        // evidence destination preflight removed\n        let journal_path = rollout_state_path(',
        'reject live canary queries before evidence output preflight',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let pipeline_status = client\n            .get_transaction_status_response_auto(transaction.hash())',
        '        let pipeline_status = client\n            .get_transaction_status_response_auto(expectations.activation_transaction_intent())',
        'reject activation-status substitution for canary status',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        if pipeline_status.status.kind != "Applied"\n            || pipeline_status.scope != "global"',
        '        if pipeline_status.status.kind != "Applied"\n            || false',
        'reject local canary pipeline status',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            committed_transaction: fresh.committed.clone(),',
        '            committed_transaction: activation_committed.clone(),',
        'reject digest-only evidence without the full committed canary',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '            finality_proof_chain: finality_proof_chain.clone(),',
        '            finality_proof_chain: receipt.body.finality_proof_chain.clone(),',
        'reject canary evidence without its post-receipt proof extension',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        let evidence = KagemushaV4TairaCanaryEvidenceV1::try_sign(',
        '        let evidence = KagemushaV4TairaCanaryEvidenceV1::new_unchecked(',
        'reject unsigned full canary evidence',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
    (KAGEMUSHA_ROLLOUT_COMPONENT,
        '        publish_root_owned(&self.output, &bytes, |published| {\n            let evidence = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(published)',
        '        fs::write(&self.output, &bytes)?;\n        {\n            let evidence = KagemushaV4TairaCanaryEvidenceV1::decode_canonical(&bytes)',
        'reject replace-capable canary publication',
        ('promotion-keyed full canary wire block proof evidence and issuer signature',)),
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
        ('exact expectations provenance and canary anchor binding',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || canary_anchor.activation_finality_receipt\n                != verified_canary.activation_finality_receipt()',
        '            || false',
        'reject a liveness anchor detached from the exact activation receipt',
        ('exact expectations provenance and canary anchor binding',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || canary_anchor.canary_transaction_wire != verified_canary.canary_transaction_wire()',
        '            || false',
        'reject a liveness anchor detached from the exact canary wire',
        ('exact expectations provenance and canary anchor binding',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        'previous.is_some_and(|id: &PeerId| id >= &target.validator_id)',
        'previous.is_some_and(|id: &PeerId| id > &target.validator_id)',
        'reject duplicate validator identities in the liveness challenge',
        ('issuer-signed fresh canary challenge with four distinct qualified targets',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '        attestation\n            .verify()',
        '        attestation\n            .verify_structure_only()',
        'reject unsigned validator liveness attestations',
        ('four signed validator identities with shared canary-rooted finality and exact tips',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '            || attestation_body.node_id != trust.validator_ids[index]',
        '            || false',
        'reject a validator attestation under the wrong node identity',
        ('four signed validator identities with shared canary-rooted finality and exact tips',)),
    (MODEL_CANARY_LIVENESS_COMPONENT,
        '        if tip != expected {', '        if false {',
        'reject validator tips detached from the shared verified chain',
        ('four signed validator identities with shared canary-rooted finality and exact tips',)),
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
        ('root-TCB pinned controller, Kagami, policy, candidate, and snapshot launch',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        .and_then(fs::canonicalize)',
        '        .map(PathBuf::from)',
        'reject an uncanonicalized controller executable identity',
        ('root-TCB pinned controller, Kagami, policy, candidate, and snapshot launch',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '    let mut kagami_pin = kagemusha_python_launcher::pin_regular(&kagami, request.kagami_sha256)?;',
        '    let mut kagami_pin = kagemusha_python_launcher::pin_regular(&kagami, [0; 32])?;',
        'reject Kagami detached from its requested digest pin',
        ('root-TCB pinned controller, Kagami, policy, candidate, and snapshot launch',)),
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
        ('bounded pre-read and post-read descriptor validation',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '        validate_bounded_identity(name, &after, bounds)?;',
        '        // post-read descriptor bounds check removed',
        'reject missing post-hash metadata and size validation',
        ('bounded pre-read and post-read descriptor validation',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        'const CANDIDATE_FILES: [CandidateFileSpec; 16] = [',
        'const CANDIDATE_FILES: [CandidateFileSpec; 15] = [',
        'reject a fifteen-file promotion candidate',
        ('CANDIDATE_FILES', 'candidate inventory declaration is not exact sixteen')),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '[(name, identity)] if valid_temp_name(name) => {',
        '[(name, identity)] if name.starts_with(TEMP_PREFIX) => {',
        'reject a non-exact temporary publication leaf',
        ('exact candidate, one-temporary, or one-final inventory state machine',)),
    (KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        '            let (_, identity) = open_member(&self.directory, &name, bounds, false)?;',
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
        ('private no-replace durable promotion-record publication',)),
    (KAGAMI, '        sync_parent(&parent.file).wrap_err("failed to durably sync promotion-record parent")?;',
        '        // promotion-record parent durability removed',
        'reject promotion-record publication without parent durability',
        ('private no-replace durable promotion-record publication',)),
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
    (KAGAMI, 'if expected.len() != 17', 'if expected.len() != 16', 'reject a sixteen-file final release verifier'),
    (KAGAMI, '        (\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n            "qualification receipt",\n        ),\n',
        '', 'reject a verifier inventory without the qualification receipt', ('17-file verifier inventory',)),
    (BUNDLE, 'const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 17;', 'const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 16;', 'reject a sixteen-file final release producer'),
    (BUNDLE, '            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n            PROMOTION_RECORD_FILE_NAME_V4,\n',
        '            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,\n            PROMOTION_RECORD_FILE_NAME_V4,\n',
        'reject a producer inventory without the qualification receipt', ('function-scoped 17-file producer inventory',)),
    (BUNDLE, 'fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()',
        'fn retired_final_release_inventory_test()', 'reject a missing producer inventory test',
        ('fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()',)),
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
    (CORE, 'change_release.as_ref().is_some_and(|release|', 'change_release.as_ref().is_none_or(|release|',
        'reject an unguarded offline-change issuance path', ('offline-change issuance window',)),
    (WORKFLOW, 'ci/check_kagemusha_production_readiness_source_support.py',
        'ci/retired_kagemusha_production_readiness_source_support.py',
        'reject a missing readiness source-support workflow filter',
        ('ci/check_kagemusha_production_readiness_source_support.py',)),
    (WORKFLOW, 'ci/check_kagemusha_recursion_source_contract.py',
        'ci/retired_kagemusha_recursion_source_contract.py',
        'reject a missing recursion source-contract workflow filter',
        ('ci/check_kagemusha_recursion_source_contract.py',)),
    (WORKFLOW, 'cargo test -p iroha_core output_membership --lib', 'cargo test -p iroha_core retired_output_membership_filter --lib',
        'reject a missing frontier-test workflow filter', ('cargo test -p iroha_core output_membership --lib',)),
)
for mutation in static_mutations:
    expect_static_mutation(*mutation)
for (required_filter, retired_filter, label) in (('cargo test -p iroha_data_model receiver_snapshot --lib',
    'cargo test -p iroha_data_model retired_receiver_snapshot_filter --lib', 'receiver-snapshot data-model'),
    ('cargo test -p iroha_core kagemusha_online_registration_ --lib', 'cargo test -p iroha_core retired_registration_filter --lib',
    'compact registration'), ('cargo test -p iroha_core active_receiver_snapshot_ --lib',
    'cargo test -p iroha_core retired_active_receiver_filter --lib', 'active-receiver resolver'),
    ('cargo test -p iroha_kagami --bin kagami atomic_activation_', 'cargo test -p iroha_kagami --bin kagami retired_activation_filter',
    'activation-policy parity'), ('cargo test -p iroha_kagami --bin kagami backing_',
    'cargo test -p iroha_kagami --bin kagami retired_backing_filter', 'ordered Taira backing')):
    expect_static_mutation(WORKFLOW, required_filter, retired_filter, f'reject a missing {label} workflow filter', (required_filter,))
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
    'reject a verifier sharing the readiness process group', ('authenticated Kagami verifier execution',)),
    ('        "--deny-all-writes",', '        "--allow-ambient-writes",',
    'reject a verifier controller with ambient filesystem writes', ('authenticated Kagami verifier execution',)),
    ('        "--deny-read-outside-allowlist",', '        "--allow-ambient-reads",',
    'reject a verifier controller with ambient filesystem reads', ('authenticated Kagami verifier execution',)),
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
    ('historical consumption plus separate current catalog validator boundary',)),
    ('            "validate_catalog_revalidation_receipt"',
    '            "validate_catalog_without_fresh_status"',
    'reject removing current catalog revalidation',
    ('historical consumption plus separate current catalog validator boundary',)),
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
