use std::fmt::Display;

#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;
use crate::{account::NewAccount, consensus::HsmBinding, domain::NewDomain};

isi! {
    /// Generic instruction for a registration of an object to the identifiable destination.
    ///
    /// Dev note: naming
    /// `RegisterBox` below is an enum that groups concrete `Register<T>` variants
    /// (e.g., `Peer`, `Domain`, `Account`, ...). It is not a heap `Box`; the
    /// "Box" suffix means "boxed-up family of variants" for easy visiting and
    /// serialization.
    #[cfg_attr(feature = "json", norito(transparent))]
    pub struct Register<O: Registered> {
        /// The object that should be registered, should be uniquely identifiable by its id.
        pub object: O::With,
    }
}

impl Register<Domain> {
    /// Constructs a new [`Register`] for a [`Domain`].
    pub fn domain(new_domain: NewDomain) -> Self {
        Self { object: new_domain }
    }
}

impl Register<Account> {
    /// Constructs a new [`Register`] for an [`Account`].
    pub fn account(new_account: NewAccount) -> Self {
        Self {
            object: new_account,
        }
    }
}

impl Register<AssetDefinition> {
    /// Constructs a new [`Register`] for an [`AssetDefinition`].
    pub fn asset_definition(new_asset_definition: NewAssetDefinition) -> Self {
        Self {
            object: new_asset_definition,
        }
    }
}

impl Register<Nft> {
    /// Constructs a new [`Register`] for an [`Nft`].
    pub fn nft(new_nft: NewNft) -> Self {
        Self { object: new_nft }
    }
}

impl Register<Role> {
    /// Constructs a new [`Register`] for a [`Role`].
    pub fn role(new_role: NewRole) -> Self {
        Self { object: new_role }
    }
}

impl Register<Trigger> {
    /// Constructs a new [`Register`] for a [`Trigger`].
    pub fn trigger(new_trigger: Trigger) -> Self {
        Self {
            object: new_trigger,
        }
    }
}

impl_display! {
    Register<O>
    where
        O: Registered,
        O::With: Display,
    =>
    "REGISTER `{}`",
    object,
}

impl_into_box! {
    RegisterPeerWithPop |
    Register<Domain> |
    Register<Account> |
    Register<AssetDefinition> |
    Register<Nft> |
    Register<Role> |
    Register<Trigger>
=> RegisterBox
}

/// Register a peer for consensus participation with a BLS Proof-of-Possession.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterPeerWithPop {
    /// Peer to register
    pub peer: PeerId,
    /// BLS-normal Proof-of-Possession bytes for `peer.public_key()`
    pub pop: Vec<u8>,
    /// Optional explicit activation height (defaults to policy-derived lead time).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub activation_at: Option<u64>,
    /// Optional expiry height for the consensus key.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub expiry_at: Option<u64>,
    /// Optional HSM binding for the consensus key.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub hsm: Option<HsmBinding>,
}

impl_display! {
    RegisterPeerWithPop => "REGISTER_PEER_WITH_POP `{}`", peer
}

impl RegisterPeerWithPop {
    /// Construct a new peer registration containing the given proof-of-possession bytes.
    #[must_use]
    pub fn new(peer: PeerId, pop: Vec<u8>) -> Self {
        Self {
            peer,
            pop,
            activation_at: None,
            expiry_at: None,
            hsm: None,
        }
    }

    /// Attach an explicit activation height; must satisfy the lead-time policy.
    #[must_use]
    pub fn with_activation_at(mut self, activation_at: u64) -> Self {
        self.activation_at = Some(activation_at);
        self
    }

    /// Attach an expiry height for the consensus key.
    #[must_use]
    pub fn with_expiry_at(mut self, expiry_at: u64) -> Self {
        self.expiry_at = Some(expiry_at);
        self
    }

    /// Attach an HSM binding for the consensus key.
    #[must_use]
    pub fn with_hsm(mut self, hsm: HsmBinding) -> Self {
        self.hsm = Some(hsm);
        self
    }
}

isi! {
    /// Generic instruction for an unregistration of an object from the identifiable destination.
    pub struct Unregister<O: Identifiable> {
        /// [`Identifiable::Id`] of the object which should be unregistered.
        pub object: O::Id,
    }
}

impl_display! {
    Unregister<O>
    where
        O: Identifiable,
        O::Id: Display,
    =>
    "UNREGISTER `{}`",
    object,
}

impl_into_box! {
    Unregister<Peer> |
    Unregister<Domain> |
    Unregister<Account> |
    Unregister<AssetDefinition> |
    Unregister<Nft> |
    Unregister<Role> |
    Unregister<Trigger>
=> UnregisterBox
}

impl Unregister<Peer> {
    /// Constructs a new [`Unregister`] for a [`Peer`].
    pub fn peer(peer_id: PeerId) -> Self {
        Self { object: peer_id }
    }
}

impl Unregister<Domain> {
    /// Constructs a new [`Unregister`] for a [`Domain`].
    pub fn domain(domain_id: DomainId) -> Self {
        Self { object: domain_id }
    }
}

impl Unregister<Account> {
    /// Constructs a new [`Unregister`] for an [`Account`].
    pub fn account(account_id: AccountId) -> Self {
        Self { object: account_id }
    }
}

impl Unregister<AssetDefinition> {
    /// Constructs a new [`Unregister`] for an [`AssetDefinition`].
    pub fn asset_definition(asset_definition_id: AssetDefinitionId) -> Self {
        Self {
            object: asset_definition_id,
        }
    }
}

impl Unregister<Nft> {
    /// Constructs a new [`Unregister`] for an [`Asset`].
    pub fn nft(nft_id: NftId) -> Self {
        Self { object: nft_id }
    }
}

impl Unregister<Role> {
    /// Constructs a new [`Unregister`] for a [`Role`].
    pub fn role(role_id: RoleId) -> Self {
        Self { object: role_id }
    }
}

impl Unregister<Trigger> {
    /// Constructs a new [`Unregister`] for a [`Trigger`].
    pub fn trigger(trigger_id: TriggerId) -> Self {
        Self { object: trigger_id }
    }
}

#[cfg(feature = "json")]
impl<O> FastJsonWrite for Register<O>
where
    O: Registered,
    O::With: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<O> FastJsonWrite for Unregister<O>
where
    O: Identifiable,
    O::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push('}');
    }
}

isi_box! {
    /// Enum with all supported [`Register`] instructions.
    pub enum RegisterBox {
        /// Register [`Peer`] (requires Proof-of-Possession).
        Peer(RegisterPeerWithPop),
        /// Register [`Domain`].
        Domain(Register<Domain>),
        /// Register [`Account`].
        Account(Register<Account>),
        /// Register [`AssetDefinition`].
        AssetDefinition(Register<AssetDefinition>),
        /// Register [`Nft`].
        Nft(Register<Nft>),
        /// Register [`Role`].
        Role(Register<Role>),
        /// Register [`Trigger`].
        Trigger(Register<Trigger>),
    }
}

enum_type! {
    pub(crate) enum RegisterType {
        Peer,
        Domain,
        Account,
        AssetDefinition,
        Nft,
        Role,
        Trigger,
    }
}

isi_box! {
    /// Enum with all supported [`Unregister`] instructions.
    pub enum UnregisterBox {
        /// Unregister [`Peer`].
        Peer(Unregister<Peer>),
        /// Unregister [`Domain`].
        Domain(Unregister<Domain>),
        /// Unregister [`Account`].
        Account(Unregister<Account>),
        /// Unregister [`AssetDefinition`].
        AssetDefinition(Unregister<AssetDefinition>),
        /// Unregister [`Nft`].
        Nft(Unregister<Nft>),
        /// Unregister [`Role`].
        Role(Unregister<Role>),
        /// Unregister [`Trigger`].
        Trigger(Unregister<Trigger>),
    }
}

enum_type! {
    pub(crate) enum UnregisterType {
        Peer,
        Domain,
        Account,
        AssetDefinition,
        Nft,
        Role,
        Trigger,
    }
}

// Seal implementations
impl crate::seal::Instruction for RegisterBox {}
impl crate::seal::Instruction for UnregisterBox {}
impl crate::seal::Instruction for RegisterPeerWithPop {}
impl crate::seal::Instruction for Register<Domain> {}
impl crate::seal::Instruction for Register<Account> {}
impl crate::seal::Instruction for Register<AssetDefinition> {}
impl crate::seal::Instruction for Register<Nft> {}
impl crate::seal::Instruction for Register<Role> {}
impl crate::seal::Instruction for Register<Trigger> {}
impl crate::seal::Instruction for Unregister<Peer> {}
impl crate::seal::Instruction for Unregister<Domain> {}
impl crate::seal::Instruction for Unregister<Account> {}
impl crate::seal::Instruction for Unregister<AssetDefinition> {}
impl crate::seal::Instruction for Unregister<Nft> {}
impl crate::seal::Instruction for Unregister<Role> {}
impl crate::seal::Instruction for Unregister<Trigger> {}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterPeerWithPop {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let peer = super::decode_aos_canonical_field::<PeerId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let pop = super::decode_aos_canonical_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let activation_at = super::decode_aos_canonical_field::<Option<u64>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expiry_at = super::decode_aos_canonical_field::<Option<u64>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let hsm = super::decode_aos_canonical_field::<Option<HsmBinding>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                peer,
                pop,
                activation_at,
                expiry_at,
                hsm,
            },
            offset,
        ))
    }
}

impl<'a, O> norito::core::DecodeFromSlice<'a> for Register<O>
where
    O: Registered,
    O::With: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
    Self: norito::codec::Decode,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let object = super::decode_aos_canonical_field::<O::With>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { object }, offset))
    }
}

impl<'a, O> norito::core::DecodeFromSlice<'a> for Unregister<O>
where
    O: Identifiable,
    O::Id: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
    Self: norito::codec::Decode,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let object = super::decode_aos_canonical_field::<O::Id>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { object }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let tag_bytes = bytes.get(..4).ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| norito::core::Error::LengthMismatch)?,
        );
        let mut offset = 4usize;
        let value = match tag {
            0 => Self::Peer(super::decode_aos_slice_field::<RegisterPeerWithPop>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::Domain(super::decode_aos_slice_field::<Register<Domain>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            2 => Self::Account(super::decode_aos_slice_field::<Register<Account>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            3 => Self::AssetDefinition(super::decode_aos_slice_field::<Register<AssetDefinition>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            4 => Self::Nft(super::decode_aos_slice_field::<Register<Nft>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            5 => Self::Role(super::decode_aos_slice_field::<Register<Role>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            6 => Self::Trigger(super::decode_aos_slice_field::<Register<Trigger>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid RegisterBox tag {tag}"
                )));
            }
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((value, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for UnregisterBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let tag_bytes = bytes.get(..4).ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| norito::core::Error::LengthMismatch)?,
        );
        let mut offset = 4usize;
        let value = match tag {
            0 => Self::Peer(super::decode_aos_slice_field::<Unregister<Peer>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::Domain(super::decode_aos_slice_field::<Unregister<Domain>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            2 => Self::Account(super::decode_aos_slice_field::<Unregister<Account>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            3 => Self::AssetDefinition(
                super::decode_aos_slice_field::<Unregister<AssetDefinition>>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?,
            ),
            4 => Self::Nft(super::decode_aos_slice_field::<Unregister<Nft>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            5 => Self::Role(super::decode_aos_slice_field::<Unregister<Role>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            6 => Self::Trigger(super::decode_aos_slice_field::<Unregister<Trigger>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid UnregisterBox tag {tag}"
                )));
            }
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((value, offset))
    }
}

// Stable wire IDs for encoding
impl RegisterBox {
    /// Norito wire identifier for boxed register instructions.
    pub const WIRE_ID: &'static str = "iroha.register";
}
impl UnregisterBox {
    /// Norito wire identifier for boxed unregister instructions.
    pub const WIRE_ID: &'static str = "iroha.unregister";
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use norito::{
        codec::{Decode, Encode},
        core::DecodeFromSlice,
    };

    use super::*;
    use crate::peer::PeerId;

    fn public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(public_key(seed))
    }

    fn domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain id")
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(domain_id(), "rose".parse().expect("asset name"))
    }

    fn nft_id() -> NftId {
        NftId::of(domain_id(), "cheshire".parse().expect("nft name"))
    }

    fn role_id() -> RoleId {
        "auditor".parse().expect("role id")
    }

    fn register_peer_with_pop() -> RegisterPeerWithPop {
        RegisterPeerWithPop {
            peer: PeerId::new(public_key(0x60)),
            pop: vec![1u8, 2, 3, 4, 5],
            activation_at: Some(10),
            expiry_at: Some(100),
            hsm: Some(HsmBinding {
                provider: "softkey".to_owned(),
                key_label: "validator-1".to_owned(),
                slot: Some(7),
            }),
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Encode + for<'de> DecodeFromSlice<'de> + PartialEq + core::fmt::Debug,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, name: &str, value: T)
    where
        T: crate::isi::Instruction
            + Encode
            + 'static
            + norito::core::NoritoSerialize
            + core::fmt::Debug,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame isi");
        let decoded = crate::isi::InstructionRegistry::decode(registry, name, &framed)
            .unwrap_or_else(|| panic!("registered {name}"))
            .unwrap_or_else(|err| panic!("decode {name}: {err}"));
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn register_peer_with_pop_roundtrip() {
        let isi = register_peer_with_pop();
        let encoded = isi.encode();
        let decoded = RegisterPeerWithPop::decode(&mut &encoded[..]).expect("decode");
        assert_eq!(decoded, isi);
    }

    #[test]
    fn register_unregister_decode_from_slice_roundtrips() {
        let account = account(0x61);
        assert_slice_roundtrip(register_peer_with_pop());
        assert_slice_roundtrip(Register::domain(Domain::new(domain_id())));
        assert_slice_roundtrip(Register::account(Account::new(account.clone())));
        assert_slice_roundtrip(Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id(),
        )));
        assert_slice_roundtrip(Register::nft(Nft::new(nft_id(), Metadata::default())));
        assert_slice_roundtrip(Register::role(Role::new(role_id(), account.clone())));

        assert_slice_roundtrip(Unregister::peer(PeerId::new(public_key(0x62))));
        assert_slice_roundtrip(Unregister::domain(domain_id()));
        assert_slice_roundtrip(Unregister::account(account));
        assert_slice_roundtrip(Unregister::asset_definition(asset_definition_id()));
        assert_slice_roundtrip(Unregister::nft(nft_id()));
        assert_slice_roundtrip(Unregister::role(role_id()));
        assert_slice_roundtrip(Unregister::trigger(
            "nightly_tick".parse().expect("trigger id"),
        ));
    }

    #[test]
    fn register_unregister_boxes_registry_decode_stable_ids() {
        let account = account(0x63);
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            std::any::type_name::<RegisterPeerWithPop>(),
            register_peer_with_pop(),
        );

        let register_cases = [
            RegisterBox::Peer(register_peer_with_pop()),
            RegisterBox::Domain(Register::domain(Domain::new(domain_id()))),
            RegisterBox::Account(Register::account(Account::new(account.clone()))),
            RegisterBox::AssetDefinition(Register::asset_definition(AssetDefinition::numeric(
                asset_definition_id(),
            ))),
            RegisterBox::Nft(Register::nft(Nft::new(nft_id(), Metadata::default()))),
            RegisterBox::Role(Register::role(Role::new(role_id(), account.clone()))),
        ];
        for value in register_cases {
            assert_registry_decodes(&registry, RegisterBox::WIRE_ID, value);
        }

        let unregister_cases = [
            UnregisterBox::Peer(Unregister::peer(PeerId::new(public_key(0x64)))),
            UnregisterBox::Domain(Unregister::domain(domain_id())),
            UnregisterBox::Account(Unregister::account(account)),
            UnregisterBox::AssetDefinition(Unregister::asset_definition(asset_definition_id())),
            UnregisterBox::Nft(Unregister::nft(nft_id())),
            UnregisterBox::Role(Unregister::role(role_id())),
            UnregisterBox::Trigger(Unregister::trigger(
                "nightly_tick".parse().expect("trigger id"),
            )),
        ];
        for value in unregister_cases {
            assert_registry_decodes(&registry, UnregisterBox::WIRE_ID, value);
        }
    }
}
