"""Keep all original Java groups and inherited public API absence assertions."""
from dataclasses import replace
from pathlib import Path
import sys
import re
import xml.etree.ElementTree as ET
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import check_sccp_java_consumer_contract as guard
from jvm_classfile import ACC_PUBLIC, ACC_STATIC, ClassFile, Method


def owners():
    classes = {n: ClassFile(n, 52, s, (), "java/lang/Object", ()) for n,s in guard.API_OWNERS.items()}
    classes["java/lang/Object"] = ClassFile("java/lang/Object", 52, None, ())
    classes["java/lang/AutoCloseable"] = ClassFile("java/lang/AutoCloseable", 52, None, (), "java/lang/Object", ())
    http = "org/hyperledger/iroha/sdk/client/HttpClientTransport"
    classes[http] = replace(classes[http], interfaces=("org/hyperledger/iroha/sdk/client/IrohaClient", "java/lang/AutoCloseable"))
    return classes


def test_original_public_absence_controls_accept_current_hierarchy():
    guard.validate_api(owners())


@pytest.mark.parametrize("owner", guard.API_OWNERS)
@pytest.mark.parametrize("method", guard.RETIRED_WRITES)
@pytest.mark.parametrize("inherited", [False, True])
def test_public_write_restoration_in_owner_or_parent_is_rejected(owner, method, inherited):
    classes = owners()
    location = owner
    if inherited:
        location = "org/hyperledger/iroha/sdk/client/RestoredParent"
        classes[location] = ClassFile(location, 52, "RestoredParent.kt", (), "java/lang/Object", ())
        classes[owner] = replace(classes[owner], interfaces=classes[owner].interfaces + (location,))
    classes[location] = replace(classes[location], methods=(Method(method, "()V", ACC_PUBLIC),))
    with pytest.raises(guard.ContractError, match="public write"): guard.validate_api(classes)


@pytest.mark.parametrize("name", tuple(guard.API_OWNERS) + guard.RELEASE8_TERMINALS)
def test_missing_owner_or_terminal_is_rejected(name):
    classes = owners(); del classes[name]
    with pytest.raises(guard.ContractError, match="missing"): guard.validate_api(classes)


@pytest.mark.parametrize("parent", ["example/Unknown", "org/hyperledger/iroha/sdk/client/Missing", next(iter(guard.API_OWNERS))])
def test_unknown_nonterminal_missing_parent_and_cycle_are_rejected(parent):
    classes = owners(); owner = next(iter(guard.API_OWNERS))
    classes[owner] = replace(classes[owner], interfaces=(parent,))
    if parent == "example/Unknown": classes[parent] = ClassFile(parent, 52, None, ())
    with pytest.raises(guard.ContractError, match="missing|cyclic|unreviewed"): guard.validate_api(classes)


def test_private_method_preserves_original_public_method_semantics():
    classes = owners(); owner = next(iter(guard.API_OWNERS))
    classes[owner] = replace(classes[owner], methods=(Method(guard.RETIRED_WRITES[0], "()V", 0),))
    guard.validate_api(classes)


def consumer(name):
    return ClassFile(name.replace(".", "/"), 52, name.rsplit(".", 1)[1]+".java", tuple(Method(g,"()V",ACC_PUBLIC) for g in guard.EXPECTED_GROUPS[name]))


CASES = tuple((name, group) for name, groups in guard.EXPECTED_GROUPS.items() for group in groups)


@pytest.mark.parametrize("name", guard.EXPECTED_GROUPS)
def test_complete_java_entrypoints_are_preserved(name): guard.validate_test_owner(consumer(name), name)


@pytest.mark.parametrize("name,group", CASES)
def test_each_omitted_java_entrypoint_is_rejected(name, group):
    owner = consumer(name)
    with pytest.raises(guard.ContractError, match="entrypoint"):
        guard.validate_test_owner(replace(owner,methods=tuple(m for m in owner.methods if m.name != group)),name)


@pytest.mark.parametrize("signature,flags", [("(I)V",ACC_PUBLIC),("()V",0),("()V",ACC_PUBLIC|ACC_STATIC)])
def test_changed_entrypoint_type_or_visibility_is_rejected(signature,flags):
    name=next(iter(guard.EXPECTED_GROUPS));owner=consumer(name)
    changed=replace(owner.methods[0],descriptor=signature,flags=flags)
    with pytest.raises(guard.ContractError,match="entrypoint"): guard.validate_test_owner(replace(owner,methods=(changed,)+owner.methods[1:]),name)


def report(name):
    groups=guard.EXPECTED_GROUPS[name]
    root=ET.Element("testsuite",name=name,tests=str(len(groups)),failures="0",errors="0",skipped="0")
    for group in groups: ET.SubElement(root,"testcase",name=group+"()",classname=name,time="0.001")
    return root


@pytest.mark.parametrize("name",guard.EXPECTED_GROUPS)
def test_complete_synthetic_report_covers_original_groups(name): guard.validate_report(ET.tostring(report(name)),name)


@pytest.mark.parametrize("name,group",CASES)
def test_each_missing_runtime_group_is_rejected(name,group):
    root=report(name);root.remove(next(c for c in root if c.attrib["name"]==group+"()"))
    with pytest.raises(guard.ContractError,match="groups"): guard.validate_report(ET.tostring(root),name)


@pytest.mark.parametrize("mutation",["duplicate","skip","failure","error","zero","owner","name","nan","infinite","negative"])
def test_failed_skipped_zero_selected_or_substituted_runtime_is_rejected(mutation):
    name=next(iter(guard.EXPECTED_GROUPS));root=report(name);case=root[0]
    if mutation=="duplicate": root.append(ET.fromstring(ET.tostring(case)))
    elif mutation in ("skip","failure","error"): ET.SubElement(case,"skipped" if mutation=="skip" else mutation)
    elif mutation=="zero": root.set("tests","0")
    elif mutation=="owner": case.set("classname","other.Owner")
    elif mutation=="name": case.set("name","differentMethod()")
    else: case.set("time",{"nan":"NaN","infinite":"Infinity","negative":"-1"}[mutation])
    with pytest.raises(guard.ContractError): guard.validate_report(ET.tostring(root),name)


def test_java_assertion_groups_registered_once_in_shared_owner():
    for name,groups in guard.EXPECTED_GROUPS.items():
        source=(ROOT/"kotlin/core-jvm/src/sccpJavaTest/java"/(name.replace(".","/")+".java")).read_text()
        for group in groups: assert source.count("@org.junit.jupiter.api.Test\n  public void "+group+"(")==1
        assert '@org.junit.jupiter.api.Tag("host-native")' in source
        assert "SCCP consumer assertions must be enabled" in source
        assert not re.search(r"(?<!Collectors)\.toList\(", source)
        for retired in ("org.hyperledger.iroha.android",".getMethods()","java.lang.reflect","List.of(","Map.of(","Set.of(",".repeat("): assert retired not in source
    for module in ("core-jvm","client-android"):
        assert "src/sccpJavaTest/java" in (ROOT/"kotlin"/module/"build.gradle.kts").read_text()


@pytest.fixture(scope="module")
def compiled_inheritance(tmp_path_factory):
    """Use javac as an independent metadata oracle without loading test classes."""
    import shutil
    import subprocess

    javac = shutil.which("javac")
    if javac is None:
        pytest.skip("JDK compiler required for the independent inheritance oracle")
    root = tmp_path_factory.mktemp("sccp-inheritance")
    source = root / "InheritanceOracle.java"
    source.write_text("""package oracle;
interface ApiParent { void submitSccpNativeMessage(); }
class SuperParent { public void submitSccpDestinationProof() {} }
public abstract class InheritanceOracle extends SuperParent implements ApiParent {}
""")
    subprocess.run([javac, "--release", "8", "-d", str(root), str(source)],
                   check=True, capture_output=True, timeout=60)
    return root, Path(javac).resolve().parents[1]


def test_parser_preserves_actual_javac_superclass_and_interfaces(compiled_inheritance):
    root, _ = compiled_inheritance
    child = guard.parse_class((root / "oracle/InheritanceOracle.class").read_bytes())
    parent = guard.parse_class((root / "oracle/SuperParent.class").read_bytes())
    interface = guard.parse_class((root / "oracle/ApiParent.class").read_bytes())
    assert child.major == parent.major == interface.major == 52
    assert child.super_name == parent.name == "oracle/SuperParent"
    assert child.interfaces == (interface.name,) == ("oracle/ApiParent",)
    assert parent.methods[-1].name == "submitSccpDestinationProof"
    assert interface.methods[-1].name == "submitSccpNativeMessage"
    assert parent.methods[-1].flags & ACC_PUBLIC
    assert interface.methods[-1].flags & ACC_PUBLIC


@pytest.mark.parametrize("parent", [b"oracle/SuperParent", b"oracle/ApiParent"])
def test_parser_rejects_parent_name_path_escape(compiled_inheritance, parent):
    root, _ = compiled_inheritance
    raw = (root / "oracle/InheritanceOracle.class").read_bytes()
    assert raw.count(parent) == 1
    raw = raw.replace(parent, parent.replace(b"oracle/", b"or..le/"))
    with pytest.raises(guard.ClassFileError, match="parent class name"):
        guard.parse_class(raw)


def test_terminals_are_actual_compiler_jdk8_symbols(compiled_inheritance):
    _, jdk_home = compiled_inheritance
    classes, records = guard.load_release8_terminals(jdk_home)
    assert set(classes) == set(guard.RELEASE8_TERMINALS)
    assert len(records) == 3
    assert all(len(record["sha256"]) == 64 for record in records)
    assert classes["java/lang/Object"].super_name is None
    assert classes["java/lang/AutoCloseable"].super_name == "java/lang/Object"
    assert [(method.name, method.descriptor) for method in classes["java/lang/AutoCloseable"].methods] == [("close", "()V")]


@pytest.mark.parametrize("mutation", ["missing", "ambiguous", "wrong_identity", "malformed"])
def test_terminal_inventory_refuses_unknown_or_malformed_symbols(tmp_path, compiled_inheritance, mutation):
    import io
    import zipfile

    root, jdk_home = compiled_inheritance
    actual = zipfile.ZipFile(jdk_home / "lib/ct.sym")
    target = tmp_path / "lib/ct.sym"
    target.parent.mkdir()
    if mutation == "malformed":
        target.write_bytes(b"invalid archive")
        with pytest.raises(zipfile.BadZipFile): guard.load_release8_terminals(tmp_path)
        return
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w") as archive:
        for name in guard.RELEASE8_TERMINALS:
            source = next(entry for entry in actual.infolist() if entry.filename.endswith("/java.base/" + name + ".sig") and "8" in entry.filename.split("/", 1)[0])
            if mutation == "missing" and name == "java/lang/Object": continue
            body = actual.read(source)
            if mutation == "wrong_identity" and name == "java/lang/Object": body = (root / "oracle/InheritanceOracle.class").read_bytes()
            archive.writestr(source.filename, body)
            if mutation == "ambiguous" and name == "java/lang/Object": archive.writestr("8X/java.base/" + name + ".sig", body)
    target.write_bytes(output.getvalue())
    with pytest.raises(guard.ContractError, match="missing|ambiguous|unexpected"):
        guard.load_release8_terminals(tmp_path)
