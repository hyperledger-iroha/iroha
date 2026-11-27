//! Interactive native Mochi attachment; only the operator initiates attachment and stop.

#![cfg(test)]

use super::*;

impl Harness {
    fn desktop_attachment(&mut self, installed: &Path) -> TestResult<()> {
        let mut parent = self.provision_parent()?;
        let layout = NativeBundleLayout::MacOs;
        let binaries =
            fixture_bundle_directory(&self.root, &layout.runtime_directory(self.root.path()))?;
        let resources =
            fixture_bundle_directory(&self.root, &layout.resources_directory(self.root.path()))?;
        let contents = self.root.open_child("Mochi.app")?.open_child("Contents")?;
        contents.write_atomic(
            "Info.plist",
            macos_info_plist("0.1.0")?.as_bytes(),
            PublishMode::CreateNew,
        )?;
        for name in ["mochi", "iroha3d", "kagami"] {
            let source = installed.join(name);
            let expected = store::pin_binary(&source)?;
            let target = binaries.path().join(name);
            fs::copy(source, &target)?;
            ensure!(
                store::pin_binary(&target)?.blake3 == expected.blake3,
                "desktop program changed during staging"
            );
        }
        resources.write_atomic(
            crate::bootstrap::NETWORK_PROFILES_FILENAME,
            &parent.profiles.encode_installation()?,
            PublishMode::CreateNew,
        )?;
        let home = self.root.create_child("desktop-home")?;
        let state = workspace_state_root(
            &home
                .path()
                .join("Library/Application Support/Iroha/developer"),
            self.workspace.path(),
        )?;
        self.child = ManagedStore::open(&state)?;
        let mut window = Command::new(binaries.path().join("mochi"))
            .arg("--workspace")
            .arg(self.workspace.path())
            .env("HOME", home.path())
            .current_dir(self.workspace.path())
            .stdin(Stdio::null())
            .stdout(self.root.open_append("desktop.stdout")?)
            .stderr(self.root.open_append("desktop.stderr")?)
            .spawn()?;
        eprintln!(
            "ATTACHMENT_DESKTOP_WINDOW={} parent=fixture alias=dpn",
            self.root.path().join("Mochi.app").display()
        );
        let result = self.observe_desktop(&mut window, &mut parent);
        if window.try_wait()?.is_none() {
            window.kill()?;
            window.wait()?;
        }
        result
    }

    fn observe_desktop(
        &mut self,
        window: &mut Child,
        parent: &mut LoopbackParent,
    ) -> TestResult<()> {
        let deadline = Instant::now() + Duration::from_secs(900);
        let status = loop {
            ensure!(
                Instant::now() < deadline && window.try_wait()?.is_none(),
                "desktop closed or exceeded the operator deadline before attachment"
            );
            if let Ok(selected) = self.child.context(None)
                && selected.name == "dpn"
                && let Some(status) = self.child.dataspace_status("dpn")?
                && status.local.context == selected
                && require_attached(&status).is_ok()
            {
                break status;
            }
            thread::sleep(Duration::from_secs(1));
        };
        let prepared = self.child.prepared("dpn")?;
        let proof = Self::verify_record(
            &parent.prepared,
            &parent.parent_config,
            &parent.node_peers,
            &mut parent.verifier,
            &status,
        )?;
        let registration = prepared.load_private_registration()?;
        ensure!(
            proof.record.anchor.registration() == &registration
                && proof.record.owner == prepared.context.load_client_config()?.account
                && prepared.context == status.local.context
                && registration.child_network_id.to_string() == prepared.context.network_id
                && status.attachment.network == "fixture",
            "desktop selection differs from parent registration"
        );
        let confirmed = status.attachment.parent_confirmed.unwrap();
        eprintln!(
            "ATTACHMENT_DESKTOP_VERIFIED parent={} child={} chain={} torii={} child_height={} parent_height={}",
            parent.anchor.network_id,
            prepared.context.network_id,
            prepared.context.chain_id,
            prepared.context.torii_url,
            confirmed.child.height,
            confirmed.parent_height
        );
        let mut listeners = super::super::super::listeners::expected_addresses(&parent.prepared)?;
        listeners.extend(super::super::super::listeners::expected_addresses(
            &prepared,
        )?);
        for root in &self
            .ingress
            .as_ref()
            .ok_or_else(|| eyre!("missing fixture ingress"))?
            .roots
        {
            let url = url::Url::parse(root)?;
            listeners.push(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                url.port().ok_or_else(|| eyre!("missing ingress port"))?,
            )));
        }
        while window.try_wait()?.is_none() {
            ensure!(
                Instant::now() < deadline,
                "operator deadline: stop dpn, then close the Mochi window"
            );
            thread::sleep(Duration::from_secs(1));
        }
        let stopped = self.child.status("dpn")?;
        ensure!(
            stopped.phase == ManagedPhase::Stopped
                && stopped.running_peers == 0
                && stopped.context == prepared.context,
            "desktop did not stop the original child before exiting"
        );
        self.stop()?;
        ensure!(
            listeners.len() == 20,
            "unexpected number of fixture listeners"
        );
        for endpoint in listeners {
            ensure!(
                std::net::TcpStream::connect_timeout(&endpoint, Duration::from_secs(1))
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::ConnectionRefused),
                "shutdown not confirmed for {endpoint}"
            );
        }
        self.verified = true;
        eprintln!("ATTACHMENT_DESKTOP_STOPPED child=0 parent=0 ingress=0 closed_listeners=20");
        Ok(())
    }
}

#[test]
#[ignore = "interactive macOS desktop: attach dpn to fixture, inspect the window, stop dpn and quit"]
fn installed_mochi_window_attachment() {
    let _resources = crate::managed::native_test_guard();
    let installed = PathBuf::from(
        std::env::var_os("IROHA_TEST_RUNTIME_DIRECTORY")
            .expect("set matching native runtime directory"),
    );
    let mut harness = Harness::new(&installed).unwrap();
    let result = harness.desktop_attachment(&installed);
    harness
        .stop()
        .expect("clean shutdown after desktop session");
    result.expect("verified native desktop attachment and UI-initiated stop");
    harness.remove_verified_custody().unwrap();
}
