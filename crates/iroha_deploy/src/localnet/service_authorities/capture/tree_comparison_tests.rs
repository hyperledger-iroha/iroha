//! One fresh original-profile tree pass preserves lazy bytes, custody and source retry.

use super::*;
use iroha_fs::PublishMode;

// Independent three-pass recipe retained for comparison with the lexical directory scopes.
impl CapturedDirectory {
    // The former standalone root and descendant reads retain their independent native brackets.
    pub(super) fn compare_inputs(&self) -> crate::managed::Result<()> {
        self.compare_inputs_with(|inputs| self.directory.compare_files(inputs))
    }

    fn revalidate_in_tree(
        &self,
        tree: &mut PrivateReadTreeScope<'_>,
    ) -> crate::managed::Result<()> {
        // Keep the native census and complete-name comparison in one fresh suffix bracket.
        // Its exit closes even empty inventories and overrides ordinary comparison errors.
        tree.read_scope(&self.directory, |reader| match &self.inventory {
            Some(expected) => {
                let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
                if reader
                    .entries(expected.len())?
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    != expected
                {
                    return Err(Error::Invalid(
                        "original service directory inventory differs".into(),
                    ));
                }
                Ok(())
            }
            None => Ok(()),
        })
    }

    fn compare_inputs_in_tree(
        &self,
        tree: &mut PrivateReadTreeScope<'_>,
    ) -> crate::managed::Result<()> {
        self.compare_inputs_with(|inputs| {
            tree.read_scope(&self.directory, |reader| reader.compare_files(inputs))
        })
    }
}

impl CapturedProfile {
    fn revalidate_in_tree(
        &self,
        tree: &mut PrivateReadTreeScope<'_>,
    ) -> crate::managed::Result<()> {
        // Keep the generation's original full checks without wrapping it in another
        // full-fallback directory view. Only genuine descendants can share its prefix.
        for (index, directory) in self.directories.iter().enumerate() {
            if index == GENERATION {
                directory.revalidate()?;
            } else {
                directory.revalidate_in_tree(tree)?;
            }
        }
        for (index, directory) in self.directories.iter().enumerate() {
            if index == GENERATION {
                directory.compare_inputs()?;
            } else {
                directory.compare_inputs_in_tree(tree)?;
            }
        }
        for (index, directory) in self.directories.iter().enumerate().rev() {
            if index == GENERATION {
                directory.revalidate()?;
            } else {
                directory.revalidate_in_tree(tree)?;
            }
        }
        Ok(())
    }

    fn original_three_pass(&self) -> crate::managed::Result<()> {
        self.directories
            .first()
            .ok_or_else(invalid)?
            .directory
            .read_tree_scope(|tree| self.revalidate_in_tree(tree))
    }
}

fn small() -> (tempfile::TempDir, CapturedProfile) {
    let temporary = tempfile::tempdir().unwrap();
    let generation = PrivateDirectory::open_or_create(temporary.path().join("generation")).unwrap();
    let child = generation.create_child("inputs").unwrap();
    child
        .write_atomic("first", b"first", PublishMode::CreateNew)
        .unwrap();
    child
        .write_atomic("second", b"second", PublishMode::CreateNew)
        .unwrap();
    let mut captured = CapturedDirectory::new(child, None).unwrap();
    let mut total = 0;
    captured
        .capture("first", 5, ConfigFileAccess::Private, &mut total)
        .unwrap();
    captured
        .capture("second", 6, ConfigFileAccess::Public, &mut total)
        .unwrap();
    let image = CapturedProfile {
        directories: vec![CapturedDirectory::new(generation, None).unwrap(), captured],
    };
    image.revalidate().unwrap();
    (temporary, image)
}

#[test]
fn complete_profile_tree_keeps_all_original_inputs_inventories_and_same_source_retry() {
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "tree-profile",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let (profile, parses) =
        count_semantic_validations(|| capture_retained(&prepared).unwrap().unwrap());
    assert_eq!(parses, 1);
    let image = &profile.retained.image;
    assert_eq!(image.directories.len(), DIRECTORY_COUNT);
    assert_eq!(DIRECTORY_COUNT, 20);
    assert_eq!(
        image
            .directories
            .iter()
            .map(|directory| directory.inputs.len())
            .sum::<usize>(),
        INPUT_COUNT
    );
    let ((), parses) = count_semantic_validations(|| {
        image.revalidate().unwrap();
        let generation = &image.directories[GENERATION];
        let input = generation
            .inputs
            .iter()
            .find(|input| input.name == "peer3.toml")
            .unwrap();
        let mut changed = input.bytes.to_vec();
        changed.extend_from_slice(b"\n# same semantics, different exact input\n");
        generation
            .directory
            .write_atomic(input.name, &changed, PublishMode::Replace)
            .unwrap();
        assert!(
            matches!(image.revalidate(), Err(Error::Invalid(message)) if message == "retained service profile input custody differs")
        );
        generation
            .directory
            .write_atomic(input.name, &input.bytes, PublishMode::Replace)
            .unwrap();
        image.revalidate().unwrap();
        let provider = &image.directories[FIRST_PROVIDER + 2];
        let expected = provider.inventory.as_ref().unwrap();
        let original_names = provider.directory.entries(expected.len()).unwrap();
        provider
            .directory
            .write_atomic("unexpected", b"not captured", PublishMode::CreateNew)
            .unwrap();
        assert_eq!(
            provider
                .directory
                .entries(expected.len() + 1)
                .unwrap()
                .len(),
            expected.len() + 1
        );
        // Both inventory boundaries retain the original exact-count admission. An extra
        // entry exceeds that bound before the complete-name comparison can run.
        for error in [
            provider.revalidate().unwrap_err(),
            generation
                .directory
                .read_tree_scope(|tree| provider.revalidate_in_tree(tree))
                .unwrap_err(),
            image.revalidate().unwrap_err(),
        ] {
            assert!(
                matches!(&error, Error::Io(native) if native.kind() == io::ErrorKind::InvalidInput && native.to_string() == "directory entry count exceeds the bound"),
                "unexpected oversized inventory refusal: {error:?}"
            );
        }
        std::fs::remove_file(provider.directory.path().join("unexpected")).unwrap();
        image.revalidate().unwrap();
        // Equal-count substitution reaches the original complete-name comparison. Move
        // and restore the same original file rather than replacing its retained bytes.
        let original_input = provider.inputs.first().unwrap();
        let original_path = provider.directory.path().join(original_input.name);
        let changed_path = provider.directory.path().join("unexpected");
        std::fs::rename(&original_path, &changed_path).unwrap();
        assert_eq!(
            provider.directory.entries(expected.len()).unwrap().len(),
            expected.len()
        );
        for error in [
            provider.revalidate().unwrap_err(),
            generation
                .directory
                .read_tree_scope(|tree| provider.revalidate_in_tree(tree))
                .unwrap_err(),
            image.revalidate().unwrap_err(),
        ] {
            assert!(
                matches!(&error, Error::Invalid(message) if message == "original service directory inventory differs"),
                "unexpected equal-count inventory refusal: {error:?}"
            );
        }
        std::fs::rename(&changed_path, &original_path).unwrap();
        assert_eq!(
            provider.directory.entries(expected.len()).unwrap(),
            original_names
        );
        assert_eq!(
            provider
                .directory
                .read(original_input.name, original_input.maximum)
                .unwrap()
                .as_slice(),
            original_input.bytes.as_slice()
        );
        image.revalidate().unwrap();
        // A captured leaf in a directory without a fixed inventory surfaces its absence.
        let tables = &image.directories[TABLES];
        let key = tables.inputs.first().unwrap();
        std::fs::remove_file(tables.directory.path().join(key.name)).unwrap();
        assert!(
            matches!(image.revalidate(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
        );
        tables
            .directory
            .write_atomic(key.name, &key.bytes, PublishMode::CreateNew)
            .unwrap();
        profile
            .retained
            .revalidate(&prepared, profile.retained.manifest())
            .unwrap();
    });
    assert_eq!(parses, 0);
}

#[test]
fn profile_tree_comparison_preserves_fixed_stack_bound_lazy_mismatch_and_empty_inputs() {
    assert_eq!(
        CapturedProfile {
            directories: Vec::new()
        }
        .revalidate()
        .unwrap_err()
        .to_string(),
        invalid().to_string()
    );
    let (_temporary, mut image) = small();
    let generation = &image.directories[GENERATION].directory;
    let captured = &image.directories[1];
    captured
        .directory
        .write_atomic("first", b"other", PublishMode::Replace)
        .unwrap();
    std::fs::remove_file(captured.directory.path().join("second")).unwrap();
    for tree_boundary in [false, true] {
        let compare = || {
            if tree_boundary {
                generation.read_tree_scope(|tree| captured.compare_inputs_in_tree(tree))
            } else {
                captured.compare_inputs()
            }
        };
        assert_eq!(compare().unwrap_err().to_string(), invalid().to_string());
    }
    captured
        .directory
        .write_atomic("first", b"first", PublishMode::Replace)
        .unwrap();
    let standalone = captured.compare_inputs().unwrap_err();
    let scoped = generation
        .read_tree_scope(|tree| captured.compare_inputs_in_tree(tree))
        .unwrap_err();
    assert!(matches!(standalone, Error::Io(error) if error.kind() == io::ErrorKind::NotFound));
    assert!(matches!(scoped, Error::Io(error) if error.kind() == io::ErrorKind::NotFound));
    captured
        .directory
        .write_atomic("second", b"oversize", PublishMode::CreateNew)
        .unwrap();
    let standalone = captured.compare_inputs().unwrap_err();
    let scoped = generation
        .read_tree_scope(|tree| captured.compare_inputs_in_tree(tree))
        .unwrap_err();
    assert_eq!(standalone.to_string(), scoped.to_string());
    captured
        .directory
        .write_atomic("second", b"second", PublishMode::Replace)
        .unwrap();
    image.revalidate().unwrap();
    generation
        .read_tree_scope(|tree| image.directories[GENERATION].compare_inputs_in_tree(tree))
        .unwrap();
    // The original fixed metadata admission rejects before reading even a missing first leaf.
    std::fs::remove_file(captured.directory.path().join("first")).unwrap();
    while image.directories[1].inputs.len() <= INPUT_COUNT {
        image.directories[1].inputs.push(Input {
            name: "../invalid",
            maximum: 0,
            access: ConfigFileAccess::Private,
            bytes: Zeroizing::new(Vec::new()),
        });
    }
    let captured = &image.directories[1];
    assert_eq!(
        captured.compare_inputs().unwrap_err().to_string(),
        invalid().to_string()
    );
    assert_eq!(
        image.directories[GENERATION]
            .directory
            .read_tree_scope(|tree| captured.compare_inputs_in_tree(tree))
            .unwrap_err()
            .to_string(),
        invalid().to_string()
    );
    assert_eq!(image.directories[1].inputs.len(), INPUT_COUNT + 1);
    image.directories[1].inputs.pop();
    assert_eq!(image.directories[1].inputs.len(), INPUT_COUNT);
    let captured = &image.directories[1];
    assert!(
        matches!(captured.compare_inputs(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    assert!(
        matches!(image.directories[GENERATION].directory.read_tree_scope(|tree| captured.compare_inputs_in_tree(tree)), Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
}

#[cfg(unix)]
#[test]
fn profile_tree_exit_refuses_real_success_mismatch_and_missing_outcomes_before_retry() {
    use std::os::unix::fs::PermissionsExt as _;
    for outcome in ["success", "mismatch", "missing"] {
        let (_temporary, image) = small();
        let generation = &image.directories[GENERATION].directory;
        let child = &image.directories[1].directory;
        match outcome {
            "mismatch" => child
                .write_atomic("first", b"other", PublishMode::Replace)
                .unwrap(),
            "missing" => std::fs::remove_file(child.path().join("first")).unwrap(),
            _ => (),
        }
        let refused = generation.read_tree_scope_with_root(|tree, reader| {
            let (first, rest) = image.directories.split_first().unwrap();
            let body = CapturedProfile::revalidate_directory(first, rest, tree, reader, &mut || Ok(()));
            match outcome {
                "success" => assert!(body.is_ok()),
                "mismatch" => assert!(matches!(&body, Err(Error::Invalid(message)) if message == "retained service profile input custody differs")),
                "missing" => assert!(matches!(&body, Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound)),
                _ => unreachable!(),
            }
            std::fs::set_permissions(generation.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
            body
        }).unwrap_err();
        assert!(
            matches!(refused, Error::Io(error) if error.kind() == io::ErrorKind::PermissionDenied)
        );
        std::fs::set_permissions(generation.path(), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        child
            .write_atomic("first", b"first", PublishMode::Replace)
            .unwrap();
        image.revalidate().unwrap();
        assert_eq!(child.read("first", 5).unwrap().as_slice(), b"first");
    }
}

#[cfg(unix)]
#[test]
fn profile_tree_keeps_native_prefix_fallback_private_leaf_suffix_and_restored_temporal_limits() {
    use std::os::unix::fs::PermissionsExt as _;
    let (temporary, image) = small();
    let generation = &image.directories[GENERATION].directory;
    let captured = &image.directories[1];
    let mut reopened = CapturedDirectory::new(
        PrivateDirectory::open(captured.directory.path()).unwrap(),
        None,
    )
    .unwrap();
    let mut total = 0;
    reopened
        .capture("first", 5, ConfigFileAccess::Private, &mut total)
        .unwrap();
    let foreign = PrivateDirectory::open_or_create(temporary.path().join("foreign")).unwrap();
    let foreign_child = foreign.create_child("inputs").unwrap();
    foreign_child
        .write_atomic("first", b"first", PublishMode::CreateNew)
        .unwrap();
    let mut unrelated = CapturedDirectory::new(foreign_child, None).unwrap();
    unrelated
        .capture("first", 5, ConfigFileAccess::Private, &mut total)
        .unwrap();
    generation.read_tree_scope(|tree| {
        std::fs::set_permissions(generation.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(image.directories[GENERATION].revalidate_in_tree(tree).is_err());
        captured.revalidate_in_tree(tree)?;
        captured.compare_inputs_in_tree(tree)?;
        std::fs::set_permissions(generation.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(reopened.compare_inputs_in_tree(tree), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        std::fs::set_permissions(generation.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(foreign.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(unrelated.compare_inputs_in_tree(tree), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        std::fs::set_permissions(foreign.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(captured.directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(captured.compare_inputs_in_tree(tree), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        std::fs::set_permissions(captured.directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = captured.directory.path().join("second");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(captured.inputs[1].access, ConfigFileAccess::Public);
        assert!(matches!(captured.compare_inputs(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        assert!(matches!(captured.compare_inputs_in_tree(tree), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        captured.compare_inputs_in_tree(tree)?;
        Ok::<_, Error>(())
    }).unwrap();
    image.revalidate().unwrap();
}

#[test]
fn scoped_profile_preserves_forward_inventory_and_ordered_input_refusals() {
    let (_temporary, mut image) = small();
    let root = &image.directories[GENERATION].directory;
    let child = root.create_child("later").unwrap();
    child
        .write_atomic("third", b"third", PublishMode::CreateNew)
        .unwrap();
    let mut later = CapturedDirectory::new(child, Some(vec![OsString::from("third")])).unwrap();
    later
        .capture("third", 5, ConfigFileAccess::Private, &mut 0)
        .unwrap();
    image.directories.push(later);
    let compare = |image: &CapturedProfile| {
        let original = image.original_three_pass().unwrap_err();
        let scoped = image.revalidate().unwrap_err();
        assert_eq!(original.to_string(), scoped.to_string());
        scoped
    };
    image.original_three_pass().unwrap();
    image.revalidate().unwrap();
    let first = &image.directories[1];
    first
        .directory
        .write_atomic("first", b"other", PublishMode::Replace)
        .unwrap();
    std::fs::remove_file(first.directory.path().join("second")).unwrap();
    let later = &image.directories[2];
    std::fs::rename(
        later.directory.path().join("third"),
        later.directory.path().join("other"),
    )
    .unwrap();
    assert!(
        matches!(compare(&image), Error::Invalid(message) if message == "original service directory inventory differs")
    );
    std::fs::rename(
        later.directory.path().join("other"),
        later.directory.path().join("third"),
    )
    .unwrap();
    assert!(
        matches!(compare(&image), Error::Invalid(message) if message == "retained service profile input custody differs")
    );
    image.directories[1].inputs.swap(0, 1);
    assert!(matches!(compare(&image), Error::Io(error) if error.kind() == io::ErrorKind::NotFound));
    image.directories[1]
        .directory
        .write_atomic("second", b"second", PublishMode::CreateNew)
        .unwrap();
    image.directories[1]
        .directory
        .write_atomic("first", b"first", PublishMode::Replace)
        .unwrap();
    image.original_three_pass().unwrap();
    image.revalidate().unwrap();
}

#[cfg(unix)]
#[test]
fn scoped_profile_closes_original_descendant_on_error_and_refuses_identical_replacement() {
    let (_temporary, image) = small();
    let (generation, descendants) = image.directories.split_first().unwrap();
    let captured = &descendants[0].directory;
    let name = captured.path();
    let moved = name.with_file_name("original");
    let result = generation
        .directory
        .read_tree_scope_with_root(|tree, root| {
            CapturedProfile::revalidate_directory(generation, descendants, tree, root, &mut || {
                std::fs::rename(name, &moved)?;
                let replacement = generation.directory.create_child("inputs")?;
                replacement.write_atomic("first", b"first", PublishMode::CreateNew)?;
                replacement.write_atomic("second", b"second", PublishMode::CreateNew)?;
                Err(invalid())
            })
        });
    assert!(matches!(result, Err(Error::Io(_))));
    assert!(image.revalidate().is_err());
    assert!(image.original_three_pass().is_err());
    std::fs::remove_dir_all(name).unwrap();
    std::fs::rename(&moved, name).unwrap();
    image.original_three_pass().unwrap();
    image.revalidate().unwrap();
}
