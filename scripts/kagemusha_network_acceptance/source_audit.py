"""Complete local package inventories and actual compiler tool custody."""
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import sys
import child_process

def local_roots(repo, metadata):
    roots = []
    for package in metadata['packages']:
        if package['source'] is not None:
            continue
        manifest = Path(package['manifest_path'])
        root = manifest.parent
        if manifest.is_symlink() or root.resolve() != root or not root.is_relative_to(repo):
            raise ValueError('local package outside original checkout or through symlink')
        if root == repo:
            raise ValueError('unexpected root package; audit must explicitly cover its inputs')
        roots.append(str(root.relative_to(repo)))
    return sorted(set(roots))


def package_paths(repo, roots):
    paths = set()
    for relative in roots:
        for directory, children, files in os.walk(repo / relative, followlinks=False):
            # Cargo target outputs and Git administrative files are not source;
            # generated build inputs derive from the recorded build script inputs.
            children[:] = [name for name in children if name not in ('.git', 'target')]
            for name in children + files:
                path = Path(directory) / name
                if path.is_symlink() or path.is_file():
                    paths.add(str(path.relative_to(repo)))
    return paths


def in_package(name, roots):
    return any(name == root or name.startswith(root + '/') for root in roots)



def tools_snapshot(repo, reader):
    paths = {}
    for name in ('rustc', 'cargo'):
        executable = Path(child_process.natural_run(
            ['rustup', 'which', name], cwd=repo, text=True, check=True, stdout=subprocess.PIPE
        ).stdout.strip()).resolve()
        paths[name] = executable
    # cargo_fast may auto-select sccache and invokes Bash/Python; record their
    # actual binaries as well as the native linker tools supplied by this host.
    for name in ('bash', 'python3', 'sccache', 'clang', 'cc', 'ld', 'ar', 'gcc', 'g++', 'c++', 'ranlib', 'as', 'ld.lld', 'llvm-ar'):
        executable = shutil.which(name)
        if executable:
            paths[name] = Path(executable).resolve()
    paths['python_runtime'] = Path(sys.executable).resolve()
    if sys.platform == 'darwin':
        # /usr/bin tools can be xcrun dispatch stubs. Bind selected tools too.
        for name in ('clang', 'ld', 'ar'):
            executable = child_process.natural_run(
                ['xcrun', '--find', name], cwd=repo, text=True, check=True, stdout=subprocess.PIPE
            ).stdout.strip()
            paths['selected_' + name] = Path(executable).resolve()
    for name in ('RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
        if os.environ.get(name):
            resolved = shutil.which(os.environ[name])
            if not resolved:
                raise ValueError('compiler override cannot be resolved')
            paths[name] = Path(resolved).resolve()
    for name in ('capture.py', 'source_audit.py', 'compiler_capture.py', 'child_process.py', 'paths.py'):
        paths[name] = Path(__file__).parent / name
    result = {name: {'path': str(path), 'sha256': hashlib.sha256(reader(path)).hexdigest()}
              for name, path in paths.items()}
    if sys.platform == 'darwin':
        result['sdk'] = {
            'path': child_process.natural_run(
                ['xcrun', '--show-sdk-path'], text=True, check=True, stdout=subprocess.PIPE
            ).stdout.strip(),
            'version': child_process.natural_run(
                ['xcrun', '--show-sdk-version'], text=True, check=True, stdout=subprocess.PIPE
            ).stdout.strip(),
        }
    return result
