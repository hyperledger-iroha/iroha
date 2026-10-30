//! Design previews rendered headlessly from the real desktop state machine.
//!
//! The harness runs the genuine Quickstart against real validators (dispatch, working state and
//! its exact outcome), then renders every running-network view on that real generation with
//! staged phases and fixture observations, and rasterizes egui's output in software. Signed
//! previews and validator logs are genuine; balances, blocks and activity are presentation
//! fixtures and assert nothing about ledger state. It needs matching `kagami` and `iroha3d`:
//!
//! ```sh
//! MOCHI_PREVIEW_RUNTIME=target/release MOCHI_PREVIEW_OUT=/tmp/mochi-previews \
//!   cargo test -p mochi-ui --features gui --bin mochi design_previews -- --ignored --nocapture
//! ```
//!
//! Each view is written as a binary PPM image; the generation is stopped afterwards.
//! `live_walkthrough_with_a_real_generation` uses the same variables and writes to `live/`: it
//! drives fee-quoted quick actions, an automatically approved deployment, the call card and
//! the Resume list through genuine validators, with no fixture observations.

use super::*;
use eframe::egui::{Color32, ColorImage, Pos2, Rect, TextureId, Theme, Vec2, epaint::Primitive};
use iroha_deploy::managed::InstalledRuntime;
use std::{collections::HashMap, io::Write as _, path::Path};

/// Software rasterizer for egui meshes, matching the premultiplied gamma-space blending of
/// the native renderer closely enough for design review.
struct Renderer {
    context: egui::Context,
    textures: HashMap<TextureId, ColorImage>,
    size: Vec2,
    pixels_per_point: f32,
    time: f64,
}

impl Renderer {
    fn new(theme: Theme) -> Self {
        let context = egui::Context::default();
        theme::install(&context);
        theme::install_fonts(&context);
        context.set_theme(theme);
        Self {
            context,
            textures: HashMap::new(),
            size: Vec2::new(1280.0, 840.0),
            pixels_per_point: 2.0,
            time: 100.0,
        }
    }

    fn frame(&mut self, desktop: &mut Desktop) -> egui::FullOutput {
        self.time += 0.1;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.pixels_per_point);
        let mut output = self.context.run_ui(input, |context| desktop.show(context));
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                match delta.pos {
                    None => {
                        self.textures.insert(*id, (**image).clone());
                    }
                    Some([x, y]) => {
                        let target = self.textures.get_mut(id).expect("patched texture exists");
                        for row in 0..image.size[1] {
                            for column in 0..image.size[0] {
                                target.pixels[(y + row) * target.size[0] + x + column] =
                                    image.pixels[row * image.size[0] + column];
                            }
                        }
                    }
                }
            }
        }
        for id in &output.textures_delta.free {
            self.textures.remove(id);
        }
        // Every update above has been applied to the software renderer's texture store.
        output.textures_delta.clear();
        output
    }

    /// Render a few frames so layout settles, then write the last one.
    fn capture(&mut self, desktop: &mut Desktop, path: &Path) {
        for _ in 0..3 {
            let _ = self.frame(desktop);
        }
        let output = self.frame(desktop);
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let width = (self.size.x * self.pixels_per_point) as usize;
        let height = (self.size.y * self.pixels_per_point) as usize;
        let mut pixels = vec![[0.0_f32; 4]; width * height];
        for primitive in &primitives {
            let Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            let clip = Rect::from_min_max(
                (primitive.clip_rect.min.to_vec2() * output.pixels_per_point).to_pos2(),
                (primitive.clip_rect.max.to_vec2() * output.pixels_per_point).to_pos2(),
            );
            let texture = self.textures.get(&mesh.texture_id);
            for triangle in mesh.indices.chunks_exact(3) {
                let vertices = [
                    &mesh.vertices[triangle[0] as usize],
                    &mesh.vertices[triangle[1] as usize],
                    &mesh.vertices[triangle[2] as usize],
                ];
                rasterize(
                    &mut pixels,
                    width,
                    height,
                    clip,
                    output.pixels_per_point,
                    vertices,
                    texture,
                );
            }
        }
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        let bytes = pixels
            .iter()
            .flat_map(|pixel| {
                [0, 1, 2].map(|channel| (pixel[channel].clamp(0.0, 1.0) * 255.0).round() as u8)
            })
            .collect::<Vec<_>>();
        file.write_all(&bytes).unwrap();
        eprintln!("wrote {}", path.display());
    }
}

fn premultiplied(color: Color32) -> [f32; 4] {
    color.to_array().map(|channel| f32::from(channel) / 255.0)
}

fn sample(texture: &ColorImage, uv: Pos2) -> [f32; 4] {
    let [width, height] = texture.size;
    let x = (uv.x * width as f32 - 0.5).clamp(0.0, (width - 1) as f32);
    let y = (uv.y * height as f32 - 0.5).clamp(0.0, (height - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: usize, y: usize| premultiplied(texture.pixels[y * width + x]);
    let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    std::array::from_fn(|channel| {
        let top = a[channel] + (b[channel] - a[channel]) * fx;
        let bottom = c[channel] + (d[channel] - c[channel]) * fx;
        top + (bottom - top) * fy
    })
}

fn rasterize(
    pixels: &mut [[f32; 4]],
    width: usize,
    height: usize,
    clip: Rect,
    scale: f32,
    vertices: [&egui::epaint::Vertex; 3],
    texture: Option<&ColorImage>,
) {
    let points = vertices.map(|vertex| vertex.pos.to_vec2() * scale);
    let edge = |a: Vec2, b: Vec2, c: Vec2| (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x);
    let area = edge(points[0], points[1], points[2]);
    if area.abs() < f32::EPSILON {
        return;
    }
    let min_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min)
        .max(clip.min.x)
        .max(0.0);
    let max_x = points
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max)
        .min(clip.max.x)
        .min(width as f32);
    let min_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min)
        .max(clip.min.y)
        .max(0.0);
    let max_y = points
        .iter()
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max)
        .min(clip.max.y)
        .min(height as f32);
    if min_x >= max_x || min_y >= max_y {
        return;
    }
    let colors = vertices.map(|vertex| premultiplied(vertex.color));
    for y in (min_y.floor() as usize)..(max_y.ceil() as usize).min(height) {
        for x in (min_x.floor() as usize)..(max_x.ceil() as usize).min(width) {
            let center = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if center.x < clip.min.x
                || center.x >= clip.max.x
                || center.y < clip.min.y
                || center.y >= clip.max.y
            {
                continue;
            }
            let weights = [
                edge(points[1], points[2], center) / area,
                edge(points[2], points[0], center) / area,
                edge(points[0], points[1], center) / area,
            ];
            if weights.iter().any(|weight| *weight < -1e-4) {
                continue;
            }
            let mut source: [f32; 4] = std::array::from_fn(|channel| {
                (0..3)
                    .map(|index| colors[index][channel] * weights[index])
                    .sum()
            });
            if let Some(texture) = texture {
                let uv = (0..3).fold(Vec2::ZERO, |sum, index| {
                    sum + vertices[index].uv.to_vec2() * weights[index]
                });
                let texel = sample(texture, uv.to_pos2());
                for channel in 0..4 {
                    source[channel] *= texel[channel];
                }
            }
            let target = &mut pixels[y * width + x];
            for channel in 0..4 {
                target[channel] = source[channel] + target[channel] * (1.0 - source[3]);
            }
        }
    }
}

#[test]
fn software_renderer_consumes_font_texture_updates() {
    let mut renderer = Renderer::new(Theme::Dark);
    let mut desktop = Desktop::model(PathBuf::from("unused"));
    for _ in 0..2 {
        let output = renderer.frame(&mut desktop);
        assert!(!output.shapes.is_empty());
        assert!(output.textures_delta.is_empty());
        assert!(
            renderer
                .textures
                .values()
                .any(|image| !image.pixels.is_empty())
        );
        assert!(!desktop.busy);
    }
}

fn wait_until(
    desktop: &mut Desktop,
    timeout: Duration,
    mut done: impl FnMut(&Desktop) -> bool,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        desktop.poll();
        if done(desktop) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Stops the genuine validators even when an assertion fails midway.
struct StopOnDrop(Option<(DeveloperWorkspace, ManagedNetwork)>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        if let Some((workspace, network)) = self.0.take() {
            let _ = workspace.stop(&network);
        }
    }
}

/// Retain the live walkthrough's diagnostics and attempt authenticated cleanup from launch onward.
struct LiveWalkthroughCleanup {
    temporary: Option<tempfile::TempDir>,
    workspace: DeveloperWorkspace,
    network: Option<ManagedNetwork>,
    stopped: bool,
}

impl LiveWalkthroughCleanup {
    fn stop(&mut self) -> UiResult<ManagedStatus> {
        // Before Ready, only this fresh private fixture can have created `local`.
        // Once captured, never resolve its name again during cleanup.
        if self.network.is_none() {
            self.network = Some(
                self.workspace
                    .network(Some("local"))
                    .map_err(|error| error.to_string())?,
            );
        }
        let status = self
            .workspace
            .stop(self.network.as_ref().expect("captured cleanup target"))
            .map_err(|error| error.to_string())?;
        if !matches!(status.phase, ManagedPhase::Stopped | ManagedPhase::Failed)
            || status.running_peers != 0
        {
            return Err(format!(
                "cleanup did not confirm zero owned validators: {status:?}"
            ));
        }
        self.stopped = true;
        eprintln!(
            "MOCHI_LIVE_STOPPED phase={:?} running_peers=0",
            status.phase
        );
        Ok(status)
    }
}

impl Drop for LiveWalkthroughCleanup {
    fn drop(&mut self) {
        if !self.stopped
            && let Err(error) = self.stop()
        {
            eprintln!("MOCHI_LIVE_CLEANUP_UNCONFIRMED: {error}");
        }
        if let Some(temporary) = self.temporary.take() {
            eprintln!("MOCHI_LIVE_DIAGNOSTICS={}", temporary.keep().display());
        }
    }
}

#[test]
fn live_walkthrough_cleanup_retains_diagnostics_before_a_generation_exists() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().to_path_buf();
    let project = path.join("project");
    let bin = path.join("bin");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&bin).unwrap();
    for name in ["kagami", "iroha3d"] {
        std::fs::copy(
            std::env::current_exe().unwrap(),
            bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
    }
    let workspace = DeveloperWorkspace::with_runtime(
        &path.join("state"),
        &project,
        InstalledRuntime::from_directory(&bin).unwrap(),
    )
    .unwrap();
    let mut cleanup = LiveWalkthroughCleanup {
        temporary: Some(temporary),
        workspace,
        network: None,
        stopped: false,
    };
    assert!(cleanup.stop().is_err());
    assert!(!cleanup.stopped);
    drop(cleanup);
    assert!(
        project.exists(),
        "unconfirmed cleanup retains original diagnostics"
    );
    std::fs::remove_dir_all(path).unwrap();
}

fn entry(
    title: &str,
    subtitle: &str,
    detail: &str,
    owner: Option<&str>,
    definition: Option<&str>,
    json: Option<String>,
) -> mochi_core::StateEntry {
    mochi_core::StateEntry {
        title: title.into(),
        subtitle: subtitle.into(),
        detail: detail.into(),
        raw: String::new(),
        domain: None,
        domain_lower: None,
        owner: owner.map(Into::into),
        owner_lower: owner.map(str::to_lowercase),
        asset_definition: definition.map(Into::into),
        asset_definition_lower: definition.map(str::to_lowercase),
        json,
        norito_bytes: None,
        search_blob: title.to_lowercase(),
    }
}

fn page(kind: StateQueryKind, entries: Vec<mochi_core::StateEntry>) -> StatePage {
    StatePage {
        kind,
        entries,
        remaining: 0,
        cursor: None,
    }
}

/// Presentation data for the running-network views, shaped like genuine observations and
/// built on the real generation's identities. It is never submitted or claimed as ledger state.
fn fixture_observations(desktop: &mut Desktop, network: &ManagedNetwork) {
    let discriminant = network.address_discriminant();
    let _guard = iroha_data_model::account::address::ChainDiscriminantGuard::enter(discriminant);
    let developer = network.prepared().context.account_id.clone();
    let definition = |seed: u8| {
        let mut bytes = [seed; 16];
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        iroha_data_model::asset::id::AssetDefinitionId::from_uuid_bytes(bytes)
            .unwrap()
            .to_string()
    };
    let (xor, matcha) = (definition(7), definition(42));
    let friends = (0..3)
        .map(|_| mochi_core::GeneratedAccount::generate().account_literal(discriminant))
        .collect::<Vec<_>>();
    let accounts = std::iter::once(developer.clone())
        .chain(friends.iter().cloned())
        .map(|account| {
            let json = format!("{{\n  \"id\": \"{account}\",\n  \"metadata\": {{}}\n}}");
            entry(
                &account,
                "Signatory ed0120…",
                "No metadata",
                None,
                None,
                Some(json),
            )
        })
        .collect::<Vec<_>>();
    let holding = |definition: &str, owner: &str, value: &str| {
        let asset = mochi_core::asset_literal(definition, owner, discriminant).unwrap();
        entry(
            &asset,
            &format!("Owner {owner}"),
            &format!("Value {value}"),
            Some(owner),
            Some(definition),
            None,
        )
    };
    let holdings = vec![
        holding(&xor, &developer, "999750"),
        holding(&matcha, &developer, "5000"),
        holding(&xor, &friends[0], "250"),
        holding(&xor, &friends[1], "75.5"),
        holding(&matcha, &friends[1], "12"),
    ];
    let definitions = [(&xor, "XOR"), (&matcha, "MATCHA")]
        .into_iter()
        .map(|(id, name)| {
            let json = format!("{{\"id\": \"{id}\", \"name\": \"{name}\"}}");
            entry(id, "Owner genesis", "Numeric", None, Some(id), Some(json))
        })
        .collect::<Vec<_>>();
    desktop.ledger = Some(accounts::LedgerSnapshot::new(
        page(StateQueryKind::Accounts, accounts.clone()),
        page(StateQueryKind::Assets, holdings),
        &page(StateQueryKind::AssetDefinitions, definitions),
    ));
    desktop.page = Some(page(StateQueryKind::Accounts, accounts));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    for height in 1..=14_u64 {
        let hash = format!(
            "{:064x}",
            u128::from(height).wrapping_mul(0x9e37_79b9_7f4a_7c15_f39c_c060_5ced_c834)
        );
        let summary = mochi_core::BlockSummary {
            height,
            hash_hex: hash,
            transaction_count: if height == 1 {
                6
            } else {
                1 + (height as usize % 3)
            },
            rejected_transaction_count: usize::from(height == 9),
            time_trigger_count: 0,
            signature_count: 4,
            view_change_index: 0,
            creation_time_ms: now - (14 - height) * 41_000,
            is_genesis: height == 1,
        };
        ledger::record_block(
            &mut desktop.blocks_feed,
            &summary,
            None,
            1200 + height as usize * 37,
        );
        push_activity(
            &mut desktop.activity,
            format!(
                "Block #{} · {} transactions · {} rejected · {} bytes · {}",
                summary.height,
                summary.transaction_count,
                summary.rejected_transaction_count,
                1200 + height * 37,
                summary.hash_hex
            ),
        );
        if height % 4 == 0 {
            push_activity(
                &mut desktop.activity,
                format!("Pipeline · Transaction Approved · 412 bytes · block {height}"),
            );
            push_activity(
                &mut desktop.activity,
                format!("Data · Asset Added · 198 bytes · 250 of {xor}"),
            );
        }
    }
    push_activity(
        &mut desktop.activity,
        "Data · Account Created · 233 bytes · new developer test account".into(),
    );
    desktop.latest_height = Some(14);
    // Every validator reports block 14; reads are paused so nothing replaces the fixture.
    desktop.block_tips = home::BlockTips {
        observed: vec![fixture_tip(14); 4],
        settled: 14,
        pending: false,
        started: Some(Instant::now() + Duration::from_secs(3600)),
    };
}

/// A validator's newest block for the Home cards; the hash is a fixture, like the blocks.
fn fixture_tip(height: u64) -> home::TipObservation {
    use iroha_data_model::prelude::{Hash, HashOf};
    Ok(Some(mochi_core::state::BlockTip {
        height,
        hash: HashOf::from_untyped_unchecked(Hash::new(height.to_be_bytes())),
    }))
}

#[test]
#[ignore = "starts genuine validators; set MOCHI_PREVIEW_RUNTIME and MOCHI_PREVIEW_OUT"]
fn design_previews_with_a_real_generation() {
    let Some(runtime) = std::env::var_os("MOCHI_PREVIEW_RUNTIME") else {
        eprintln!("MOCHI_PREVIEW_RUNTIME is unset; nothing rendered");
        return;
    };
    let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("coffee-shop");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project.join("coffee.ko"),
        "seiyaku Coffee { view fn quote(int cups) authorize(anyone) -> int { return cups * 10; } }",
    )
    .unwrap();
    let workspace = DeveloperWorkspace::with_runtime(
        &temporary.path().join("state"),
        &project,
        InstalledRuntime::from_directory(Path::new(&runtime)).unwrap(),
    )
    .unwrap();
    let mut desktop = Desktop::model(project.clone());
    desktop.install_opened(inspect_workspace(workspace.clone()).unwrap());
    let mut light = Renderer::new(Theme::Light);
    desktop.brand = brand::Brand::load(&light.context);
    light.capture(&mut desktop, &out.join("01-welcome.ppm"));

    // The genuine Quickstart: dispatch, working state and its exact outcome.
    desktop.quickstart();
    assert!(desktop.busy, "Quickstart dispatched");
    light.capture(&mut desktop, &out.join("02-quickstart-working.ppm"));
    assert!(
        wait_until(&mut desktop, Duration::from_secs(120), |desktop| !desktop
            .busy),
        "localnet creation finished"
    );
    light.capture(&mut desktop, &out.join("03-quickstart-outcome.ppm"));
    let network = workspace.network(Some("local")).unwrap();
    let _stop = StopOnDrop(Some((workspace.clone(), network.clone())));
    let failure = desktop.error.take();

    // Running-network views on the real generation, with staged phases and fixture
    // observations. Automatic observation and streams are paused so nothing replaces them.
    let stage = |desktop: &mut Desktop, phase, failure: Option<String>| {
        let running: u8 = if phase == ManagedPhase::Ready { 4 } else { 0 };
        desktop.install_selection(Selection {
            network: network.clone(),
            phase,
            running: usize::from(running),
            validators: (0..running)
                .map(|index| ManagedValidatorStatus {
                    index,
                    state: mochi_core::developer::ManagedValidatorState::Running,
                })
                .collect(),
            failure,
            attachment: Ok(None),
        });
        desktop.blocks = None;
        desktop.events = None;
        desktop.last_poll = Instant::now() + Duration::from_secs(3600);
        desktop.last_ledger_refresh = Instant::now() + Duration::from_secs(3600);
    };
    stage(&mut desktop, ManagedPhase::Starting, None);
    light.capture(&mut desktop, &out.join("04-home-starting.ppm"));
    stage(&mut desktop, ManagedPhase::Ready, None);
    fixture_observations(&mut desktop, &network);
    light.capture(&mut desktop, &out.join("05-home.ppm"));
    // Validator 4 is still two blocks short of the height an earlier read saw.
    desktop.block_tips.observed[3] = fixture_tip(12);
    light.capture(&mut desktop, &out.join("05b-home-validator-behind.ppm"));
    desktop.block_tips.observed[3] = fixture_tip(14);
    desktop.view = View::Accounts;
    light.capture(&mut desktop, &out.join("06-accounts.ppm"));
    let developer = network.prepared().context.account_id.clone();
    let funding = desktop.ledger.as_ref().unwrap().holdings_of(&developer)[0]
        .asset
        .to_owned();
    desktop.quick.kind = accounts::QuickKind::NewAccount;
    desktop.quick.asset = funding;
    desktop.quick.fund = "250".into();
    desktop.quick.generated = Some(mochi_core::GeneratedAccount::generate());
    desktop.review_quick_action();
    // The validator quotes fees in the background before anything is signed.
    assert!(
        wait_until(&mut desktop, Duration::from_secs(60), |desktop| {
            desktop.preview.is_some() || desktop.error.is_some()
        }),
        "fee quote finished"
    );
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    assert!(desktop.preview.is_some(), "a genuine signed preview");
    light.capture(&mut desktop, &out.join("07-accounts-review.ppm"));
    desktop.view = View::Blocks;
    light.capture(&mut desktop, &out.join("08-blocks.ppm"));
    desktop.view = View::Activity;
    light.capture(&mut desktop, &out.join("09-activity.ppm"));
    desktop.view = View::Endpoints;
    light.capture(&mut desktop, &out.join("10-endpoints.ppm"));
    // Long pages once more at full height so their lower sections are reviewed too.
    light.size.y = 1500.0;
    light.capture(&mut desktop, &out.join("10b-endpoints-full.ppm"));
    desktop.view = View::Accounts;
    light.capture(&mut desktop, &out.join("07b-accounts-full.ppm"));
    light.size.y = 840.0;
    desktop.view = View::State;
    light.capture(&mut desktop, &out.join("11-state.ppm"));
    desktop.view = View::Composer;
    light.capture(&mut desktop, &out.join("12-transactions.ppm"));
    desktop.view = View::Contracts;
    desktop.contract_path = "coffee.ko".into();
    desktop.receipt = None;
    light.capture(&mut desktop, &out.join("13-contracts.ppm"));
    desktop.view = View::Logs;
    desktop.logs_source = Some(0);
    desktop.read_logs();
    assert!(wait_until(
        &mut desktop,
        Duration::from_secs(30),
        |desktop| !desktop.busy
    ));
    light.capture(&mut desktop, &out.join("14-logs.ppm"));

    let mut dark = Renderer::new(Theme::Dark);
    desktop.brand = brand::Brand::load(&dark.context);
    desktop.view = View::Home;
    dark.capture(&mut desktop, &out.join("15-home-dark.ppm"));
    desktop.view = View::Accounts;
    dark.capture(&mut desktop, &out.join("16-accounts-dark.ppm"));
    desktop.view = View::Endpoints;
    dark.capture(&mut desktop, &out.join("17-endpoints-dark.ppm"));

    desktop.brand = brand::Brand::load(&light.context);
    desktop.view = View::Home;
    stage(&mut desktop, ManagedPhase::Stopped, None);
    light.capture(&mut desktop, &out.join("18-home-stopped.ppm"));
    stage(&mut desktop, ManagedPhase::Failed, failure);
    light.capture(&mut desktop, &out.join("19-home-failed.ppm"));
}

/// A setup hook with a parameter, a mutable entrypoint and two views, so the call card's
/// prefill, Call and View all run against the genuine validators.
const LIVE_CONTRACT: &str = "seiyaku Coffee { state int served; hajimari(int start) { served = start; } kotoage fn brew(int cups) authorize(anyone) { served = served + cups; } view fn quote(int cups) authorize(anyone) -> int { return cups * 10; } view fn total() authorize(anyone) -> int { return served; } }";

/// Wait out background observation, as the disabled controls of a busy desktop would.
fn settle(desktop: &mut Desktop) {
    assert!(
        wait_until(desktop, Duration::from_secs(60), |desktop| !desktop.busy),
        "background work finished"
    );
}

/// Read every validator's newest block as Home does on arrival, until all of them report
/// the newest block the observed validator streamed.
fn fresh_block_tips(desktop: &mut Desktop) {
    assert!(
        wait_until(desktop, Duration::from_secs(30), |desktop| {
            let observed = &desktop.block_tips.observed;
            !observed.is_empty()
                && observed.iter().all(
                    |tip| matches!(tip, Ok(Some(tip)) if Some(tip.height) == desktop.latest_height),
                )
        }),
        "every validator reports the newest streamed block: {:?}",
        desktop.block_tips.observed
    );
}

/// Render `frames` more frames (0.1 s of animation each), then capture.
fn capture_after(renderer: &mut Renderer, desktop: &mut Desktop, frames: usize, path: &Path) {
    for _ in 0..frames {
        let _ = renderer.frame(desktop);
    }
    renderer.capture(desktop, path);
}

/// One View or Call through the call card's own action; returns its shown result.
fn run_call(desktop: &mut Desktop, entrypoint: &str, arguments: &str, mutate: bool) -> String {
    settle(desktop);
    desktop.call.entrypoint = entrypoint.into();
    desktop.call.arguments = arguments.into();
    desktop.run_contract(mutate);
    assert!(
        wait_until(desktop, Duration::from_secs(120), |desktop| desktop
            .call
            .output
            .is_some()),
        "{entrypoint} finished"
    );
    match desktop.call.output.clone() {
        Some(Ok(output)) => output,
        other => panic!("{entrypoint} failed: {other:?}"),
    }
}

#[test]
#[ignore = "starts genuine validators; set MOCHI_PREVIEW_RUNTIME and MOCHI_PREVIEW_OUT"]
fn live_walkthrough_with_a_real_generation() {
    let runtime = std::env::var_os("MOCHI_PREVIEW_RUNTIME").expect("MOCHI_PREVIEW_RUNTIME");
    let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"))
        .join("live");
    std::fs::create_dir_all(&out).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("coffee-shop");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("coffee.ko"), LIVE_CONTRACT).unwrap();
    let workspace = DeveloperWorkspace::with_runtime(
        &temporary.path().join("state"),
        &project,
        InstalledRuntime::from_directory(Path::new(&runtime)).unwrap(),
    )
    .unwrap();
    eprintln!("MOCHI_LIVE_STORE={}", workspace.state_root().display());
    let mut cleanup = LiveWalkthroughCleanup {
        temporary: Some(temporary),
        workspace: workspace.clone(),
        network: None,
        stopped: false,
    };
    let mut desktop = Desktop::model(project.clone());
    desktop.install_opened(inspect_workspace(workspace.clone()).unwrap());
    let mut light = Renderer::new(Theme::Light);
    desktop.brand = brand::Brand::load(&light.context);

    // Genuine Quickstart; the header and welcome card count its elapsed time.
    let started = Instant::now();
    desktop.quickstart();
    assert!(desktop.busy && desktop.foreground, "Quickstart dispatched");
    std::thread::sleep(Duration::from_secs(3));
    light.capture(&mut desktop, &out.join("01-quickstart-elapsed.ppm"));
    assert!(
        wait_until(&mut desktop, Duration::from_secs(180), |desktop| !desktop
            .busy),
        "localnet creation finished"
    );
    eprintln!("LIVE quickstart outcome after {:?}", started.elapsed());
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    assert!(desktop.ready(), "Quickstart selects the Ready localnet");
    let network = workspace.network(Some("local")).unwrap();
    cleanup.network = Some(network.clone());
    capture_after(
        &mut light,
        &mut desktop,
        8,
        &out.join("02-ready-petals.ppm"),
    );

    // Genuine observation: canonical streams and collection reads, no fixtures.
    assert!(
        wait_until(&mut desktop, Duration::from_secs(60), |desktop| {
            desktop.ledger.is_some() || desktop.ledger_error.is_some()
        }),
        "ledger read finished"
    );
    assert!(desktop.ledger_error.is_none(), "{:?}", desktop.ledger_error);
    // Home reads every genuine validator's newest block; none is out of sync.
    assert!(
        wait_until(&mut desktop, Duration::from_secs(30), |desktop| {
            !desktop.block_tips.observed.is_empty()
        }),
        "block tips read"
    );
    let tips = &desktop.block_tips;
    eprintln!("LIVE block tips: {:?}", tips.observed);
    let sync = home::tip_sync(&tips.observed, tips.settled);
    assert!(
        sync.iter().all(|sync| *sync == home::TipSync::Current),
        "{sync:?}"
    );
    let developer = network.prepared().context.account_id.clone();
    let ledger = desktop.ledger.as_ref().unwrap();
    let funding = ledger.holdings_of(&developer).first().map(|holding| {
        eprintln!(
            "LIVE ledger: {} accounts; developer holds {} {}",
            ledger.accounts.entries.len(),
            holding.value,
            ledger.asset_label(holding.definition)
        );
        holding.asset.to_owned()
    });
    let funding = funding.expect("the generated developer account is funded");
    settle(&mut desktop);
    light.capture(&mut desktop, &out.join("03-home-live.ppm"));

    // A quick action previews with the validator's fee quote, then commits.
    desktop.view = View::Accounts;
    desktop.quick.kind = accounts::QuickKind::NewAccount;
    desktop.quick.asset = funding;
    desktop.quick.fund = "25".into();
    desktop.quick.generated = Some(mochi_core::GeneratedAccount::generate());
    desktop.review_quick_action();
    assert!(
        wait_until(&mut desktop, Duration::from_secs(60), |desktop| {
            desktop.preview.is_some() || desktop.error.is_some()
        }),
        "fee quote finished"
    );
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    let fees = desktop.preview.as_ref().unwrap().fees().to_vec();
    eprintln!("LIVE quoted fees: {fees:?}");
    assert!(!fees.is_empty(), "the observed validator quoted fees");
    // Tall enough to show the review card and its signed fee limits.
    light.size.y = 1500.0;
    light.capture(&mut desktop, &out.join("04-accounts-quoted-review.ppm"));
    light.size.y = 840.0;
    let height = desktop.latest_height;
    let outcome = submit_and_settle(&mut desktop);
    assert!(
        matches!(outcome, Ok(SubmissionOutcome::Applied { .. })),
        "{outcome:?}"
    );
    eprintln!("LIVE submission: {:?}", desktop.notice);
    assert!(
        wait_until(&mut desktop, Duration::from_secs(60), |desktop| desktop
            .latest_height
            > height),
        "the committed block reached the stream"
    );
    let newest = desktop.blocks_feed.front().unwrap();
    eprintln!(
        "LIVE newest block #{}: {} transactions, {} rejected",
        newest.height, newest.transactions, newest.rejected
    );
    assert_eq!(newest.rejected, 0, "the quoted transaction applied");
    desktop.view = View::Home;
    settle(&mut desktop);
    fresh_block_tips(&mut desktop);
    light.capture(&mut desktop, &out.join("05-home-pulse.ppm"));

    // Automatic approval: the deployment dispatches without the dialog and Activity says why.
    desktop.view = View::Contracts;
    desktop.contract_path = "coffee.ko".into();
    desktop.auto_approve = true;
    desktop.auto_approve_cap = "1000".into();
    settle(&mut desktop);
    desktop.deploy(false);
    assert!(
        wait_until(&mut desktop, Duration::from_secs(300), |desktop| {
            desktop.receipt.is_some() || desktop.error.is_some() || desktop.review.is_some()
        }),
        "deployment finished"
    );
    if let Some(review) = &desktop.review {
        panic!("automatic approval still asked: {:?}", review.summary);
    }
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    let approval = desktop
        .activity
        .iter()
        .find(|line| line.contains("automatically"))
        .cloned();
    eprintln!("LIVE {approval:?}");
    assert!(
        approval.is_some(),
        "Activity records the automatic approval"
    );
    capture_after(
        &mut light,
        &mut desktop,
        8,
        &out.join("06-deployed-petals.ppm"),
    );

    // The call card is filled in from the deployment's alias and declared setup hook.
    eprintln!(
        "LIVE call card: {} {} {}",
        desktop.call.alias, desktop.call.entrypoint, desktop.call.arguments
    );
    assert!(!desktop.call.alias.is_empty());
    assert_eq!(desktop.call.entrypoint, "hajimari");
    assert_eq!(desktop.call.arguments, "{\"start\": \"0\"}");
    light.size.y = 1500.0;
    light.capture(&mut desktop, &out.join("07-call-card-prefilled.ppm"));
    eprintln!(
        "LIVE hajimari: {}",
        run_call(&mut desktop, "hajimari", "{\"start\": \"2\"}", true)
    );
    eprintln!(
        "LIVE brew: {}",
        run_call(&mut desktop, "brew", "{\"cups\": \"3\"}", true)
    );
    let total = run_call(&mut desktop, "total", "{}", false);
    assert!(total.contains('5'), "2 + 3 cups served: {total}");
    let quote = run_call(&mut desktop, "quote", "{\"cups\": \"3\"}", false);
    assert!(quote.contains("30"), "{quote}");
    light.capture(&mut desktop, &out.join("08-call-card-view.ppm"));

    // The Resume card lists the saved deployment without a typed path.
    settle(&mut desktop);
    desktop.saved_deployments = None;
    let _ = light.frame(&mut desktop);
    assert!(
        wait_until(&mut desktop, Duration::from_secs(30), |desktop| desktop
            .saved_deployments
            .is_some()),
        "saved deployments read"
    );
    let saved = desktop.saved_deployments.clone().unwrap().unwrap();
    eprintln!("LIVE saved deployments: {saved:?}");
    assert_eq!(saved.len(), 1);
    assert!(saved[0].has_receipt);
    desktop.journal_path = saved[0].journal.display().to_string();
    light.capture(&mut desktop, &out.join("09-resume-picker.ppm"));
    light.size.y = 840.0;

    // Every State listing reads through a path the node admits, with genuine rows.
    desktop.view = View::State;
    for kind in StateQueryKind::all() {
        settle(&mut desktop);
        desktop.query = kind;
        desktop.page = None;
        desktop.fetch_state(false);
        assert!(
            wait_until(&mut desktop, Duration::from_secs(30), |desktop| {
                desktop.page.is_some() || desktop.error.is_some()
            }),
            "{kind:?} read finished"
        );
        assert!(desktop.error.is_none(), "{kind:?}: {:?}", desktop.error);
        let page = desktop.page.as_ref().unwrap();
        eprintln!(
            "LIVE state {kind:?}: {} entries, {} remaining, more: {}",
            page.entries.len(),
            page.remaining,
            page.cursor.is_some()
        );
        assert!(!page.entries.is_empty(), "{kind:?} lists genuine state");
    }
    light.capture(&mut desktop, &out.join("09b-state-peers.ppm"));

    // The wider live chain pulse over the genuine blocks, then Home in both themes.
    desktop.view = View::Blocks;
    light.capture(&mut desktop, &out.join("10-blocks-live.ppm"));
    desktop.view = View::Home;
    fresh_block_tips(&mut desktop);
    light.capture(&mut desktop, &out.join("11-home-final.ppm"));
    let mut dark = Renderer::new(Theme::Dark);
    desktop.brand = brand::Brand::load(&dark.context);
    dark.capture(&mut desktop, &out.join("12-home-final-dark.ppm"));
    drop(desktop);
    let stopped = cleanup
        .stop()
        .expect("live walkthrough cleanup must be authenticated");
    assert_eq!(stopped.phase, ManagedPhase::Stopped);
    assert_eq!(stopped.running_peers, 0);
}

/// Review the selected quick action through the observed validator's fee quote.
fn review_quick(desktop: &mut Desktop) {
    settle(desktop);
    desktop.review_quick_action();
    assert!(
        wait_until(desktop, Duration::from_secs(60), |desktop| {
            desktop.preview.is_some() || desktop.error.is_some()
        }),
        "review finished"
    );
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
}

/// Submit the reviewed transaction once and wait for the network to settle it.
fn submit_and_settle(desktop: &mut Desktop) -> UiResult<SubmissionOutcome> {
    settle(desktop);
    desktop.submit_reviewed();
    assert!(
        wait_until(desktop, Duration::from_secs(90), |desktop| desktop
            .submission
            .as_ref()
            .is_some_and(|submission| submission.outcome.is_some())),
        "submission settled"
    );
    desktop
        .submission
        .as_ref()
        .and_then(|submission| submission.outcome.clone())
        .expect("settled outcome")
}

/// Wait until the developer's observed balance of `definition` reads `value`.
fn wait_for_balance(desktop: &mut Desktop, developer: &str, definition: &str, value: &str) {
    assert!(
        wait_until(desktop, Duration::from_secs(60), |desktop| {
            desktop.ledger.as_ref().is_some_and(|ledger| {
                ledger
                    .holdings_of(developer)
                    .iter()
                    .any(|holding| holding.definition == definition && holding.value == value)
            })
        }),
        "the developer holds {value} of {definition}"
    );
}

#[test]
#[ignore = "starts genuine validators; set MOCHI_PREVIEW_RUNTIME and MOCHI_PREVIEW_OUT"]
fn live_mint_walkthrough_with_a_real_generation() {
    let Some(runtime) = std::env::var_os("MOCHI_PREVIEW_RUNTIME") else {
        eprintln!("MOCHI_PREVIEW_RUNTIME is unset; nothing rendered");
        return;
    };
    let out = PathBuf::from(std::env::var_os("MOCHI_PREVIEW_OUT").expect("MOCHI_PREVIEW_OUT"))
        .join("mint");
    std::fs::create_dir_all(&out).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("mint-shop");
    std::fs::create_dir(&project).unwrap();
    let workspace = DeveloperWorkspace::with_runtime(
        &temporary.path().join("state"),
        &project,
        InstalledRuntime::from_directory(Path::new(&runtime)).unwrap(),
    )
    .unwrap();
    let mut desktop = Desktop::model(project.clone());
    desktop.install_opened(inspect_workspace(workspace.clone()).unwrap());
    let mut light = Renderer::new(Theme::Light);
    desktop.brand = brand::Brand::load(&light.context);
    desktop.quickstart();
    assert!(
        wait_until(&mut desktop, Duration::from_secs(180), |desktop| !desktop
            .busy),
        "localnet creation finished"
    );
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    assert!(desktop.ready(), "Quickstart selects the Ready localnet");
    let network = workspace.network(Some("local")).unwrap();
    let _stop = StopOnDrop(Some((workspace.clone(), network.clone())));
    assert!(
        wait_until(&mut desktop, Duration::from_secs(60), |desktop| {
            desktop.ledger.is_some() || desktop.ledger_error.is_some()
        }),
        "ledger read finished"
    );
    assert!(desktop.ledger_error.is_none(), "{:?}", desktop.ledger_error);
    let developer = network.prepared().context.account_id.clone();
    assert!(
        !desktop.owns_mintable_token(),
        "a fresh developer owns no token, so Home's Mint opens New token"
    );
    desktop.view = View::Accounts;
    light.size.y = 1500.0;

    // Genesis owns XOR: the form warns before review and the network says why it refuses.
    let xor = {
        let ledger = desktop.ledger.as_ref().unwrap();
        let xor = ledger
            .definitions_for(&developer)
            .into_iter()
            .find(|definition| ledger.asset_label(definition) == "XOR")
            .expect("the fee asset is listed")
            .to_owned();
        assert_eq!(
            ledger.mint_right(&xor, &developer),
            accounts::MintRight::NotOwner
        );
        xor
    };
    desktop.quick.kind = accounts::QuickKind::Mint;
    desktop.quick.definition = xor.clone();
    desktop.quick.quantity = "100".into();
    settle(&mut desktop);
    light.capture(&mut desktop, &out.join("01-mint-xor-warning.ppm"));
    review_quick(&mut desktop);
    let fees = desktop.preview.as_ref().unwrap().fees().to_vec();
    eprintln!("LIVE quoted fees: {fees:?}");
    let xor_balance = |desktop: &Desktop| {
        desktop.ledger.as_ref().and_then(|ledger| {
            ledger
                .holdings_of(&developer)
                .iter()
                .find(|holding| holding.definition == xor)
                .map(|holding| holding.value.to_owned())
        })
    };
    let before = xor_balance(&desktop);
    match submit_and_settle(&mut desktop) {
        Ok(SubmissionOutcome::Rejected { reason }) => {
            eprintln!("LIVE XOR mint rejected: {reason}");
            assert!(
                reason.contains("Can't mint an asset without owning its definition"),
                "{reason}"
            );
        }
        other => panic!("minting genesis-owned XOR must be rejected: {other:?}"),
    }
    assert!(desktop.error.is_none(), "{:?}", desktop.error);
    light.capture(&mut desktop, &out.join("02-mint-xor-rejected.ppm"));
    settle(&mut desktop);
    wait_until(&mut desktop, Duration::from_secs(10), |desktop| {
        xor_balance(desktop) != before
    });
    eprintln!(
        "LIVE developer XOR around the rejection: {before:?} -> {:?}",
        xor_balance(&desktop)
    );

    // New token: lease coffee.universal, register Coffee there and mint the first supply.
    desktop.preview = None;
    desktop.submission = None;
    desktop.quick.kind = accounts::QuickKind::NewToken;
    desktop.quick.token.name = "Coffee".into();
    desktop.quick.token.label = "coffee".into();
    desktop.quick.token.supply = "1000".into();
    review_quick(&mut desktop);
    let preview = desktop.preview.clone().unwrap();
    eprintln!(
        "LIVE token review: lease {:?}; {:?}",
        preview.lease(),
        preview.summaries()
    );
    assert!(
        preview.lease().is_some_and(|lease| lease.amount.is_some()),
        "a fresh domain is leased at a quoted price"
    );
    assert_eq!(preview.instructions().len(), 3, "lease, registration, mint");
    light.capture(&mut desktop, &out.join("03-new-token-review.ppm"));
    let outcome = submit_and_settle(&mut desktop);
    assert!(
        matches!(outcome, Ok(SubmissionOutcome::Applied { .. })),
        "{outcome:?}"
    );
    assert_eq!(
        desktop.quick.kind,
        accounts::QuickKind::Mint,
        "the new token is the one to mint next"
    );
    let coffee = desktop.quick.definition.clone();
    wait_for_balance(&mut desktop, &developer, &coffee, "1000");
    assert_eq!(
        desktop
            .ledger
            .as_ref()
            .unwrap()
            .mint_right(&coffee, &developer),
        accounts::MintRight::Owner
    );
    assert!(desktop.owns_mintable_token());
    light.capture(&mut desktop, &out.join("04-new-token-applied.ppm"));

    // As its owner, the developer mints more.
    desktop.preview = None;
    desktop.submission = None;
    desktop.quick.recipient.clear();
    desktop.quick.quantity = "250".into();
    review_quick(&mut desktop);
    let outcome = submit_and_settle(&mut desktop);
    assert!(
        matches!(outcome, Ok(SubmissionOutcome::Applied { .. })),
        "{outcome:?}"
    );
    wait_for_balance(&mut desktop, &developer, &coffee, "1250");
    light.capture(&mut desktop, &out.join("05-mint-coffee-applied.ppm"));
    let mut dark = Renderer::new(Theme::Dark);
    dark.size.y = 1500.0;
    desktop.brand = brand::Brand::load(&dark.context);
    dark.capture(&mut desktop, &out.join("06-mint-coffee-applied-dark.ppm"));

    // Creating the same token again is refused before review.
    desktop.preview = None;
    desktop.submission = None;
    desktop.quick.kind = accounts::QuickKind::NewToken;
    let discriminant = network.address_discriminant();
    let _address_profile =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(discriminant);
    assert_eq!(
        desktop
            .quick
            .problem(discriminant, &developer, desktop.ledger.as_ref())
            .as_deref(),
        Some("Coffee already exists in coffee.universal: mint more from Mint.")
    );
}
