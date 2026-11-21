//! Prepare a public seat registration for the existing account-authenticated Parliament draft route.
//!
//! Run `parliament_credential_draft NETWORK_ID ACCOUNT_I105 BODY.nrt ASSIGNMENT_HEX WALLET_DIR` with the
//! authenticated body-context frame and seat assignment of the named account. The private wallet directory retains the reusable credential secret
//! and the exact registration across interruption and retry. Standard output is the public JSON
//! instruction accepted by `/v1/gov/anonymous-parliament/credentials/draft`. The wallet owner
//! confirms the draft and uses their ordinary account signing workflow.

use std::{
    fs::File,
    io::{self, Read as _},
    path::Path,
};

use eyre::{OptionExt, Result, WrapErr, bail, ensure};
use iroha::data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        anonymous_ballot::{
            BALLOT_CONTEXT_MAX_BYTES_V1, BallotBodyContextV1, RegisterParliamentCredentialV1,
            RegistrationProofBytesV1,
        },
        types::AssignmentId,
    },
};
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_parliament_ballot::{
    registration::verify_registration_v1,
    wallet::{CREDENTIAL_SECRET_BYTES, CredentialHandleV1, prepare_registration_v1},
};

// Body, authority and assignment each have a fixed bounded representation. The registration
// adds their canonical identifiers, its commitment and authorization to the fixed proof.
const REGISTRATION_MAX: usize = RegistrationProofBytesV1::LENGTH + 4096;

fn read_identity(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(BALLOT_CONTEXT_MAX_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= BALLOT_CONTEXT_MAX_BYTES_V1,
        "identity frame exceeds the input bound"
    );
    Ok(bytes)
}

fn publish_once(directory: &PrivateDirectory, name: &str, bytes: &[u8]) -> Result<()> {
    match directory.write_atomic(name, bytes, PublishMode::CreateNew) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    directory.sync()?;
    Ok(())
}

fn prepare(
    owner: &PrivateDirectory,
    body: BallotBodyContextV1,
    authority: &AccountId,
    assignment: &AssignmentId,
) -> Result<RegisterParliamentCredentialV1> {
    body.validate()?;
    ensure!(
        body.governance_attempt_id.as_bytes() != &[0; 32]
            && body.election_attempt_id.as_bytes() != &[0; 32]
            && assignment.as_bytes() != &[0; 32],
        "empty attempt or assignment identifier"
    );
    // The exact public enrollment must win create-only publication before generating a
    // secret or producing a proof. This private record is never sent to ballot ingress.
    let binding = norito::encode_canonical(&(body, authority.clone(), *assignment))?;
    if owner
        .read_optional("enrollment.nrt", binding.len())?
        .is_none()
    {
        if owner
            .read_optional("credential", CREDENTIAL_SECRET_BYTES)?
            .is_some()
        {
            // A concurrent call may have installed the binding and credential since the
            // first read. Preserve an existing credential whose origin is unknown.
            ensure!(
                owner
                    .read_optional("enrollment.nrt", binding.len())?
                    .is_some(),
                "existing credential has no enrollment binding"
            );
        }
        publish_once(owner, "enrollment.nrt", &binding)?;
    }
    ensure!(
        owner.read("enrollment.nrt", binding.len())?.as_slice() == binding,
        "wallet directory belongs to another enrollment"
    );
    if owner
        .read_optional("credential", CREDENTIAL_SECRET_BYTES)?
        .is_none()
    {
        let generated = CredentialHandleV1::generate_v1(body)?;
        publish_once(owner, "credential", generated.export_secret_v1().as_slice())?;
    }
    let secret = owner.read("credential", CREDENTIAL_SECRET_BYTES)?;
    let handle = CredentialHandleV1::import_v1(
        body,
        secret
            .as_slice()
            .try_into()
            .wrap_err("stored credential length")?,
        None,
    )?;
    if owner
        .read_optional("registration.nrt", REGISTRATION_MAX)?
        .is_none()
    {
        let instruction = RegisterParliamentCredentialV1 {
            governance_attempt_id: body.governance_attempt_id,
            election_attempt_id: body.election_attempt_id,
            registration: prepare_registration_v1(&handle, authority, assignment)?,
        };
        publish_once(
            owner,
            "registration.nrt",
            &norito::encode_canonical(&instruction)?,
        )?;
    }
    // The winner of a concurrent create or a previously stored draft is verified again
    // before releasing any instruction for account signing.
    let verified: RegisterParliamentCredentialV1 =
        norito::decode_canonical(&owner.read("registration.nrt", REGISTRATION_MAX)?)?;
    ensure!(
        verified.governance_attempt_id == body.governance_attempt_id
            && verified.election_attempt_id == body.election_attempt_id
            && verified.registration.commitment == *handle.commitment(),
        "stored registration belongs to another enrollment"
    );
    verify_registration_v1(&body, authority, assignment, &verified.registration)?;
    Ok(verified)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [network, account, body_path, assignment, directory_path] = args.as_slice() else {
        bail!(
            "usage: parliament_credential_draft NETWORK_ID ACCOUNT_I105 BODY.nrt ASSIGNMENT_HEX WALLET_DIR"
        );
    };
    let network: NetworkId = network
        .to_str()
        .ok_or_eyre("invalid network identifier text")?
        .parse()?;
    let account = AccountId::parse_encoded(
        account
            .to_str()
            .ok_or_eyre("invalid account identifier text")?,
    )?;
    let body: BallotBodyContextV1 =
        norito::decode_canonical(&read_identity(Path::new(body_path))?)?;
    ensure!(
        network == body.network_id,
        "body context belongs to another network"
    );
    let assignment = AssignmentId::from_hex_str(
        assignment
            .to_str()
            .ok_or_eyre("invalid assignment identifier text")?,
    )?;
    let owner = PrivateDirectory::open_or_create(directory_path)?;
    let instruction = prepare(&owner, body, &account, &assignment)?;
    println!("{}", norito::json::to_json(&instruction)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::config::Config;
    use iroha::data_model::governance::anonymous_ballot::ParliamentBallotBoardRecordV1;

    fn body() -> BallotBodyContextV1 {
        let board: ParliamentBallotBoardRecordV1 = norito::decode_canonical(include_bytes!(
            "../../../fixtures/governance/anonymous_ballot_http_v1/board.nrt"
        ))
        .unwrap();
        board.context.body_context
    }

    #[test]
    fn registration_roundtrips_and_retries_keep_the_exact_credential_and_public_draft() -> Result<()>
    {
        let config = Config::load_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/client.toml"),
        )?;
        let temp = tempfile::tempdir()?;
        let owner = PrivateDirectory::open_or_create(temp.path().join("seat"))?;
        let assignment = AssignmentId::new([0x31; 32]);
        let first = prepare(&owner, body(), &config.account, &assignment)?;
        let secret = owner.read("credential", CREDENTIAL_SECRET_BYTES)?;
        let json = norito::json::to_json(&first)?;
        let draft: RegisterParliamentCredentialV1 = norito::json::from_str(&json)?;
        assert_eq!(draft, first);
        assert!(
            prepare(
                &owner,
                body(),
                &config.account,
                &AssignmentId::new([0x42; 32])
            )
            .is_err()
        );
        assert_eq!(
            prepare(&owner, body(), &config.account, &assignment)?,
            first
        );
        assert_eq!(secret, owner.read("credential", CREDENTIAL_SECRET_BYTES)?);
        let other = &*iroha_test_samples::BOB_ID;
        assert_ne!(&config.account, other);
        assert!(prepare(&owner, body(), other, &assignment).is_err());
        let mut altered = first.clone();
        altered.election_attempt_id =
            iroha::data_model::governance::types::BodyElectionAttemptId::new([0x33; 32]);
        owner.write_atomic(
            "registration.nrt",
            &norito::encode_canonical(&altered)?,
            PublishMode::Replace,
        )?;
        assert!(prepare(&owner, body(), &config.account, &assignment).is_err());
        Ok(())
    }

    #[test]
    fn simultaneous_distinct_enrollments_cannot_share_a_credential_directory() -> Result<()> {
        let config = Config::load_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/client.toml"),
        )?;
        let temp = tempfile::tempdir()?;
        let owner = PrivateDirectory::open_or_create(temp.path().join("seat"))?;
        let start = std::sync::Barrier::new(2);
        let assignments = [AssignmentId::new([0x51; 32]), AssignmentId::new([0x52; 32])];
        let results = std::thread::scope(|scope| {
            let calls = assignments.each_ref().map(|id| {
                scope.spawn(|| {
                    start.wait();
                    prepare(&owner, body(), &config.account, id)
                })
            });
            calls.map(|thread| thread.join().expect("registration worker completed"))
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        for (id, result) in assignments.iter().zip(results) {
            let resumed = prepare(&owner, body(), &config.account, id);
            match result {
                Ok(original) => assert_eq!(resumed?, original),
                Err(_) => assert!(resumed.is_err()),
            }
        }
        Ok(())
    }

    #[test]
    fn only_bounded_body_frames_are_read() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("body.nrt");
        let bytes = norito::encode_canonical(&body())?;
        std::fs::write(&path, &bytes)?;
        assert_eq!(read_identity(&path)?, bytes);
        std::fs::write(&path, vec![0; BALLOT_CONTEXT_MAX_BYTES_V1 + 1])?;
        assert!(read_identity(&path).is_err());
        Ok(())
    }

    #[test]
    fn preexisting_credential_without_enrollment_is_preserved_and_refused() -> Result<()> {
        let config = Config::load_file(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/client.toml"),
        )?;
        let temp = tempfile::tempdir()?;
        let owner = PrivateDirectory::open_or_create(temp.path().join("seat"))?;
        owner.write_atomic(
            "credential",
            &[0x65; CREDENTIAL_SECRET_BYTES],
            PublishMode::CreateNew,
        )?;
        assert!(prepare(&owner, body(), &config.account, &AssignmentId::new([5; 32])).is_err());
        assert_eq!(
            owner
                .read("credential", CREDENTIAL_SECRET_BYTES)?
                .as_slice(),
            &[0x65; CREDENTIAL_SECRET_BYTES]
        );
        assert!(
            owner
                .read_optional("registration.nrt", REGISTRATION_MAX)?
                .is_none()
        );
        Ok(())
    }
}
