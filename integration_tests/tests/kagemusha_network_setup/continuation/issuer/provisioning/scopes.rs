//! Exact application-session scope originals and Native permit digest derivation.
use super::*;

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct Originals {
    pub audience: Original,
    pub release: Original,
    pub service_origin: Original,
}

pub(super) struct Scopes {
    pub audience: Vec<u8>,
    pub release: Vec<u8>,
    pub service_origin: Vec<u8>,
    pub operator_digest: [u8; 32],
    pub release_digest: [u8; 32],
    pub service_origin_digest: [u8; 32],
}

pub(super) fn read(originals: &Originals, inputs: &mut Inputs) -> Result<Scopes> {
    from_originals(
        inputs.read(&originals.audience, 512)?,
        inputs.read(&originals.release, PUBLIC_MAX)?,
        inputs.read(&originals.service_origin, 2048)?,
    )
}

fn exact_text(original: &[u8], maximum: usize) -> Result<&str> {
    let text = std::str::from_utf8(original).map_err(|_| eyre!("UTF-8 scope original required"))?;
    ensure!(
        !text.is_empty()
            && text.len() <= maximum
            && text.trim() == text
            && !text.chars().any(char::is_control),
        "bounded exact scope text required"
    );
    Ok(text)
}

pub(super) fn from_originals(
    audience: Vec<u8>,
    release: Vec<u8>,
    service_origin: Vec<u8>,
) -> Result<Scopes> {
    exact_text(&audience, 512)?;
    let origin = exact_text(&service_origin, 2048)?;
    let url = reqwest::Url::parse(origin).map_err(|_| eyre!("HTTPS scope origin required"))?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && url.origin().ascii_serialization() == origin
            && url.port() != Some(0),
        "canonical HTTPS scope origin required"
    );
    // Match the application loader's DNS grammar, with IP literals admitted by the URL parser.
    if let Some(host) = url.host_str() {
        if !host.starts_with('[') {
            ensure!(
                host.len() <= 253
                    && host.split('.').all(|label| {
                        !label.is_empty()
                            && label.len() <= 63
                            && !label.starts_with('-')
                            && !label.ends_with('-')
                            && label
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    }),
                "bounded scope origin host required"
            );
        }
    } else {
        return Err(eyre!("scope origin host required"));
    }
    use KagemushaEnrollmentPermitScopeRoleV1 as Role;
    let operator_digest = kagemusha_enrollment_permit_scope_digest_v1(Role::Fi, &audience)?;
    let release_digest = kagemusha_enrollment_permit_scope_digest_v1(Role::Release, &release)?;
    let service_origin_digest =
        kagemusha_enrollment_permit_scope_digest_v1(Role::ServiceOrigin, &service_origin)?;
    Ok(Scopes {
        audience,
        release,
        service_origin,
        operator_digest,
        release_digest,
        service_origin_digest,
    })
}

#[cfg(test)]
#[path = "scopes/tests.rs"]
mod tests;
