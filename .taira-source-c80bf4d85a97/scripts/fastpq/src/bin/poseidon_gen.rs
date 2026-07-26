use std::{cmp::min, collections::HashSet, fmt::Write as _};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use fastpq_isi::{
    params::{CANONICAL_PARAMETER_SETS, GOLDILOCKS_FP2, POSEIDON2_SHA3, StarkParameterSet},
    poseidon::{FIELD_MODULUS, PoseidonSponge},
};

const DOMAIN_TAG: &str = "fastpq:v1:domain_roots";
const PACKED_LIMB_BYTES: usize = 7;
const GOLDILOCKS_TWO_ADICITY: u32 = 32;
const FIELD_MODULUS_U128: u128 = FIELD_MODULUS as u128;
const TWO_ADIC_COMPONENT: u128 = 1u128 << GOLDILOCKS_TWO_ADICITY;
const CANONICAL_CONST_NAMES: [&str; 2] = ["FASTPQ_CANONICAL_BALANCED", "FASTPQ_CANONICAL_LATENCY"];

#[derive(Parser)]
#[command(author, version, about = "FASTPQ Poseidon-derived constant generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Regenerate trace/LDE domain roots and coset generators.
    DomainRoots {
        /// Domain tag used to seed the Poseidon sponge.
        #[arg(long, default_value = DOMAIN_TAG)]
        seed: String,
        /// Output format (Rust snippet or plain table).
        #[arg(long, value_enum, default_value_t = OutputFormat::Rust)]
        format: OutputFormat,
        /// Optional filter by parameter name or constant identifier.
        #[arg(long, value_name = "NAME")]
        filter: Vec<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Rust,
    Table,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::DomainRoots {
            seed,
            format,
            filter,
        } => run_domain_roots(&seed, format, &filter)?,
    }
    Ok(())
}

fn run_domain_roots(seed: &str, format: OutputFormat, filters: &[String]) -> Result<()> {
    let derived = derive_all_constants(seed)?;
    if derived.len() != CANONICAL_CONST_NAMES.len() {
        anyhow::bail!(
            "expected {} canonical parameter sets, found {}",
            CANONICAL_CONST_NAMES.len(),
            derived.len()
        );
    }
    let filter_set = build_filter(filters);

    match format {
        OutputFormat::Rust => render_rust(&derived, seed, filter_set.as_ref())?,
        OutputFormat::Table => render_table(&derived, seed, filter_set.as_ref())?,
    }

    Ok(())
}

fn build_filter(filters: &[String]) -> Option<HashSet<String>> {
    if filters.is_empty() {
        None
    } else {
        Some(
            filters
                .iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
        )
    }
}

fn render_rust(
    derived: &[(StarkParameterSet, DerivedConstants)],
    seed: &str,
    filter: Option<&HashSet<String>>,
) -> Result<()> {
    println!("// Derived FASTPQ domain constants (seed: {seed}, mode: domain-roots)\n");

    for (const_name, entry) in CANONICAL_CONST_NAMES.iter().copied().zip(derived.iter()) {
        let (set, constants) = entry;
        if !should_emit(filter, set, const_name) {
            continue;
        }

        let mut buffer = String::new();
        writeln!(
            buffer,
            "pub const {const_name}: StarkParameterSet = StarkParameterSet {{"
        )?;
        writeln!(buffer, "    name: \"{}\",", set.name)?;
        writeln!(
            buffer,
            "    target_security_bits: {},",
            set.target_security_bits
        )?;
        writeln!(buffer, "    grinding_bits: {},", set.grinding_bits)?;
        writeln!(buffer, "    trace_log_size: {},", set.trace_log_size)?;
        writeln!(
            buffer,
            "    trace_root: {},",
            format_hex(constants.trace_root)
        )?;
        writeln!(buffer, "    lde_log_size: {},", set.lde_log_size)?;
        writeln!(buffer, "    lde_root: {},", format_hex(constants.lde_root))?;
        writeln!(
            buffer,
            "    permutation_size: {},",
            format_usize(set.permutation_size as usize)
        )?;
        match set.lookup_log_size {
            Some(value) => writeln!(buffer, "    lookup_log_size: Some({value}),")?,
            None => writeln!(buffer, "    lookup_log_size: None,")?,
        }
        writeln!(
            buffer,
            "    omega_coset: {},",
            format_hex(constants.omega_coset)
        )?;
        let field_repr = render_field_descriptor(set);
        let hash_repr = render_hash_descriptor(set);
        writeln!(buffer, "    field: {},", field_repr)?;
        writeln!(buffer, "    hash: {},", hash_repr)?;
        writeln!(buffer, "    fri: FriParameters {{")?;
        writeln!(buffer, "        arity: {},", set.fri.arity)?;
        writeln!(buffer, "        blowup_factor: {},", set.fri.blowup_factor)?;
        writeln!(
            buffer,
            "        max_reductions: {},",
            set.fri.max_reductions
        )?;
        writeln!(buffer, "        queries: {},", set.fri.queries)?;
        writeln!(buffer, "    }},")?;
        writeln!(buffer, "}};\n")?;

        print!("{buffer}");
    }

    Ok(())
}

fn render_table(
    derived: &[(StarkParameterSet, DerivedConstants)],
    seed: &str,
    filter: Option<&HashSet<String>>,
) -> Result<()> {
    println!(
        "# FASTPQ domain roots (seed: {seed}, mode: domain-roots)\n\
         | constant | parameter | trace_root | lde_root | omega_coset |\n\
         |----------|-----------|------------|----------|-------------|"
    );

    for (const_name, entry) in CANONICAL_CONST_NAMES.iter().copied().zip(derived.iter()) {
        let (set, constants) = entry;
        if !should_emit(filter, set, const_name) {
            continue;
        }
        println!(
            "| {const_name} | {} | {} | {} | {} |",
            set.name,
            format_hex(constants.trace_root),
            format_hex(constants.lde_root),
            format_hex(constants.omega_coset)
        );
    }

    Ok(())
}

fn should_emit(
    filter: Option<&HashSet<String>>,
    set: &StarkParameterSet,
    const_name: &str,
) -> bool {
    match filter {
        None => true,
        Some(names) => {
            let name = set.name.to_ascii_lowercase();
            let const_name_lower = const_name.to_ascii_lowercase();
            names.contains(&name) || names.contains(&const_name_lower)
        }
    }
}

fn render_field_descriptor(set: &StarkParameterSet) -> String {
    if set.field == GOLDILOCKS_FP2 {
        "GOLDILOCKS_FP2".to_string()
    } else {
        format!(
            "FieldDescriptor {{ name: \"{}\", modulus_decimal: \"{}\", extension_degree: {} }}",
            set.field.name, set.field.modulus_decimal, set.field.extension_degree
        )
    }
}

fn render_hash_descriptor(set: &StarkParameterSet) -> String {
    if set.hash == POSEIDON2_SHA3 {
        "POSEIDON2_SHA3".to_string()
    } else {
        format!(
            "HashDescriptor {{ trace_commitment: \"{}\", transcript: \"{}\" }}",
            set.hash.trace_commitment, set.hash.transcript
        )
    }
}

fn derive_all_constants(seed: &str) -> Result<Vec<(StarkParameterSet, DerivedConstants)>> {
    let mut rng = DomainRng::new(seed);
    CANONICAL_PARAMETER_SETS
        .iter()
        .map(|set| {
            let constants = rng.constants_for(set)?;
            Ok((*set, constants))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct DerivedConstants {
    trace_root: u64,
    lde_root: u64,
    omega_coset: u64,
}

struct DomainRng {
    base: PoseidonSponge,
    counter: u64,
}

impl DomainRng {
    fn new(domain: &str) -> Self {
        let mut sponge = PoseidonSponge::new();
        absorb_tag(&mut sponge, domain.as_bytes());
        Self {
            base: sponge,
            counter: 0,
        }
    }

    fn next_counter(&mut self) -> u64 {
        let value = self.counter;
        self.counter = self.counter.wrapping_add(1);
        value
    }

    fn constants_for(&mut self, set: &StarkParameterSet) -> Result<DerivedConstants> {
        let mut seeded = self.base.clone();
        absorb_tag(&mut seeded, set.name.as_bytes());
        seeded.absorb(u64::from(set.trace_log_size));
        seeded.absorb(u64::from(set.lde_log_size));

        let mut trace_sponge = seeded.clone();
        absorb_tag(&mut trace_sponge, b"trace_root");
        trace_sponge.absorb(self.next_counter());
        let trace_root = derive_primitive_root(&mut trace_sponge, set.trace_log_size)
            .with_context(|| format!("failed to derive trace root for {}", set.name))?;

        let mut lde_sponge = seeded.clone();
        absorb_tag(&mut lde_sponge, b"lde_root");
        lde_sponge.absorb(self.next_counter());
        let lde_root = derive_primitive_root(&mut lde_sponge, set.lde_log_size)
            .with_context(|| format!("failed to derive lde root for {}", set.name))?;

        let mut coset_sponge = seeded;
        absorb_tag(&mut coset_sponge, b"omega_coset");
        coset_sponge.absorb(self.next_counter());
        let omega_coset = derive_coset_generator(&mut coset_sponge, set.trace_log_size)
            .with_context(|| format!("failed to derive coset generator for {}", set.name))?;

        Ok(DerivedConstants {
            trace_root,
            lde_root,
            omega_coset,
        })
    }
}

fn absorb_tag(sponge: &mut PoseidonSponge, tag: &[u8]) {
    sponge.absorb(tag.len() as u64 % FIELD_MODULUS);
    for limb in pack_bytes(tag) {
        sponge.absorb(limb);
    }
}

fn pack_bytes(bytes: &[u8]) -> Vec<u64> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(PACKED_LIMB_BYTES));
    let mut offset = 0usize;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        let take = min(remaining, PACKED_LIMB_BYTES);
        let mut chunk = [0u8; 8];
        chunk[..take].copy_from_slice(&bytes[offset..offset + take]);
        let limb = u64::from_le_bytes(chunk);
        assert!(
            limb < FIELD_MODULUS,
            "packed limb {limb:#x} exceeds field modulus {FIELD_MODULUS:#x}"
        );
        limbs.push(limb);
        offset += take;
    }

    limbs
}

fn derive_primitive_root(sponge: &mut PoseidonSponge, log_size: u32) -> Result<u64> {
    if log_size == 0 {
        return Ok(1);
    }

    let exponent = (FIELD_MODULUS_U128 - 1) >> log_size;
    loop {
        let candidate = sponge.squeeze_element();
        if candidate == 0 {
            continue;
        }
        let root = pow_mod(candidate, exponent);
        if root == 1 {
            continue;
        }
        let half_order = 1u128 << (log_size - 1);
        if pow_mod(root, half_order) != 1 {
            return Ok(root);
        }
    }
}

fn derive_coset_generator(sponge: &mut PoseidonSponge, trace_log_size: u32) -> Result<u64> {
    let trace_order = 1u128 << trace_log_size;
    loop {
        let candidate = sponge.squeeze_element();
        if candidate == 0 {
            continue;
        }
        let coset = pow_mod(candidate, TWO_ADIC_COMPONENT);
        if coset == 1 {
            continue;
        }
        if pow_mod(coset, trace_order) != 1 {
            return Ok(coset);
        }
    }
}

fn pow_mod(mut base: u64, mut exponent: u128) -> u64 {
    let mut result = 1u64;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = mul_mod(result, base);
        }
        base = mul_mod(base, base);
        exponent >>= 1;
    }
    result
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    u64::try_from(product % FIELD_MODULUS_U128).expect("multiplication result fits modulus")
}

fn format_hex(value: u64) -> String {
    let digits = format!("{value:016x}");
    let mut formatted = String::with_capacity(2 + digits.len() + digits.len() / 4);
    formatted.push_str("0x");
    for (idx, ch) in digits.chars().enumerate() {
        if idx != 0 && idx % 4 == 0 {
            formatted.push('_');
        }
        formatted.push(ch);
    }
    formatted
}

fn format_usize(value: usize) -> String {
    let digits: Vec<char> = value.to_string().chars().collect();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.iter().enumerate() {
        formatted.push(*ch);
        let remaining = digits.len() - idx - 1;
        if remaining > 0 && remaining % 3 == 0 {
            formatted.push('_');
        }
    }
    formatted
}
