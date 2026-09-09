//! Exact account-signature verification for the in-process stream fixtures.

use axum::http::{HeaderMap, Method, StatusCode, Uri};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha::client::{
    AccountClient, canonical_network_request_signature_message,
    canonical_request_account_header_value,
};
use iroha_crypto::Signature;
use iroha_data_model::{AccountId, NetworkId};
use parking_lot::Mutex;
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub(super) struct StreamAuthority {
    account: AccountId,
    network: NetworkId,
    nonces: Mutex<HashSet<String>>,
}
impl StreamAuthority {
    pub(super) fn new(reader: &AccountClient) -> Self {
        Self {
            account: reader.authority().clone(),
            network: *reader.network_id(),
            nonces: Mutex::default(),
        }
    }
    pub(super) fn accepted(&self) -> usize {
        self.nonces.lock().len()
    }
    pub(super) fn verify(&self, headers: &HeaderMap, uri: &Uri) -> Result<(), StatusCode> {
        let header = |name| {
            let mut values = headers.get_all(name).iter();
            let value = values
                .next()
                .and_then(|v| v.to_str().ok())
                .ok_or(StatusCode::UNAUTHORIZED)?;
            if values.next().is_some() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(value)
        };
        let account = canonical_request_account_header_value(&self.account)
            .map_err(|_| StatusCode::FORBIDDEN)?;
        if header("x-iroha-account")? != account {
            return Err(StatusCode::FORBIDDEN);
        }
        let key = self.account.try_signatory().ok_or(StatusCode::FORBIDDEN)?;
        let timestamp = header("x-iroha-timestamp-ms")?
            .parse::<u64>()
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| StatusCode::UNAUTHORIZED)?
            .as_millis();
        if now.abs_diff(u128::from(timestamp)) > 60_000 {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let nonce = header("x-iroha-nonce")?;
        let signature = STANDARD
            .decode(header("x-iroha-signature")?)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let url = format!("http://localhost{uri}")
            .parse()
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let message = canonical_network_request_signature_message(
            &self.network,
            &Method::GET,
            &url,
            &[],
            timestamp,
            nonce,
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
        Signature::from_bytes(&signature)
            .verify(key, &message)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let mut nonces = self.nonces.lock();
        if nonces.len() >= 4096 || !nonces.insert(nonce.to_owned()) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::client::canonical_request_signature_header_value;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    #[test]
    fn signed_upgrade_binds_network_route_authority_freshness_and_nonce() {
        let key =
            KeyPair::try_from_seed(b"mochi-stream-auth-test".to_vec(), Algorithm::Ed25519).unwrap();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"genesis")));
        let account = AccountId::new(key.public_key().clone());
        let authority = StreamAuthority {
            account: account.clone(),
            network,
            nonces: Mutex::default(),
        };
        let timestamp: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap();
        let uri: Uri = "/v1/blocks/stream".parse().unwrap();
        let signed = |timestamp, nonce: &str, network| {
            let mut headers = HeaderMap::new();
            let message = canonical_network_request_signature_message(
                &network,
                &Method::GET,
                &"http://localhost/v1/blocks/stream".parse().unwrap(),
                &[],
                timestamp,
                nonce,
            )
            .unwrap();
            let signature = Signature::try_new(key.private_key(), &message).unwrap();
            headers.insert(
                "x-iroha-account",
                canonical_request_account_header_value(&account)
                    .unwrap()
                    .parse()
                    .unwrap(),
            );
            headers.insert(
                "x-iroha-timestamp-ms",
                timestamp.to_string().parse().unwrap(),
            );
            headers.insert("x-iroha-nonce", nonce.parse().unwrap());
            headers.insert(
                "x-iroha-signature",
                canonical_request_signature_header_value(&signature)
                    .unwrap()
                    .parse()
                    .unwrap(),
            );
            headers
        };
        let headers = signed(timestamp, "AAAAAAAAAAAAAAAA", network);
        assert_eq!(authority.verify(&headers, &uri), Ok(()));
        assert_eq!(authority.accepted(), 1);
        assert_eq!(
            authority.verify(&headers, &uri),
            Err(StatusCode::UNAUTHORIZED)
        );
        let headers = signed(timestamp, "BBBBBBBBBBBBBBBB", network);
        assert_eq!(
            authority.verify(&headers, &"/v1/events/ws".parse().unwrap()),
            Err(StatusCode::UNAUTHORIZED)
        );
        let wrong_network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
        assert_eq!(
            authority.verify(&signed(timestamp, "CCCCCCCCCCCCCCCC", wrong_network), &uri),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            authority.verify(
                &signed(timestamp - 120_000, "DDDDDDDDDDDDDDDD", network),
                &uri
            ),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut duplicate = signed(timestamp, "EEEEEEEEEEEEEEEE", network);
        duplicate.append("x-iroha-nonce", "EEEEEEEEEEEEEEEE".parse().unwrap());
        assert_eq!(
            authority.verify(&duplicate, &uri),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut tampered = signed(timestamp, "FFFFFFFFFFFFFFFF", network);
        tampered.insert(
            "x-iroha-signature",
            STANDARD.encode([0u8; 64]).parse().unwrap(),
        );
        assert_eq!(
            authority.verify(&tampered, &uri),
            Err(StatusCode::UNAUTHORIZED)
        );
        let mut wrong_account = signed(timestamp, "GGGGGGGGGGGGGGGG", network);
        wrong_account.insert("x-iroha-account", "invalid".parse().unwrap());
        assert_eq!(
            authority.verify(&wrong_account, &uri),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            authority.accepted(),
            1,
            "rejected inputs must not reserve replay nonces"
        );
        assert_eq!(authority.verify(&headers, &uri), Ok(()));
        assert_eq!(authority.accepted(), 2);
    }
}
