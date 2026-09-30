import os
import pwd
import re
import resource
import stat
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path


HARDENING_MARKER = Path(
    "/var/lib/soracloud/materialization/.inrou-guest-hardening-v1"
)
HARDENING_MARKER_BODY = (
    "inrou-guest-hardening-v1\n"
    "root-password-locked=1\n"
    "root-shell-nologin=1\n"
    "ssh-units-masked=1\n"
)
SERVICE_ROOT = "/var/lib/soracloud/service"
SSH_UNITS = ("ssh.service", "ssh.socket", "sshd.service", "sshd.socket")
MOUNTINFO_ESCAPE = re.compile(r"\\([0-7]{3})")


def _read_text(path):
    return Path(path).read_text(encoding="utf-8").strip()


def _unified_cgroup_directory():
    matches = []
    for line in _read_text("/proc/self/cgroup").splitlines():
        hierarchy, controllers, relative_path = line.split(":", 2)
        if hierarchy == "0" and controllers == "":
            matches.append(relative_path)
    if len(matches) != 1 or not matches[0].startswith("/"):
        raise RuntimeError("the Inrou guest must expose one unified cgroup-v2 path")
    directory = Path("/sys/fs/cgroup") / matches[0].lstrip("/")
    if not directory.is_dir():
        raise RuntimeError("the Inrou app cgroup is not visible in cgroupfs")
    return directory


def _decode_mountinfo_path(value):
    return MOUNTINFO_ESCAPE.sub(lambda match: chr(int(match.group(1), 8)), value)


def _service_tmpfs():
    matches = []
    service_device = os.stat(SERVICE_ROOT).st_dev
    service_device_id = f"{os.major(service_device)}:{os.minor(service_device)}"
    for line in _read_text("/proc/self/mountinfo").splitlines():
        fields = line.split()
        separator = fields.index("-")
        if (
            _decode_mountinfo_path(fields[4]) != SERVICE_ROOT
            or fields[2] != service_device_id
        ):
            continue
        options = set(fields[5].split(","))
        options.update(fields[separator + 3].split(","))
        matches.append((int(fields[0]), fields[separator + 1], options))
    if not matches:
        raise RuntimeError("the Inrou service root must have an exact mount")
    _, mount_type, mount_options = max(matches, key=lambda match: match[0])
    return mount_type, mount_options


def _tcp_port_is_listening(port):
    for table in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            lines = _read_text(table).splitlines()[1:]
        except FileNotFoundError:
            continue
        for line in lines:
            fields = line.split()
            if len(fields) < 4 or fields[3] != "0A":
                continue
            if int(fields[1].rsplit(":", 1)[1], 16) == port:
                return True
    return False


def _ssh_unit_is_masked(unit):
    path = Path("/etc/systemd/system") / unit
    return path.is_symlink() and path.resolve(strict=True) == Path("/dev/null")


def _no_new_privileges():
    for line in _read_text("/proc/self/status").splitlines():
        if line.startswith("NoNewPrivs:"):
            return line.split(":", 1)[1].strip()
    raise RuntimeError("NoNewPrivs is missing from /proc/self/status")


def _attestation_body():
    cgroup = _unified_cgroup_directory()
    mount_type, mount_options = _service_tmpfs()
    mount_stat = os.stat(SERVICE_ROOT)
    filesystem = os.statvfs(SERVICE_ROOT)
    marker_stat = HARDENING_MARKER.stat()
    marker_body = HARDENING_MARKER.read_text(encoding="utf-8")
    root = pwd.getpwnam("root")
    inrou = pwd.getpwnam("inrou")
    nofile_soft, nofile_hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    supplementary_gids = sorted(
        gid for gid in os.getgroups() if gid != os.getegid()
    )
    fields = (
        ("schema_version", "1"),
        ("uid", str(os.geteuid())),
        ("gid", str(os.getegid())),
        ("supplementary_gids", ",".join(map(str, supplementary_gids))),
        ("inrou_shell", inrou.pw_shell),
        ("no_new_privs", _no_new_privileges()),
        ("limit_nofile_soft", str(nofile_soft)),
        ("limit_nofile_hard", str(nofile_hard)),
        ("tasks_max", _read_text(cgroup / "pids.max")),
        ("cpu_max", _read_text(cgroup / "cpu.max")),
        ("memory_max", _read_text(cgroup / "memory.max")),
        ("memory_swap_max", _read_text(cgroup / "memory.swap.max")),
        ("service_mount_type", mount_type),
        (
            "service_mount_total_bytes",
            str(filesystem.f_blocks * filesystem.f_frsize),
        ),
        ("service_mount_uid", str(mount_stat.st_uid)),
        ("service_mount_gid", str(mount_stat.st_gid)),
        ("service_mount_mode", f"{stat.S_IMODE(mount_stat.st_mode):04o}"),
        ("service_mount_rw", str(int("rw" in mount_options))),
        ("service_mount_nosuid", str(int("nosuid" in mount_options))),
        ("service_mount_nodev", str(int("nodev" in mount_options))),
        ("service_mount_noexec", str(int("noexec" in mount_options))),
        ("root_shell", root.pw_shell),
        ("hardening_marker_body", str(int(marker_body == HARDENING_MARKER_BODY))),
        ("hardening_marker_uid", str(marker_stat.st_uid)),
        ("hardening_marker_gid", str(marker_stat.st_gid)),
        ("hardening_marker_mode", f"{stat.S_IMODE(marker_stat.st_mode):04o}"),
        *(
            (f"{unit}_masked", str(int(_ssh_unit_is_masked(unit))))
            for unit in SSH_UNITS
        ),
        ("ssh_port_22_listening", str(int(_tcp_port_is_listening(22)))),
    )
    return "".join(f"{key}={value}\n" for key, value in fields).encode("utf-8")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/healthz":
            body = b"ok\n"
        elif self.path == "/attestation-v1":
            try:
                body = _attestation_body()
            except (KeyError, OSError, RuntimeError, ValueError):
                body = b"guest attestation unavailable\n"
                self.send_response(500)
                self.send_header("Content-Type", "text/plain; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


HTTPServer(("0.0.0.0", int(os.environ["PORT"])), Handler).serve_forever()
