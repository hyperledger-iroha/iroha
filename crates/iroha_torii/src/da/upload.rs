//! Bounded restartable transport for the ordinary signed ingest.

use super::*;
use iroha_fs::{OwnerDirectory, PrivateDirectory, PublishMode};
use iroha_torii_shared::da::upload::{
    DA_UPLOAD_MAX_BYTES, DA_UPLOAD_PART_BYTES, DaUploadAction, DaUploadDescriptor, DaUploadPart,
    DaUploadProgress, DaUploadRequest,
};
use std::{collections::HashSet, io};

// One active upload per producer and a fixed total staging budget of at most 1 GiB.
const MAX_SESSIONS: usize = 16;
const MAX_PARTS: usize = DA_UPLOAD_MAX_BYTES / DA_UPLOAD_PART_BYTES;
const METADATA: &str = "session.json";

#[derive(norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct Session {
    upload: DaUploadDescriptor,
    expires: u64,
}

enum Outcome {
    Progress(u32),
    Ready(Box<DaIngestRequest>),
}

type UploadError = (StatusCode, String);
fn storage(error: impl std::fmt::Display) -> UploadError {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!("DA upload storage: {error}"),
    )
}

fn clear_session(dir: PrivateDirectory) -> Result<(), UploadError> {
    for entry in dir.entries(MAX_PARTS + 4).map_err(storage)? {
        dir.remove_private(entry).map_err(storage)?;
    }
    dir.remove_empty().map_err(storage)
}

fn session_record(dir: &PrivateDirectory) -> Result<Option<Session>, UploadError> {
    dir.read_optional(METADATA, 4096)
        .map_err(storage)?
        .map(|bytes| norito::json::from_slice(&bytes).map_err(storage))
        .transpose()
}

fn staging_root(spool: &Path) -> Result<PrivateDirectory, UploadError> {
    if spool.as_os_str().is_empty() {
        return Err(storage("missing DA ingest directory"));
    }
    let owner = OwnerDirectory::open_or_create(spool).map_err(storage)?;
    match owner.create_private_child("uploads") {
        Ok(dir) => Ok(dir),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            owner.open_private_child("uploads").map_err(storage)
        }
        Err(error) => Err(storage(error)),
    }
}

fn write_part(
    dir: &PrivateDirectory,
    part: DaUploadPart,
    length: usize,
    next: u32,
) -> Result<u32, UploadError> {
    let index = part.index as usize;
    if part.index > next || index >= length.div_ceil(DA_UPLOAD_PART_BYTES) {
        return Err((
            StatusCode::CONFLICT,
            "DA upload part is outside the contiguous prefix".into(),
        ));
    }
    let required = DA_UPLOAD_PART_BYTES.min(length - index * DA_UPLOAD_PART_BYTES);
    if part.bytes_b64.len() != required.div_ceil(3) * 4 {
        return Err((StatusCode::BAD_REQUEST, "incorrect DA part length".into()));
    }
    let bytes = BASE64
        .decode(part.bytes_b64)
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid DA part encoding".into()))?;
    if bytes.len() != required {
        return Err((
            StatusCode::BAD_REQUEST,
            "incorrect decoded DA part length".into(),
        ));
    }
    let name = format!("part-{index}");
    if part.index < next {
        if dir.read(&name, required).map_err(storage)?.as_slice() != bytes.as_slice() {
            return Err((
                StatusCode::CONFLICT,
                "different bytes already occupy this DA part".into(),
            ));
        }
        return Ok(next);
    }
    dir.write_atomic(name, &bytes, PublishMode::CreateNew)
        .map_err(storage)?;
    Ok(next + 1)
}

fn operate(
    spool: &Path,
    owner: &AccountId,
    envelope: DaUploadRequest,
    now: u64,
    ttl: Duration,
) -> Result<Outcome, UploadError> {
    let DaUploadRequest { upload, action } = envelope;
    let length = usize::try_from(upload.wire_bytes)
        .ok()
        .filter(|n| (1..=DA_UPLOAD_MAX_BYTES).contains(n))
        .ok_or_else(|| {
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                "invalid DA upload wire length".into(),
            )
        })?;
    if let DaUploadAction::Finish(request) = &action
        && (!request.payload.is_empty()
            || &request.owner != owner
            || request.network_id != upload.network_id
            || request.lane_id != upload.lane_id
            || request.epoch != upload.epoch)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "DA finish metadata does not match authenticated upload".into(),
        ));
    }
    let root = staging_root(spool)?;
    let lock = root.open_lock("lock").map_err(storage)?;
    if matches!(&action, DaUploadAction::Release) {
        // An accepted upload can release its reservation while another producer is writing.
        lock.lock().map_err(storage)?;
    } else {
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DA upload staging is busy".into(),
            ),
            std::fs::TryLockError::Error(error) => storage(error),
        })?;
    }
    root.require_same_lock("lock", &lock).map_err(storage)?;
    let owner_name = blake3_hash(&to_bytes(owner).map_err(storage)?)
        .to_hex()
        .to_string();
    let mut session = root.open_child_optional(&owner_name).map_err(storage)?;
    if let Some(dir) = session.take() {
        match session_record(&dir)? {
            Some(record) if now < record.expires => {
                if record.upload != upload {
                    return Err((
                        StatusCode::CONFLICT,
                        "producer already has another active DA upload".into(),
                    ));
                }
                session = Some(dir);
            }
            _ => clear_session(dir)?,
        }
    }

    if matches!(action, DaUploadAction::Release) {
        if let Some(dir) = session {
            clear_session(dir)?;
        }
        return Ok(Outcome::Progress(0));
    }
    let dir = match session {
        Some(dir) => dir,
        None if matches!(action, DaUploadAction::Begin) => {
            let entries = root.entries(MAX_SESSIONS + 1).map_err(storage)?;
            if entries.len() > MAX_SESSIONS {
                let mut occupied = 0;
                for entry in entries {
                    if entry == "lock" {
                        continue;
                    }
                    let dir = root.open_child(entry).map_err(storage)?;
                    if session_record(&dir)?.is_some_and(|record| now < record.expires) {
                        occupied += 1;
                    } else {
                        clear_session(dir)?;
                    }
                }
                if occupied >= MAX_SESSIONS {
                    return Err((
                        StatusCode::TOO_MANY_REQUESTS,
                        "DA upload staging capacity is full".into(),
                    ));
                }
            }
            let dir = root.create_child(&owner_name).map_err(storage)?;
            let expires = now
                .checked_add(ttl.as_secs().max(1))
                .ok_or_else(|| storage("upload expiry overflow"))?;
            let data = norito::json::to_vec(&Session { upload, expires }).map_err(storage)?;
            dir.write_atomic(METADATA, &data, PublishMode::CreateNew)
                .map_err(storage)?;
            dir
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                "DA upload expired or was never started".into(),
            ));
        }
    };
    let entries: HashSet<_> = dir
        .entries(MAX_PARTS + 4)
        .map_err(storage)?
        .into_iter()
        .collect();
    let count = length.div_ceil(DA_UPLOAD_PART_BYTES);
    let next = (0..count)
        .find(|index| !entries.contains(std::ffi::OsStr::new(&format!("part-{index}"))))
        .unwrap_or(count);
    match action {
        DaUploadAction::Begin => Ok(Outcome::Progress(next as u32)),
        DaUploadAction::Put(part) => {
            write_part(&dir, part, length, next as u32).map(Outcome::Progress)
        }
        DaUploadAction::Finish(mut request) => {
            if next != count {
                return Err((StatusCode::CONFLICT, "DA upload is incomplete".into()));
            }
            let mut payload = Vec::with_capacity(length);
            for index in 0..count {
                let required = DA_UPLOAD_PART_BYTES.min(length - payload.len());
                let bytes = dir
                    .read(format!("part-{index}"), required)
                    .map_err(storage)?;
                if bytes.len() != required {
                    return Err(storage("staged DA part has the wrong length"));
                }
                payload.extend_from_slice(&bytes);
            }
            if BlobDigest::from_hash(blake3_hash(&payload)) != upload.wire_hash {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "DA upload wire digest mismatch".into(),
                ));
            }
            request.payload = payload;
            Ok(Outcome::Ready(request))
        }
        DaUploadAction::Release => unreachable!("release completed before session loading"),
    }
}

/// Authenticate, stage or finalize a resumable DA ingest.
pub async fn handler_post_da_parts(
    State(app): State<SharedAppState>,
    Extension(principal): Extension<crate::app_auth::VerifiedCanonicalRequest>,
    headers: HeaderMap,
    utils::extractors::JsonOnly(envelope): utils::extractors::JsonOnly<DaUploadRequest>,
) -> Result<Response, ResponseError> {
    let format = utils::negotiate_response_format(headers.get(axum::http::header::ACCEPT))
        .map_err(ResponseError::from)?;
    let fail = |(status, message): UploadError| {
        ResponseError::from(build_error_response(status, &message, format))
    };
    reject_emergency_fast_da_service(app.as_ref(), format)?;
    if &envelope.upload.network_id != app.state.network_id_ref() {
        return Err(fail((
            StatusCode::FORBIDDEN,
            "DA upload targets a different network".into(),
        )));
    }
    admission_snapshot_for_request(
        &app,
        &principal.account,
        envelope.upload.lane_id,
        envelope.upload.epoch,
    )
    .map_err(fail)?;
    let finishing = matches!(&envelope.action, DaUploadAction::Finish(_));
    // Bound restored request bodies through admission as well as during the disk read.
    let finish_permit = if finishing {
        Some(Arc::new(
            Arc::clone(&app.da_upload_finishes)
                .try_acquire_owned()
                .map_err(|_| {
                    fail((
                        StatusCode::SERVICE_UNAVAILABLE,
                        "DA upload finalization capacity is saturated".into(),
                    ))
                })?,
        ))
    } else {
        None
    };
    let worker_permit = finish_permit.clone();
    let release = finishing.then(|| DaUploadRequest {
        upload: envelope.upload,
        action: DaUploadAction::Release,
    });
    let spool = app.da_ingest.manifest_store_dir.clone();
    let owner = principal.account.clone();
    let ttl = app.da_ingest.replay_cache_ttl;
    let outcome =
        run_da_ingest_compute_job(Arc::clone(&app.da_ingest_compute_inflight), move || {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(storage)?
                .as_secs();
            let _guard = worker_permit;
            operate(&spool, &owner, envelope, now, ttl)
        })
        .await
        .map_err(fail)?;
    match outcome {
        Outcome::Progress(next_part) => Ok(utils::respond_with_format(
            &DaUploadProgress { next_part },
            format,
        )),
        Outcome::Ready(request) => {
            let response = handler_post_da_ingest(
                State(Arc::clone(&app)),
                Extension(principal.clone()),
                headers,
                utils::extractors::JsonOnly(*request),
            )
            .await?;
            if response.extensions().get::<AcceptedDaIngest>().is_some()
                && let Some(release) = release
            {
                let spool = app.da_ingest.manifest_store_dir.clone();
                // Finishes already own bounded capacity. Carry it through physical cleanup
                // so a saturated compute pool or a dropped HTTP future cannot leak a session.
                let cleanup = panic_recovery::spawn_blocking_recoverable(move || {
                    let _reservation = finish_permit;
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(storage)?
                        .as_secs();
                    operate(&spool, &principal.account, release, now, ttl).map(|_| ())
                });
                match panic_recovery::join_recoverable(cleanup).await {
                    Ok(Ok(())) => {}
                    Ok(Err((_, reason))) => {
                        warn!(%reason, "DA upload cleanup deferred until release or expiry")
                    }
                    Err(reason) => warn!(%reason, "DA upload cleanup worker failed"),
                }
            }
            Ok(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_upload_can_release_its_slot_during_another_staging_operation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        let request = super::super::tests::sample_request();
        let upload = DaUploadDescriptor {
            network_id: request.network_id,
            lane_id: request.lane_id,
            epoch: request.epoch,
            wire_bytes: 1,
            wire_hash: BlobDigest::from_hash(blake3_hash(&[1])),
        };
        let message = |action| DaUploadRequest { upload, action };
        let ttl = Duration::from_secs(60);
        assert!(matches!(
            operate(
                &path,
                &request.owner,
                message(DaUploadAction::Begin),
                1,
                ttl
            ),
            Ok(Outcome::Progress(0))
        ));
        let dir = staging_root(&path).unwrap();
        let lock = dir.open_lock("lock").unwrap();
        lock.lock().unwrap();
        assert!(matches!(
            operate(
                &path,
                &request.owner,
                message(DaUploadAction::Begin),
                2,
                ttl
            ),
            Err((StatusCode::SERVICE_UNAVAILABLE, _))
        ));
        std::thread::scope(|scope| {
            let (started, entered) = std::sync::mpsc::channel();
            let release = message(DaUploadAction::Release);
            let owner = &request.owner;
            let path = &path;
            let worker = scope.spawn(move || {
                started.send(()).unwrap();
                operate(path, owner, release, 2, ttl)
            });
            entered.recv().unwrap();
            drop(lock);
            assert!(matches!(worker.join().unwrap(), Ok(Outcome::Progress(0))));
        });
        let mut next_upload = upload;
        next_upload.wire_hash = BlobDigest::from_hash(blake3_hash(&[2]));
        assert!(matches!(
            operate(
                &path,
                &request.owner,
                DaUploadRequest {
                    upload: next_upload,
                    action: DaUploadAction::Begin
                },
                3,
                ttl
            ),
            Ok(Outcome::Progress(0))
        ));
    }

    #[test]
    fn upload_restores_signed_bytes_after_reopen_and_rejects_conflicting_parts() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        let mut original = super::super::tests::sample_request();
        let mut bytes = vec![0xa5; DA_UPLOAD_PART_BYTES + 5];
        bytes[DA_UPLOAD_PART_BYTES] = 0x17;
        original.payload.clone_from(&bytes);
        let owner = original.owner.clone();
        let upload = DaUploadDescriptor {
            network_id: original.network_id,
            lane_id: original.lane_id,
            epoch: original.epoch,
            wire_bytes: bytes.len() as u64,
            wire_hash: BlobDigest::from_hash(blake3_hash(&bytes)),
        };
        let call = |action, now| {
            operate(
                &path,
                &owner,
                DaUploadRequest { upload, action },
                now,
                Duration::from_secs(10),
            )
        };
        let part = |index, bytes| {
            DaUploadAction::Put(DaUploadPart {
                index,
                bytes_b64: BASE64.encode(bytes),
            })
        };
        assert!(matches!(
            call(DaUploadAction::Begin, 1),
            Ok(Outcome::Progress(0))
        ));
        assert!(matches!(
            call(part(1, &bytes[DA_UPLOAD_PART_BYTES..]), 2),
            Err((StatusCode::CONFLICT, _))
        ));
        assert!(matches!(
            call(part(0, &bytes[..DA_UPLOAD_PART_BYTES]), 2),
            Ok(Outcome::Progress(1))
        ));
        assert!(matches!(
            call(DaUploadAction::Begin, 3),
            Ok(Outcome::Progress(1))
        ));
        let mut bad = bytes[..DA_UPLOAD_PART_BYTES].to_vec();
        bad[0] ^= 1;
        assert!(matches!(
            call(part(0, &bad), 4),
            Err((StatusCode::CONFLICT, _))
        ));
        assert!(matches!(
            call(part(0, &bytes[..DA_UPLOAD_PART_BYTES]), 5),
            Ok(Outcome::Progress(1))
        ));
        let mut metadata = original.clone();
        metadata.payload.clear();
        assert!(matches!(
            call(DaUploadAction::Finish(Box::new(metadata.clone())), 5),
            Err((StatusCode::CONFLICT, _))
        ));
        assert!(matches!(
            call(part(1, &bytes[DA_UPLOAD_PART_BYTES..]), 5),
            Ok(Outcome::Progress(2))
        ));
        let Ok(Outcome::Ready(restored)) =
            call(DaUploadAction::Finish(Box::new(metadata.clone())), 6)
        else {
            panic!("expected complete upload")
        };
        assert_eq!(*restored, original);
        let mut wrong = metadata.clone();
        wrong.epoch += 1;
        assert!(matches!(
            call(DaUploadAction::Finish(Box::new(wrong)), 6),
            Err((StatusCode::BAD_REQUEST, _))
        ));
        assert!(matches!(
            call(DaUploadAction::Begin, 11),
            Ok(Outcome::Progress(0))
        ));
        assert!(matches!(
            call(DaUploadAction::Release, 11),
            Ok(Outcome::Progress(0))
        ));
        assert!(matches!(
            call(DaUploadAction::Finish(Box::new(metadata)), 12),
            Err((StatusCode::NOT_FOUND, _))
        ));
    }
}
