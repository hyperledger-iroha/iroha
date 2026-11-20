//! Four disposable Sora validators, independent of production testnets and of the child store.

use super::*;

fn direct_client(bytes: &[u8], port: u16) -> TestResult<zeroize::Zeroizing<String>> {
    let mut table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
        std::str::from_utf8(bytes)?,
        "generated loopback parent client",
    )?);
    let origin = format!("http://127.0.0.1:{port}/");
    ensure!(
        table.get("torii_url").and_then(toml::Value::as_str) == Some(origin.as_str()),
        "generated parent client differs from the reserved loopback listener"
    );
    table.insert("http_transport_policy".into(), "direct_loopback".into());
    Ok(zeroize::Zeroizing::new(toml::to_string(&*table)?))
}

impl Harness {
    pub(super) fn start_loopback_parent(&mut self) -> TestResult<PreparedLocalnet> {
        let ports = LocalnetPorts::reserve()?;
        let deadline = Instant::now() + COMMAND_TIMEOUT;
        let mut generator = Command::new(&self.kagami)
            .args([
                "sora-network",
                "generate",
                "--profile",
                "nexus",
                "--peers",
                "4",
                "--bind-host",
                "127.0.0.1",
                "--public-host",
                "127.0.0.1",
                "--chain-id",
                "mochi-loopback",
                "--chain-discriminant",
                "753",
                "--base-api-port",
            ])
            .arg(ports.base_api.to_string())
            .arg("--base-p2p-port")
            .arg(ports.base_p2p.to_string())
            .arg("--out-dir")
            .arg(self.parent.path())
            .current_dir(self.workspace.path())
            .env("PATH", self.root.path().join("empty-path"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(self.root.open_append("parent-generation.stderr")?)
            .spawn()?;
        loop {
            if let Some(exit) = generator.try_wait()? {
                ensure!(exit.success(), "loopback parent generation failed: {exit}");
                break;
            }
            if Instant::now() >= deadline {
                generator.kill()?;
                generator.wait()?;
                return Err(eyre!("loopback parent generation timed out"));
            }
            thread::sleep(Duration::from_millis(50));
        }
        let client_path = self.parent.path().join("client.toml");
        let bytes = self.parent.read("client.toml", MAX_METADATA)?;
        let encoded = direct_client(&bytes, ports.base_api)?;
        self.parent
            .write_atomic("client.toml", encoded.as_bytes(), PublishMode::Replace)?;
        let (config, _) =
            Config::load_bytes_with_musubi_publication(&client_path, encoded.as_bytes())
                .map_err(|_| eyre!("invalid generated parent client"))?;
        let peers = (0..4_u16)
            .map(|index| ManagedPeer {
                config_path: self.parent.path().join(format!("peer{index}.toml")),
                torii_url: format!("http://127.0.0.1:{}/", ports.base_api + index),
                log_name: format!("peer{index}.log"),
            })
            .collect::<Vec<_>>();
        let prepared = PreparedLocalnet {
            service_profile: crate::localnet::LocalnetServiceProfile::Standard,
            context: ManagedContext {
                name: "parent".into(),
                chain_id: config.chain.to_string(),
                network_id: config.network_id.to_string(),
                account_id: config
                    .account
                    .to_i105_for_discriminant(config.account_chain_discriminant)?,
                dataspace_id: 0,
                dataspace_alias: "universal".into(),
                torii_url: peers[0].torii_url.clone(),
                client_config: client_path,
            },
            peers,
        };
        prepared.context.load_client_config()?;
        listeners::expected_addresses(&prepared)?;
        let daemon = self
            .kagami
            .with_file_name(format!("iroha3d{}", std::env::consts::EXE_SUFFIX));
        drop(ports);
        for (index, peer) in prepared.peers.iter().enumerate() {
            let log = self.parent.open_append(&peer.log_name)?;
            self.parent_peers.spawn(
                Command::new(&daemon)
                    .args(["--sora", "--sumeragi-assert-fresh-key", "--config"])
                    .arg(&peer.config_path)
                    .current_dir(self.parent.path().join(format!("state/peer{index}")))
                    .stdin(Stdio::null())
                    .stdout(log.try_clone()?)
                    .stderr(log),
            )?;
        }
        self.parent_peers.prove_ready(&prepared)?;
        Ok(prepared)
    }
}

#[test]
fn direct_client_requires_the_original_loopback_endpoint_and_preserves_identity() {
    let source = b"torii_url = 'http://127.0.0.1:9010/'\nchain = 'fixture'\nhttp_transport_policy = 'ordinary'";
    let encoded = direct_client(source, 9010).unwrap();
    let table: toml::Table = encoded.parse().unwrap();
    assert_eq!(table["chain"].as_str(), Some("fixture"));
    assert_eq!(
        table["http_transport_policy"].as_str(),
        Some("direct_loopback")
    );
    assert!(direct_client(source, 9011).is_err());
    assert!(direct_client(b"torii_url = 'https://example.invalid/'", 9010).is_err());
}
