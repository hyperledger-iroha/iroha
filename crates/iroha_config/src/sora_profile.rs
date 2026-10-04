//! Apply Sora runtime defaults while respecting explicit TOML configuration.

use crate::{base::read::ConfigReader, parameters::actual};

/// Source selections captured before parser defaults erase the distinction between
/// omitted and explicitly configured values.
///
/// Capture this from the same loaded sources used to parse the runtime configuration.
/// Apply it only when the caller has selected the Sora runtime profile. An explicit
/// topology field selects the entire parsed geometry, including default-valued fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoraProfileSelection {
    topology: bool,
    storage: bool,
    discovery: bool,
}

impl SoraProfileSelection {
    /// Capture explicit selections from all loaded TOML sources, including `extends`.
    #[must_use]
    pub fn from_reader(reader: &ConfigReader) -> Self {
        Self::from_contains(|path| reader.contains_toml_parameter(path))
    }

    /// Capture explicit selections from a flattened configuration table.
    ///
    /// Callers that support `extends` must use [`Self::from_reader`] after loading
    /// those sources, rather than supplying only the leaf table here.
    #[must_use]
    pub fn from_table(table: &toml::Table) -> Self {
        Self::from_contains(|path| {
            let Some((first, rest)) = path.split_first() else {
                return false;
            };
            let mut value = table.get(*first);
            for segment in rest {
                value = value.and_then(|value| value.get(*segment));
            }
            value.is_some()
        })
    }

    fn from_contains(contains: impl Fn(&[&str]) -> bool) -> Self {
        Self {
            topology: [
                "lane_count",
                "lane_catalog",
                "dataspace_catalog",
                "routing_policy",
            ]
            .into_iter()
            .any(|field| contains(&["nexus", field])),
            storage: contains(&["sorafs", "storage", "enabled"]),
            discovery: contains(&["sorafs", "discovery", "discovery_enabled"]),
        }
    }

    /// Apply the selected Sora defaults to the configuration parsed from these sources.
    ///
    /// Explicit geometry is preserved as a coherent unit. Omitted geometry may
    /// receive the bundled catalogs. Explicit storage and discovery settings retain
    /// their parsed values; trust and runtime validation remain the parser's responsibility.
    pub fn apply(self, config: &mut actual::Root) {
        let storage_enabled = config.torii.sorafs_storage.enabled;
        let discovery_enabled = config.torii.sorafs_discovery.discovery_enabled;
        if self.topology {
            config.apply_sora_service_defaults();
        } else {
            config.apply_sora_profile();
        }
        if self.storage {
            config.torii.sorafs_storage.enabled = storage_enabled;
        }
        if self.discovery {
            config.torii.sorafs_discovery.discovery_enabled = discovery_enabled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::toml::TomlSource;

    #[test]
    fn table_and_reader_select_every_explicit_geometry_field() {
        for source in [
            "[nexus]\nlane_count = 1",
            "[nexus]\nlane_catalog = []",
            "[nexus]\ndataspace_catalog = []",
            "[nexus.routing_policy]",
        ] {
            let table: toml::Table = source.parse().expect("explicit geometry");
            let reader = ConfigReader::new().with_toml_source(TomlSource::inline(table.clone()));
            let selected = SoraProfileSelection::from_table(&table);
            assert!(selected.topology, "{source}");
            assert_eq!(selected, SoraProfileSelection::from_reader(&reader));
            let _ = reader.into_result();
        }
    }

    #[test]
    fn unrelated_nexus_settings_do_not_select_geometry() {
        let table: toml::Table = "[nexus.storage]\nlocal_budget_bytes = 4096"
            .parse()
            .expect("storage settings");
        let selection = SoraProfileSelection::from_table(&table);
        assert!(!selection.topology);
        assert!(!selection.storage);
        assert!(!selection.discovery);
    }

    #[test]
    fn source_selection_keeps_explicit_false_service_settings() {
        let table: toml::Table =
            "[sorafs.storage]\nenabled = false\n[sorafs.discovery]\ndiscovery_enabled = false"
                .parse()
                .expect("service opt-outs");
        let reader = ConfigReader::new().with_toml_source(TomlSource::inline(table.clone()));
        let selection = SoraProfileSelection::from_reader(&reader);
        assert!(selection.storage);
        assert!(selection.discovery);
        assert!(!selection.topology);
        assert_eq!(selection, SoraProfileSelection::from_table(&table));
        let _ = reader.into_result();
    }

    #[test]
    fn loaded_sources_preserve_inherited_geometry_selection() {
        let inherited: toml::Table = "[nexus]\nlane_count = 1".parse().expect("base geometry");
        let child: toml::Table = "[nexus.storage]\nlocal_budget_bytes = 4096"
            .parse()
            .expect("child storage");
        let reader = ConfigReader::new()
            .with_toml_source(TomlSource::inline(inherited))
            .with_toml_source(TomlSource::inline(child.clone()));
        assert!(!SoraProfileSelection::from_table(&child).topology);
        assert!(SoraProfileSelection::from_reader(&reader).topology);
        let _ = reader.into_result();
    }
}
