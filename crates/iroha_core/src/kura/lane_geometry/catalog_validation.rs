fn geometry_file_identity(metadata: &SecureMetadata) -> GeometryFileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        GeometryFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        let volume_serial_number = metadata.volume_serial_number();
        let file_index = metadata.file_index();
        let unsupported_nonce = if volume_serial_number.is_some() && file_index.is_some() {
            0
        } else {
            // Some Windows filesystems do not expose stable volume/file IDs. A fresh nonce makes
            // every subsequent comparison fail closed instead of treating all paths as equal.
            UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed)
        };
        GeometryFileIdentity {
            volume_serial_number,
            file_index,
            unsupported_nonce,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::sync::atomic::Ordering;
        let _ = metadata;
        GeometryFileIdentity {
            unsupported_nonce: UNSUPPORTED_GEOMETRY_IDENTITY_NONCE.fetch_add(1, Ordering::Relaxed),
        }
    }
}
fn checked_geometry_file_identity(
    metadata: &SecureMetadata,
    path: &Path,
) -> Result<GeometryFileIdentity> {
    let identity = geometry_file_identity(metadata);
    #[cfg(windows)]
    if identity.unsupported_nonce != 0 {
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "Windows filesystem did not expose a stable volume and file identity",
            ),
            path.to_path_buf(),
        ));
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = identity;
        return Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "lane geometry requires stable filesystem object identities",
            ),
            path.to_path_buf(),
        ));
    }
    #[cfg(any(unix, windows))]
    {
        let _ = path;
        Ok(identity)
    }
}
