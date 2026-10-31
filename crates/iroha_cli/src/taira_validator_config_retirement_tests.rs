//! Focused semantic and public receipt tests for exact validator config retirement.
use super::*;

#[test]
fn retirement_preserves_every_other_toml_value() {
    let original = b"private_key = 'untouched'\n[genesis]\nfile = '/private/genesis.signed'\n[sumeragi]\nrecords_dir = '/state/records'\n[zk]\nenable_verifiers = true\n[zk.halo2]\nk = 16\n[zk.stark]\nlimit = 7\n";
    let (projected, changed) = project(original).unwrap();
    assert!(changed);
    let mut expected: toml::Table = toml::from_str(std::str::from_utf8(original).unwrap()).unwrap();
    expected
        .get_mut("zk")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .remove("halo2");
    let actual: toml::Table = toml::from_str(std::str::from_utf8(&projected).unwrap()).unwrap();
    assert_eq!(actual, expected);
    let (again, changed) = project(&projected).unwrap();
    assert!(!changed);
    assert_eq!(again.as_slice(), projected.as_slice());
}

#[test]
fn retirement_noop_preserves_exact_original_bytes() {
    for original in [
        b"# comment\n[zk]\nenable_verifiers = true\n".as_slice(),
        b"[sumeragi]\nrecords_dir = 'state'\n",
    ] {
        let (output, changed) = project(original).unwrap();
        assert!(!changed);
        assert_eq!(output.as_slice(), original);
    }
    let (output, changed) = project(b"[zk.halo2]\n").unwrap();
    assert!(changed);
    let table: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
    assert!(table["zk"].as_table().unwrap().is_empty());
}

#[test]
fn retirement_rejects_inherited_malformed_or_non_table_inputs() {
    for bytes in [
        b"extends = 'elsewhere'\n[zk.halo2]\n".as_slice(),
        b"profile = 'other'\n",
        b"zk = 9",
        b"[zk]\nhalo2 = 'not table'",
        b"bad = [",
        b"",
        &[255u8],
    ] {
        assert!(project(bytes).is_err());
    }
    assert!(project(&vec![b' '; LIMIT as usize + 1]).is_err());
}
