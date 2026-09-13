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
        "x-iroha-fanout-routes-attempted: 2",
        "x-iroha-fanout-routes-succeeded: 2",
        "x-iroha-fanout-routes-failed: 0",
        "x-iroha-fanout-routes-denied: 0",
        "x-iroha-fanout-routes-unavailable: 0",
        "x-iroha-fanout-routes-not-found: 0",
    ]
    .join("\r\n")
}

fn page(items: &[Permission]) -> Result<Vec<u8>> {
    norito::json::to_vec(&norito::json!({ "total": (items.len()), "items": (items.to_vec()) }))
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
fn public_permission_pages_traverse_more_than_500_items_and_empty_final_page() -> Result<()> {
    let first = (0..500)
        .map(|index| Permission::new(format!("UnrelatedPermission{index}"), Json::new(())))
        .collect::<Vec<_>>();
    let register: Permission = CanRegisterSmartContractCode.into();
    let manage: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    }
    .into();
    // The public merger reports the returned page count, even after route deduplication.
    let second = [first[0].clone(), register.clone(), manage.clone()];
    let (result, requests) = read_scripted_permissions(vec![
        (complete_headers(), page(&first)?),
        (complete_headers(), page(&second)?),
        (complete_headers(), page(&[])?),
    ])?;
    let authorization = result?;
    assert_eq!(authorization.register_code_permission, register);
    assert_eq!(authorization.manage_alias_permission, manage);
    assert_eq!(requests.len(), 3);
    for (request, offset) in requests.iter().zip([0, 500, 1000]) {
        let first_line = request.lines().next().expect("request line");
        assert!(first_line.starts_with("GET /v1/accounts/"));
        assert!(first_line.ends_with(&format!(
            "/permissions?limit=500&offset={offset}&count_mode=exact HTTP/1.1"
        )));
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
        (headers.replace("effective-v1", "direct-only"), page(&[])?),
        (
            headers.replace("routes-succeeded: 2", "routes-succeeded: 1"),
            page(&[])?,
        ),
        (
            headers.replace("routes-unavailable: 0", "routes-unavailable: 1"),
            page(&[])?,
        ),
        (
            format!("{headers}\r\nx-iroha-account-permission-semantics: effective-v1"),
            page(&[])?,
        ),
        (headers.clone(), br#"{"total": 501, "items": []}"#.to_vec()),
    ] {
        let (result, requests) = read_scripted_permissions(vec![(headers, body)])?;
        assert!(result.is_err());
        assert_eq!(requests.len(), 1);
    }
    // A genuinely empty complete response is an actionable missing grant, not authorization.
    let (result, requests) = read_scripted_permissions(vec![(headers, page(&[])?)])?;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("CanRegisterSmartContractCode")
    );
    assert_eq!(requests.len(), 1);
    Ok(())
}
