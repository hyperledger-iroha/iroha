//! `World`-related ISI implementations.

use super::prelude::*;
use crate::prelude::*;

/// Iroha Special Instructions that have `World` as their target.
pub mod isi {
    use eyre::Result;
    use iroha_data_model::prelude::*;
    use iroha_telemetry::metrics;

    use super::*;

    impl<W: WorldTrait> Execute<W> for Register<Peer> {
        type Error = Error;

        #[metrics(+"register_peer")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let peer_id = self.object.id;

            wsv.modify_world(|world| {
                if !world.trusted_peers_ids.insert(peer_id.clone()) {
                    return Err(Error::Repetition(
                        InstructionType::Register,
                        IdBox::PeerId(peer_id),
                    ));
                }
                Ok(PeerEvent::Added(peer_id).into())
            })
        }
    }

    impl<W: WorldTrait> Execute<W> for Unregister<Peer> {
        type Error = Error;

        #[metrics(+"unregister_peer")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let peer_id = self.object_id;
            wsv.modify_world(|world| {
                if world.trusted_peers_ids.remove(&peer_id).is_none() {
                    return Err(FindError::Peer(peer_id).into());
                }
                Ok(PeerEvent::Removed(peer_id).into())
            })
        }
    }

    impl<W: WorldTrait> Execute<W> for Register<Domain> {
        type Error = Error;

        #[metrics("register_domain")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let domain = self.object;
            let domain_id = domain.id.clone();
            domain_id
                .name
                .validate_len(wsv.config.ident_length_limits)
                .map_err(Error::Validate)?;

            wsv.modify_world(|world| {
                world.domains.insert(domain_id.clone(), domain);
                Ok(DomainEvent::Created(domain_id).into())
            })?;

            wsv.metrics.domains.inc();
            Ok(())
        }
    }

    impl<W: WorldTrait> Execute<W> for Unregister<Domain> {
        type Error = Error;

        #[metrics("unregister_domain")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let domain_id = self.object_id;
            wsv.modify_world(|world| {
                world.domains.remove(&domain_id);
                Ok(DomainEvent::Deleted(domain_id).into())
            })?;

            wsv.metrics.domains.dec();
            Ok(())
        }
    }

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> Execute<W> for Register<Role> {
        type Error = Error;

        #[metrics(+"register_role")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let role_id = self.object.id.clone();

            wsv.modify_world(|world| {
                world.roles.insert(role_id.clone(), self.object);
                Ok(RoleEvent::Created(role_id).into())
            })
        }
    }

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> Execute<W> for Unregister<Role> {
        type Error = Error;

        #[metrics("unregister_peer")]
        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Self::Error> {
            let role_id = self.object_id;
            wsv.modify_world(|world| {
                world.roles.remove(&role_id);
                for mut domain in world.domains.iter_mut() {
                    for account in domain.accounts.values_mut() {
                        let _ = account.roles.remove(&role_id);
                    }
                }

                Ok(RoleEvent::Deleted(role_id).into())
            })
        }
    }
}

/// Query module provides `IrohaQuery` Peer related implementations.
pub mod query {
    use eyre::Result;
    use iroha_data_model::prelude::*;
    use iroha_logger::log;

    use super::*;
    use crate::smartcontracts::query::Error;

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> ValidQuery<W> for FindAllRoles {
        #[log]
        fn execute(&self, wsv: &WorldStateView<W>) -> Result<Self::Output, Error> {
            Ok(wsv
                .world
                .roles
                .iter()
                .map(|role| role.value().clone())
                .collect())
        }
    }

    impl<W: WorldTrait> ValidQuery<W> for FindAllPeers {
        #[log]
        fn execute(&self, wsv: &WorldStateView<W>) -> Result<Self::Output, Error> {
            Ok(wsv.peers())
        }
    }
}
