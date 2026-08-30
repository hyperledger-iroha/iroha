# Executed lexically in sumeragi_v2_release_receipt_test.py; do not import directly.


def test_receipt_replay_supervision_has_no_forbidden_process_controls() -> None:
    _case_release_approval_four_class_binding_and_path_free_archives()
    source = SCRIPT.read_text(encoding="utf-8")
    for forbidden in (
        "import signal",
        "os.kill(",
        "os.killpg(",
        ".kill(",
        ".terminate(",
        "start_new_session",
        "def _abort",
        "wait(timeout=",
    ):
        assert forbidden not in source


@pytest.mark.parametrize(
    ("timeout_seconds", "maximum_output_bytes", "program", "message"),
    [
        (
            0,
            1024,
            "import time; time.sleep(0.05)",
            "exceeded its timeout",
        ),
        (
            5,
            32,
            "import sys; "
            "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
            "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush()",
            "output exceeds its closed limit",
        ),
    ],
    ids=("timeout", "output"),
)
def test_bounded_replay_finishes_naturally_before_latched_limit_failure(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    timeout_seconds: int,
    maximum_output_bytes: int,
    program: str,
    message: str,
) -> None:
    module = load_writer_module()
    monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", timeout_seconds)
    interpreter = Path(sys.executable).resolve(strict=True)
    sentinel = tmp_path / "replay-natural-completion"
    child = (
        f"{program}; from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(module.ReceiptError, match=message):
        module._run_bounded_replay(
            interpreter,
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            name="fixture validator",
            maximum_output_bytes=maximum_output_bytes,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def test_bounded_replay_finishes_naturally_after_child_cancels_stdin(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_writer_module()
    monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", 5)
    interpreter = Path(sys.executable).resolve(strict=True)
    sentinel = tmp_path / "stdin-cancellation-natural-completion"
    child = (
        "import sys,time; from pathlib import Path; sys.stdin.close(); "
        "time.sleep(0.05); "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(module.ReceiptError, match="cancelled its bounded stdin replay"):
        module._run_bounded_replay(
            interpreter,
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            name="fixture validator",
            stdin_data=b"x" * (1024 * 1024),
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def test_bounded_python_validator_finishes_naturally_before_timeout_failure(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_writer_module()
    monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", 0)
    sentinel = tmp_path / "validator-natural-completion"
    sentinel.write_text("pending", encoding="utf-8")
    (tmp_path / "validator_support.py").write_text(
        "VALUE = 'complete'\n", encoding="utf-8"
    )
    checker = tmp_path / "validator.py"
    checker.write_text(
        "import validator_support\n"
        "import time\n"
        "from pathlib import Path\n"
        "time.sleep(0.05)\n"
        f"Path({str(sentinel)!r}).write_text(validator_support.VALUE, encoding='utf-8')\n",
        encoding="utf-8",
    )

    with pytest.raises(module.ReceiptError, match="exceeded its timeout"):
        module._run_bounded_python_validator(
            checker,
            [],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            name="fixture Python validator",
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"
    assert not (tmp_path / "__pycache__").exists()


def test_bounded_replay_latches_supervisor_cancellation_until_natural_exit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_writer_module()
    monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", 5)
    real_selector = module.selectors.DefaultSelector

    class CancellingSelector:
        def __init__(self) -> None:
            self._selector = real_selector()
            self._cancelled = False

        def __getattr__(self, name: str) -> object:
            return getattr(self._selector, name)

        def select(self, timeout: float | None = None) -> object:
            if not self._cancelled:
                self._cancelled = True
                raise KeyboardInterrupt
            return self._selector.select(timeout)

    monkeypatch.setattr(module.selectors, "DefaultSelector", CancellingSelector)
    interpreter = Path(sys.executable).resolve(strict=True)
    sentinel = tmp_path / "supervisor-cancellation-natural-completion"
    child = (
        "import time; from pathlib import Path; time.sleep(0.05); "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(KeyboardInterrupt):
        module._run_bounded_replay(
            interpreter,
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            name="fixture validator",
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


@pytest.mark.parametrize(
    "fault_method", ("register", "select", "read", "wait")
)
def test_bounded_replay_drains_after_generic_supervisor_exception(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    fault_method: str,
) -> None:
    module = load_writer_module()
    monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", 5)
    real_selector = module.selectors.DefaultSelector

    class FaultingSelector:
        def __init__(self) -> None:
            self._selector = real_selector()
            self._failed = False

        def __getattr__(self, name: str) -> object:
            return getattr(self._selector, name)

        def register(self, *args: object, **kwargs: object) -> object:
            if fault_method == "register" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.register(*args, **kwargs)

        def select(self, timeout: float | None = None) -> object:
            if fault_method == "select" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.select(timeout)

    monkeypatch.setattr(module.selectors, "DefaultSelector", FaultingSelector)
    if fault_method == "read":
        real_read = module.os.read
        real_popen = module.subprocess.Popen
        read_armed = False
        read_failed = False

        def faulting_read(descriptor: int, size: int) -> bytes:
            nonlocal read_failed
            if read_armed and not read_failed:
                read_failed = True
                raise RuntimeError("injected supervisor failure")
            return real_read(descriptor, size)

        def arming_popen(*args: object, **kwargs: object) -> object:
            nonlocal read_armed
            process = real_popen(*args, **kwargs)
            read_armed = True
            return process

        monkeypatch.setattr(module.os, "read", faulting_read)
        monkeypatch.setattr(module.subprocess, "Popen", arming_popen)
    if fault_method == "wait":
        real_popen = module.subprocess.Popen

        class FaultingProcess:
            def __init__(self, *args: object, **kwargs: object) -> None:
                self._process = real_popen(*args, **kwargs)
                self._failed = False

            def __getattr__(self, name: str) -> object:
                return getattr(self._process, name)

            def wait(self) -> int:
                if not self._failed:
                    self._failed = True
                    raise RuntimeError("injected supervisor failure")
                return self._process.wait()

        monkeypatch.setattr(module.subprocess, "Popen", FaultingProcess)
    interpreter = Path(sys.executable).resolve(strict=True)
    sentinel = tmp_path / "supervisor-exception-natural-completion"
    child = (
        "import sys; "
        "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
        "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush(); "
        "from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(RuntimeError, match="injected supervisor failure"):
        module._run_bounded_replay(
            interpreter,
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            name="fixture validator",
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"
