//! Canonical native package geometry shared by installers and installed runtime consumers.

use std::path::{Path, PathBuf};

use super::{Error, Result};

/// Stable native macOS application identity; signing and notarization are separate release steps.
pub const MOCHI_APPLICATION_ID: &str = "org.hyperledger.iroha.mochi";

/// Native CLI package geometry; Kagami and its matching daemon need no desktop application.
#[derive(Debug, Clone, Copy)]
pub struct KagamiBundleLayout;

impl KagamiBundleLayout {
    /// Exact directory containing the two native CLI programs on every supported host.
    #[must_use]
    pub fn runtime_directory(bundle_root: &Path) -> PathBuf {
        bundle_root.join("bin")
    }

    /// Optional independently authenticated authority, colocated with the CLI runtime.
    #[must_use]
    pub fn profiles_path(bundle_root: &Path) -> PathBuf {
        Self::runtime_directory(bundle_root).join(crate::bootstrap::NETWORK_PROFILES_FILENAME)
    }
}

/// The shipped desktop runtime layout for each supported native operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBundleLayout {
    /// One application containing every executable and its resources.
    MacOs,
    /// Linux native executables in the package's `bin` directory.
    Linux,
    /// Windows native executables in the package's `bin` directory.
    Windows,
}

impl NativeBundleLayout {
    /// Layout for the current native host.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    /// Exact directory containing the matching Mochi, Kagami and daemon programs.
    #[must_use]
    pub fn runtime_directory(self, bundle_root: &Path) -> PathBuf {
        bundle_root.join(match self {
            Self::MacOs => "Mochi.app/Contents/MacOS",
            Self::Linux | Self::Windows => "bin",
        })
    }

    /// Exact resource directory; callers must not search alternate locations.
    #[must_use]
    pub fn resources_directory(self, bundle_root: &Path) -> PathBuf {
        match self {
            Self::MacOs => bundle_root.join("Mochi.app/Contents/Resources"),
            Self::Linux | Self::Windows => self.runtime_directory(bundle_root),
        }
    }

    /// Optional independently authenticated network installation artifact.
    #[must_use]
    pub fn profiles_path(self, bundle_root: &Path) -> PathBuf {
        self.resources_directory(bundle_root)
            .join(crate::bootstrap::NETWORK_PROFILES_FILENAME)
    }

    /// Exact native program path, including the Windows suffix when applicable.
    #[must_use]
    pub fn executable(self, bundle_root: &Path, name: &str) -> PathBuf {
        self.runtime_directory(bundle_root).join(format!(
            "{name}{}",
            if self == Self::Windows { ".exe" } else { "" }
        ))
    }
}

/// Render the canonical application metadata from the Mochi package's release version.
///
/// # Errors
/// Only a three-component decimal release version is accepted. Prerelease labels, XML text,
/// leading zeroes and versions exceeding the local 64-byte metadata bound are rejected.
pub fn macos_info_plist(version: &str) -> Result<String> {
    let parts = version.split('.').collect::<Vec<_>>();
    if version.len() > 64
        || parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(Error::Invalid(
            "Mochi app version must be a canonical numeric major.minor.patch release".into(),
        ));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n\
<key>CFBundleDevelopmentRegion</key><string>en</string>\n\
<key>CFBundleDisplayName</key><string>Mochi</string>\n\
<key>CFBundleExecutable</key><string>mochi</string>\n\
<key>CFBundleIdentifier</key><string>{MOCHI_APPLICATION_ID}</string>\n\
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>\n\
<key>CFBundleName</key><string>Mochi</string>\n\
<key>CFBundlePackageType</key><string>APPL</string>\n\
<key>CFBundleShortVersionString</key><string>{version}</string>\n\
<key>CFBundleVersion</key><string>{version}</string>\n\
<key>NSHighResolutionCapable</key><true/>\n\
</dict></plist>\n"
    ))
}

/// Use application resources for desktop packages and colocated profiles for CLI/loose runtimes.
pub(super) fn runtime_profiles_path(directory: &Path) -> Result<PathBuf> {
    let application = directory
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"));
    if let Some(application) = application {
        if directory != application.join("Contents/MacOS") {
            return Err(Error::Invalid(
                "Mochi application runtime has a noncanonical layout".into(),
            ));
        }
        for path in [
            application.to_path_buf(),
            application.join("Contents"),
            directory.to_path_buf(),
            application.join("Contents/Resources"),
        ] {
            let metadata = std::fs::symlink_metadata(path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(Error::Invalid(
                    "Mochi application directories must be direct directories".into(),
                ));
            }
        }
        let metadata = std::fs::symlink_metadata(application.join("Contents/Info.plist"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Error::Invalid(
                "Mochi application metadata must be a direct file".into(),
            ));
        }
        Ok(application
            .join("Contents/Resources")
            .join(crate::bootstrap::NETWORK_PROFILES_FILENAME))
    } else {
        // Loose target/debug and target/release programs are an explicit development mode.
        // Packaged macOS callers must use the application geometry, never an outer bin fallback.
        Ok(directory.join(crate::bootstrap::NETWORK_PROFILES_FILENAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_paths_have_one_exact_runtime_and_resource_location() {
        let root = Path::new("package");
        assert_eq!(
            NativeBundleLayout::MacOs.executable(root, "kagami"),
            root.join("Mochi.app/Contents/MacOS/kagami")
        );
        assert_eq!(
            NativeBundleLayout::Windows.executable(root, "kagami"),
            root.join("bin/kagami.exe")
        );
        assert_eq!(
            NativeBundleLayout::Linux.executable(root, "kagami"),
            root.join("bin/kagami")
        );
        assert_eq!(
            NativeBundleLayout::MacOs.profiles_path(root),
            root.join("Mochi.app/Contents/Resources/network-profiles.nrt")
        );
    }

    #[test]
    fn app_metadata_has_exact_identity_and_numeric_release_version() {
        let plist = macos_info_plist("0.1.0").unwrap();
        assert!(plist.contains(MOCHI_APPLICATION_ID));
        assert!(macos_info_plist("10000.100.0").is_ok());
        assert!(macos_info_plist(&format!("{}.0.0", "1".repeat(65))).is_err());
        assert!(plist.contains("<key>CFBundleExecutable</key><string>mochi</string>"));
        for version in ["", "1", "1.2", "1.2.3.4", "1.2.3-rc.1", "1.02.3", "1.2.<"] {
            assert!(macos_info_plist(version).is_err(), "{version}");
        }
    }
}
