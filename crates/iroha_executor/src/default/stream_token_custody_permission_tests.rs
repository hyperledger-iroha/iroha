#[test]
fn stream_token_custody_visitor_requires_exact_provider_even_at_genesis() {
    use iroha_data_model::{
        isi::sorafs::MutateSorafsStreamTokenCustody,
        sorafs::stream_token_custody::{
            SorafsStreamTokenCustodyActionV1, SorafsStreamTokenCustodyRevocationV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;
    let instruction = MutateSorafsStreamTokenCustody {
        provider_id: sample_provider_id(),
        expected_revision: 1,
        expected_digest: [7; 32],
        action: SorafsStreamTokenCustodyActionV1::Revoke(SorafsStreamTokenCustodyRevocationV1 {
            signer: true,
            attester: false,
        }),
    };
    let visit = sorafs::visit_mutate_stream_token_custody;
    assert_denied_without_permission(instruction.clone(), visit);
    assert_denied_with_permission(instruction.clone(), CanSetSorafsPricing.into(), visit);
    assert_denied_with_permission(
        instruction.clone(),
        CanManageSorafsStreamTokenCustody {
            provider_id: ProviderId::new([99; 32]),
        }
        .into(),
        visit,
    );
    assert_allowed_with_permission(
        instruction.clone(),
        CanManageSorafsStreamTokenCustody {
            provider_id: instruction.provider_id,
        }
        .into(),
        visit,
    );
    with_mock_permissions(Vec::new(), || {
        let mut executor = MockExecutor::new(true);
        visit(&mut executor, &instruction);
        assert!(
            executor.verdict().is_err(),
            "genesis cannot infer custody authority"
        );
    });
    with_mock_permissions(
        vec![
            CanManageSorafsStreamTokenCustody {
                provider_id: instruction.provider_id,
            }
            .into(),
        ],
        || {
            let mut executor = MockExecutor::new(true);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "an explicitly seeded exact token remains usable"
            );
        },
    );
}

#[test]
fn stream_token_custody_box_dispatch_preserves_permission_gate() {
    use iroha_data_model::{
        isi::sorafs::MutateSorafsStreamTokenCustody,
        sorafs::stream_token_custody::{
            SorafsStreamTokenCustodyActionV1, SorafsStreamTokenCustodyRevocationV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;
    let instruction: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: sample_provider_id(),
        expected_revision: 1,
        expected_digest: [7; 32],
        action: SorafsStreamTokenCustodyActionV1::Revoke(SorafsStreamTokenCustodyRevocationV1 {
            signer: false,
            attester: true,
        }),
    }
    .into();
    assert_denied_without_permission(instruction.clone(), super::visit_instruction);
    assert_allowed_with_permission(
        instruction,
        CanManageSorafsStreamTokenCustody {
            provider_id: sample_provider_id(),
        }
        .into(),
        super::visit_instruction,
    );
}
