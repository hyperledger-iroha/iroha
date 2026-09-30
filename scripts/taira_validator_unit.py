#!/usr/bin/env python3
"""Render a Taira validator systemd unit without reading runtime signing inputs.

Requires Python 3.10+ and explicit absolute Linux paths to retained owner-private
signers. Output is a new public mode-0644 unit, ready for native inventory
assembly and never installed or overwritten here. Referenced signers stay private.
Validate it with systemd-analyze verify on the deployment host and bind its exact
bytes into the native public-reset inventory before installation.

Type=exec waits for systemd to execute the inline custody launcher. Native
process attestation and HTTP readiness still wait for the actual daemon.
The public beacon credential path is optional only during initial key setup;
configured beacon custody uses FD 200 and is required for beacon readiness.
Initial units use config.toml; --config-file beacon.toml selects the separately
authenticated provider-config transition without changing the initial artifacts.

First boot (Sumeragi record provenance, specs/sumeragi.md section 7.4): every
unit starts iroha3d_taira --config ... --sora and adds
--sumeragi-assert-fresh-key only when its launcher consumes the one-shot token
/var/lib/taira/ROLE/sumeragi-first-boot. Missing safety history never
authorizes the assertion: only --arm-first-boot creates the token, run as root
on the stopped validator host before the first start of its fresh state root,
and it refuses when the configured sumeragi-records directory or
sumeragi-installation.log exists. The launcher refuses to start while a token
sits beside existing history, and removes the token durably before it execs the
daemon, so no restart repeats the assertion and no lost record store renews it.
"""
import argparse
import os
from pathlib import Path, PurePosixPath
import stat

CUSTODY = '''reserved_fds = (198, 199, 200)
for reserved_fd in reserved_fds:
    try:
        os.fstat(reserved_fd)
    except OSError as error:
        if error.errno != errno.EBADF: raise
    else:
        raise RuntimeError("Taira signer descriptor is already occupied")
staged = []

def armed_first_boot():
    # Only the explicit first-boot step creates this token. Missing safety
    # history alone never authorizes the fresh-key assertion.
    try:
        token = os.lstat(first_boot_token)
    except FileNotFoundError:
        return None
    root = os.lstat(state_root)
    if not stat.S_ISDIR(root.st_mode) or root.st_uid != os.geteuid() or root.st_mode & 0o022:
        raise RuntimeError("untrusted Taira validator state root")
    if not stat.S_ISREG(token.st_mode) or token.st_uid != os.geteuid() or token.st_mode & 0o7777 != 0o600 or token.st_nlink != 1 or token.st_size != 0:
        raise RuntimeError("untrusted Taira first-boot token")
    for history in sumeragi_history:
        try:
            os.lstat(history)
        except FileNotFoundError:
            continue
        raise RuntimeError("Taira first boot refused: Sumeragi safety history already exists")
    return (token.st_dev, token.st_ino)

def sync_state_root():
    root_fd = os.open(state_root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0))
    try: os.fsync(root_fd)
    finally: os.close(root_fd)

# Refuse an unusable first boot before any signer copy exists.
first_boot = armed_first_boot()
first_boot_consumed = False

def stage_signer(source_path, target_fd, expected_size, label):
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    source_fd = os.open(source_path, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | nofollow)
    if source_fd in reserved_fds:
        os.close(source_fd)
        raise RuntimeError("source open reached a reserved Taira signer descriptor")
    launch_path = source_path + ".fd" + str(target_fd)
    launch_fd = None
    launch_created = False
    launch_ready = False
    secret = bytearray()
    secret_view = None
    try:
        source_before = os.fstat(source_fd)
        size = source_before.st_size
        if not stat.S_ISREG(source_before.st_mode) or source_before.st_uid != os.geteuid() or source_before.st_mode & 0o7777 != 0o600 or source_before.st_nlink != 1:
            raise RuntimeError("untrusted persistent Taira " + label + " file")
        if (expected_size is not None and size != expected_size) or (expected_size is None and not 0 < size <= 16 * 1024 * 1024):
            raise RuntimeError("invalid persistent Taira " + label + " length")
        secret = bytearray(size)
        secret_view = memoryview(secret)
        try:
            stale = os.lstat(launch_path)
        except FileNotFoundError:
            stale = None
        if stale is not None:
            if not stat.S_ISREG(stale.st_mode) or stale.st_uid != os.geteuid() or stale.st_mode & 0o7777 != 0o600 or stale.st_nlink != 1 or stale.st_size not in (0, size):
                raise RuntimeError("untrusted stale Taira FD" + str(target_fd) + " launch file")
            os.unlink(launch_path)
        launch_fd = os.open(launch_path, os.O_RDWR | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | nofollow, 0o600)
        launch_created = True
        if launch_fd in reserved_fds:
            raise RuntimeError("launch open reached a reserved Taira signer descriptor")
        offset = 0
        while offset < len(secret):
            count = os.readv(source_fd, [secret_view[offset:]])
            if count == 0: raise RuntimeError("short Taira " + label + " source")
            offset += count
        source_after = os.fstat(source_fd)
        stable_fields = ("st_dev", "st_ino", "st_uid", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
        if any(getattr(source_before, field) != getattr(source_after, field) for field in stable_fields):
            raise RuntimeError("Taira " + label + " source changed while staging")
        offset = 0
        while offset < len(secret):
            count = os.write(launch_fd, secret_view[offset:])
            if count == 0: raise RuntimeError("short Taira FD" + str(target_fd) + " launch write")
            offset += count
        os.fsync(launch_fd)
        os.lseek(launch_fd, 0, os.SEEK_SET)
        launch_stat = os.fstat(launch_fd)
        if not stat.S_ISREG(launch_stat.st_mode) or launch_stat.st_uid != os.geteuid() or launch_stat.st_mode & 0o7777 != 0o600 or launch_stat.st_nlink != 1 or launch_stat.st_size != size:
            raise RuntimeError("untrusted Taira FD" + str(target_fd) + " launch file")
        os.dup2(launch_fd, target_fd, inheritable=True)
        staged.append((target_fd, launch_path))
        launch_ready = True
    finally:
        for index in range(len(secret)): secret[index] = 0
        if secret_view is not None: secret_view.release()
        # A write/fsync/dup2 failure can leave a full copy before it joins
        # staged. Erase that owned copy while its descriptor is still open.
        if launch_created and not launch_ready and launch_fd is not None:
            try:
                os.ftruncate(launch_fd, 0)
                os.fsync(launch_fd)
            except OSError: pass
        try: os.close(source_fd)
        except OSError: pass
        if launch_fd is not None:
            try: os.close(launch_fd)
            except OSError: pass
        if launch_created and not launch_ready:
            try: os.unlink(launch_path)
            except OSError: pass

try:
    stage_signer(runtime_key, 198, 71, "runtime signer")
    stage_signer(mint_finality_seed, 199, 32, "mint-finality seed")
    if global_beacon_credential is not None:
        stage_signer(global_beacon_credential, 200, None, "global-beacon credential")
    launch_argv = cmd
    if first_boot is not None:
        if armed_first_boot() != first_boot:
            raise RuntimeError("Taira first-boot token changed while staging")
        # Consume the token durably before the daemon exists: no later start,
        # even one that finds no safety history, repeats the assertion.
        os.unlink(first_boot_token)
        first_boot_consumed = True
        sync_state_root()
        launch_argv = cmd + ['--sumeragi-assert-fresh-key']
    os.execv(cmd[0], launch_argv)
finally:
    # Successful foreground exec never returns. Failed staging/exec must not
    # leave an extra full seed copy or close an unrelated inherited descriptor.
    for staged_fd, staged_path in reversed(staged):
        try:
            os.ftruncate(staged_fd, 0)
            os.fsync(staged_fd)
        except OSError:
            pass
        try: os.close(staged_fd)
        except OSError: pass
        try: os.unlink(staged_path)
        except OSError: pass
    if first_boot_consumed:
        # The daemon never ran, so its unused first-boot token is restored.
        token_fd = os.open(first_boot_token, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0), 0o600)
        try:
            os.fchmod(token_fd, 0o600)
            os.fsync(token_fd)
        finally:
            os.close(token_fd)
        sync_state_root()
'''

ROLES = tuple(f"taira-validator-{index}" for index in range(1, 5))
CONFIG_FILES = ("config.toml", "beacon.toml")
STATE_ROOT_PARENT = "/var/lib/taira"
FIRST_BOOT_TOKEN = "sumeragi-first-boot"
# `[sumeragi] records_dir` and `installation_log` of the native Taira validator
# config (`iroha taira public-reset materialize-validator-config`), inside the
# validator state root.
SUMERAGI_HISTORY = ("sumeragi-records", "sumeragi-installation.log")


def checked_path(value, label):
    path = PurePosixPath(value)
    if (not value.startswith("/") or value.startswith("//") or str(path) != value
            or ".." in path.parts or any(ord(char) < 32 or ord(char) == 127 for char in value)
            or path.name in ("", ".", "..")):
        raise ValueError(label + " must be an explicit canonical absolute Linux path")
    return value


def validator_state_root(role):
    if role not in ROLES:
        raise ValueError("unknown validator role")
    return STATE_ROOT_PARENT + "/" + role


def first_boot_paths(role, state_root=None):
    """Return the state root, the first-boot token and the Sumeragi history paths.

    Units always use /var/lib/taira/ROLE; tests pass a disposable canonical root.
    """
    root = validator_state_root(role)
    if state_root is not None:
        root = checked_path(state_root, "validator state root")
    return root, root + "/" + FIRST_BOOT_TOKEN, tuple(root + "/" + name for name in SUMERAGI_HISTORY)


def launcher(role, runtime_key, mint_finality_seed, global_beacon_credential=None, *,
             config_file="config.toml", state_root=None):
    if role not in ROLES:
        raise ValueError("unknown validator role")
    if config_file not in CONFIG_FILES:
        raise ValueError("config file must be config.toml or beacon.toml")
    runtime_key = checked_path(runtime_key, "signer input")
    mint_finality_seed = checked_path(mint_finality_seed, "signer input")
    state_root, first_boot_token, sumeragi_history = first_boot_paths(role, state_root)
    paths = (runtime_key, runtime_key + ".fd198", mint_finality_seed, mint_finality_seed + ".fd199",
             first_boot_token, *sumeragi_history)
    if global_beacon_credential is not None:
        global_beacon_credential = checked_path(global_beacon_credential, "signer input")
        paths += (global_beacon_credential, global_beacon_credential + ".fd200")
    if len(set(paths)) != len(paths):
        raise ValueError("retained signer, launch-copy and first-boot paths must all be distinct")
    current = f"/srv/taira/{role}/current"
    cmd = [current + "/bin/iroha3d_taira", "--config", current + "/config/" + config_file, "--sora"]
    # Native consumers truncate only their independent owner-private, single-link
    # RW launch copies. Retained native sources remain intact for restart.
    return ("import errno\nimport os\nimport stat\n"
            + f"runtime_key = {runtime_key!r}\nmint_finality_seed = {mint_finality_seed!r}\n"
            + f"global_beacon_credential = {global_beacon_credential!r}\n"
            + f"state_root = {state_root!r}\nfirst_boot_token = {first_boot_token!r}\n"
            + f"sumeragi_history = {sumeragi_history!r}\n"
            + f"cmd = {cmd!r}\n" + CUSTODY)


def systemd_argument(value):
    # systemd.syntax C escaping, then systemd.exec specifier/environment escaping.
    # This is one quoted argv element, without a shell or external launcher file.
    out = value.replace("\\", "\\\\").replace('"', '\\"')
    out = out.replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")
    out = out.replace("%", "%%").replace("$", "$$")
    return '"' + out + '"'


def render(role, runtime_key, mint_finality_seed, global_beacon_credential=None, *, config_file="config.toml"):
    code = launcher(role, runtime_key, mint_finality_seed, global_beacon_credential, config_file=config_file)
    compile(code, "<signed-unit-inline-python>", "exec")
    return (f"[Unit]\nDescription=Taira {role}\nAfter=network.target\n\n"
            "[Service]\nType=exec\nUser=root\nGroup=root\nUMask=0077\n"
            f"WorkingDirectory={validator_state_root(role)}\n"
            "CPUQuota=100%\nMemoryMax=2147483648\nRestart=on-failure\nRestartSec=5s\n"
            f"ExecStart=/usr/bin/python3 -c {systemd_argument(code)}\n"
            "\n[Install]\nWantedBy=multi-user.target\n")


def arm_first_boot(role, *, state_root=None):
    """Create the one-shot token that makes the unit's next start assert the fresh key.

    Run as root on the validator host, with the validator stopped, before the
    first start of its fresh state root. Refuses unless the state root is a
    directory of this user that nobody else can write and neither Sumeragi
    history path exists; never replaces an existing token.
    """
    root, token, history = first_boot_paths(role, state_root)
    info = os.lstat(root)
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or info.st_mode & 0o022:
        raise RuntimeError("validator state root must be a directory of this user that nobody else can write")
    for path in history:
        try:
            os.lstat(path)
        except FileNotFoundError:
            continue
        raise RuntimeError("Sumeragi safety history already exists at " + path + "; this is not a first boot")
    try:
        descriptor = os.open(token, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW, 0o600)
    except FileExistsError:
        raise RuntimeError("first-boot token " + token + " already exists") from None
    try:
        os.fchmod(descriptor, 0o600)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)
    return token


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--role", choices=ROLES, required=True)
    parser.add_argument("--arm-first-boot", action="store_true", help="Render nothing; as root on the stopped validator host, before the first start of its fresh state root, create the one-shot token that makes the next start pass --sumeragi-assert-fresh-key; refuses when Sumeragi safety history or a token exists")
    parser.add_argument("--runtime-key", help="Required to render. Actual retained owner-0600, single-link, 71-byte signer path on the approved guest; never read here")
    parser.add_argument("--mint-finality-seed", help="Required to render. Actual retained owner-0600, single-link, 32-byte raw mint-finality seed path on the approved guest; never read here")
    parser.add_argument("--global-beacon-credential", help="Retained owner-0600, single-link native beacon credential on the approved guest; consumed launch copy at FD 200; omit only for initial key setup; never read here")
    parser.add_argument("--config-file", choices=CONFIG_FILES, help="Exact retained initial config (config.toml, the default) or authenticated beacon provider transition")
    parser.add_argument("--output", type=Path, help="Required to render. Fresh iroha3d-ROLE.service file; never overwritten")
    args = parser.parse_args()
    rendering = {"--runtime-key": args.runtime_key, "--mint-finality-seed": args.mint_finality_seed,
                 "--global-beacon-credential": args.global_beacon_credential,
                 "--config-file": args.config_file, "--output": args.output}
    if args.arm_first_boot:
        given = [flag for flag, value in rendering.items() if value is not None]
        if given:
            parser.error("--arm-first-boot takes only --role, not " + ", ".join(given))
        try:
            token = arm_first_boot(args.role)
        except (OSError, RuntimeError) as error:
            parser.exit(1, f"{parser.prog}: first boot not armed: {error}\n")
        print(token)
        return
    missing = [flag for flag in ("--runtime-key", "--mint-finality-seed", "--output") if rendering[flag] is None]
    if missing:
        parser.error("the following arguments are required to render: " + ", ".join(missing))
    if args.output.name != f"iroha3d-{args.role}.service":
        parser.error("output filename must match the canonical role unit name")
    try:
        content = render(args.role, args.runtime_key, args.mint_finality_seed, args.global_beacon_credential,
                         config_file=args.config_file or "config.toml").encode()
    except ValueError as error:
        parser.error(str(error))
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(content)
        output.flush()
        # Match the native validator_unit artifact role even under UMask=0077.
        # Publish only the completed public unit; signer files are never opened.
        os.fchmod(output.fileno(), 0o644)
        os.fsync(output.fileno())
    print(args.output)


if __name__ == "__main__":
    main()
