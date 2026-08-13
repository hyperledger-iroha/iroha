"""Terminal publication cases executed by the parent release-receipt suite."""

def private_output(tmp_path: Path) -> tuple[Path, Path]:
    directory = tmp_path / "private-output"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    return directory, directory / "RELEASE_COMPLETED.json"


def test_terminal_publication_is_no_clobber_and_durable(tmp_path: Path) -> None:
    _case_release_approval_file_protection_and_bounds_fail_closed()
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    data = canonical_json({"result": "release-complete"})
    revalidations = 0

    def revalidate() -> None:
        nonlocal revalidations
        revalidations += 1

    module._publish_terminal_receipt(output, data, revalidate=revalidate)

    metadata = output.lstat()
    assert output.read_bytes() == data
    assert metadata.st_mode & 0o7777 == 0o400
    assert metadata.st_nlink == 1
    assert revalidations == 2
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_completes_short_writes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    data = b"terminal receipt with deliberately short writes\n"
    real_write = module.os.write
    real_close = module.os.close
    directory_metadata = directory.stat()
    injected_close_failure = False

    def short_write(descriptor: int, pending: object) -> int:
        return real_write(descriptor, pending[:3])

    def close_with_final_directory_failure(descriptor: int) -> None:
        nonlocal injected_close_failure
        metadata = os.fstat(descriptor)
        fail = (
            not injected_close_failure
            and stat.S_ISDIR(metadata.st_mode)
            and (metadata.st_dev, metadata.st_ino)
            == (directory_metadata.st_dev, directory_metadata.st_ino)
        )
        real_close(descriptor)
        if fail:
            injected_close_failure = True
            raise OSError("fixture close reported failure after durable publication")

    monkeypatch.setattr(module.os, "write", short_write)
    monkeypatch.setattr(module.os, "close", close_with_final_directory_failure)

    module._publish_terminal_receipt(output, data, revalidate=lambda: None)

    assert injected_close_failure
    assert output.read_bytes() == data
    assert output.stat().st_nlink == 1
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_rejects_zero_length_write_progress(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    monkeypatch.setattr(module.os, "write", lambda _descriptor, _pending: 0)

    with pytest.raises(module.ReceiptError, match="write made no progress"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


@pytest.mark.parametrize("failure_call", [1, 2, 3])
def test_terminal_publication_fsync_failure_cleans_every_owned_name(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure_call: int,
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    real_fsync = module.os.fsync
    calls = 0

    def failing_fsync(descriptor: int) -> None:
        nonlocal calls
        calls += 1
        if calls == failure_call:
            raise OSError("fixture fsync failure")
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", failing_fsync)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_link_failure_cleans_stage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)

    def fail_link(*_args: object, **_kwargs: object) -> None:
        raise OSError("fixture link failure")

    monkeypatch.setattr(module.os, "link", fail_link)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_cleans_a_link_created_before_reported_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    real_link = module.os.link

    def link_then_fail(*args: object, **kwargs: object) -> None:
        real_link(*args, **kwargs)
        raise OSError("fixture post-link failure")

    monkeypatch.setattr(module.os, "link", link_then_fail)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


@pytest.mark.parametrize("failure_kind", ["file", "directory"])
def test_evidence_durability_fsync_failure_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure_kind: str,
) -> None:
    module = load_writer_module()
    directory = tmp_path / "evidence"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    artifact = directory / "artifact"
    artifact.write_bytes(b"progress evidence\n")
    artifact.chmod(0o400)
    path_contract = module._capture_path_contract(
        artifact,
        "fixture evidence",
        expected_sha256=sha256(artifact),
        expected_mode=0o400,
        expected_owner=os.geteuid(),
        expected_nlink=1,
        expected_size=artifact.stat().st_size,
    )
    directory_contract = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    real_fsync = module.os.fsync

    def fail_selected(descriptor: int) -> None:
        is_directory = stat.S_ISDIR(os.fstat(descriptor).st_mode)
        if is_directory == (failure_kind == "directory"):
            raise OSError("fixture evidence durability failure")
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", fail_selected)

    with pytest.raises(module.ReceiptError, match="fsync failed"):
        module._fsync_receipt_inputs([directory_contract, path_contract])


def test_evidence_durability_rejects_directory_inventory_drift(
    tmp_path: Path,
) -> None:
    module = load_writer_module()
    directory = tmp_path / "evidence"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    contract = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    (directory / "late-artifact").write_bytes(b"late\n")

    with pytest.raises(module.ReceiptError, match="changed before fsync"):
        module._fsync_receipt_inputs([contract])


def test_evidence_durability_orders_files_before_bottom_up_directories(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_writer_module()
    parent = tmp_path / "evidence"
    child = parent / "nested"
    child.mkdir(parents=True, mode=0o700)
    parent.chmod(0o700)
    child.chmod(0o700)
    artifact = child / "artifact"
    artifact.write_bytes(b"progress evidence\n")
    artifact.chmod(0o400)
    contracts = [
        module._capture_directory_contract(parent, "fixture parent"),
        module._capture_path_contract(
            artifact,
            "fixture evidence",
            expected_sha256=sha256(artifact),
            expected_mode=0o400,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=artifact.stat().st_size,
        ),
        module._capture_directory_contract(child, "fixture child"),
    ]
    inode_names = {
        (contract.device, contract.inode): contract.path.name
        for contract in contracts
    }
    observed: list[str] = []
    real_fsync = module.os.fsync

    def record_fsync(descriptor: int) -> None:
        metadata = os.fstat(descriptor)
        observed.append(inode_names[(metadata.st_dev, metadata.st_ino)])
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", record_fsync)

    module._fsync_receipt_inputs(contracts)

    assert observed == ["artifact", "nested", "evidence"]


@pytest.mark.parametrize("mutation_revalidation", [1, 2])
def test_terminal_publication_revalidation_failure_cleans_terminal_names(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation_revalidation: int,
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    evidence = tmp_path / "evidence.tsv"
    evidence.write_bytes(b"schema_version\t1\nresult\tpassed\n")
    receipt_data = b"terminal receipt\n"
    if mutation_revalidation == 1:
        replacement = tmp_path / "replacement.tsv"
        replacement.write_bytes(b"schema_version\t1\nresult\tforged\n")
        original_digest = sha256(evidence)
        parse_snapshot = module._tsv_fields_from_snapshot

        def replace_after_semantic_validation(
            evidence_snapshot: object, name: str
        ) -> dict[str, str]:
            fields = parse_snapshot(evidence_snapshot, name)
            os.replace(replacement, evidence)
            return fields

        monkeypatch.setattr(
            module,
            "_tsv_fields_from_snapshot",
            replace_after_semantic_validation,
        )
        evidence_snapshot, fields = module._load_tsv(
            evidence, "fixture completion"
        )
        artifact = module._artifact(evidence_snapshot)
        assert fields == {"schema_version": "1", "result": "passed"}
        assert artifact == {"path": str(evidence), "sha256": original_digest}
        assert sha256(evidence) != artifact["sha256"]
        snapshot = module._snapshot_contract(evidence_snapshot)
        receipt_data = canonical_json({"evidence": artifact})
    else:
        snapshot = module._capture_path_contract(
            evidence,
            "fixture evidence",
            expected_sha256=sha256(evidence),
        )
    calls = 0

    def revalidate() -> None:
        nonlocal calls
        calls += 1
        if mutation_revalidation == 2 and calls == mutation_revalidation:
            evidence.write_bytes(b"forged evidence\n")
        module._revalidate_receipt_inputs([snapshot])

    with pytest.raises(module.ReceiptError, match="aggregate evidence"):
        module._publish_terminal_receipt(
            output, receipt_data, revalidate=revalidate
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))
    if mutation_revalidation == 1:
        cases = (
            (b"schema_version\t1\r\n", 1024, "canonical LF-only text"),
            (b"schema_version\t1", 1024, "canonical LF-only text"),
            (b"schema_version\t" + b"1" * 32 + b"\n", 16, "size limit"),
        )
        for index, (data, maximum_bytes, expected) in enumerate(cases):
            malformed = tmp_path / f"malformed-{index}.tsv"
            malformed.write_bytes(data)
            with pytest.raises(module.ReceiptError, match=expected):
                module._load_tsv(
                    malformed,
                    f"malformed fixture {index}",
                    maximum_bytes=maximum_bytes,
                )


@pytest.mark.parametrize("existing_kind", ["regular", "symlink", "hardlink"])
def test_terminal_publication_never_overwrites_existing_terminal_name(
    tmp_path: Path, existing_kind: str
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    protected = directory / "protected"
    protected.write_bytes(b"protected bytes\n")
    if existing_kind == "regular":
        output.write_bytes(b"previous terminal receipt\n")
    elif existing_kind == "symlink":
        output.symlink_to(protected.name)
    elif existing_kind == "hardlink":
        os.link(protected, output)
    else:
        raise AssertionError(existing_kind)
    before = output.lstat()
    protected_bytes = protected.read_bytes()

    with pytest.raises(module.ReceiptError, match="overwrite is forbidden"):
        module._publish_terminal_receipt(
            output, b"replacement receipt\n", revalidate=lambda: None
        )

    after = output.lstat()
    assert (after.st_dev, after.st_ino) == (before.st_dev, before.st_ino)
    assert protected.read_bytes() == protected_bytes
    assert not list(directory.glob(f".{output.name}.stage.*"))


@pytest.mark.parametrize("parent_state", ["missing", "mode-0755"])
def test_terminal_publication_rejects_unsafe_output_directory(
    tmp_path: Path, parent_state: str
) -> None:
    module = load_writer_module()
    directory = tmp_path / "unsafe-output"
    if parent_state == "mode-0755":
        directory.mkdir(mode=0o700)
        directory.chmod(0o755)
    output = directory / "RELEASE_COMPLETED.json"

    with pytest.raises(module.ReceiptError, match="terminal receipt output directory"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()


@pytest.mark.parametrize("mutation", ["content", "mode", "inode", "hardlink"])
def test_aggregate_snapshot_revalidation_rejects_late_mutation(
    tmp_path: Path, mutation: str
) -> None:
    module = load_writer_module()
    evidence = tmp_path / "aggregate-evidence"
    evidence.write_bytes(b"original evidence\n")
    snapshot = module._capture_path_contract(
        evidence,
        "fixture evidence",
        expected_sha256=sha256(evidence),
    )
    if mutation == "content":
        evidence.write_bytes(b"mutated evidence!\n")
    elif mutation == "mode":
        evidence.chmod(0o400)
    elif mutation == "inode":
        evidence.unlink()
        evidence.write_bytes(b"original evidence\n")
    elif mutation == "hardlink":
        os.link(evidence, tmp_path / "aggregate-evidence-alias")
    else:
        raise AssertionError(mutation)

    with pytest.raises(module.ReceiptError, match="aggregate evidence"):
        module._revalidate_receipt_inputs([snapshot])


@pytest.mark.parametrize("mutation", ["mode", "entry", "inode"])
def test_aggregate_directory_revalidation_rejects_late_mutation(
    tmp_path: Path, mutation: str
) -> None:
    module = load_writer_module()
    directory = tmp_path / "aggregate-directory"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    snapshot = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    if mutation == "mode":
        directory.chmod(0o755)
    elif mutation == "entry":
        (directory / "late-entry").write_bytes(b"late evidence\n")
    elif mutation == "inode":
        directory.rmdir()
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)
    else:
        raise AssertionError(mutation)

    with pytest.raises(module.ReceiptError, match="aggregate evidence directory"):
        module._revalidate_receipt_inputs([snapshot])
