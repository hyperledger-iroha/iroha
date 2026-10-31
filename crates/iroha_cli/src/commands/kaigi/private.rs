//! Private-call orchestration: secret custody, ledger context reads and local proving.
//!
//! The ledger owns the participation sequence, usage segment and roster root, so
//! every action starts from one signed Torii read of the caller's standing. A
//! wrong secret or an inadmissible action fails here, before proof work or a fee.

use super::ParsedKaigiPrivacyArtifacts;
use crate::RunContext;
use eyre::{Result, WrapErr as _, bail, ensure, eyre};
use iroha::data_model::{
    kaigi::{KaigiId, KaigiRecord, scalar::KaigiAuthorizationScalarV1},
    prelude::AccountId,
};
use iroha_core_zk::kaigi_client_v1::{
    KaigiAuthorizationActionV1, KaigiAuthorizationArtifactsV1, KaigiAuthorizationContextV1,
    KaigiAuthorizationWitnessV1, KaigiUsageArtifactsV1, kaigi_authorization_commitment_v1,
    kaigi_authorization_context_v1, kaigi_usage_context_v1, kaigi_usage_host_commitment_v1,
    prove_kaigi_authorization_artifacts_v1, prove_kaigi_usage_artifacts_v1,
    sample_kaigi_blinding_v1,
};
use iroha_crypto::Hash;
use norito::json::Value;
use rand::{rand_core::TryRng as _, rngs::SysRng};
use std::{
    fs,
    io::{ErrorKind, Read as _, Write as _},
    path::Path,
    str::FromStr as _,
    time::Duration,
};
use zeroize::Zeroizing;

/// Canonical scalar hex plus a trailing newline; anything longer is not a secret file.
const SECRET_FILE_MAX_BYTES: u64 = 66;

/// A participant action that needs a fresh proof against current call state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CallAction {
    Join,
    Leave,
    HostEnd,
}
impl CallAction {
    const fn relation(self) -> KaigiAuthorizationActionV1 {
        match self {
            Self::Join => KaigiAuthorizationActionV1::Join,
            Self::Leave => KaigiAuthorizationActionV1::Leave,
            Self::HostEnd => KaigiAuthorizationActionV1::HostEnd,
        }
    }
}

/// Whether an action may mint a secret. Only starting a participation can: every
/// later action must open the commitment that the original secret produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecretPolicy {
    CreateIfMissing,
    MustExist,
}

/// The caller's standing in one call, parsed from a single state view.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag is an independent fact the participation route reports."
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CallStanding {
    private: bool,
    ended: bool,
    is_host: bool,
    active: bool,
    original_account: AccountId,
    host: Option<AccountId>,
    sequence: Option<u64>,
    active_commitment: Option<KaigiAuthorizationScalarV1>,
    roster_root: Hash,
    segments_recorded: u32,
}
impl CallStanding {
    fn from_response(
        value: &Value,
        parse_account: impl Fn(&str) -> Result<AccountId>,
    ) -> Result<Self> {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("Kaigi participation response omitted `{key}`"))
        };
        let flag = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_bool)
                .ok_or_else(|| eyre!("Kaigi participation response omitted `{key}`"))
        };
        let roster_root = text("roster_root_hex")?;
        let segments_recorded = value
            .get("segments_recorded")
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("Kaigi participation response omitted `segments_recorded`"))?;
        Ok(Self {
            private: text("privacy_mode")? == "private",
            ended: text("status")? == "ended",
            is_host: flag("is_host")?,
            active: flag("active")?,
            original_account: parse_account(text("original_account_id")?)?,
            host: value
                .get("host_account_id")
                .and_then(Value::as_str)
                .map(&parse_account)
                .transpose()?,
            sequence: value.get("sequence").and_then(Value::as_u64),
            active_commitment: value
                .get("active_commitment_hex")
                .and_then(Value::as_str)
                .map(super::parse_scalar)
                .transpose()?,
            roster_root: Hash::from_str(roster_root).wrap_err("invalid roster root in response")?,
            segments_recorded: u32::try_from(segments_recorded)
                .wrap_err("usage segment count exceeds u32")?,
        })
    }

    /// Reject what Core would reject, with the next step a person can take.
    fn ensure_admissible(&self, call_id: &KaigiId, action: CallAction) -> Result<()> {
        ensure!(
            self.private,
            "Kaigi {call_id} is transparent, so it takes no proof: omit --secret-file"
        );
        ensure!(!self.ended, "Kaigi {call_id} has already ended");
        match action {
            CallAction::Join => {
                ensure!(
                    !self.is_host,
                    "the host of Kaigi {call_id} is already part of the call and cannot join it"
                );
                ensure!(
                    !self.active,
                    "this account already holds a live participation in Kaigi {call_id}; leave before rejoining"
                );
            }
            CallAction::Leave => {
                ensure!(
                    !self.is_host,
                    "the host of Kaigi {call_id} cannot leave it: use `iroha app kaigi end` instead"
                );
                ensure!(
                    self.active,
                    "this account holds no live participation in Kaigi {call_id}"
                );
            }
            CallAction::HostEnd => {
                ensure!(self.is_host, "only the host may end Kaigi {call_id}");
            }
        }
        Ok(())
    }

    /// Prefer the ledger's answer, since it is authoritative when revealed. The call
    /// view withholds the host until an account has a retained relationship with it.
    fn resolve_host(&self, call_id: &KaigiId, hint: Option<&AccountId>) -> Result<AccountId> {
        match (&self.host, hint) {
            (Some(host), Some(hint)) => {
                ensure!(
                    host == hint,
                    "--host does not match the recorded host of Kaigi {call_id}"
                );
                Ok(host.clone())
            }
            (Some(host), None) => Ok(host.clone()),
            (None, Some(hint)) => Ok(hint.clone()),
            (None, None) => bail!(
                "private Kaigi {call_id} does not reveal its host to this account: pass the host's account ID with --host"
            ),
        }
    }

    fn sequence(&self) -> Result<u64> {
        self.sequence
            .ok_or_else(|| eyre!("Kaigi participation response omitted the private sequence"))
    }

    /// Compare locally so a wrong secret costs neither proof work nor a fee.
    fn ensure_opens(&self, computed: &KaigiAuthorizationScalarV1, path: &Path) -> Result<()> {
        ensure!(
            self.active_commitment.as_ref() == Some(computed),
            "{} does not open this account's stored Kaigi commitment; use the secret file from when it created or joined this call",
            path.display()
        );
        Ok(())
    }
}

fn read_standing<C: RunContext>(
    context: &C,
    client: &iroha::client::Client,
    call_id: &KaigiId,
) -> Result<CallStanding> {
    let response = client
        .get_kaigi_call_participation_json(call_id)
        .wrap_err_with(|| format!("failed to read this account's standing in Kaigi {call_id}"))?;
    CallStanding::from_response(&response, |literal| {
        crate::resolve_account_id(context, literal)
    })
}

/// Read this account's standing, so a caller can tell whether a proof is still current.
pub(super) fn call_standing<C: RunContext>(context: &C, call_id: &KaigiId) -> Result<CallStanding> {
    let client = context.client_from_config()?;
    read_standing(context, &client, call_id)
}

/// A random wait of at most `limit_ms`. Without randomness a retry is only more likely
/// to collide again, which is no reason to fail the action.
pub(super) fn retry_spread(limit_ms: u64) -> Duration {
    let random = SysRng.try_next_u64().unwrap_or(0);
    Duration::from_millis(random % (limit_ms + 1))
}

/// Prove a host create against the deterministic empty roster.
pub(super) fn authorize_create<C: RunContext>(
    context: &C,
    call_id: &KaigiId,
    host: &AccountId,
    secret_path: &Path,
) -> Result<ParsedKaigiPrivacyArtifacts> {
    ensure!(
        host == &context.config().account,
        "private Kaigi creation must be signed by the host: configure the CLI with the host account"
    );
    let client = context.client_from_config()?;
    let secret = load_secret(secret_path, SecretPolicy::CreateIfMissing)?;
    let empty_root = KaigiRecord::compute_roster_root(&[]);
    let proof_context = kaigi_authorization_context_v1(
        *client.network_id(),
        call_id,
        host,
        host,
        0,
        KaigiAuthorizationActionV1::HostCreate,
        &empty_root,
    )
    .map_err(report)?;
    prove_authorization(proof_context, &secret)
}

/// Prove a join, leave or host end against the caller's current standing.
pub(super) fn authorize_call_action<C: RunContext>(
    context: &C,
    call_id: &KaigiId,
    action: CallAction,
    host_hint: Option<&AccountId>,
    secret_path: &Path,
) -> Result<ParsedKaigiPrivacyArtifacts> {
    let client = context.client_from_config()?;
    let standing = read_standing(context, &client, call_id)?;
    standing.ensure_admissible(call_id, action)?;
    let host = standing.resolve_host(call_id, host_hint)?;
    let policy = if action == CallAction::Join {
        SecretPolicy::CreateIfMissing
    } else {
        SecretPolicy::MustExist
    };
    let secret = load_secret(secret_path, policy)?;
    let proof_context = kaigi_authorization_context_v1(
        *client.network_id(),
        call_id,
        &host,
        &standing.original_account,
        standing.sequence()?,
        action.relation(),
        &standing.roster_root,
    )
    .map_err(report)?;
    if action != CallAction::Join {
        let computed = kaigi_authorization_commitment_v1(&proof_context, &secret.witness()?)
            .map_err(report)?;
        standing.ensure_opens(&computed, secret_path)?;
    }
    prove_authorization(proof_context, &secret)
}

/// Prove the next usage segment for the host of a private call.
pub(super) fn authorize_usage<C: RunContext>(
    context: &C,
    call_id: &KaigiId,
    duration_ms: u64,
    billed_gas: u64,
    secret_path: &Path,
) -> Result<KaigiUsageArtifactsV1> {
    let client = context.client_from_config()?;
    let standing = read_standing(context, &client, call_id)?;
    ensure!(
        standing.private,
        "Kaigi {call_id} is transparent, so it takes no proof: omit --secret-file"
    );
    ensure!(!standing.ended, "Kaigi {call_id} has already ended");
    ensure!(
        standing.is_host,
        "only the host may record usage for Kaigi {call_id}"
    );
    let host = standing.resolve_host(call_id, None)?;
    let secret = load_secret(secret_path, SecretPolicy::MustExist)?;
    let proof_context = kaigi_usage_context_v1(
        *client.network_id(),
        call_id,
        &host,
        &standing.roster_root,
        standing.segments_recorded,
        duration_ms,
        billed_gas,
    )
    .map_err(report)?;
    let computed =
        kaigi_usage_host_commitment_v1(&proof_context, &secret.witness()?).map_err(report)?;
    standing.ensure_opens(&computed, secret_path)?;
    eprintln!("generating Kaigi usage proof...");
    prove_kaigi_usage_artifacts_v1(proof_context, secret.witness()?).map_err(report)
}

fn prove_authorization(
    proof_context: KaigiAuthorizationContextV1,
    secret: &KaigiSecret,
) -> Result<ParsedKaigiPrivacyArtifacts> {
    eprintln!("generating Kaigi authorization proof...");
    let KaigiAuthorizationArtifactsV1 {
        commitment,
        nullifier,
        roster_root,
        proof,
    } = prove_kaigi_authorization_artifacts_v1(proof_context, secret.witness()?).map_err(report)?;
    Ok(ParsedKaigiPrivacyArtifacts {
        commitment: Some(commitment),
        nullifier: Some(nullifier),
        roster_root: Some(roster_root),
        proof: Some(proof),
    })
}

fn report(error: impl std::fmt::Display) -> eyre::Report {
    eyre!("{error}")
}

/// Stored private blinding. The witness type consumes its input, so each proving
/// step takes a fresh copy of these bytes; both copies are erased on drop.
struct KaigiSecret {
    bytes: Zeroizing<[u8; 32]>,
}
impl KaigiSecret {
    fn witness(&self) -> Result<KaigiAuthorizationWitnessV1> {
        let mut copy = Zeroizing::new(*self.bytes);
        KaigiAuthorizationWitnessV1::take_blinding(&mut copy).map_err(report)
    }
}

fn load_secret(path: &Path, policy: SecretPolicy) -> Result<KaigiSecret> {
    match read_secret_file(path) {
        Ok(secret) => Ok(secret),
        Err(error)
            if policy == SecretPolicy::CreateIfMissing
                && error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == ErrorKind::NotFound) =>
        {
            create_secret_file(path)
        }
        Err(error) => Err(error),
    }
}

fn read_secret_file(path: &Path) -> Result<KaigiSecret> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        eyre::Report::new(error).wrap_err(format!(
            "cannot read Kaigi secret file {}: it is created when you create or join a call",
            path.display()
        ))
    })?;
    ensure!(
        metadata.is_file(),
        "Kaigi secret path {} must be a regular file, not a link or directory",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "Kaigi secret file {} must be accessible only by its owner (chmod 600)",
            path.display()
        );
    }
    let mut encoded = Zeroizing::new(String::new());
    fs::File::open(path)
        .wrap_err_with(|| format!("cannot open Kaigi secret file {}", path.display()))?
        .take(SECRET_FILE_MAX_BYTES + 1)
        .read_to_string(&mut encoded)
        .wrap_err_with(|| format!("cannot read Kaigi secret file {}", path.display()))?;
    parse_secret(&encoded).wrap_err_with(|| format!("invalid Kaigi secret file {}", path.display()))
}

fn parse_secret(encoded: &str) -> Result<KaigiSecret> {
    let trimmed = encoded.trim_end_matches('\n');
    ensure!(
        trimmed.len() == 64,
        "expected 64 hexadecimal characters of one canonical scalar"
    );
    let mut bytes = Zeroizing::new([0_u8; 32]);
    hex::decode_to_slice(trimmed, &mut bytes[..]).wrap_err("expected hexadecimal text")?;
    let secret = KaigiSecret { bytes };
    // A malformed secret is rejected here rather than when the first proof needs it.
    secret
        .witness()
        .wrap_err("expected a canonical nonzero Pasta scalar")?;
    Ok(secret)
}

fn create_secret_file(path: &Path) -> Result<KaigiSecret> {
    let bytes = sample_kaigi_blinding_v1(|bytes| SysRng.try_fill_bytes(bytes)).map_err(report)?;
    let secret = KaigiSecret { bytes };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        create_private_dir_all(parent)
            .wrap_err_with(|| format!("cannot create directory {}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        // Another process won the creation race: that secret is now the one to use.
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return read_secret_file(path),
        Err(error) => {
            return Err(eyre::Report::new(error).wrap_err(format!(
                "cannot create Kaigi secret file {}",
                path.display()
            )));
        }
    };
    let mut encoded = Zeroizing::new(hex::encode(*secret.bytes));
    encoded.push('\n');
    file.write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
        .wrap_err_with(|| format!("cannot write Kaigi secret file {}", path.display()))?;
    eprintln!(
        "created Kaigi secret {}; keep it safe, because leaving or ending this call requires it",
        path.display()
    );
    Ok(secret)
}

fn create_private_dir_all(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::{NetworkId, kaigi::KaigiId};
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_model_base::domain::DomainId;
    use tempfile::tempdir;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn call() -> KaigiId {
        KaigiId::new(
            DomainId::try_new("kaigi", "universal").unwrap(),
            "standup".parse().unwrap(),
        )
    }

    fn standing() -> CallStanding {
        CallStanding {
            private: true,
            ended: false,
            is_host: false,
            active: false,
            original_account: account(2),
            host: None,
            sequence: Some(1),
            active_commitment: None,
            roster_root: KaigiRecord::compute_roster_root(&[]),
            segments_recorded: 0,
        }
    }

    #[test]
    fn standing_parses_a_private_participant_response() {
        let root = hex::encode(<[u8; 32]>::from(KaigiRecord::compute_roster_root(&[])));
        let commitment = "11".repeat(31) + "00";
        let host = account(1);
        let subject = account(2);
        let response = norito::json!({
            "call_id": "kaigi.universal:standup",
            "privacy_mode": "private",
            "status": "active",
            "is_host": false,
            "active": true,
            "original_account_id": (subject.to_string()),
            "host_account_id": (host.to_string()),
            "sequence": 3,
            "active_commitment_hex": commitment,
            "roster_root_hex": root,
            "segments_recorded": 4,
        });
        let parsed =
            CallStanding::from_response(&response, crate::resolve_account_id_with).unwrap();
        assert!(parsed.private && parsed.active && !parsed.is_host && !parsed.ended);
        assert_eq!(parsed.original_account, subject);
        assert_eq!(parsed.host, Some(host));
        assert_eq!(parsed.sequence, Some(3));
        assert!(parsed.active_commitment.is_some());
        assert_eq!(parsed.segments_recorded, 4);
    }

    #[test]
    fn standing_rejects_a_response_missing_required_fields() {
        let response = norito::json!({ "privacy_mode": "private" });
        let error =
            CallStanding::from_response(&response, crate::resolve_account_id_with).unwrap_err();
        assert!(error.to_string().contains("omitted"), "{error}");
    }

    #[test]
    fn inadmissible_actions_fail_with_a_next_step() {
        let id = call();
        let mut transparent = standing();
        transparent.private = false;
        let error = transparent
            .ensure_admissible(&id, CallAction::Join)
            .unwrap_err();
        assert!(error.to_string().contains("omit --secret-file"), "{error}");

        let mut ended = standing();
        ended.ended = true;
        assert!(ended.ensure_admissible(&id, CallAction::Join).is_err());

        let mut joined = standing();
        joined.active = true;
        assert!(joined.ensure_admissible(&id, CallAction::Join).is_err());
        assert!(joined.ensure_admissible(&id, CallAction::Leave).is_ok());
        assert!(joined.ensure_admissible(&id, CallAction::HostEnd).is_err());

        let absent = standing();
        assert!(absent.ensure_admissible(&id, CallAction::Join).is_ok());
        assert!(absent.ensure_admissible(&id, CallAction::Leave).is_err());

        let mut host = standing();
        host.is_host = true;
        host.active = true;
        assert!(host.ensure_admissible(&id, CallAction::HostEnd).is_ok());
        assert!(host.ensure_admissible(&id, CallAction::Join).is_err());
        let error = host.ensure_admissible(&id, CallAction::Leave).unwrap_err();
        assert!(error.to_string().contains("iroha app kaigi end"), "{error}");
    }

    #[test]
    fn host_resolution_prefers_the_ledger_and_explains_a_missing_hint() {
        let id = call();
        let host = account(1);
        let other = account(3);
        let hidden = standing();
        let error = hidden.resolve_host(&id, None).unwrap_err();
        assert!(error.to_string().contains("--host"), "{error}");
        assert_eq!(hidden.resolve_host(&id, Some(&host)).unwrap(), host);
        let mut revealed = standing();
        revealed.host = Some(host.clone());
        assert_eq!(revealed.resolve_host(&id, None).unwrap(), host);
        assert_eq!(revealed.resolve_host(&id, Some(&host)).unwrap(), host);
        assert!(revealed.resolve_host(&id, Some(&other)).is_err());
    }

    #[test]
    fn secret_files_are_created_private_and_read_back_exactly() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/kaigi.secret");
        assert!(load_secret(&path, SecretPolicy::MustExist).is_err());
        assert!(!path.exists(), "a read-only action must not mint a secret");
        let created = load_secret(&path, SecretPolicy::CreateIfMissing).unwrap();
        let reread = load_secret(&path, SecretPolicy::MustExist).unwrap();
        assert_eq!(*created.bytes, *reread.bytes);
        let again = load_secret(&path, SecretPolicy::CreateIfMissing).unwrap();
        assert_eq!(*created.bytes, *again.bytes, "an existing secret is kept");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            let error = load_secret(&path, SecretPolicy::MustExist)
                .err()
                .expect("group-readable secret is rejected");
            assert!(error.to_string().contains("chmod 600"), "{error}");
        }
    }

    #[test]
    fn secret_parsing_rejects_everything_but_one_canonical_nonzero_scalar() {
        assert!(parse_secret(&format!("{}\n", "11".repeat(32))).is_ok());
        for invalid in [
            String::new(),
            "11".repeat(31),
            "11".repeat(33),
            "zz".repeat(32),
            // Zero, and the all-ones value above the Pasta modulus.
            "00".repeat(32),
            "ff".repeat(32),
        ] {
            assert!(parse_secret(&invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn a_wrong_secret_is_detected_before_proving() {
        let path = Path::new("kaigi.secret");
        let host = account(1);
        let context = kaigi_authorization_context_v1(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([7; 32]))),
            &call(),
            &host,
            &host,
            0,
            KaigiAuthorizationActionV1::HostEnd,
            &KaigiRecord::compute_roster_root(&[]),
        )
        .unwrap();
        let right = parse_secret(&"11".repeat(32)).unwrap();
        let wrong = parse_secret(&"22".repeat(32)).unwrap();
        let stored =
            kaigi_authorization_commitment_v1(&context, &right.witness().unwrap()).unwrap();
        let mut host_standing = standing();
        host_standing.active_commitment = Some(stored);
        assert!(host_standing.ensure_opens(&stored, path).is_ok());
        let computed =
            kaigi_authorization_commitment_v1(&context, &wrong.witness().unwrap()).unwrap();
        let error = host_standing.ensure_opens(&computed, path).unwrap_err();
        assert!(error.to_string().contains("does not open"), "{error}");
    }
}
