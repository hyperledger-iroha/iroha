//! Actual generated-genesis command execution and source replacement across reply boundaries.

use super::*;
use crate::kura::scaling_evidence::export::launcher::prepare::assemble::tests::Fixture;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    sync::Mutex,
};

fn from_fixture(fixture: &Fixture) -> Args {
    let mut values = arguments();
    let binding = fixture.bindings().signed_genesis;
    set(
        &mut values,
        "--signed-genesis",
        binding.path.to_str().unwrap(),
    );
    set(
        &mut values,
        "--signed-genesis-sha256",
        hex::encode(binding.sha256),
    );
    set(&mut values, "--signed-genesis-max-bytes", binding.max_bytes);
    set(&mut values, "--network-id", fixture.genesis().network_id);
    set(
        &mut values,
        "--block-store",
        fixture.block_store().to_str().unwrap(),
    );
    set(
        &mut values,
        "--merge-log",
        fixture.merge_log().to_str().unwrap(),
    );
    let reader = fixture.reader_limits();
    for (flag, value) in [
        ("--first-height", 1),
        ("--last-height", 1),
        ("--max-committed-blocks", reader.max_committed_blocks),
        ("--max-store-data-bytes", reader.max_store_data_bytes),
        ("--max-carrier-bytes", reader.max_carrier_bytes as u64),
        ("--max-merge-log-bytes", reader.max_merge_log_bytes),
        ("--max-merge-frames", reader.max_merge_frames),
        ("--reader-max-output-bytes", reader.max_output_bytes),
        (
            "--max-decode-allocation-bytes",
            reader.max_decode_allocation_bytes as u64,
        ),
        ("--owner-uid", u64::from(reader.owner_uid)),
    ] {
        set(&mut values, flag, value);
    }
    Parse::try_parse_from(values).unwrap().args
}

#[test]
fn actual_stopped_tip_command_observes_full_one_and_four_lane_store_under_original_genesis() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes);
        // The generator's signed public genesis uses File::create and can be
        // 0644 under the ordinary umask; it contains no private signing key.
        fs::set_permissions(
            fixture.bindings().signed_genesis.path,
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let mut writer = BufWriter::new(Vec::new());
        from_fixture(&fixture).run(&mut writer).unwrap();
        let bytes = writer.into_inner().unwrap();
        let value: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 6);
        assert_eq!(value["committed_height"].as_u64(), Some(2));
        let binding = fixture.bindings().signed_genesis;
        assert_eq!(
            value["genesis_sha256"].as_str(),
            Some(hex::encode(binding.sha256).as_str())
        );
        assert_eq!(
            value["genesis_bytes"].as_u64(),
            Some(fs::metadata(binding.path).unwrap().len())
        );
        assert!(!fixture.output_path().exists());
        let mut wrong = from_fixture(&fixture);
        wrong.signed_genesis_sha256[0] ^= 1;
        let mut writer = BufWriter::new(Vec::new());
        assert!(wrong.run(&mut writer).is_err());
        assert!(writer.into_inner().unwrap().is_empty());
    }
}

struct ReplyState {
    bytes: Vec<u8>,
    replaced: bool,
}

struct ReplacingSink<'a> {
    path: &'a Path,
    stash: &'a Path,
    on_flush: bool,
    state: Arc<Mutex<ReplyState>>,
}
impl ReplacingSink<'_> {
    fn replace(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.replaced {
            return Ok(());
        }
        let bytes = fs::read(self.path)?;
        fs::rename(self.path, self.stash)?;
        let mut replacement = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.path)?;
        replacement.write_all(&bytes)?;
        replacement.sync_all()?;
        state.replaced = true;
        Ok(())
    }
}
impl Write for ReplacingSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.state.lock().unwrap().bytes.extend_from_slice(bytes);
        if !self.on_flush {
            self.replace()?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.on_flush {
            self.replace()?;
        }
        Ok(())
    }
}

#[test]
fn actual_command_rejects_original_genesis_and_core_replacement_during_write_and_flush() {
    let fixture = Fixture::new(4);
    let genesis = fixture.bindings().signed_genesis.path;
    let marker = fixture.block_store().join("blocks.count.norito");
    for path in [&genesis, &marker] {
        for on_flush in [false, true] {
            let stash = path.with_extension("original-stopped-tip-test");
            assert!(!stash.exists());
            let state = Arc::new(Mutex::new(ReplyState {
                bytes: Vec::new(),
                replaced: false,
            }));
            let sink = ReplacingSink {
                path,
                stash: &stash,
                on_flush,
                state: state.clone(),
            };
            let mut writer = BufWriter::with_capacity(1, sink);
            assert!(from_fixture(&fixture).run(&mut writer).is_err());
            drop(writer);
            let observed = state.lock().unwrap();
            assert!(observed.replaced);
            assert!(!observed.bytes.is_empty());
            assert_eq!(observed.bytes.last(), Some(&b'\n'));
            assert!(!fixture.output_path().exists());
            // Restore only after the failed consuming command has dropped every owner.
            fs::remove_file(path).unwrap();
            fs::rename(stash, path).unwrap();
        }
    }
}
