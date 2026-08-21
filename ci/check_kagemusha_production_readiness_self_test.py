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
baseline = {READINESS: read(READINESS, []),
    READINESS_SOURCE_CONTRACT: read(READINESS_SOURCE_CONTRACT, []),
    READINESS_SELF_TEST: read(READINESS_SELF_TEST, []),
    MODEL: read_reviewed_model([], {}), MODEL_COMPONENT: read(MODEL_COMPONENT, []),
    MODEL_VERIFIER_COMPONENT: read(MODEL_VERIFIER_COMPONENT, []), PRIVACY: read(PRIVACY, []), PRIVACY_PROTOCOL: read(PRIVACY_PROTOCOL,
    []), CATALOG: read_reviewed_catalog([], {}), CORE: read(CORE, []), KAGAMI: read(KAGAMI, []), BUNDLE: read(BUNDLE, []),
    BUNDLE_SOURCE_SEAL_INPUTS: read(BUNDLE_SOURCE_SEAL_INPUTS, []),
    WORKFLOW: read(WORKFLOW, []), PRODUCTION_IOS_EVIDENCE_MODULE: read(PRODUCTION_IOS_EVIDENCE_MODULE, [])}
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
    (READINESS_SOURCE_CONTRACT,
        'globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1") is not True',
        'False', 'reject a detached readiness source-contract provider',
        ('_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1',), -1),
    (READINESS_SELF_TEST, 'globals().get("_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1") is not True',
        'False', 'reject a detached readiness self-test helper', ('_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1',), -1),
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
        '            require_production_root_custody(descriptor, label)\n        except BaseException:\n            os.close(descriptor)\n            raise\n        source_contract_bytes = read_pinned_descriptor(',
        '            # source-contract provider custody removed\n        except BaseException:\n            os.close(descriptor)\n            raise\n        source_contract_bytes = read_pinned_descriptor(',
        'reject a source-contract provider without root custody',
        ('root-custodied source-closure-authenticated source-contract bytes',)),
    (READINESS,
        '    source_contract_bytes = authenticated_readiness_source_contract_bytes',
        '    source_contract_bytes = (root / READINESS_SOURCE_CONTRACT).read_bytes()',
        'reject a promotion path reopening the source-contract provider',
        ('authenticated byte-only readiness source-contract dispatch',
         'exactly one candidate-only path read')),
    (READINESS,
        '        code = compile(\n            source_contract_bytes,\n            READINESS_SOURCE_CONTRACT,',
        '        code = compile(\n            (root / READINESS_SOURCE_CONTRACT).read_bytes(),\n            READINESS_SOURCE_CONTRACT,',
        'reject execution of reopened source-contract provider bytes',
        ('authenticated byte-only readiness source-contract dispatch',
         'exactly one candidate-only path read')),
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
    (KAGAMI, 'if expected.len() != 17', 'if expected.len() != 16', 'reject a sixteen-file final release verifier'),
    (KAGAMI, '        (\n            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,\n            "qualification receipt",\n        ),\n',
        '', 'reject a verifier inventory without the qualification receipt', ('function-scoped 17-file verifier inventory',)),
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
        'reject an unguarded offline-change issuance path', ('offline-change withdrawal-height issuance check',)),
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
