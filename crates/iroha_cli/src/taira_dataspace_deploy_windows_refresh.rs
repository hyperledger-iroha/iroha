//! Explicit private unsigned-refresh intent, commit and original-file rollback.
//!
//! A durable intent precedes old-to-backup and new-to-canonical no-replace moves. Until a
//! durable commit, failure restores the original object, or preserves it for explicit Plan
//! recovery. A committed transaction only finishes cleanup. Canonical absence between moves
//! is never a new operation: every reader refuses the four reserved transaction markers.
//! Control stages are non-authoritative, purpose-specific, bounded private leftovers; they
//! are neither adopted nor deleted. Native identities remain local to their original host.

use super::*;
use iroha_fs::FileMoveStep;
use norito::codec::Encode as _;
use std::collections::BTreeSet;

const INTENT: &str = ".refresh-intent.json";
const INTENT_STAGE: &str = ".staging-refresh-intent-";
const COMMIT: &str = ".refresh-commit.json";
const BACKUP: &str = ".refresh-original";
const QUARANTINE: &str = ".refresh-replacement";
const COMMIT_STAGE: &str = ".staging-refresh-commit-";
pub(super) const CONTROL_BOUND: usize = 64 * 1024;

// Unpublished control stages are never inputs and are never adopted or deleted by recovery.
// Only their exact purpose and random-name shape are admitted while a definition exists;
// durable intent/commit/backup/discard names always remain behind the recovery barrier.
pub(super) fn unsigned_control_stage(name: &str) -> bool {
    [INTENT_STAGE, COMMIT_STAGE]
        .iter()
        .filter_map(|prefix| name.strip_prefix(prefix))
        .any(|nonce| {
            nonce.len() == 32
                && nonce
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Original {
    identity: String,
    stable_snapshot: String,
    size: u64,
    sha256: String,
}

impl Original {
    fn valid(&self) -> bool {
        let mut parts = self.identity.split(':');
        matches!(parts.next(), Some("1" | "2"))
            && parts.next().is_some_and(|value| lower_hex(value, 16))
            && parts.next().is_some_and(|value| lower_hex(value, 32))
            && parts.next().is_none()
            && self.size <= MAX_BYTES as u64
            && lower_hex(&self.sha256, 64)
            && lower_hex(&self.stable_snapshot, 64)
    }

    fn capture(file: &mut MovableFile) -> Result<Self> {
        let snapshot = file.snapshot()?;
        let bytes = read_movable(file, MAX_BYTES)?;
        let mut local = snapshot.local_parts();
        // A native rename changes only this timestamp. Identity, custody, length and mtime
        // stay bound; private policy is independently rechecked on every recovered original.
        local.5 = [0, 0];
        let (host, volume, object) = file.identity()?.local_parts();
        Ok(Self {
            identity: format!("{host}:{volume:016x}:{}", hex::encode(object)),
            stable_snapshot: digest(&("iroha:unsigned-refresh-snapshot:v1", local).encode()),
            size: u64::try_from(bytes.len())?,
            sha256: digest(&bytes),
        })
    }

    fn require(&self, file: &mut MovableFile) -> Result<()> {
        require(
            *self == Self::capture(file)?,
            "refresh original identity, bytes or private custody changed",
        )
    }
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn context_digest(context: &(String, NetworkId, String)) -> String {
    digest(
        &(
            "iroha:unsigned-refresh-context:v1",
            context.0.as_str(),
            context.1,
            context.2.as_str(),
        )
            .encode(),
    )
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Intent {
    schema: u32,
    operation: String,
    context_sha256: String,
    target: String,
    nonce: String,
    old: Original,
    new: Original,
}

impl Intent {
    fn decode(bytes: &[u8]) -> Result<Self> {
        require(
            bytes.len() <= CONTROL_BOUND,
            "refresh control exceeds bound",
        )?;
        let value: Self = json::from_slice(bytes)?;
        operation_id(&value.operation)?;
        require(
            value.schema == 1
                && matches!(value.target.as_str(), "plan.json" | "definition.json")
                && value.nonce.len() == 32
                && value
                    .nonce
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && value.old.valid()
                && value.new.valid()
                && lower_hex(&value.context_sha256, 64)
                && value.old.identity != value.new.identity
                && json::to_vec(&value)? == bytes,
            "invalid or noncanonical unsigned refresh intent",
        )?;
        Ok(value)
    }

    fn staging(&self) -> String {
        format!(".staging-{}-{}", self.target, self.nonce)
    }
    fn backup(&self) -> String {
        BACKUP.to_owned()
    }
    fn quarantine(&self) -> String {
        QUARANTINE.to_owned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    IntentDurable,
    OldPublished,
    NewPublished,
    NewDurable,
    CommitPublished,
    CommitDurable,
}

impl Journal {
    pub(in super::super) fn require_no_refresh(&self) -> Result<()> {
        self.revalidate()?;
        for name in [INTENT, COMMIT, BACKUP, QUARANTINE] {
            match std::fs::symlink_metadata(self.path.join(name)) {
                Ok(_) => eyre::bail!(
                    "unsigned refresh requires recovery; run explicit dataspace plan before status or apply"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.revalidate()
    }

    pub(in super::super) fn replace_unprepared_plan(
        &self,
        expected: &PlanV1,
        replacement: &PlanV1,
    ) -> Result<()> {
        expected.verify()?;
        replacement.verify()?;
        require(
            expected.operation_id == replacement.operation_id,
            "unsigned refresh cannot change operation identity",
        )?;
        self.require_unprepared_plan(expected)?;
        self.refresh_native(
            "plan.json",
            &json::to_vec(expected)?,
            &json::to_vec(replacement)?,
        )
    }

    pub(in super::super) fn refresh_native(
        &self,
        target: &str,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<()> {
        let context = definition::preflight_identity(self)?;
        self.refresh_native_observed(target, expected, replacement, &context, &mut |_| Ok(()))
    }

    fn refresh_native_observed(
        &self,
        target: &str,
        expected: &[u8],
        replacement: &[u8],
        context: &(String, NetworkId, String),
        observe: &mut impl FnMut(Step) -> Result<()>,
    ) -> Result<()> {
        self.require_no_refresh()?;
        require(
            matches!(target, "plan.json" | "definition.json") && replacement.len() <= MAX_BYTES,
            "invalid refresh target or bound",
        )?;
        self.require_unsigned_entries(target == "plan.json")?;
        let mut old = self.directory.open_movable_private(target)?;
        require(
            read_movable(&mut old, MAX_BYTES)? == expected,
            "unsigned refresh expected original changed",
        )?;
        let nonce = hex::encode(rand::random::<[u8; 16]>());
        let staging = format!(".staging-{target}-{nonce}");
        let mut writer = self
            .directory
            .create_retained_private(&staging, MAX_BYTES)?;
        writer.write_all(replacement)?;
        let mut new = writer.finish_private()?;
        let operation = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| eyre!("invalid journal operation name"))?
            .to_owned();
        operation_id(&operation)?;
        let intent = Intent {
            schema: 1,
            operation,
            context_sha256: context_digest(context),
            target: target.into(),
            nonce,
            old: Original::capture(&mut old)?,
            new: Original::capture(&mut new)?,
        };
        let bytes = json::to_vec(&intent)?;
        Intent::decode(&bytes)?;
        let intent_file = self.publish_control(INTENT_STAGE, INTENT, &bytes, || Ok(()))?;
        let mut old_moved = false;
        let mut new_moved = false;
        let mut committed = false;
        let mut commit_file = None;
        let result: Result<()> = (|| {
            observe(Step::IntentDurable)?;
            intent_file.revalidate()?;
            self.require_refresh_entries(&intent, context)?;
            intent.old.require(&mut old)?;
            intent.new.require(&mut new)?;
            old.rename_private_no_replace_observed(intent.backup(), |step| {
                if step == FileMoveStep::Published {
                    old_moved = true;
                    observe(Step::OldPublished)
                        .map_err(|error| std::io::Error::other(error.to_string()))?;
                }
                Ok(())
            })?;
            new.rename_private_no_replace_observed(target, |step| {
                if step == FileMoveStep::Published {
                    new_moved = true;
                    observe(Step::NewPublished)
                        .map_err(|error| std::io::Error::other(error.to_string()))?;
                }
                Ok(())
            })?;
            intent.old.require(&mut old)?;
            intent.new.require(&mut new)?;
            self.require_refresh_entries(&intent, context)?;
            observe(Step::NewDurable)?;
            intent_file.revalidate()?;
            commit_file = Some(self.publish_control(COMMIT_STAGE, COMMIT, &bytes, || {
                committed = true;
                observe(Step::CommitPublished)
            })?);
            observe(Step::CommitDurable)?;
            intent.old.require(&mut old)?;
            intent.new.require(&mut new)?;
            intent_file.revalidate()?;
            commit_file
                .as_ref()
                .ok_or_else(|| eyre!("commit original missing"))?
                .revalidate()?;
            self.require_refresh_entries(&intent, context)?;
            Ok(())
        })();
        if let Err(error) = result {
            if committed {
                return Err(error.wrap_err(
                    "refresh commit may be durable; originals retained for explicit recovery",
                ));
            }
            // All transitions borrow the owners: even post-rename failures leave both exact
            // handles available for rollback. A failed rollback preserves intent and backup.
            intent
                .old
                .require(&mut old)
                .wrap_err("refresh original changed; preserve intent for recovery")?;
            intent
                .new
                .require(&mut new)
                .wrap_err("refresh replacement changed; preserve both originals for recovery")?;
            if new_moved {
                new.rename_private_no_replace_observed(intent.quarantine(), |_| Ok(()))
                    .wrap_err(
                        "refresh rollback requires recovery; new original could not be parked",
                    )?;
            }
            if old_moved {
                old.rename_private_no_replace_observed(target, |_| Ok(()))
                    .wrap_err(
                        "refresh rollback requires recovery; original could not be restored",
                    )?;
            }
            intent.old.require(&mut old)?;
            self.directory.sync()?;
            let new_name = if new_moved {
                intent.quarantine()
            } else {
                intent.staging()
            };
            self.remove_movable(&new_name, new)?;
            self.remove_movable(INTENT, intent_file)?;
            intent.old.require(&mut old)?;
            return Err(error);
        }
        // Commitment is already durable. Cleanup cannot turn it into a failed refresh.
        let cleanup = (|| {
            self.remove_movable(&intent.backup(), old)?;
            self.remove_movable(INTENT, intent_file)?;
            intent.new.require(&mut new)?;
            self.revalidate()?;
            self.remove_movable(
                COMMIT,
                commit_file.ok_or_else(|| eyre!("commit original missing"))?,
            )?;
            Ok::<_, eyre::Report>(())
        })();
        let _ = cleanup;
        intent
            .new
            .require(&mut new)
            .wrap_err("refresh committed; canonical original failed its final custody check")?;
        self.revalidate()
            .wrap_err("refresh committed; final namespace custody failed")
    }

    fn publish_control(
        &self,
        staging: &str,
        name: &str,
        bytes: &[u8],
        published: impl FnOnce() -> Result<()>,
    ) -> Result<MovableFile> {
        require(
            bytes.len() <= CONTROL_BOUND,
            "refresh control exceeds bound",
        )?;
        let staging = format!("{staging}{}", hex::encode(rand::random::<[u8; 16]>()));
        let mut writer = self
            .directory
            .create_retained_private(&staging, CONTROL_BOUND)?;
        writer.write_all(bytes)?;
        let mut original = writer.finish_private()?;
        let mut published = Some(published);
        original.rename_private_no_replace_observed(name, |step| {
            if step == FileMoveStep::Published {
                published.take().expect("one native publication")()
                    .map_err(|error| std::io::Error::other(error.to_string()))?;
            }
            Ok(())
        })?;
        require(
            read_movable(&mut original, CONTROL_BOUND)? == bytes,
            "refresh control readback changed",
        )?;
        self.revalidate()?;
        Ok(original)
    }

    fn remove_movable(&self, name: &str, original: MovableFile) -> Result<()> {
        let snapshot = original.snapshot()?;
        drop(original);
        require(
            self.directory.remove_private_if_snapshot(name, &snapshot)?,
            "refresh original disappeared before cleanup",
        )?;
        self.revalidate()
    }

    fn require_refresh_entries(
        &self,
        intent: &Intent,
        context: &(String, NetworkId, String),
    ) -> Result<()> {
        require(
            self.path.file_name().and_then(|name| name.to_str()) == Some(intent.operation.as_str())
                && context_digest(context) == intent.context_sha256,
            "refresh belongs to another operation, definition or trust",
        )?;
        let controlled = [
            INTENT.to_owned(),
            COMMIT.to_owned(),
            intent.backup(),
            intent.quarantine(),
            intent.staging(),
            intent.target.clone(),
        ];
        for name in self.entry_names()? {
            let name = name
                .to_str()
                .ok_or_else(|| eyre!("invalid refresh filename"))?;
            if name == "preflight-finality" {
                finality::validate_preflight_identity(self, context.clone())?;
            } else if !controlled.iter().any(|entry| entry == name) {
                require(
                    matches!(name, "lock" | "definition.json")
                        || (intent.target == "plan.json" && name == "plan.json")
                        || unsigned_staging_name(name)
                        || unsigned_control_stage(name),
                    "signed, deployment or unrelated evidence prevents refresh recovery",
                )?;
                self.require_private_regular(std::ffi::OsStr::new(name))?;
            }
        }
        self.revalidate()
    }

    /// Only explicit Plan can recover. Status and Apply inspect no partially published target.
    pub(in super::super) fn recover_native_refresh(
        &self,
        allow: bool,
        context: &(String, NetworkId, String),
    ) -> Result<()> {
        let names: BTreeSet<_> = self.entry_names()?.into_iter().collect();
        if !names.iter().any(|name| {
            name.to_str()
                .is_some_and(|name| name.starts_with(".refresh-"))
        }) {
            return Ok(());
        }
        require(
            allow,
            "unsigned refresh requires recovery; run explicit dataspace plan before status or apply",
        )?;
        let mut intent_file = self.admit_refresh_file(&names, INTENT)?;
        let mut commit_file = self.admit_refresh_file(&names, COMMIT)?;
        require(
            intent_file.is_some() || commit_file.is_some(),
            "refresh lost its durable intent or commit; preserve all originals",
        )?;
        let intent = if let Some(file) = intent_file.as_mut() {
            Some(Intent::decode(&read_movable(file, CONTROL_BOUND)?)?)
        } else {
            None
        };
        let commit = if let Some(file) = commit_file.as_mut() {
            Some(Intent::decode(&read_movable(file, CONTROL_BOUND)?)?)
        } else {
            None
        };
        require(
            intent
                .as_ref()
                .zip(commit.as_ref())
                .is_none_or(|(intent, commit)| intent == commit),
            "refresh intent and commit conflict",
        )?;
        let record = commit
            .as_ref()
            .or(intent.as_ref())
            .ok_or_else(|| eyre!("refresh record missing"))?;
        self.require_refresh_entries(record, context)?;
        let mut canonical = self.admit_refresh_file(&names, &record.target)?;
        let mut backup = self.admit_refresh_file(&names, &record.backup())?;
        let mut staged = self.admit_refresh_file(&names, &record.staging())?;
        let mut parked = self.admit_refresh_file(&names, &record.quarantine())?;
        if let Some(file) = backup.as_mut() {
            record.old.require(file)?;
        }
        if let Some(file) = staged.as_mut() {
            record.new.require(file)?;
        }
        if let Some(file) = parked.as_mut() {
            record.new.require(file)?;
        }
        require(
            staged.is_none() || parked.is_none(),
            "refresh has duplicate staged originals",
        )?;
        if commit.is_some() {
            require(
                staged.is_none() && parked.is_none(),
                "committed refresh has contradictory temporary originals",
            )?;
            let current = canonical
                .as_mut()
                .ok_or_else(|| eyre!("committed refresh lost its canonical original"))?;
            record.new.require(current)?;
            if let Some(file) = backup.take() {
                self.remove_movable(&record.backup(), file)?;
            }
            if let Some(file) = intent_file.take() {
                self.remove_movable(INTENT, file)?;
            }
            self.remove_movable(
                COMMIT,
                commit_file
                    .take()
                    .ok_or_else(|| eyre!("commit original missing"))?,
            )?;
            record.new.require(current)?;
        } else {
            let mut old = if let Some(mut current) = canonical.take() {
                if Original::capture(&mut current)? == record.old {
                    require(backup.is_none(), "refresh has two old originals")?;
                    current
                } else {
                    record.new.require(&mut current)?;
                    require(
                        staged.is_none() && parked.is_none(),
                        "refresh has duplicate new originals",
                    )?;
                    let mut old = backup
                        .take()
                        .ok_or_else(|| eyre!("refresh lost its original backup"))?;
                    current.rename_private_no_replace_observed(record.quarantine(), |_| Ok(()))?;
                    parked = Some(current);
                    old.rename_private_no_replace_observed(&record.target, |_| Ok(()))?;
                    old
                }
            } else {
                let mut old = backup
                    .take()
                    .ok_or_else(|| eyre!("refresh lost its canonical and backup originals"))?;
                old.rename_private_no_replace_observed(&record.target, |_| Ok(()))?;
                old
            };
            record.old.require(&mut old)?;
            require(
                staged.is_none() || parked.is_none(),
                "refresh has duplicate staged originals",
            )?;
            if let Some(file) = staged.take() {
                self.remove_movable(&record.staging(), file)?;
            }
            if let Some(file) = parked.take() {
                self.remove_movable(&record.quarantine(), file)?;
            }
            self.remove_movable(
                INTENT,
                intent_file
                    .take()
                    .ok_or_else(|| eyre!("intent original missing"))?,
            )?;
            record.old.require(&mut old)?;
            self.directory.sync()?;
        }
        self.require_no_refresh()
    }

    fn admit_refresh_file(
        &self,
        names: &BTreeSet<std::ffi::OsString>,
        name: &str,
    ) -> Result<Option<MovableFile>> {
        if names.contains(std::ffi::OsStr::new(name)) {
            // A file admitted by the inventory may not disappear later and become None.
            Ok(Some(self.directory.open_movable_private(name)?))
        } else {
            self.revalidate()?;
            Ok(None)
        }
    }
}

#[cfg(test)]
#[path = "taira_dataspace_deploy_windows_refresh_tests.rs"]
mod tests;
