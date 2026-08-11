#[test]
fn connect_open_rejects_identity_substitution_and_replayed_sequence() {
    let network_hash = Hash::new(b"connect-open-exact-network");
    let network_id = network_id_from_raw_bytes(network_hash.as_ref()).expect("network id");
    let app_pk = [0x41u8; 32];
    let nonce = [0x51u8; 16];
    let sid = connect_sdk::derive_session_id(&network_id, &app_pk, &nonce);
    let mut out_ptr: *mut c_uchar = ptr::null_mut();
    let mut out_len: c_ulong = 0;
    let mut call = |candidate_sid: &[u8; 32],
                    candidate_network: &[u8],
                    candidate_app_pk: &[u8; 32],
                    candidate_nonce: &[u8; 16],
                    sequence: u64| unsafe {
        connect_norito_encode_control_open_ext(
            candidate_sid.as_ptr(),
            0,
            sequence,
            candidate_app_pk.as_ptr(),
            candidate_app_pk.len() as c_ulong,
            candidate_nonce.as_ptr(),
            candidate_nonce.len() as c_ulong,
            ptr::null(),
            0,
            candidate_network.as_ptr(),
            candidate_network.len() as c_ulong,
            ptr::null(),
            0,
            &mut out_ptr,
            &mut out_len,
        )
    };
    let mut wrong_sid = sid;
    wrong_sid[0] ^= 1;
    assert_eq!(
        call(&wrong_sid, network_hash.as_ref(), &app_pk, &nonce, 1),
        ERR_CONNECT_IDENTITY
    );
    let substituted_network = Hash::new(b"connect-open-other-network");
    assert_eq!(
        call(&sid, substituted_network.as_ref(), &app_pk, &nonce, 1),
        ERR_CONNECT_IDENTITY
    );
    let mut substituted_app = app_pk;
    substituted_app[0] ^= 1;
    assert_eq!(
        call(&sid, network_hash.as_ref(), &substituted_app, &nonce, 1),
        ERR_CONNECT_IDENTITY
    );
    let mut substituted_nonce = nonce;
    substituted_nonce[0] ^= 1;
    assert_eq!(
        call(&sid, network_hash.as_ref(), &app_pk, &substituted_nonce, 1),
        ERR_CONNECT_IDENTITY
    );
    assert_eq!(call(&sid, network_hash.as_ref(), &app_pk, &nonce, 2), -2);
    assert_eq!(
        call(&sid, &network_hash.as_ref()[..31], &app_pk, &nonce, 1),
        -2
    );
    drop(call);
    let invalid_metadata = br#"{"name":" demo"}"#;
    assert_eq!(
        unsafe {
            connect_norito_encode_control_open_ext(
                sid.as_ptr(),
                0,
                1,
                app_pk.as_ptr(),
                app_pk.len() as c_ulong,
                nonce.as_ptr(),
                nonce.len() as c_ulong,
                invalid_metadata.as_ptr(),
                invalid_metadata.len() as c_ulong,
                network_hash.as_ref().as_ptr(),
                Hash::LENGTH as c_ulong,
                ptr::null(),
                0,
                &mut out_ptr,
                &mut out_len,
            )
        },
        -4
    );
    assert_eq!(
        unsafe {
            connect_norito_encode_control_open_ext(
                sid.as_ptr(),
                0,
                1,
                app_pk.as_ptr(),
                app_pk.len() as c_ulong,
                nonce.as_ptr(),
                nonce.len() as c_ulong,
                ptr::null(),
                0,
                network_hash.as_ref().as_ptr(),
                Hash::LENGTH as c_ulong,
                ptr::null(),
                1,
                &mut out_ptr,
                &mut out_len,
            )
        },
        -1
    );
    assert!(out_ptr.is_null());
    assert_eq!(out_len, 0);
}

#[test]
fn connect_approval_verifier_binds_exact_identity_account_and_relay() {
    let network_hash = Hash::new(b"connect-approval-exact-network");
    let network_id = network_id_from_raw_bytes(network_hash.as_ref()).expect("network id");
    let app_pk = [0x61u8; 32];
    let nonce = [0x62u8; 16];
    let sid = connect_sdk::derive_session_id(&network_id, &app_pk, &nonce);
    let wallet_pk = [0x63u8; 32];
    let keypair =
        KeyPair::try_from_seed([0x64u8; 32], Algorithm::Ed25519).expect("approval signer");
    let account = CString::new(AccountId::new(keypair.public_key().clone()).to_string())
        .expect("account c string");
    let relay_token = b"exact-relay-token";
    let mut preimage_ptr: *mut c_uchar = ptr::null_mut();
    let mut preimage_len: c_ulong = 0;
    let preimage_status = unsafe {
        connect_norito_connect_approval_preimage(
            network_hash.as_ref().as_ptr(),
            Hash::LENGTH as c_ulong,
            sid.as_ptr(),
            sid.len() as c_ulong,
            app_pk.as_ptr(),
            app_pk.len() as c_ulong,
            nonce.as_ptr(),
            nonce.len() as c_ulong,
            wallet_pk.as_ptr(),
            wallet_pk.len() as c_ulong,
            account.as_ptr(),
            account.as_bytes().len() as c_ulong,
            ptr::null(),
            0,
            ptr::null(),
            0,
            relay_token.as_ptr() as *const c_char,
            relay_token.len() as c_ulong,
            &mut preimage_ptr,
            &mut preimage_len,
        )
    };
    assert_eq!(preimage_status, 0);
    let preimage = unsafe { slice::from_raw_parts(preimage_ptr, preimage_len as usize) };
    let signature = Signature::new(keypair.private_key(), preimage);
    let algorithm = b"ed25519";
    let verify = |candidate_network: &[u8],
                  candidate_sid: &[u8; 32],
                  candidate_app: &[u8; 32],
                  candidate_nonce: &[u8; 16],
                  candidate_wallet: &[u8; 32],
                  candidate_account: &CString,
                  permissions: Option<&[u8]>,
                  relay: &[u8]| unsafe {
        connect_norito_connect_verify_approval(
            candidate_network.as_ptr(),
            candidate_network.len() as c_ulong,
            candidate_sid.as_ptr(),
            candidate_sid.len() as c_ulong,
            candidate_app.as_ptr(),
            candidate_app.len() as c_ulong,
            candidate_nonce.as_ptr(),
            candidate_nonce.len() as c_ulong,
            candidate_wallet.as_ptr(),
            candidate_wallet.len() as c_ulong,
            candidate_account.as_ptr(),
            candidate_account.as_bytes().len() as c_ulong,
            permissions.map_or(ptr::null(), |value| value.as_ptr()),
            permissions.map_or(0, |value| value.len() as c_ulong),
            ptr::null(),
            0,
            relay.as_ptr() as *const c_char,
            relay.len() as c_ulong,
            algorithm.as_ptr() as *const c_char,
            algorithm.len() as c_ulong,
            signature.payload().as_ptr(),
            signature.payload().len() as c_ulong,
        )
    };
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &wallet_pk,
            &account,
            None,
            relay_token,
        ),
        0
    );
    let mut substituted_nonce = nonce;
    substituted_nonce[0] ^= 1;
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &substituted_nonce,
            &wallet_pk,
            &account,
            None,
            relay_token,
        ),
        ERR_CONNECT_IDENTITY
    );
    let wrong_network = Hash::new(b"connect-approval-wrong-network");
    assert_eq!(
        verify(
            wrong_network.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &wallet_pk,
            &account,
            None,
            relay_token,
        ),
        ERR_CONNECT_IDENTITY
    );
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &wallet_pk,
            &account,
            None,
            b"substituted-relay-token",
        ),
        ERR_CONNECT_APPROVAL
    );
    let substituted_app = [0x71u8; 32];
    let substituted_sid = connect_sdk::derive_session_id(&network_id, &substituted_app, &nonce);
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &substituted_sid,
            &substituted_app,
            &nonce,
            &wallet_pk,
            &account,
            None,
            relay_token,
        ),
        ERR_CONNECT_APPROVAL
    );
    let substituted_wallet = [0x72u8; 32];
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &substituted_wallet,
            &account,
            None,
            relay_token,
        ),
        ERR_CONNECT_APPROVAL
    );
    let other_keypair = KeyPair::try_from_seed([0x73u8; 32], Algorithm::Ed25519)
        .expect("alternate approval signer");
    let substituted_account =
        CString::new(AccountId::new(other_keypair.public_key().clone()).to_string())
            .expect("alternate account c string");
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &wallet_pk,
            &substituted_account,
            None,
            relay_token,
        ),
        ERR_CONNECT_APPROVAL
    );
    let substituted_permissions = br#"{"methods":["SIGN_REQUEST_TX"],"events":[]}"#;
    assert_eq!(
        verify(
            network_hash.as_ref(),
            &sid,
            &app_pk,
            &nonce,
            &wallet_pk,
            &account,
            Some(substituted_permissions),
            relay_token,
        ),
        ERR_CONNECT_APPROVAL
    );
    unsafe { free(preimage_ptr as *mut _) };
}
