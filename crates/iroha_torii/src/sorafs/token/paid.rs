//! Bound a resumable signed-account request to one native signer operation.
//! Only its unsigned original is stored on disk. A recovered original is read-only at the signer.
use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};
use sorafs_manifest::provider_advert::account_read::RegisteredAccountReadV1;
use std::path::Path;
use tokio::sync::oneshot;

const SLOTS: usize = 64;
const MAX_RECORD: usize = 8192;
type Error = StreamTokenIssuerError;

pub(in crate::sorafs) fn valid_operation(value: &str) -> bool {
    let Some((id, cursor)) = value.split_once(':') else {
        return false;
    };
    id.len() == 64
        && id.bytes().any(|b| b != b'0')
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && cursor.parse::<u64>().is_ok_and(|n| n.to_string() == cursor)
}

pub(in crate::sorafs) struct Request<'a> {
    pub subject: StreamTokenQuotaSubject,
    pub operation: &'a str,
    pub manifest_id: &'a str,
    pub manifest_cid: &'a [u8],
    pub provider: [u8; 32],
    pub profile: &'a str,
    pub overrides: TokenOverrides,
    pub policy: &'a RegisteredAccountReadV1,
}

/// Exact account-authenticated request and locally selected manifest. It identifies only
/// a running worker and carries no authority to sign, recover or release a token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::sorafs) struct StatusKey([u8; 32]);
impl StatusKey {
    pub(in crate::sorafs) fn new(
        subject: StreamTokenQuotaSubject,
        canonical_request: &[u8],
        manifest_cid: &[u8],
        profile: &str,
    ) -> Self {
        Self(digest(
            "iroha.sorafs.paid-token.active-request.v1",
            &[
                &subject.0,
                canonical_request,
                manifest_cid,
                profile.as_bytes(),
            ],
        ))
    }
}
fn digest(domain: &'static str, parts: &[&[u8]]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key(domain);
    for p in parts {
        h.update(&(p.len() as u64).to_be_bytes());
        h.update(p);
    }
    *h.finalize().as_bytes()
}

impl Request<'_> {
    fn identity(&self, key: &[u8; 32]) -> Result<([u8; 32], [u8; 32]), Error> {
        let address = digest(
            "iroha.sorafs.paid-token.address.v1",
            &[&self.subject.0, &self.provider, self.operation.as_bytes()],
        );
        let policy =
            norito::encode_canonical(self.policy).map_err(|_| Error::SignerEvidenceInvalid)?;
        let o = self.overrides;
        let overrides = norito::encode_canonical(&(
            o.ttl_secs,
            o.max_streams,
            o.rate_limit_bytes,
            o.requests_per_minute,
        ))
        .map_err(|_| Error::SignerEvidenceInvalid)?;
        let binding = digest(
            "iroha.sorafs.paid-token.request.v1",
            &[
                &address,
                key,
                self.manifest_id.as_bytes(),
                self.manifest_cid,
                self.profile.as_bytes(),
                &overrides,
                &policy,
            ],
        );
        Ok((address, binding))
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sorafs::paid_account_token::OriginalV1")]
struct Original {
    address: [u8; 32],
    binding: [u8; 32],
    body: StreamTokenBodyV1,
    remaining: u32,
}
impl Original {
    fn token_id(&self) -> Result<String, Error> {
        let mut template = self.body.clone();
        template.token_id.clear();
        let bytes = template
            .to_canonical_bytes()
            .map_err(|_| Error::SignerEvidenceInvalid)?;
        let hash = digest(
            "iroha.sorafs.paid-token.original.v1",
            &[
                &self.address,
                &self.binding,
                &bytes,
                &self.remaining.to_be_bytes(),
            ],
        );
        Ok(hex::encode(&hash[..16]))
    }
    fn validate(&self) -> Result<(), Error> {
        validate_token_body(&self.body)?;
        if self.token_id()? != self.body.token_id {
            return Err(Error::SignerEvidenceInvalid);
        }
        Ok(())
    }
}

// A completed signature can be redelivered only with a fresh, authenticated request,
// current account policy and fresh native custody admission. It is never serialized here.
struct Ready(TokenIssue);
impl Drop for Ready {
    fn drop(&mut self) {
        iroha_crypto::zeroize_value_for_confidential_discard(&mut self.0.token.signature);
    }
}
enum State {
    Preparing,
    Active,
    Ready(Ready),
    Failed(Error),
}
pub(super) struct Entry {
    status: StatusKey,
    address: [u8; 32],
    binding: [u8; 32],
    state: State,
    expires: u64,
}

struct Claim<'a> {
    issuer: &'a StreamTokenIssuer,
    slot: u8,
    active: bool,
}
impl Claim<'_> {
    fn set(&mut self, state: State, expires: u64) -> Result<(), Error> {
        let mut jobs = self
            .issuer
            .paid_jobs
            .lock()
            .map_err(|_| Error::IssuanceQuotaStateUnavailable)?;
        let entry = jobs
            .get_mut(&self.slot)
            .ok_or(Error::IssuanceQuotaStateUnavailable)?;
        entry.state = state;
        entry.expires = expires;
        Ok(())
    }
}
impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if self.active
            && let Ok(mut jobs) = self.issuer.paid_jobs.lock()
        {
            jobs.remove(&self.slot);
        }
    }
}

impl StreamTokenIssuer {
    /// The original worker already holds its normal and heavy permits. Polling its status
    /// must not require a second permit. All Ready delivery takes the ordinary admission path.
    pub(in crate::sorafs) fn paid_work_active(
        &self,
        status: StatusKey,
        current_epoch: u64,
    ) -> bool {
        // This status-only read never contends on the signer's history lock, which may
        // enclose durable finality work. Actual delivery samples the trusted clock again.
        current_epoch >= self.max_seen_epoch.load(Ordering::Acquire)
            && self.paid_jobs.lock().is_ok_and(|jobs| {
                jobs.values().any(|e| {
                    e.status == status
                        && matches!(e.state, State::Active)
                        && current_epoch < e.expires
                })
            })
    }
    pub(in crate::sorafs) fn paid_release_time(
        &self,
        body: &StreamTokenBodyV1,
    ) -> Result<(), Error> {
        let now = self.signer.now_unix_ms()? / 1000;
        self.observe_epoch(now)?;
        if now < body.issued_at || now >= body.ttl_epoch {
            return Err(Error::SignerStateChanged);
        }
        Ok(())
    }
    pub(in crate::sorafs) fn issue_paid(
        &self,
        path: &Path,
        req: Request<'_>,
        status: StatusKey,
        acknowledge: oneshot::Sender<()>,
    ) -> Result<TokenIssue, Error> {
        if !valid_operation(req.operation) || norito::core::decode_limits_active() {
            return Err(Error::SignerEvidenceInvalid);
        }
        let (address, binding) = req.identity(&self.verifying_key_bytes())?;
        let slot = address[0] % SLOTS as u8;
        let now = self.signer.now_unix_ms()? / 1000;
        self.observe_epoch(now)?;
        let busy = || Error::IssuanceQuotaCapacityExceeded { capacity: SLOTS };
        let invalid = || Error::SignerEvidenceInvalid;
        let unavailable = || Error::RuntimeSignerUnavailable;
        let mut jobs = self
            .paid_jobs
            .lock()
            .map_err(|_| Error::IssuanceQuotaStateUnavailable)?;
        let cached = if let Some(e) = jobs.get(&slot) {
            if e.address == address && e.binding != binding {
                return Err(invalid());
            }
            if e.binding == binding {
                match &e.state {
                    State::Active if now >= e.expires => return Err(Error::SignerStateChanged),
                    State::Active => {
                        let _ = acknowledge.send(());
                        return Err(unavailable());
                    }
                    State::Preparing => return Err(busy()),
                    State::Ready(r) if now < e.expires => Some(TokenIssue {
                        token: r.0.token.clone(),
                        remaining_quota: r.0.remaining_quota,
                    }),
                    State::Failed(_) => {
                        let Some(Entry {
                            state: State::Failed(error),
                            ..
                        }) = jobs.remove(&slot)
                        else {
                            unreachable!()
                        };
                        return Err(error);
                    }
                    State::Ready(_) => return Err(Error::SignerStateChanged),
                }
            } else {
                if matches!(e.state, State::Preparing | State::Active) || now < e.expires {
                    return Err(busy());
                }
                None
            }
        } else {
            None
        };
        if let Some(mut issue) = cached {
            drop(jobs);
            self.before_admission(&issue.token.body)?;
            issue.remaining_quota =
                self.remaining_after_delivery(req.subject, req.policy, issue.remaining_quota)?;
            return Ok(issue);
        }
        jobs.insert(
            slot,
            Entry {
                status,
                address,
                binding,
                state: State::Preparing,
                expires: now,
            },
        );
        drop(jobs);
        let mut claim = Claim {
            issuer: self,
            slot,
            active: true,
        };
        let dir = PrivateDirectory::open_or_create(path).map_err(|_| unavailable())?;
        let lease = dir
            .open_lock(format!("slot-{slot}.lock"))
            .map_err(|_| unavailable())?;
        lease.try_lock().map_err(|_| busy())?;
        let name = format!("slot-{slot}.nrt");
        let existing = dir
            .read_optional(&name, MAX_RECORD)
            .map_err(|_| unavailable())?;
        let previous = existing
            .as_ref()
            .map(|b| {
                norito::decode_canonical_with_limits::<Original>(
                    b,
                    norito::DecodeLimits::new(512, MAX_RECORD, 4096, 128 * 1024, 16),
                )
                .map_err(|_| invalid())
            })
            .transpose()?;
        if let Some(p) = &previous {
            p.validate()?;
            if p.address[0] % SLOTS as u8 != slot {
                return Err(invalid());
            }
            if p.address == address {
                if p.binding != binding {
                    return Err(invalid());
                }
                if now >= p.body.ttl_epoch {
                    return Err(Error::SignerStateChanged);
                }
            } else if now < p.body.ttl_epoch {
                return Err(busy());
            }
        }
        let (original, recover) = match previous {
            Some(p) if p.address == address => (p, true),
            _ => {
                let (body, limit) = self.prepare_token(
                    req.manifest_cid.to_vec(),
                    req.provider,
                    req.profile.to_owned(),
                    req.overrides,
                    Some(req.policy),
                )?;
                let remaining = self.reserve_issuance_budget(req.subject, Instant::now(), limit)?;
                let mut original = Original {
                    address,
                    binding,
                    body,
                    remaining,
                };
                original.body.token_id = original.token_id()?;
                original.validate()?;
                let bytes = norito::encode_canonical(&original).map_err(|_| invalid())?;
                if bytes.len() > MAX_RECORD {
                    return Err(invalid());
                }
                dir.write_atomic(
                    &name,
                    &bytes,
                    if existing.is_some() {
                        PublishMode::Replace
                    } else {
                        PublishMode::CreateNew
                    },
                )
                .map_err(|_| unavailable())?;
                (original, false)
            }
        };
        let expires = original.body.ttl_epoch;
        claim.set(State::Active, expires)?;
        let _ = acknowledge.send(());
        let result = if recover {
            self.signer.recover_undelivered(original.body)
        } else {
            self.signer.sign_paid(original.body)
        };
        let issue = match result {
            Ok(token) => TokenIssue {
                token,
                remaining_quota: original.remaining,
            },
            Err(err) => {
                claim.set(State::Failed(err), expires)?;
                claim.active = false;
                return Err(unavailable());
            }
        };
        claim.set(
            State::Ready(Ready(TokenIssue {
                token: issue.token.clone(),
                remaining_quota: issue.remaining_quota,
            })),
            expires,
        )?;
        claim.active = false;
        Ok(issue)
    }
}

#[cfg(test)]
#[path = "paid_tests.rs"]
mod tests;
