impl Client {
    fn request_without_operator_or_token_auth(
        &self,
        method: HttpMethod,
        url: Url,
    ) -> DefaultRequestBuilder {
        let headers = self.headers.iter().filter(|(name, _)| {
            ![
                "authorization",
                "x-api-token",
                "x-iroha-witness",
                HEADER_ACCOUNT,
                HEADER_SIGNATURE,
                HEADER_TIMESTAMP_MS,
                HEADER_NONCE,
                HEADER_OPERATOR_PUBLIC_KEY,
                HEADER_OPERATOR_TIMESTAMP_MS,
                HEADER_OPERATOR_NONCE,
                HEADER_OPERATOR_SIGNATURE,
            ]
            .iter()
            .any(|reserved| name.eq_ignore_ascii_case(reserved))
        });
        let mut builder = DefaultRequestBuilder::new(method, url).headers(headers);
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        builder
    }

    fn operator_signed_request(
        &self,
        method: HttpMethod,
        url: Url,
        body: Vec<u8>,
    ) -> Result<DefaultRequestBuilder> {
        let operator_key_pair = self
            .operator_key_pair
            .as_ref()
            .ok_or_else(|| eyre!("operator signing key is required before request dispatch"))?;
        let timestamp_ms: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let nonce = Self::signed_request_nonce()?;
        let message = Self::operator_network_request_message(
            &self.network_id,
            &method,
            &url,
            &body,
            timestamp_ms,
            nonce.as_str(),
        );
        let signature = Signature::try_new(operator_key_pair.private_key(), &message)
            .wrap_err("failed to sign operator request headers")?;
        let public_key = operator_key_pair.public_key().to_string();
        let timestamp = timestamp_ms.to_string();
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.payload());
        let builder = self
            .request_without_operator_or_token_auth(method, url)
            .header(HEADER_OPERATOR_PUBLIC_KEY, &public_key)
            .header(HEADER_OPERATOR_TIMESTAMP_MS, &timestamp)
            .header(HEADER_OPERATOR_NONCE, &nonce)
            .header(HEADER_OPERATOR_SIGNATURE, &signature_b64);

        if body.is_empty() {
            Ok(builder)
        } else {
            Ok(builder.body(body))
        }
    }
}
