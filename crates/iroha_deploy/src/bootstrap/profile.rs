//! Installed discovery roots and exact network expectations for native bootstrap.

use std::{collections::BTreeMap, path::Path};

use norito::{Decode, Encode};
use url::Url;

use super::{BootstrapError, Result, decode, validate_network_name};

/// Canonical native bundle filename containing independently installed network profiles.
pub const NETWORK_PROFILES_FILENAME: &str = "network-profiles.nrt";
/// Maximum profiles in one authenticated installation artifact.
pub const MAX_INSTALLED_NETWORK_PROFILES: usize = 32;
/// Maximum canonical installation artifact before decoding or retaining it.
pub const MAX_INSTALLED_PROFILE_BYTES: usize = 128 * 1024;
const MAX_TORII_ROOT_BYTES: usize = 2048;

/// One installed network selection, with HTTPS or numeric-loopback HTTP for initial discovery.
/// Native bootstrap retains the exact discovered genesis identity and verifies consensus proofs.
#[derive(Clone, Debug)]
pub struct InstalledNetworkProfile {
    network_name: String,
    torii_root: Url,
    expected_chain_id: String,
    account_chain_discriminant: u16,
}

impl InstalledNetworkProfile {
    /// Select a canonical discovery root and exact chain/address expectations.
    /// Remote networks require HTTPS; explicit numeric-loopback development roots may use HTTP.
    /// The installed root provides initial discovery; later responses cannot replace a retained
    /// genesis identity or substitute these chain and account-address selections.
    ///
    /// # Errors
    /// Invalid network label, chain, address prefix, credentials, noncanonical URL or bounds.
    pub fn new(
        network_name: String,
        torii_root: &str,
        expected_chain_id: String,
        account_chain_discriminant: u16,
    ) -> Result<Self> {
        validate_network_name(&network_name)?;
        expected_chain_id
            .parse::<iroha_model_base::chain::ChainId>()
            .map_err(|_| BootstrapError::Invalid("invalid installed chain identity"))?;
        iroha::config::resolve_account_chain_discriminant(None, Some(account_chain_discriminant))
            .map_err(|_| BootstrapError::Invalid("invalid installed account address prefix"))?;
        if torii_root.len() > MAX_TORII_ROOT_BYTES {
            return Err(BootstrapError::Invalid(
                "installed Torii root exceeds byte bound",
            ));
        }
        let url = Url::parse(torii_root)
            .map_err(|_| BootstrapError::Invalid("invalid installed Torii root"))?;
        let numeric_loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if url.as_str() != torii_root
            || !(url.scheme() == "https" || url.scheme() == "http" && numeric_loopback)
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.path().ends_with('/')
        {
            return Err(BootstrapError::Invalid(
                "installed Torii root requires canonical credential-free HTTPS or numeric-loopback HTTP with a trailing slash",
            ));
        }
        Ok(Self {
            network_name,
            torii_root: url,
            expected_chain_id,
            account_chain_discriminant,
        })
    }

    /// Exact installed CLI selection, without an inferred official-network fallback.
    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    /// Exact installed root used for native discovery and proof retrieval.
    pub fn torii_root(&self) -> &Url {
        &self.torii_root
    }

    /// Chain identity that the discovered signed genesis must match exactly.
    pub fn chain_id(&self) -> &str {
        &self.expected_chain_id
    }

    /// Account-address prefix required for native account and faucet discovery.
    pub const fn account_chain_discriminant(&self) -> u16 {
        self.account_chain_discriminant
    }

    fn record(&self) -> ProfileRecord {
        ProfileRecord {
            network_name: self.network_name.clone(),
            torii_root: self.torii_root.to_string(),
            expected_chain_id: self.expected_chain_id.clone(),
            account_chain_discriminant: self.account_chain_discriminant,
        }
    }
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::InstalledNetworkProfileV1")]
struct ProfileRecord {
    network_name: String,
    torii_root: String,
    expected_chain_id: String,
    account_chain_discriminant: u16,
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::bootstrap::InstalledNetworkProfilesV1")]
struct ProfileRecords {
    profiles: Vec<ProfileRecord>,
}

/// Bounded, exactly named profiles supplied by an authenticated native installation.
/// An empty set is valid; network selections must be supplied by the installation.
#[derive(Clone, Debug)]
pub struct InstalledNetworkProfiles {
    profiles: BTreeMap<String, InstalledNetworkProfile>,
}

impl InstalledNetworkProfiles {
    /// Construct an installation set from independent explicit profile inputs.
    ///
    /// # Errors
    /// More than the finite profile count or a duplicate network selection.
    pub fn new(profiles: Vec<InstalledNetworkProfile>) -> Result<Self> {
        if profiles.len() > MAX_INSTALLED_NETWORK_PROFILES {
            return Err(BootstrapError::Invalid(
                "installed network profile count exceeds bound",
            ));
        }
        let mut selected = BTreeMap::new();
        for profile in profiles {
            if selected
                .insert(profile.network_name().to_owned(), profile)
                .is_some()
            {
                return Err(BootstrapError::Invalid(
                    "duplicate installed network profile",
                ));
            }
        }
        Ok(Self { profiles: selected })
    }

    /// Decode bytes already authenticated by installation custody, never bytes downloaded as a response.
    ///
    /// # Errors
    /// Noncanonical, unsorted, duplicate, oversized or invalid installation profiles.
    pub fn from_installation_bytes(bytes: &[u8]) -> Result<Self> {
        let records: ProfileRecords = decode(bytes, MAX_INSTALLED_PROFILE_BYTES)?;
        if records.profiles.len() > MAX_INSTALLED_NETWORK_PROFILES
            || records
                .profiles
                .windows(2)
                .any(|pair| pair[0].network_name >= pair[1].network_name)
        {
            return Err(BootstrapError::Invalid(
                "installed profiles must be bounded and strictly ordered",
            ));
        }
        Self::new(
            records
                .profiles
                .into_iter()
                .map(|record| {
                    InstalledNetworkProfile::new(
                        record.network_name,
                        &record.torii_root,
                        record.expected_chain_id,
                        record.account_chain_discriminant,
                    )
                })
                .collect::<Result<_>>()?,
        )
    }

    /// Load a current-user or operating-system-owned installation through retained no-follow custody.
    /// Foreign-writable files, unsafe ancestry, links and reparse points are refused.
    /// The caller must have independently authenticated the installation that supplied it.
    ///
    /// # Errors
    /// Missing or unsafe filesystem custody, bounded read failure or malformed profiles.
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_installation_bytes(&iroha_fs::read_regular(path, MAX_INSTALLED_PROFILE_BYTES)?)
    }

    /// Encode the sole first-release bundle layout in deterministic selection order.
    ///
    /// # Errors
    /// The canonical record exceeds the same bounded decoder used on installation.
    pub fn encode_installation(&self) -> Result<Vec<u8>> {
        let records = ProfileRecords {
            profiles: self
                .profiles
                .values()
                .map(InstalledNetworkProfile::record)
                .collect(),
        };
        let bytes = norito::encode_canonical(&records)
            .map_err(|_| BootstrapError::Invalid("cannot encode installed network profiles"))?;
        Self::from_installation_bytes(&bytes)?;
        Ok(bytes)
    }

    /// Installed network choices in canonical order, for CLI and desktop selection.
    pub fn names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }

    /// Select an exact installed name; unknown networks never fall back to another trust root.
    ///
    /// # Errors
    /// The requested selection is absent from this installation.
    pub fn select(&self, network_name: &str) -> Result<&InstalledNetworkProfile> {
        self.profiles
            .get(network_name)
            .ok_or(BootstrapError::Invalid(
                "selected network has no installed profile",
            ))
    }
}

#[cfg(test)]
mod tests;
