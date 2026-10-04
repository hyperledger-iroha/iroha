//! Real SDK HTTP traversal of public effective-permission pages and fail-closed response checks.
use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Instant,
};

fn complete_headers() -> String {
    [
        "Content-Type: application/json",
        "x-iroha-account-permission-semantics: effective-v1",
    ]
    .join("\r\n")
}

fn page(items: &[Permission], has_more: bool) -> Result<Vec<u8>> {
    norito::json::to_vec(&iroha::collections::Page {
        items: items.to_vec(),
        total: None,
        next_cursor: has_more.then(|| "next-permissions".to_owned()),
    })
    .map_err(Into::into)
}

fn read_scripted_permissions(
    responses: Vec<(String, Vec<u8>)>,
) -> Result<(Result<DeploymentAuthorization>, Vec<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let server = thread::spawn(move || -> Result<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut requests = Vec::new();
        for (headers, body) in responses {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "permission HTTP fixture timed out accepting request"
                            ));
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            // Accepted sockets may inherit the listener's nonblocking mode on macOS.
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte)?;
                request.push(byte[0]);
                if request.len() > 64 * 1024 {
                    return Err(eyre!("permission request headers exceeded fixture bound"));
                }
            }
            let headers_text = String::from_utf8(request.clone())?;
            let length = headers_text
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body_bytes = vec![0; length];
            stream.read_exact(&mut body_bytes)?;
            request.extend(body_bytes);
            requests.push(String::from_utf8(request)?);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\n{headers}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            stream.write_all(&body)?;
        }
        Ok(requests)
    });
    let (mut config, record) = crate::service_tests::fixture()?;
    config.torii_api_url = format!("http://{address}/").parse()?;
    let service = DeploymentService::new(config)?;
    let result = read_authorization(
        &service.client,
        &service.config.account,
        &record.preflight.contract_alias,
        record.preflight.dataspace_id,
    );
    let requests = server
        .join()
        .map_err(|_| eyre!("permission fixture panicked"))??;
    Ok((result, requests))
}

#[test]
fn public_permission_pages_follow_explicit_exhaustion_without_an_empty_probe() -> Result<()> {
    let first = (0..500)
        .map(|index| Permission::new(format!("UnrelatedPermission{index}"), Json::new(())))
        .collect::<Vec<_>>();
    let manage: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    // Duplicate tokens across pages do not change the effective permission set.
    let second = [first[0].clone(), manage.clone()];
    let (result, requests) = read_scripted_permissions(vec![
        (complete_headers(), page(&first, true)?),
        (complete_headers(), page(&second, false)?),
    ])?;
    let authorization = result?;
    assert_eq!(authorization.manage_alias_permission, manage);
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        let first_line = request.lines().next().expect("request line");
        assert!(first_line.starts_with("POST /v1/accounts/"));
        assert!(first_line.ends_with("/permissions/query HTTP/1.1"));
        let query = iroha::collections::ListQuery::from_json_value(norito::json::from_str(
            request.split_once("\r\n\r\n").unwrap().1,
        )?)?;
        assert_eq!(query.limit, Some(500));
        assert_eq!(
            query.cursor.as_deref(),
            (index > 0).then_some("next-permissions")
        );
        let headers = request.to_ascii_lowercase();
        assert!(headers.contains("\r\nx-iroha-account:"));
        assert!(headers.contains("\r\nx-iroha-signature:"));
        assert!(headers.contains("\r\nx-iroha-timestamp-ms:"));
        assert!(headers.contains("\r\nx-iroha-nonce:"));
    }
    Ok(())
}

#[test]
fn public_permission_pages_reject_incomplete_or_misrepresented_evidence() -> Result<()> {
    let headers = complete_headers();
    for (headers, body) in [
        (
            headers.replace("effective-v1", "direct-only"),
            page(&[], false)?,
        ),
        (
            format!("{headers}\r\nx-iroha-account-permission-semantics: effective-v1"),
            page(&[], false)?,
        ),
        (
            headers.clone(),
            br#"{"items": [], "next_cursor": null, "has_more": false}"#.to_vec(),
        ),
        (headers.clone(), br#"{"total": 0, "items": []}"#.to_vec()),
        (
            headers.clone(),
            br#"{"items": [], "next_cursor": 7}"#.to_vec(),
        ),
        (headers.clone(), page(&[], true)?),
    ] {
        let (result, requests) = read_scripted_permissions(vec![(headers, body)])?;
        assert!(result.is_err());
        assert_eq!(requests.len(), 1);
    }
    // A genuinely empty complete response is an actionable missing grant, not authorization.
    let (result, requests) = read_scripted_permissions(vec![(headers, page(&[], false)?)])?;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("CanManageAccountAlias")
    );
    assert_eq!(requests.len(), 1);
    Ok(())
}

#[test]
fn complete_first_page_never_exceeds_the_default_fetch_budget() -> Result<()> {
    let manage: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    for count in [14, 500] {
        let mut items = (1..count)
            .map(|index| Permission::new(format!("UnrelatedPermission{index}"), Json::new(())))
            .collect::<Vec<_>>();
        items.push(manage.clone());
        let (result, requests) =
            read_scripted_permissions(vec![(complete_headers(), page(&items, false)?)])?;
        assert_eq!(result?.manage_alias_permission, manage);
        assert_eq!(
            requests.len(),
            1,
            "an absent cursor must stop without an extra probe"
        );
    }
    Ok(())
}

#[test]
fn short_collection_page_cannot_override_explicit_cursor_continuation() -> Result<()> {
    let manage: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    let (result, requests) = read_scripted_permissions(vec![
        (
            complete_headers(),
            page(std::slice::from_ref(&manage), true)?,
        ),
        (complete_headers(), page(&[], false)?),
    ])?;
    assert_eq!(result?.manage_alias_permission, manage);
    assert_eq!(requests.len(), 2);
    Ok(())
}

#[test]
fn permission_policy_read_rejects_repeated_continuation_cursors() -> Result<()> {
    let token = Permission::new("UnrelatedPermission".to_owned(), Json::new(()));
    let (result, requests) = read_scripted_permissions(vec![
        (
            complete_headers(),
            page(std::slice::from_ref(&token), true)?,
        ),
        (
            complete_headers(),
            page(std::slice::from_ref(&token), true)?,
        ),
    ])?;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cursor did not advance")
    );
    assert_eq!(requests.len(), 2);
    Ok(())
}
