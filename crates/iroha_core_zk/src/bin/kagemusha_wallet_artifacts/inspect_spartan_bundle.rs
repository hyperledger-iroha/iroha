//! Offline comparison with the generation compiler's independently retained public pins.
use super::*;
use zk_spartan::native::{MAX_PROGRAM_BUNDLE_BYTES, ProgramBundleGeometry, ProgramBundleV2};

const PROFILE_MAX: usize = 4096;

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Dimensions {
    x: u32,
    y: u32,
}

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PublicProfile {
    schema: String,
    primary: Vec<String>,
    secondary: Vec<String>,
    primary_dimensions: Dimensions,
    secondary_dimensions: Dimensions,
}

fn pins(words: &[String]) -> io::Result<[[u8; 32]; 5]> {
    let mut pins = [[0; 32]; 5];
    if words.len() != pins.len() {
        return Err(invalid("expected exactly five compiler pins per curve"));
    }
    for (pin, word) in pins.iter_mut().zip(words) {
        *pin = digest(word)?;
    }
    Ok(pins)
}

pub(super) fn run(bundle: &str, profile: &str) -> io::Result<()> {
    let profile = Original::open(profile, PROFILE_MAX, None)?;
    let input: PublicProfile = checked(
        json::from_slice(&profile.bytes),
        "closed five-kind inspection profile",
    )?;
    if input.schema != "iroha.kagemusha.spartan.public-program-inspection.v2" {
        return Err(invalid("unsupported five-kind inspection profile"));
    }
    let primary = pins(&input.primary)?;
    let secondary = pins(&input.secondary)?;
    let dimensions = [input.primary_dimensions, input.secondary_dimensions].map(|d| [d.x, d.y]);
    let bundle = Original::open(bundle, MAX_PROGRAM_BUNDLE_BYTES, None)?;
    let geometry =
        ProgramBundleV2::inspect_originals(&bundle.bytes, &primary, &secondary, dimensions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    profile.recheck()?;
    bundle.recheck()?;
    println!(
        "program_bundle_version=2 kinds=5 bytes={}",
        bundle.bytes.len()
    );
    for (
        curve,
        ProgramBundleGeometry {
            constraint_bits,
            row_bits,
            column_bits,
            first_phase_rows,
            link_phase_rows,
            fixed_rows,
            folded_claim_words,
        },
    ) in ["primary", "secondary"].into_iter().zip(geometry)
    {
        println!(
            "curve={curve} constraint_bits={constraint_bits} row_bits={row_bits} column_bits={column_bits} first_phase_rows={first_phase_rows} link_phase_rows={link_phase_rows} fixed_rows={fixed_rows} folded_claim_words={folded_claim_words}"
        );
    }
    Ok(())
}
