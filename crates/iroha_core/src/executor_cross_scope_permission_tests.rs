/// Cross-scope delegation negatives carried over from the retired Rust executor SDK suite.
mod cross_scope_permission_tests {
    use super::*;
    use iroha_data_model::nexus::UniversalAccountId;

    #[test]
    fn scoped_delegation_never_widens_to_a_sibling_scope() {
        let sponsor = checked_account_id();
        let other_sponsor = checked_account_id();
        let holder = checked_account_id();
        let subject = checked_account_id();
        let dataspace = DataSpaceId::new(10);
        let hbl = DomainId::try_new("hbl", "sbp").expect("HBL domain");
        let ubl = DomainId::try_new("ubl", "sbp").expect("UBL domain");
        let uaid = |seed: &[u8]| UniversalAccountId::from_hash(Hash::new(seed));
        let program = |sponsor: &AccountId| {
            FeeSponsorProgramId::new(sponsor.clone(), "retail".parse().expect("program name"))
        };
        let held: BTreeSet<Permission> = BTreeSet::from([
            executor_permission::nexus::CanManageFeeSponsorProgram {
                sponsor: sponsor.clone(),
            }
            .into(),
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForUaid {
                dataspace,
                uaid: uaid(b"uaid::hbl-customer"),
            }
            .into(),
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace,
                domain: hbl.clone(),
            }
            .into(),
            executor_permission::query::CanReadAccountData {
                account: subject.clone(),
            }
            .into(),
        ]);
        let mut world = World::with(
            [],
            [
                Account::new(sponsor.clone()).build(&sponsor),
                Account::new(other_sponsor.clone()).build(&other_sponsor),
                Account::new(holder.clone()).build(&holder),
                Account::new(subject.clone()).build(&subject),
            ],
            [],
        );
        world.account_permissions.insert(holder.clone(), held);
        let state = state_for_testing(world);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
        let state_transaction = block.transaction();
        let allowed = |authority: &AccountId, permission: Permission| {
            initial_permission_delegation_allowed(&state_transaction, authority, &permission, None)
                .expect("delegation decision")
        };

        // Fee-sponsor enrollment follows the exact sponsor of the held manager token.
        assert!(allowed(
            &holder,
            executor_permission::nexus::CanEnrollFeeSponsorProgram {
                program_id: program(&sponsor),
            }
            .into(),
        ));
        assert!(!allowed(
            &holder,
            executor_permission::nexus::CanEnrollFeeSponsorProgram {
                program_id: program(&other_sponsor),
            }
            .into(),
        ));
        // Space-directory leaves propagate only their exact UAID or account domain.
        assert!(!allowed(
            &holder,
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForUaid {
                dataspace,
                uaid: uaid(b"uaid::ubl-customer"),
            }
            .into(),
        ));
        assert!(!allowed(
            &holder,
            executor_permission::nexus::CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace,
                domain: ubl,
            }
            .into(),
        ));
        // An exact account reader may use, but never propagate, the subject's grant.
        let read: Permission = executor_permission::query::CanReadAccountData {
            account: subject.clone(),
        }
        .into();
        assert!(!allowed(&holder, read.clone()));
        assert!(allowed(&subject, read));
    }
}
