// These tests exercise actual broker boundaries with deterministic software keys and isolated
// socket pairs. Typed instruction ownership is not ledger eligibility or hardware qualification.
mod native_role_authorization_tests {
    use super::*;
    use iroha_torii::SorafsNativeTransactionSignerRoleV1 as Role;
    use std::io::Read as _;

    const ROLES: [Role; 4] = [
        Role::ProofOutcome,
        Role::Repair,
        Role::Reserve,
        Role::Orderbook,
    ];

    fn slot(role: Role) -> IrohaRuntimeProviderSlotV1 {
        match role {
            Role::ProofOutcome => IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            Role::Repair => IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
            Role::Reserve => IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
            Role::Orderbook => IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
        }
    }

    fn server_state(role: Role) -> (BrokerServerStateV1, Arc<ServerTestNativeSigner>) {
        let backend = Arc::new(ServerTestNativeSigner::exact(role));
        let catalog =
            IrohaRuntimeProviderBindingsV1::qualified_native_transaction_signers_for_test(
                "server-test-chain",
                [(slot(role), backend.binding())],
            )
            .with_network_id_for_test(network_id());
        let backends = RuntimeProviderBrokerBackendsV1::new();
        let backends = match role {
            Role::ProofOutcome => backends.with_proof_outcome_transaction_signer(backend.clone()),
            Role::Repair => backends.with_repair_transaction_signer(backend.clone()),
            Role::Reserve => backends.with_reserve_transaction_signer(backend.clone()),
            Role::Orderbook => backends.with_orderbook_transaction_signer(backend.clone()),
        };
        (
            prepare_test_server_state(&catalog, backends).expect("exact native role backend"),
            backend,
        )
    }

    fn rejected_role_payloads(role: Role, authority: &AccountId) -> Vec<TransactionPayload> {
        let exact = native_signer_test_payload(role, authority.clone());
        let mut payloads = ROLES
            .into_iter()
            .filter(|other| *other != role)
            .map(|other| native_signer_test_payload(other, authority.clone()))
            .collect::<Vec<_>>();
        let mut empty = exact.clone();
        empty.instructions = Executable::Instructions(Vec::new().into());
        payloads.push(empty);
        let mut log = exact.clone();
        log.instructions = Executable::Instructions(
            vec![
                iroha_data_model::isi::Log::new(
                    iroha_data_model::Level::INFO,
                    "unrelated native signer instruction".into(),
                )
                .into(),
            ]
            .into(),
        );
        payloads.push(log);
        let mut duplicate = exact;
        duplicate.instructions = Executable::Instructions(
            vec![
                native_signer_test_instruction(role),
                native_signer_test_instruction(role),
            ]
            .into(),
        );
        payloads.push(duplicate);
        payloads
    }

    fn rejected_network_payloads(role: Role, authority: &AccountId) -> [TransactionPayload; 2] {
        let mut foreign = native_signer_test_payload(role, authority.clone());
        foreign.domain =
            iroha_data_model::transaction::TransactionDomain::Network(test_network_id(0x16));
        let mut missing = native_signer_test_payload(role, authority.clone());
        missing.domain = iroha_data_model::transaction::TransactionDomain::Genesis;
        [foreign, missing]
    }

    #[test]
    fn native_role_payload_rejection_precedes_server_backend_io() {
        for role in ROLES {
            let (state, backend) = server_state(role);
            let binding = &state.catalog[0];
            let exact = native_transaction_signer_binding_from_wire(binding).unwrap();
            let probes = backend.probe_calls.load(Ordering::Relaxed);
            for payload in rejected_role_payloads(role, exact.authority()) {
                let request = make_operation_request(
                    TEST_SESSION_ID,
                    1,
                    binding.clone(),
                    state.observations[0].metadata_digest,
                    OPERATION_NATIVE_TRANSACTION_SIGN_V1,
                    encode_native_transaction_payload(&payload)
                        .expect("canonical unrelated payload"),
                )
                .unwrap();
                assert_eq!(
                    validate_operation_request(&request),
                    Err(BrokerError::Rejected)
                );
                assert_eq!(
                    validate_operation_request_for_session(
                        &request,
                        &state.chain_id,
                        &state.network_id
                    ),
                    Err(BrokerError::Rejected)
                );
                assert_eq!(
                    sign_native_transaction(&state, binding, payload),
                    Err(BrokerError::Rejected)
                );
                assert_eq!(backend.probe_calls.load(Ordering::Relaxed), probes);
                assert_eq!(backend.sign_calls.load(Ordering::Relaxed), 0);
            }
            for payload in rejected_network_payloads(role, exact.authority()) {
                assert_eq!(
                    sign_native_transaction(&state, binding, payload),
                    Err(BrokerError::Rejected)
                );
                assert_eq!(backend.probe_calls.load(Ordering::Relaxed), probes);
                assert_eq!(backend.sign_calls.load(Ordering::Relaxed), 0);
            }
            let payload = native_signer_test_payload(role, exact.authority().clone());
            let signed = sign_native_transaction(&state, binding, payload.clone())
                .expect("bad caller input does not disable the backend");
            assert_eq!(signed.payload(), &payload);
            signed.verify_signature().unwrap();
            assert_eq!(backend.sign_calls.load(Ordering::Relaxed), 1);
        }
    }

    fn client(role: Role) -> (NativeTransactionBrokerCore, UnixStream) {
        let catalog = native_signer_test_catalog();
        let configured = catalog
            .iter()
            .find(|binding| binding.slot() == slot(role))
            .unwrap();
        let binding = ProviderBindingWireV1::try_from_binding(configured).unwrap();
        let exact_binding = configured.native_signer_binding().unwrap().clone();
        let (stream, peer) = UnixStream::pair().expect("isolated native broker session");
        // A regressed probe/transport call fails promptly and leaves observable bytes on the peer.
        stream.set_nonblocking(true).unwrap();
        peer.set_nonblocking(true).unwrap();
        let session = Arc::new(BrokerSession {
            decode_pool: new_test_process_pool(),
            connection: Mutex::new(BrokerConnection {
                stream,
                session_id: TEST_SESSION_ID,
                next_request_id: 1,
                poison_reason: None,
            }),
            chain_id: catalog.chain_id().to_owned(),
            network_id: *catalog.network_id(),
            endpoint: EndpointPolicy::for_test(PathBuf::from("unused-native-role-broker.sock")),
            requested_catalog: vec![binding.clone()],
        });
        (
            NativeTransactionBrokerCore {
                session,
                metadata_digest: observation(&binding).metadata_digest,
                binding,
                exact_binding,
            },
            peer,
        )
    }

    fn assert_refused(
        role: Role,
        core: &NativeTransactionBrokerCore,
        payload: TransactionPayload,
        raw: bool,
    ) {
        macro_rules! reject {
            ($raw:ident, $proxy:ident, $trait_name:ident, $error:ident) => {
                if raw {
                    assert_eq!(
                        iroha_torii::$trait_name::sign(&$raw { core: core.clone() }, payload),
                        Err(iroha_torii::$error::Refused)
                    );
                } else {
                    assert_eq!(
                        iroha_torii::$trait_name::sign(&$proxy { core: core.clone() }, payload),
                        Err(iroha_torii::$error::Refused)
                    );
                }
            };
        }
        match role {
            Role::ProofOutcome => reject!(
                RawProofOutcomeBrokerSigner,
                ProofOutcomeBrokerSigner,
                SoraFsProofOutcomeTransactionSigner,
                SoraFsProofOutcomeSigningError
            ),
            Role::Repair => reject!(
                RawRepairBrokerSigner,
                RepairBrokerSigner,
                SoraFsRepairTransactionSigner,
                SoraFsRepairTransactionSigningError
            ),
            Role::Reserve => reject!(
                RawReserveBrokerSigner,
                ReserveBrokerSigner,
                SoraFsReserveTransactionSigner,
                SoraFsReserveTransactionSigningError
            ),
            Role::Orderbook => reject!(
                RawOrderbookBrokerSigner,
                OrderbookBrokerSigner,
                SoraFsOrderbookTransactionSigner,
                SoraFsOrderbookTransactionSigningError
            ),
        }
    }

    fn assert_no_transport(core: &NativeTransactionBrokerCore, peer: &mut UnixStream) {
        let connection = core.session.connection.lock().unwrap();
        assert_eq!(
            connection.next_request_id, 1,
            "no probe or signing request was admitted"
        );
        assert!(
            connection.poison_reason.is_none(),
            "caller input rejection must leave this session usable"
        );
        assert_eq!(
            peer.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock,
            "no bytes reached the broker transport"
        );
    }

    #[test]
    fn native_role_proxy_and_raw_reject_before_probes_or_transport() {
        for role in ROLES {
            for raw in [false, true] {
                let (core, mut peer) = client(role);
                for payload in rejected_role_payloads(role, core.exact_binding.authority()) {
                    assert_refused(role, &core, payload, raw);
                    assert_no_transport(&core, &mut peer);
                }
            }
        }
    }

    #[test]
    fn native_network_proxy_and_raw_reject_without_poisoning_or_transport() {
        for role in ROLES {
            for raw in [false, true] {
                let (core, mut peer) = client(role);
                for payload in rejected_network_payloads(role, core.exact_binding.authority()) {
                    assert_refused(role, &core, payload, raw);
                    assert_no_transport(&core, &mut peer);
                }
            }
        }
    }
}
