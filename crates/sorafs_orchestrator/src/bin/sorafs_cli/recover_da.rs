//! Recover a cold DA package through independent gateways using a certified manifest digest.

use super::{
    HttpClient, RedirectPolicy, Url, deploy_publication, gateway_url_for_file, parse_digest_hex,
};
use iroha_data_model::da::manifest::DaManifestV1;
use iroha_fs::{OwnerDirectory, PublishMode};
use std::{
    io::{self, Read as _},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const RECOVERY_TIMEOUT: Duration = Duration::from_secs(600);

struct Gateways {
    client: HttpClient,
    bases: Vec<String>,
    cid: String,
    deadline: Instant,
}

impl Gateways {
    fn verified_file(
        &mut self,
        name: String,
        limit: usize,
        hash: &[u8],
    ) -> io::Result<Option<Vec<u8>>> {
        for index in 0..self.bases.len() {
            let budget = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "DA recovery deadline elapsed")
                })?;
            let url =
                gateway_url_for_file(&self.bases[index], &self.cid, std::slice::from_ref(&name))
                    .map_err(io::Error::other)?;
            let Ok(response) = self
                .client
                .get(url)
                .timeout(budget.min(Duration::from_secs(15)))
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .send()
            else {
                continue;
            };
            if response.status() != reqwest::StatusCode::OK
                || response.content_length().is_some_and(|n| n > limit as u64)
            {
                continue;
            }
            let mut bytes = Vec::new();
            if response
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .is_ok()
                && bytes.len() <= limit
                && blake3::hash(&bytes).as_bytes() == hash
            {
                self.bases.rotate_left(index);
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }
}

pub(super) fn run(args: Vec<String>) -> Result<(), String> {
    let mut bases = Vec::new();
    let mut cid = None;
    let mut manifest_hash = None;
    let mut output = None;
    for arg in &args {
        let (key, value) = arg.split_once('=').ok_or("expected --option=value")?;
        match key {
            "--gateway" if bases.len() < 16 => {
                let url = Url::parse(value).map_err(|e| e.to_string())?;
                deploy_publication::endpoint(&url, "sorafs/cid/")?;
                bases.push(url.to_string());
            }
            "--cid"
                if cid.is_none()
                    && value.len() <= 256
                    && value.strip_prefix('b').is_some_and(|s| {
                        !s.is_empty()
                            && s.bytes()
                                .all(|c| c.is_ascii_lowercase() || (b'2'..=b'7').contains(&c))
                    }) =>
            {
                cid = Some(value.to_owned())
            }
            "--manifest-hash" if manifest_hash.is_none() => {
                manifest_hash = Some(parse_digest_hex(value)?)
            }
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            _ => return Err(format!("invalid or repeated DA recovery option {key}")),
        }
    }
    if bases.is_empty() {
        return Err("DA recovery requires --gateway=URL".into());
    }
    let mut gateways = Gateways {
        client: HttpClient::builder()
            .redirect(RedirectPolicy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| e.to_string())?,
        bases,
        cid: cid.ok_or("DA recovery requires --cid=BASE32")?,
        deadline: Instant::now() + RECOVERY_TIMEOUT,
    };
    let hash = manifest_hash
        .ok_or("DA recovery requires --manifest-hash=HEX from the certified DA commitment")?;
    let output = output.ok_or("DA recovery requires --output=PATH")?;
    let file = output.file_name().ok_or("output requires a filename")?;
    let bytes = gateways
        .verified_file(
            "manifest.norito".into(),
            DaManifestV1::MAX_ENCODED_BYTES,
            &hash,
        )
        .map_err(|e| e.to_string())?
        .ok_or("no gateway supplied the certified DA manifest")?;
    let manifest = DaManifestV1::decode_canonical(&bytes).map_err(|e| e.to_string())?;
    let payload = sorafs_car::da_recovery::recover_payload(&manifest, |chunk| {
        gateways.verified_file(
            format!("chunk_{:05}.bin", chunk.index),
            chunk.length as usize,
            chunk.commitment.as_ref(),
        )
    })
    .map_err(|e| e.to_string())?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    OwnerDirectory::open_or_create(parent)
        .and_then(|dir| dir.write_atomic(file, &payload, PublishMode::Replace))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verified_read_uses_the_second_gateway_after_a_digest_mismatch() {
        use std::{
            io::{BufRead as _, BufReader, Write as _},
            net::TcpListener,
            thread,
        };
        let serve = |body: &'static [u8]| {
            let socket = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}/", socket.local_addr().unwrap());
            let worker = thread::spawn(move || {
                let (mut stream, _) = socket.accept().unwrap();
                let mut headers = BufReader::new(&stream).lines();
                assert_eq!(
                    headers.next().unwrap().unwrap(),
                    "GET /sorafs/cid/babc/chunk_00000.bin HTTP/1.1"
                );
                for header in headers {
                    if header.unwrap().is_empty() {
                        break;
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            });
            (base, worker)
        };
        let (first, first_worker) = serve(b"bad");
        let (second, second_worker) = serve(b"new");
        let mut sources = Gateways {
            client: HttpClient::builder().no_proxy().build().unwrap(),
            bases: vec![first, second.clone()],
            cid: "babc".into(),
            deadline: Instant::now() + Duration::from_secs(10),
        };
        assert_eq!(
            sources
                .verified_file("chunk_00000.bin".into(), 3, blake3::hash(b"new").as_bytes())
                .unwrap()
                .as_deref(),
            Some(&b"new"[..])
        );
        assert_eq!(sources.bases[0], second);
        first_worker.join().unwrap();
        second_worker.join().unwrap();
    }
    #[test]
    fn requires_a_trusted_digest_and_safe_gateway_before_reading() {
        let args = |url: &str| {
            [
                "--cid=babc".into(),
                "--output=out".into(),
                format!("--gateway={url}"),
            ]
            .to_vec()
        };
        assert!(
            run(args("https://example.invalid"))
                .unwrap_err()
                .contains("manifest-hash")
        );
        assert!(
            run(args("http://example.invalid"))
                .unwrap_err()
                .contains("HTTPS")
        );
    }
}
