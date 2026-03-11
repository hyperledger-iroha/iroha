//! Structures, traits and impls related to `Role`s.

use std::{collections::BTreeMap, format, string::String, vec::Vec};

use iroha_data_model_derive::model;

pub use self::model::*;
use crate::{
    Identifiable, Name, Registered, Registrable,
    account::AccountId,
    permission::{Permission, Permissions},
};

#[model]
mod model {
    use derive_more::{Constructor, Display, FromStr};
    use getset::Getters;
    use iroha_data_model_derive::IdEqOrdHash;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Identification of a role.
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        FromStr,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[getset(get = "pub")]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct RoleId {
        /// Role name, should be unique .
        pub name: Name,
    }

    /// Role is a tag for a set of permission tokens.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Decode, Encode, IntoSchema)]
    #[display("{id}")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Role {
        /// Unique name of the role.
        pub id: RoleId,
        /// Permission tokens.
        pub permissions: Permissions,
        /// Permission grant epochs (block heights), keyed by permission.
        ///
        /// This map is populated by the node at execution time (e.g. when a role is
        /// registered or a permission is granted/revoked). Clients typically
        /// don't provide it and start with an empty map.
        #[norito(default)]
        pub permission_epochs: BTreeMap<Permission, u64>,
    }

    /// Builder for [`Role`]
    #[derive(Debug, Display, Clone, Getters, IdEqOrdHash, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[getset(get = "pub")]
    #[display("{grant_to}: {inner}")]
    pub struct NewRole {
        /// Role definition being created.
        #[id(transparent)]
        pub inner: Role,
        /// First owner
        pub grant_to: AccountId,
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for RoleId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.name, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for RoleId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let name = Name::json_deserialize(parser)?;
        Ok(Self { name })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for NewRole {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("id", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.inner.id, out);
        out.push(',');
        norito::json::write_json_string("permissions", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.inner.permissions, out);
        out.push(',');
        norito::json::write_json_string("grant_to", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.grant_to, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for NewRole {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut id: Option<RoleId> = None;
        let mut permissions: Option<Permissions> = None;
        let mut grant_to: Option<AccountId> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "id" => {
                    if id.is_some() {
                        return Err(norito::json::Error::duplicate_field("id"));
                    }
                    id = Some(visitor.parse_value::<RoleId>()?);
                }
                "permissions" => {
                    if permissions.is_some() {
                        return Err(norito::json::Error::duplicate_field("permissions"));
                    }
                    permissions = Some(visitor.parse_value::<Permissions>()?);
                }
                "grant_to" => {
                    if grant_to.is_some() {
                        return Err(norito::json::Error::duplicate_field("grant_to"));
                    }
                    grant_to = Some(visitor.parse_value::<AccountId>()?);
                }
                _other => {
                    visitor.skip_value()?;
                    // Unknown fields are ignored for forward compatibility.
                }
            }
        }
        visitor.finish()?;

        let id = id.ok_or_else(|| norito::json::Error::missing_field("id"))?;
        let permissions =
            permissions.ok_or_else(|| norito::json::Error::missing_field("permissions"))?;
        let grant_to = grant_to.ok_or_else(|| norito::json::Error::missing_field("grant_to"))?;

        Ok(Self {
            inner: Role {
                id,
                permissions,
                permission_epochs: BTreeMap::new(),
            },
            grant_to,
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for Role {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("id", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.id, out);
        out.push(',');
        norito::json::write_json_string("permissions", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.permissions, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Role {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut id: Option<RoleId> = None;
        let mut permissions: Option<Permissions> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "id" => {
                    if id.is_some() {
                        return Err(norito::json::Error::duplicate_field("id"));
                    }
                    id = Some(visitor.parse_value::<RoleId>()?);
                }
                "permissions" => {
                    if permissions.is_some() {
                        return Err(norito::json::Error::duplicate_field("permissions"));
                    }
                    permissions = Some(visitor.parse_value::<Permissions>()?);
                }
                _other => {
                    visitor.skip_value()?;
                    // Ignore unknown fields for forward compatibility.
                }
            }
        }
        visitor.finish()?;

        let id = id.ok_or_else(|| norito::json::Error::missing_field("id"))?;
        let permissions =
            permissions.ok_or_else(|| norito::json::Error::missing_field("permissions"))?;

        Ok(Self {
            id,
            permissions,
            permission_epochs: BTreeMap::new(),
        })
    }
}

impl Role {
    /// Constructor.
    #[inline]
    pub fn new(id: RoleId, grant_to: AccountId) -> <Self as Registered>::With {
        NewRole::new(id, grant_to)
    }

    /// Get an iterator over [`permissions`](Permission) of the `Role`
    #[inline]
    pub fn permissions(&self) -> impl ExactSizeIterator<Item = &Permission> {
        self.permissions.iter()
    }

    /// Return the recorded epoch for the provided permission.
    #[inline]
    #[must_use]
    pub fn permission_epoch(&self, permission: &Permission) -> Option<u64> {
        self.permission_epochs.get(permission).copied()
    }

    /// Borrow the permission epoch map.
    #[inline]
    #[must_use]
    pub fn permission_epochs(&self) -> &BTreeMap<Permission, u64> {
        &self.permission_epochs
    }

    /// Fill missing permission epoch entries and drop stale ones.
    pub fn ensure_permission_epochs(&mut self, default_epoch: u64) {
        self.permission_epochs
            .retain(|perm, _| self.permissions.contains(perm));
        for perm in &self.permissions {
            self.permission_epochs
                .entry(perm.clone())
                .or_insert(default_epoch);
        }
    }
}

impl NewRole {
    /// Constructor
    #[must_use]
    #[inline]
    fn new(id: RoleId, grant_to: AccountId) -> Self {
        Self {
            grant_to,
            inner: Role {
                id,
                permissions: Permissions::new(),
                permission_epochs: BTreeMap::new(),
            },
        }
    }

    /// Add permission to the [`Role`]
    #[must_use]
    #[inline]
    pub fn add_permission(self, perm: impl Into<Permission>) -> Self {
        self.add_permission_with_epoch(perm, 0)
    }

    /// Add permission to the [`Role`] with an explicit epoch.
    #[must_use]
    #[inline]
    pub fn add_permission_with_epoch(mut self, perm: impl Into<Permission>, epoch: u64) -> Self {
        let perm = perm.into();
        self.inner.permissions.insert(perm.clone());
        // `epoch == 0` is treated as the implicit/default value and is omitted
        // from the sparse `permission_epochs` map to keep payloads compact.
        if epoch != 0 {
            self.inner.permission_epochs.entry(perm).or_insert(epoch);
        }
        self
    }
}

impl Registered for Role {
    type With = NewRole;
}

impl Registrable for NewRole {
    type Target = Role;

    #[inline]
    fn build(self, _authority: &AccountId) -> Self::Target {
        self.inner
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use iroha_primitives::json::Json;

    use super::*;
    use crate::permission::Permission;

    #[test]
    fn role_json_roundtrip() {
        let name: Name = "auditor".parse().expect("role name");
        let id = RoleId::new(name);
        let mut permissions = Permissions::new();
        permissions.insert(Permission::new(
            "can_audit".into(),
            Json::new(norito::json!({"level": "basic"})),
        ));
        let role = Role {
            id,
            permissions,
            // Epoch `0` is the implicit default and omitted from JSON.
            permission_epochs: BTreeMap::new(),
        };

        let json = norito::json::to_json(&role).expect("serialize role");
        let mut decoded: Role = norito::json::from_json(&json).expect("deserialize role");

        assert_eq!(decoded, role);
        assert_eq!(decoded.permissions, role.permissions);
        assert!(decoded.permission_epochs().is_empty());

        decoded.ensure_permission_epochs(0);
        assert_eq!(decoded.permission_epochs().len(), 1);
        assert_eq!(
            decoded.permission_epoch(decoded.permissions().next().expect("permission")),
            Some(0)
        );
    }

    #[test]
    fn role_permission_epochs_capture_epoch() {
        use iroha_crypto::{Algorithm, KeyPair};

        let name: Name = "auditor".parse().expect("role name");
        let id = RoleId::new(name);
        let perm = Permission::new("can_audit".into(), Json::new(norito::json!({})));
        let _domain: crate::domain::DomainId = "wonderland".parse().unwrap();
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let account_id = AccountId::new(keypair.public_key().clone());
        let role = Role::new(id, account_id.clone())
            .add_permission_with_epoch(perm.clone(), 42)
            .build(&account_id);
        assert_eq!(role.permission_epoch(&perm), Some(42));
    }

    #[test]
    fn role_ensure_permission_epochs_fills_missing_and_prunes() {
        let name: Name = "auditor".parse().expect("role name");
        let id = RoleId::new(name);
        let perm = Permission::new("can_audit".into(), Json::new(norito::json!({})));
        let mut permissions = Permissions::new();
        permissions.insert(perm.clone());
        let mut role = Role {
            id,
            permissions,
            permission_epochs: BTreeMap::new(),
        };

        role.ensure_permission_epochs(9);
        assert_eq!(role.permission_epoch(&perm), Some(9));

        role.permissions.clear();
        role.ensure_permission_epochs(9);
        assert!(role.permission_epochs.is_empty());
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this module.
pub mod prelude {
    pub use super::{NewRole, Role, RoleId};
}

// Provide a slice-based decoder for Role to satisfy event enum derives
// that require `DecodeFromSlice` under strict-safe builds.
impl<'a> norito::core::DecodeFromSlice<'a> for Role {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        // Use the adaptive bare decoder; it consumes the full slice.
        let mut cur = std::io::Cursor::new(bytes);
        let value = <Self as norito::codec::Decode>::decode(&mut cur)?;
        Ok((value, bytes.len()))
    }
}
