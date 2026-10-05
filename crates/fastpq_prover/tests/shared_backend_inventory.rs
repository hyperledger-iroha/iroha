//! Checks `specs/fastpq/shared_backend_inventory.json` against the workspace source.
//!
//! The inventory lists every owner of FASTPQ-style field/AIR/FRI/transcript/
//! commitment code, the production consumers of each owner and every importer.
//! These tests re-derive each importer set from the source with the matching
//! rules recorded in the inventory and fail when a listed path or symbol is
//! gone, when an importer appears or disappears (file by file, also inside a
//! listed tree), when a constant equal to the Goldilocks modulus is defined in
//! an unlisted file, when a `*stark*`/`*fri*` source is not covered, when an
//! IVM step chip module is added at any depth without being listed, when a
//! sealed q77 relation type is added without being listed, when a path recorded
//! as having no engine starts importing one, or when a crate starts or stops
//! depending on the FASTPQ crates.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use norito::json::Value;

const INVENTORY: &str = "specs/fastpq/shared_backend_inventory.json";

/// Plan roles that revision 8 requires the inventory to enumerate.
const REQUIRED_CONSUMERS: [&str; 13] = [
    "ordinary-effects",
    "axt",
    "race-v1",
    "classed-race-v1",
    "ivm-step-chips",
    "generic-semantic-verification",
    "soracloud-vk-bfv",
    "x509-main-ca",
    "ivm-private-notes",
    "pq-masp",
    "atomic-private-settlement",
    "zk-ace",
    "zk-ams-qpcs",
];

/// Proof families that must stay listed as distinct and never as migration targets.
const DISTINCT_FAMILIES: [&str; 4] = ["halo2-ipa", "bulletproofs", "lattice-pcs", "spartan-nova"];

/// The Goldilocks prime `2^64 - 2^32 + 1`.
const GOLDILOCKS: u128 = (1 << 64) - (1 << 32) + 1;

/// Shortest identifier fragment a rule may match on.
const MIN_FRAGMENT_BYTES: usize = 12;

/// Kinds a retained or engine utility may have.
const UTILITY_KINDS: [&str; 6] = [
    "public-io",
    "limits",
    "cli-trace-builder",
    "observer",
    "accelerator",
    "arithmetic",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn inventory() -> &'static Value {
    static DOCUMENT: OnceLock<Value> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        let text = fs::read_to_string(workspace_root().join(INVENTORY)).expect("read inventory");
        norito::json::from_str(&text).expect("inventory is valid JSON")
    })
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or_else(|| panic!("inventory object has no `{key}` field"))
}

fn optional<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .filter(|found| !found.is_null())
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    field(value, key)
        .as_array()
        .unwrap_or_else(|| panic!("inventory field `{key}` is not an array"))
}

fn optional_list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    optional(value, key).map_or(&[], |found| {
        found
            .as_array()
            .unwrap_or_else(|| panic!("inventory field `{key}` is not an array"))
    })
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    field(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("inventory field `{key}` is not a string"))
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    optional_list(value, key)
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("inventory list `{key}` holds a non-string"))
                .to_owned()
        })
        .collect()
}

const fn is_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Remove everything from `//` to the end of each line.
fn strip_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    for line in source.split('\n') {
        stripped.push_str(line.find("//").map_or(line, |cut| &line[..cut]));
        stripped.push('\n');
    }
    stripped
}

/// Whether `needle` occurs with identifier boundaries on its identifier ends.
fn has_token(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let pattern = needle.as_bytes();
    let (Some(&first), Some(&last)) = (pattern.first(), pattern.last()) else {
        return false;
    };
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + pattern.len();
        let left = !(is_identifier(first) && start > 0 && is_identifier(bytes[start - 1]));
        let right = !(is_identifier(last) && end < bytes.len() && is_identifier(bytes[end]));
        if left && right {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Identifier runs of a needle, for example `["iroha_core_zk", "stark"]`.
fn identifier_parts(needle: &str) -> Vec<&str> {
    needle
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|part| !part.is_empty())
        .collect()
}

/// Whether `path` is `prefix` itself or lies under the directory `prefix`.
fn under(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        path == prefix || (prefix.ends_with('/') && path.starts_with(prefix.as_str()))
    })
}

/// Role of an importer derived from its path alone.
fn role_of(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path.contains("/fuzz/") || path.starts_with("fuzz/") {
        "fuzz"
    } else if path.contains("/benches/") {
        "bench"
    } else if path.contains("/tests/")
        || path.starts_with("integration_tests/")
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.rs")
        || name.ends_with("test_support.rs")
        || name.ends_with("_fixtures.rs")
    {
        "test"
    } else if path.contains("/src/bin/")
        || path.starts_with("xtask/")
        || path.starts_with("tools/")
        || path.starts_with("scripts/")
        || path.starts_with("crates/iroha_cli/")
    {
        "cli"
    } else {
        "source"
    }
}

/// One matching rule of the inventory.
struct Rule {
    /// Tokens matched at identifier boundaries.
    needles: Vec<String>,
    /// Identifier fragments: any identifier that contains one matches.
    fragments: Vec<String>,
    also: Vec<String>,
    within: Vec<String>,
    exclude: Vec<String>,
}

fn rules(owner: &Value) -> Vec<Rule> {
    list(owner, "detect")
        .iter()
        .map(|rule| {
            let rule = Rule {
                needles: strings(rule, "needles"),
                fragments: strings(rule, "fragments"),
                also: strings(rule, "also"),
                within: strings(rule, "within"),
                exclude: strings(rule, "exclude"),
            };
            assert!(
                !rule.needles.is_empty() || !rule.fragments.is_empty(),
                "a detect rule needs a needle or a fragment"
            );
            for fragment in &rule.fragments {
                // Short fragments would match unrelated identifiers.
                assert!(
                    fragment.len() >= MIN_FRAGMENT_BYTES && fragment.bytes().all(is_identifier),
                    "fragment `{fragment}` is not an identifier fragment of at least \
                     {MIN_FRAGMENT_BYTES} bytes"
                );
            }
            rule
        })
        .collect()
}

/// Every `detect` block of the inventory with the paths its owner excludes.
fn detect_blocks() -> Vec<(String, &'static Value, Vec<String>)> {
    let document = inventory();
    let mut blocks = Vec::new();
    for engine in list(document, "engines") {
        let mut owned = strings(engine, "owner_paths");
        owned.extend(strings(engine, "shared_owner_paths"));
        blocks.push((format!("engine {}", text(engine, "id")), engine, owned));
    }
    for consumer in list(document, "consumers") {
        if let Some(facade) = optional(consumer, "facade") {
            blocks.push((
                format!("consumer {} facade", text(consumer, "id")),
                facade,
                strings(facade, "owner_paths"),
            ));
        }
    }
    for (section, label) in [
        ("retained_utilities", "utility"),
        ("engine_utilities", "engine utility"),
    ] {
        for utility in list(document, section) {
            blocks.push((
                format!("{label} {}", text(utility, "symbol")),
                utility,
                vec![text(utility, "defined_in").to_owned()],
            ));
        }
    }
    blocks
}

/// What one source file contains, outside line comments.
struct Tokens {
    /// Sorted indices of the watched identifiers that occur.
    identifiers: Vec<u16>,
    /// Sorted indices of the watched fragments some identifier contains.
    fragments: Vec<u16>,
    /// Whether the file may define the Goldilocks modulus and needs the exact check.
    modulus_candidate: bool,
}

/// The scanned workspace: every source file and the watched identifiers it uses.
struct Scan {
    root: PathBuf,
    files: Vec<String>,
    /// Sorted watched identifiers.
    watched: Vec<String>,
    /// Sorted watched identifier fragments.
    fragments: Vec<String>,
    /// Per file, what it contains.
    tokens: Vec<Tokens>,
}

impl Scan {
    fn build() -> Self {
        let document = inventory();
        let scan = field(document, "scan");
        assert_eq!(text(scan, "extension"), ".rs");
        let root = workspace_root();
        let skip: BTreeSet<String> = strings(scan, "skip_directories").into_iter().collect();
        let mut files = Vec::new();
        for scan_root in strings(scan, "roots") {
            collect(&root, &root.join(&scan_root), &skip, ".rs", &mut files);
        }
        let excluded = strings(scan, "exclude_files");
        files.retain(|path| !excluded.contains(path));
        files.sort();
        let mut watched = BTreeSet::new();
        let mut fragments = BTreeSet::new();
        for (_, block, _) in detect_blocks() {
            for rule in rules(block) {
                for needle in rule.needles.iter().chain(&rule.also) {
                    watched.extend(identifier_parts(needle).into_iter().map(str::to_owned));
                }
                fragments.extend(rule.fragments);
            }
        }
        let watched: Vec<String> = watched.into_iter().collect();
        let fragments: Vec<String> = fragments.into_iter().collect();
        let tokens = files
            .iter()
            .map(|path| {
                let bytes = fs::read(root.join(path)).expect("read source file");
                scan_tokens(&bytes, &watched, &fragments)
            })
            .collect();
        Self {
            root,
            files,
            watched,
            fragments,
            tokens,
        }
    }

    fn get() -> &'static Self {
        static SCAN: OnceLock<Scan> = OnceLock::new();
        SCAN.get_or_init(Self::build)
    }

    fn contains_identifier(&self, file: usize, identifier: &str) -> bool {
        let index = self
            .watched
            .binary_search_by(|candidate| candidate.as_str().cmp(identifier))
            .unwrap_or_else(|_| panic!("identifier `{identifier}` is not watched"));
        let index = u16::try_from(index).expect("watched identifier index fits u16");
        self.tokens[file].identifiers.binary_search(&index).is_ok()
    }

    fn contains_fragment(&self, file: usize, fragment: &str) -> bool {
        let index = self
            .fragments
            .binary_search_by(|candidate| candidate.as_str().cmp(fragment))
            .unwrap_or_else(|_| panic!("fragment `{fragment}` is not watched"));
        let index = u16::try_from(index).expect("watched fragment index fits u16");
        self.tokens[file].fragments.binary_search(&index).is_ok()
    }

    fn stripped(&self, file: usize) -> String {
        strip_comments(&String::from_utf8_lossy(
            &fs::read(self.root.join(&self.files[file])).expect("read source file"),
        ))
    }

    /// Whether the comment-stripped file contains `needle` as a bounded token.
    fn matches(&self, file: usize, needle: &str, stripped: &mut Option<String>) -> bool {
        let parts = identifier_parts(needle);
        if !parts
            .iter()
            .all(|part| self.contains_identifier(file, part))
        {
            return false;
        }
        if parts.len() == 1 && parts[0] == needle {
            return true;
        }
        has_token(stripped.get_or_insert_with(|| self.stripped(file)), needle)
    }

    /// Files matched by `rules`, outside `owned`.
    fn detect(&self, rules: &[Rule], owned: &[String]) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for (file, path) in self.files.iter().enumerate() {
            if under(path, owned) {
                continue;
            }
            let mut stripped = None;
            let matched = rules.iter().any(|rule| {
                (rule.within.is_empty() || under(path, &rule.within))
                    && !under(path, &rule.exclude)
                    && (rule
                        .needles
                        .iter()
                        .any(|needle| self.matches(file, needle, &mut stripped))
                        || rule
                            .fragments
                            .iter()
                            .any(|fragment| self.contains_fragment(file, fragment)))
                    && rule
                        .also
                        .iter()
                        .all(|needle| self.matches(file, needle, &mut stripped))
            });
            if matched {
                found.insert(path.clone());
            }
        }
        found
    }
}

/// Collect files with `extension` below `directory`, relative to `root`.
fn collect(
    root: &Path,
    directory: &Path,
    skip: &BTreeSet<String>,
    extension: &str,
    files: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries {
        let entry = entry.expect("directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            if !name.starts_with('.') && !skip.contains(&name) {
                collect(root, &path, skip, extension, files);
            }
        } else if kind.is_file() && name.ends_with(extension) {
            files.push(
                path.strip_prefix(root)
                    .expect("path under workspace root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// Value of one Rust integer literal token, with or without a type suffix.
fn literal_value(token: &str) -> Option<u128> {
    let compact: String = token
        .chars()
        .filter(|&character| character != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    let (radix, digits) = [("0x", 16), ("0o", 8), ("0b", 2)]
        .into_iter()
        .find_map(|(prefix, radix)| compact.strip_prefix(prefix).map(|rest| (radix, rest)))
        .unwrap_or((10, compact.as_str()));
    // No suffix letter is a hexadecimal digit, so stripping one is unambiguous.
    let digits = [
        "u128", "i128", "usize", "isize", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ]
    .iter()
    .find_map(|suffix| digits.strip_suffix(suffix))
    .unwrap_or(digits);
    u128::from_str_radix(digits, radix).ok()
}

/// Scan one source file outside line comments: the watched identifiers and
/// fragments it contains, and whether it may define the Goldilocks modulus.
///
/// A fragment consists of identifier characters only, so it can occur only
/// inside an identifier-like token and a substring search of the
/// comment-stripped text is exact.
///
/// A file is a modulus candidate when it holds a literal equal to the modulus,
/// an identifier naming Goldilocks or `MOD_P`, `u32::MAX`, or a shift by 64.
/// Candidates get the exact constant check of [`modulus_definitions`].
fn scan_tokens(bytes: &[u8], watched: &[String], fragments: &[String]) -> Tokens {
    let mut identifiers = BTreeSet::new();
    let mut modulus_candidate = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if is_identifier(byte) {
            let start = index;
            while index < bytes.len() && is_identifier(bytes[index]) {
                index += 1;
            }
            let token = &bytes[start..index];
            if byte.is_ascii_digit() {
                modulus_candidate |= core::str::from_utf8(token)
                    .ok()
                    .and_then(literal_value)
                    .is_some_and(|value| value == GOLDILOCKS);
                continue;
            }
            modulus_candidate |= token == b"MOD_P"
                || token
                    .windows(b"GOLDILOCKS".len())
                    .any(|window| window.eq_ignore_ascii_case(b"GOLDILOCKS"))
                || (token == b"MAX" && bytes[..start].ends_with(b"u32::"));
            if let Ok(position) =
                watched.binary_search_by(|candidate| candidate.as_bytes().cmp(token))
            {
                identifiers.insert(u16::try_from(position).expect("watched index fits u16"));
            }
        } else {
            if byte == b'<' && bytes.get(index + 1) == Some(&b'<') {
                let rest = &bytes[index + 2..];
                let digits = rest.iter().position(|next| !next.is_ascii_whitespace());
                modulus_candidate |= digits.is_some_and(|skip| {
                    rest[skip..].starts_with(b"64")
                        && !rest.get(skip + 2).copied().is_some_and(is_identifier)
                });
            }
            index += 1;
        }
    }
    // The raw search rejects almost every file; only a hit pays for stripping.
    let text = String::from_utf8_lossy(bytes);
    let mut stripped = None;
    let fragments = fragments
        .iter()
        .enumerate()
        .filter(|(_, fragment)| {
            text.contains(fragment.as_str())
                && stripped
                    .get_or_insert_with(|| strip_comments(&text))
                    .contains(fragment.as_str())
        })
        .map(|(position, _)| u16::try_from(position).expect("fragment index fits u16"))
        .collect();
    Tokens {
        identifiers: identifiers.into_iter().collect(),
        fragments,
        modulus_candidate,
    }
}

/// Listed file importers with roles, and listed importer trees with the
/// matching files each one covers.
fn listed_importers(
    owner: &Value,
) -> (BTreeMap<String, String>, BTreeMap<String, BTreeSet<String>>) {
    let mut files = BTreeMap::new();
    let mut trees = BTreeMap::new();
    for importer in list(owner, "importers") {
        if let Some(tree) = optional(importer, "tree") {
            let tree = tree.as_str().expect("tree is a string").to_owned();
            let covered: BTreeSet<String> = strings(importer, "files").into_iter().collect();
            assert_eq!(
                covered.len(),
                list(importer, "files").len(),
                "tree {tree} lists a file twice"
            );
            assert!(
                trees.insert(tree.clone(), covered).is_none(),
                "tree {tree} is listed twice"
            );
        } else {
            files.insert(
                text(importer, "path").to_owned(),
                text(importer, "role").to_owned(),
            );
        }
    }
    (files, trees)
}

/// Every path the inventory names, with a flag for directory entries.
fn listed_paths() -> BTreeSet<String> {
    let document = inventory();
    let mut paths = BTreeSet::new();
    for (_, block, owned) in detect_blocks() {
        paths.extend(owned);
        let (files, trees) = listed_importers(block);
        paths.extend(files.into_keys());
        for (tree, covered) in trees {
            paths.insert(tree);
            paths.extend(covered);
        }
    }
    for consumer in list(document, "consumers") {
        for entry in list(consumer, "entry_points") {
            paths.insert(text(entry, "path").to_owned());
        }
        for modules in optional_list(consumer, "module_sets") {
            paths.insert(text(modules, "declared_in").to_owned());
        }
    }
    for relation in list(field(document, "canonical_owner"), "sealed_relations") {
        paths.insert(text(relation, "implemented_in").to_owned());
        if let Some(gate) = optional(relation, "test_gate") {
            paths.insert(text(gate, "declared_in").to_owned());
        }
    }
    for absent in list(document, "roles_without_an_engine") {
        paths.extend(strings(absent, "checked_paths"));
        for evidence in list(absent, "evidence") {
            paths.insert(text(evidence, "path").to_owned());
        }
    }
    for family in list(document, "distinct_families") {
        paths.extend(strings(family, "owner_paths"));
    }
    for site in list(document, "goldilocks_modulus_definitions") {
        paths.insert(text(site, "path").to_owned());
    }
    for dependent in list(document, "crate_dependents") {
        paths.insert(text(dependent, "manifest").to_owned());
    }
    paths
}

/// One lexical token of a constant initializer.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Lexeme {
    Number(u128),
    /// An identifier or a `::` path.
    Path(String),
    /// An operator or delimiter: `( ) + - * << >> | & ^`.
    Symbol(&'static str),
}

/// Split a constant initializer into lexemes, or `None` for anything outside
/// the small arithmetic subset [`constant_value`] evaluates.
fn lex(expression: &str) -> Option<Vec<Lexeme>> {
    let bytes = expression.as_bytes();
    let mut lexemes = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
        } else if byte.is_ascii_digit() {
            let start = index;
            while index < bytes.len() && is_identifier(bytes[index]) {
                index += 1;
            }
            lexemes.push(Lexeme::Number(literal_value(&expression[start..index])?));
        } else if is_identifier(byte) {
            let start = index;
            while index < bytes.len()
                && (is_identifier(bytes[index])
                    || (bytes[index] == b':' && bytes.get(index + 1) == Some(&b':'))
                    || (bytes[index] == b':' && index > start && bytes[index - 1] == b':'))
            {
                index += 1;
            }
            lexemes.push(Lexeme::Path(expression[start..index].to_owned()));
        } else {
            let symbol = ["<<", ">>", "(", ")", "+", "-", "*", "|", "&", "^"]
                .into_iter()
                .find(|symbol| expression[index..].starts_with(symbol))?;
            lexemes.push(Lexeme::Symbol(symbol));
            index += symbol.len();
        }
    }
    Some(lexemes)
}

/// One binary operator of a precedence level and its checked evaluation.
type BinaryOperator = (&'static str, fn(u128, u128) -> Option<u128>);

/// Evaluator of the integer constant expressions that can spell a field
/// modulus: literals, the unsigned `MAX` constants, `as` casts, `from`
/// conversions, parentheses and `+ - * << >> & ^ |` with Rust precedence.
struct ConstantExpression {
    lexemes: Vec<Lexeme>,
    position: usize,
}

impl ConstantExpression {
    fn peek(&self) -> Option<&Lexeme> {
        self.lexemes.get(self.position)
    }

    fn take(&mut self, symbol: &'static str) -> bool {
        let found = self.peek() == Some(&Lexeme::Symbol(symbol));
        if found {
            self.position += 1;
        }
        found
    }

    /// One binary precedence level: `operand (operator operand)*`.
    fn level(
        &mut self,
        operators: &[BinaryOperator],
        operand: fn(&mut Self) -> Option<u128>,
    ) -> Option<u128> {
        let mut value = operand(self)?;
        while let Some(&(_, apply)) = operators.iter().find(|(symbol, _)| self.take(symbol)) {
            value = apply(value, operand(self)?)?;
        }
        Some(value)
    }

    fn or(&mut self) -> Option<u128> {
        self.level(&[("|", |left, right| Some(left | right))], Self::xor)
    }

    fn xor(&mut self) -> Option<u128> {
        self.level(&[("^", |left, right| Some(left ^ right))], Self::and)
    }

    fn and(&mut self) -> Option<u128> {
        self.level(&[("&", |left, right| Some(left & right))], Self::shift)
    }

    fn shift(&mut self) -> Option<u128> {
        self.level(
            &[
                ("<<", |left, right| {
                    let shifted = left.checked_shl(u32::try_from(right).ok()?)?;
                    (shifted >> right == left).then_some(shifted)
                }),
                (">>", |left, right| {
                    left.checked_shr(u32::try_from(right).ok()?)
                }),
            ],
            Self::sum,
        )
    }

    fn sum(&mut self) -> Option<u128> {
        self.level(
            &[("+", u128::checked_add), ("-", u128::checked_sub)],
            Self::product,
        )
    }

    fn product(&mut self) -> Option<u128> {
        self.level(&[("*", u128::checked_mul)], Self::cast)
    }

    fn cast(&mut self) -> Option<u128> {
        let mut value = self.primary()?;
        while self.peek() == Some(&Lexeme::Path("as".to_owned())) {
            self.position += 1;
            let Some(Lexeme::Path(target)) = self.peek().cloned() else {
                return None;
            };
            self.position += 1;
            value &= unsigned_max(&target)?;
        }
        Some(value)
    }

    fn primary(&mut self) -> Option<u128> {
        match self.peek().cloned()? {
            Lexeme::Number(value) => {
                self.position += 1;
                Some(value)
            }
            Lexeme::Symbol("(") => {
                self.position += 1;
                let value = self.or()?;
                self.take(")").then_some(value)
            }
            Lexeme::Path(path) => {
                self.position += 1;
                if let Some(target) = path.strip_suffix("::MAX") {
                    unsigned_max(target)
                } else if let Some(target) = path.strip_suffix("::from") {
                    // A lossless widening conversion of its parenthesized argument.
                    let maximum = unsigned_max(target)?;
                    let value = self.primary()?;
                    (value <= maximum).then_some(value)
                } else {
                    None
                }
            }
            Lexeme::Symbol(_) => None,
        }
    }
}

/// Maximum of an unsigned integer type name; `usize` is 64 bits wide here.
fn unsigned_max(name: &str) -> Option<u128> {
    Some(match name {
        "u8" => u128::from(u8::MAX),
        "u16" => u128::from(u16::MAX),
        "u32" => u128::from(u32::MAX),
        "u64" | "usize" => u128::from(u64::MAX),
        "u128" => u128::MAX,
        _ => return None,
    })
}

/// Value of an integer constant initializer, when it uses only the subset above.
fn constant_value(expression: &str) -> Option<u128> {
    let mut parser = ConstantExpression {
        lexemes: lex(expression)?,
        position: 0,
    };
    let value = parser.or()?;
    (parser.position == parser.lexemes.len()).then_some(value)
}

/// `(name, type, initializer)` of every `const` or `static` item of a
/// comment-stripped source, across line breaks.
///
/// Items without an initializer, `const fn`, const generics and raw pointer
/// types are not items. Initializers longer than 512 bytes are skipped.
fn constant_items(source: &str) -> Vec<(String, String, String)> {
    let bytes = source.as_bytes();
    let mut items = Vec::new();
    for keyword in ["const", "static"] {
        let mut from = 0;
        while let Some(offset) = source[from..].find(keyword) {
            let start = from + offset;
            let end = start + keyword.len();
            from = end;
            let preceded = start > 0 && is_identifier(bytes[start - 1]);
            let followed = bytes.get(end).copied().is_some_and(is_identifier);
            let pointer = source[..start].trim_end().ends_with('*');
            if preceded || followed || pointer {
                continue;
            }
            let rest = source[end..].trim_start();
            let rest = rest.strip_prefix("mut ").map_or(rest, str::trim_start);
            let name_len = rest.bytes().take_while(|&byte| is_identifier(byte)).count();
            let (name, rest) = rest.split_at(name_len);
            let Some(rest) = rest.trim_start().strip_prefix(':') else {
                continue;
            };
            if name.is_empty() || rest.starts_with(':') {
                continue;
            }
            // The type ends at the first `=`; any other terminator means this is
            // a declaration without an initializer or a const generic.
            let Some(type_len) = rest.find(['=', ';', ',', '>', '{', ')']) else {
                continue;
            };
            let initializer = &rest[type_len..];
            if !initializer.starts_with('=')
                || initializer.starts_with("==")
                || initializer.starts_with("=>")
            {
                continue;
            }
            let initializer = &initializer[1..];
            let mut depth = 0_usize;
            let length = initializer.bytes().position(|byte| {
                match byte {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }
                byte == b';' && depth == 0
            });
            if let Some(length) = length.filter(|&length| length <= 512) {
                items.push((
                    name.to_owned(),
                    rest[..type_len].trim().to_owned(),
                    initializer[..length].trim().to_owned(),
                ));
            }
        }
    }
    items
}

/// Whether a constant name claims to be the Goldilocks modulus.
fn names_goldilocks_modulus(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name == "MOD_P"
        || (name.contains("GOLDILOCKS")
            && ["MODULUS", "PRIME", "ORDER"]
                .iter()
                .any(|word| name.contains(word)))
}

/// Whether an initializer only renames another constant: a path with an
/// optional widening cast.
fn is_alias(initializer: &str) -> bool {
    let path = ["as u64", "as u128", "as usize"]
        .iter()
        .find_map(|cast| initializer.strip_suffix(cast))
        .unwrap_or(initializer)
        .trim();
    !path.is_empty()
        && !path.as_bytes()[0].is_ascii_digit()
        && path.bytes().all(|byte| is_identifier(byte) || byte == b':')
}

/// Names of the constants of a comment-stripped source that define the
/// Goldilocks modulus.
///
/// A `u64` or `u128` constant defines it when its initializer evaluates to the
/// modulus in any spelling, on one line or several. A constant whose name
/// claims the modulus also counts when its initializer cannot be evaluated,
/// unless it only renames another constant.
fn modulus_definitions(source: &str) -> Vec<String> {
    constant_items(source)
        .into_iter()
        .filter(|(name, kind, initializer)| {
            ["u64", "u128"].contains(&kind.as_str())
                && constant_value(initializer).map_or_else(
                    || names_goldilocks_modulus(name) && !is_alias(initializer),
                    |value| value == GOLDILOCKS,
                )
        })
        .map(|(name, _, _)| name)
        .collect()
}

/// Non-test `mod name;` declarations of a comment-stripped module file.
///
/// A declaration directly under a `#[cfg(test)]` attribute and a module named
/// `tests` are test modules.
fn declared_modules(source: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let mut test_gated = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#[") {
            test_gated |= trimmed.starts_with("#[cfg(test)]");
            continue;
        }
        let gated = core::mem::take(&mut test_gated);
        let mut words = trimmed.split_whitespace().peekable();
        if words.peek().is_some_and(|word| word.starts_with("pub")) {
            words.next();
        }
        if words.next() != Some("mod") {
            continue;
        }
        if let Some(name) = words.next().and_then(|word| word.strip_suffix(';'))
            && !gated
            && name != "tests"
            && name.bytes().all(is_identifier)
        {
            modules.insert(name.to_owned());
        }
    }
    modules
}

/// Whether `mod name;` in a comment-stripped module file is declared under a
/// `#[cfg(test)]` attribute.
fn is_test_gated_module(source: &str, name: &str) -> bool {
    let mut test_gated = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#[") {
            test_gated |= trimmed.starts_with("#[cfg(test)]");
            continue;
        }
        let gated = core::mem::take(&mut test_gated);
        if has_token(trimmed, &format!("mod {name};")) {
            return gated;
        }
    }
    false
}

/// File of the child module `name` of the module file `parent`, if it exists.
fn child_module_file(root: &Path, parent: &str, name: &str) -> Option<String> {
    let directory = parent
        .strip_suffix("/mod.rs")
        .or_else(|| parent.strip_suffix(".rs"))?;
    [
        format!("{directory}/{name}.rs"),
        format!("{directory}/{name}/mod.rs"),
    ]
    .into_iter()
    .find(|candidate| root.join(candidate).is_file())
}

/// Every module file at or below `module` that declares non-test child
/// modules, with those children.
fn module_tree(root: &Path, module: &str, found: &mut BTreeMap<String, BTreeSet<String>>) {
    let source = strip_comments(&fs::read_to_string(root.join(module)).expect("read module"));
    let children = declared_modules(&source);
    if children.is_empty() {
        return;
    }
    for child in &children {
        let file = child_module_file(root, module, child).unwrap_or_else(|| {
            panic!("module `{child}` of {module} has no file; `#[path]` modules are not followed")
        });
        module_tree(root, &file, found);
    }
    found.insert(module.to_owned(), children);
}

/// Workspace crates a manifest depends on, among `crates`.
///
/// A dependency is a key of any `dependencies` table (`name = ...`,
/// `name.workspace = true`), a `[...dependencies.name]` table, or any entry
/// renamed with `package = "name"` in either form.
fn manifest_dependencies(manifest: &str, crates: &[&str]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or(line).trim();
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']').trim_matches('[');
            let mut parts = header.rsplit('.');
            let last = parts.next().unwrap_or(header);
            let is_table = last.ends_with("dependencies");
            // `[dependencies.name]`: the table itself names the dependency.
            let entry = (!is_table
                && parts
                    .next()
                    .is_some_and(|parent| parent.ends_with("dependencies")))
            .then_some(last);
            in_dependencies = is_table || entry.is_some();
            if let Some(name) =
                entry.and_then(|name| crates.iter().find(|&&crate_name| crate_name == name))
            {
                found.insert((*name).to_owned());
            }
            continue;
        }
        if !in_dependencies {
            continue;
        }
        for &name in crates {
            let keyed = line.strip_prefix(name).is_some_and(|rest| {
                let rest = rest.trim_start();
                rest.starts_with('=') || rest.starts_with('.')
            });
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if keyed || compact.contains(&format!("package=\"{name}\"")) {
                found.insert(name.to_owned());
            }
        }
    }
    found
}

/// Whether a source path names a STARK or FRI owner by a path component.
fn names_stark_or_fri(path: &str) -> bool {
    path.trim_end_matches(".rs")
        .split(['/', '_'])
        .any(|component| component == "stark" || component == "fri")
}

#[test]
fn matching_primitives_follow_the_recorded_rules() {
    assert!(has_token("use fastpq_prover::Error;", "fastpq_prover::"));
    assert!(!has_token(
        "use my_fastpq_prover::Error;",
        "fastpq_prover::"
    ));
    assert!(has_token(
        "iroha_core_zk::stark::verify()",
        "iroha_core_zk::stark"
    ));
    assert!(!has_token(
        "iroha_core_zk::stark_open_verify()",
        "iroha_core_zk::stark"
    ));
    assert!(has_token("super::stark::F", "stark::"));
    assert!(!has_token("super::zk_stark::F", "stark::"));
    assert!(!has_token("aggregate_stark_v1()", "aggregate_stark"));
    assert!(has_token("x(aggregate_stark)", "aggregate_stark"));
    assert!(!has_token("", "stark"));
    assert!(!has_token("stark", ""));

    assert_eq!(
        strip_comments("a // fastpq_prover::\nb \"http://x\" c\n/// doc\n"),
        "a \nb \"http:\n\n\n"
    );
    assert_eq!(
        identifier_parts("iroha_core_zk::stark"),
        ["iroha_core_zk", "stark"]
    );
    assert_eq!(identifier_parts("stark::"), ["stark"]);

    let prefixes = ["crates/a/".to_owned(), "crates/b/file.rs".to_owned()];
    assert!(under("crates/a/src/lib.rs", &prefixes));
    assert!(under("crates/b/file.rs", &prefixes));
    assert!(!under("crates/b/file.rs.bak", &prefixes));
    assert!(!under("crates/ab/src/lib.rs", &prefixes));

    assert_eq!(role_of("crates/x/fuzz/fuzz_targets/t.rs"), "fuzz");
    assert_eq!(role_of("crates/x/benches/b.rs"), "bench");
    assert_eq!(role_of("crates/x/src/a/tests.rs"), "test");
    assert_eq!(role_of("crates/x/src/a_tests.rs"), "test");
    assert_eq!(role_of("integration_tests/tests/a.rs"), "test");
    assert_eq!(role_of("crates/x/src/bin/tool.rs"), "cli");
    assert_eq!(role_of("crates/iroha_cli/src/audit.rs"), "cli");
    assert_eq!(role_of("crates/x/src/lib.rs"), "source");

    let watched = ["fastpq_prover".to_owned(), "stark".to_owned()];
    let fragments = ["bfv_native_stark".to_owned(), "STARK_WRAPPER".to_owned()];
    let scan = |bytes: &[u8]| {
        let tokens = scan_tokens(bytes, &watched, &fragments);
        (
            tokens.identifiers,
            tokens.fragments,
            tokens.modulus_candidate,
        )
    };
    assert_eq!(
        scan(b"use fastpq_prover::x; // stark\nlet zk_stark = 1;"),
        (vec![0], vec![], false)
    );
    assert_eq!(
        scan(b"stark::verify(); fastpq_prover_x"),
        (vec![1], vec![], false)
    );
    // A fragment matches anywhere inside an identifier, whatever verb or
    // suffix surrounds it, also inside a string, and never in a line comment.
    assert_eq!(
        scan(b"encode_bfv_native_stark_key_v1(); const MAX_STARK_WRAPPER_BYTES: u8 = 1;"),
        (vec![], vec![0, 1], false)
    );
    assert_eq!(
        scan(b"let id = \"bfv_native_stark_v1\";"),
        (vec![], vec![0], false)
    );
    assert_eq!(
        scan(b"bfv_native::stark(); BFV_NATIVE_STARK; stark_wrapper"),
        (vec![1], vec![], false)
    );
    assert_eq!(
        scan(b"// encode_bfv_native_stark_key_v1 MAX_STARK_WRAPPER_BYTES"),
        (vec![], vec![], false)
    );
    for spelling in [
        &b"const P: u64 = 0xffff_ffff_0000_0001;"[..],
        b"const P: u64 = 0xFFFFFFFF00000001u64;",
        b"const P: u128 = (1u128 << 64) - (1u128 << 32) + 1;",
        b"const P: u64 = 18446744069414584321;",
        b"const P: u64 = 18_446_744_069_414_584_321_u64;",
        b"const P: u64 =\n    u64::MAX - u32::MAX as u64 + 1;",
        b"const P: u128 = (1 <<64) - 0xffff_ffff;",
        b"const GoldilocksPrime: u64 = prime();",
        b"const MOD_P: u64 = prime();",
    ] {
        assert!(scan(spelling).2, "{}", String::from_utf8_lossy(spelling));
    }
    assert!(!scan(b"// 0xffff_ffff_0000_0001").2);
    assert!(!scan(b"let x = 0xffff_ffff_0000_0002;").2);
    assert!(!scan(b"let y = u64::MAX - 1; let z = x << 640; let w = x << 6;").2);

    assert_eq!(literal_value("0xffff_ffff_0000_0001"), Some(GOLDILOCKS));
    assert_eq!(literal_value("0xFFFFFFFF00000001u64"), Some(GOLDILOCKS));
    assert_eq!(literal_value("18446744069414584321_u128"), Some(GOLDILOCKS));
    assert_eq!(literal_value("1u128"), Some(1));
    assert_eq!(literal_value("0b101"), Some(5));
    assert_eq!(literal_value("0o17"), Some(15));
    assert_eq!(literal_value("0xffu8"), Some(255));
    assert_eq!(literal_value("12abc"), None);

    // Every spelling of the modulus is found by value, on one line or several.
    for (source, name) in [
        (
            "pub(crate) const GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;",
            "GOLDILOCKS_MODULUS_V1",
        ),
        (
            "const MOD_P: u128 = (1u128 << 64) - (1u128 << 32) + 1;",
            "MOD_P",
        ),
        ("const P: u64 = 0xFFFFFFFF00000001;", "P"),
        ("const P: u64 = 18446744069414584321;", "P"),
        ("const P: u128 = (1_u128 << 64) - (1_u128 << 32) + 1;", "P"),
        // The form used by the SoraCloud BFV owner, split over two lines.
        (
            "pub const BFV_MODULUS_V1: u64 =\n    u64::MAX - u32::MAX as u64 + 1;",
            "BFV_MODULUS_V1",
        ),
        ("const P: u64 = u64::MAX - u64::from(u32::MAX) + 1;", "P"),
        ("const P: u64 = (u32::MAX as u64) << 32 | 1;", "P"),
        ("const P: u64 = 0xffff_ffff_0000_0000 + 1;", "P"),
        ("static P: u128 = 4_294_967_295 * 4_294_967_296 + 1;", "P"),
        ("pub static mut P: u64 = 0xffff_ffff_0000_0001;", "P"),
        // A name that claims the modulus with an initializer the evaluator
        // cannot follow is listed as well.
        (
            "const GOLDILOCKS_PRIME: u64 = goldilocks_prime();",
            "GOLDILOCKS_PRIME",
        ),
        ("const MOD_P: u64 = HIGH << 32 | 1;", "MOD_P"),
    ] {
        assert_eq!(modulus_definitions(source), [name], "{source}");
    }
    for source in [
        "let p = 0xffff_ffff_0000_0001_u64;",
        "const P: u64 = 0xffff_ffff_0000_0002;",
        "const P: u64 = u64::MAX - u32::MAX as u64;",
        "const P: usize = 0xffff_ffff_0000_0001;",
        "const VALUES: [u64; 2] = [0, 0xffff_ffff_0000_0001];",
        // Renaming another constant is not a second definition.
        "const FIELD_MODULUS: u64 = crate::field::GOLDILOCKS_MODULUS_V1;",
        "const GOLDILOCKS_MODULUS_U128: u128 = GOLDILOCKS_MODULUS as u128;",
        "const MOD_P: u64 = MODULUS;",
        // A name alone is not enough when the value is something else.
        "const GOLDILOCKS_MODULUS_BITS: u64 = 64;",
        "trait Field { const GOLDILOCKS_MODULUS: u64; }",
        "fn f<const N: usize>(p: *const u64) -> u64 { 0xffff_ffff_0000_0001 }",
        "const fn modulus() -> u64 { 0xffff_ffff_0000_0001 }",
    ] {
        assert!(modulus_definitions(source).is_empty(), "{source}");
    }
    assert_eq!(
        constant_value("(1 << 4) | 3 ^ 1 & 3"),
        Some(16 | (3 ^ (1 & 3)))
    );
    assert_eq!(constant_value("1 + 2 * 3 - 4"), Some(3));
    assert_eq!(constant_value("300 as u8"), Some(44));
    assert_eq!(constant_value("u128::from(u8::MAX) >> 4"), Some(15));
    assert_eq!(constant_value("u8::from(300)"), None);
    assert_eq!(constant_value("1 - 2"), None);
    assert_eq!(constant_value("1 << 128"), None);
    assert_eq!(constant_value("u128::MAX << 1"), None);
    assert_eq!(constant_value("(1 + 2"), None);
    assert_eq!(constant_value("1 + 2)"), None);
    assert_eq!(constant_value("OTHER + 1"), None);
    assert_eq!(constant_value("1 as i8"), None);
    assert_eq!(constant_value("1 / 2"), None);
    assert_eq!(
        constant_items("const A: u8 = 1;\npub static B: &[u8] = &[1; 2];\nconst C: u8;"),
        [
            ("A".to_owned(), "u8".to_owned(), "1".to_owned()),
            ("B".to_owned(), "&[u8]".to_owned(), "&[1; 2]".to_owned()),
        ]
    );
    assert!(names_goldilocks_modulus("ZK_X509_GOLDILOCKS_MODULUS_V1"));
    assert!(names_goldilocks_modulus("GoldilocksPrime"));
    assert!(names_goldilocks_modulus("MOD_P"));
    assert!(!names_goldilocks_modulus("MOD_P_U64"));
    assert!(!names_goldilocks_modulus("GOLDILOCKS_DIGEST384_BYTES"));
    assert!(is_alias("super::MODULUS as u128"));
    assert!(is_alias("MODULUS"));
    assert!(!is_alias("prime()"));
    assert!(!is_alias("1"));
    assert!(!is_alias(""));

    assert_eq!(
        declared_modules("mod alu;\npub(crate) mod word;\n#[cfg(test)]\nmod tests;\nmod x {\n")
            .into_iter()
            .collect::<Vec<_>>(),
        ["alu", "word"]
    );
    // Any module under `#[cfg(test)]` is a test module, whatever its name and
    // whatever other attributes stand between; the gate does not leak onward.
    let gated = "mod view;\n#[cfg(test)]\n#[path = \"x.rs\"]\npub(super) mod segmented_tests;\n\nmod sorted;\n#[cfg(test)]\nfn helper() {}\nmod late;\n";
    assert_eq!(
        declared_modules(gated).into_iter().collect::<Vec<_>>(),
        ["late", "sorted", "view"]
    );
    assert!(is_test_gated_module(gated, "segmented_tests"));
    assert!(!is_test_gated_module(gated, "sorted"));
    assert!(!is_test_gated_module(gated, "late"));
    assert!(!is_test_gated_module(gated, "absent"));

    let crates = ["fastpq_prover", "fastpq_isi"];
    let dependencies = |manifest: &str| {
        manifest_dependencies(manifest, &crates)
            .into_iter()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        dependencies("[dependencies]\nfastpq_prover = { workspace = true }\n"),
        ["fastpq_prover"]
    );
    assert_eq!(
        dependencies("[dev-dependencies]\nfastpq_isi.workspace = true\n"),
        ["fastpq_isi"]
    );
    assert_eq!(
        dependencies(
            "[workspace.dependencies]\nfastpq_prover = { path = \"crates/fastpq_prover\" }\n"
        ),
        ["fastpq_prover"]
    );
    // Table form, target-specific tables and renamed packages are dependencies.
    assert_eq!(
        dependencies("[dependencies.fastpq_prover]\nworkspace = true\n"),
        ["fastpq_prover"]
    );
    assert_eq!(
        dependencies("[target.'cfg(unix)'.build-dependencies.fastpq_isi]\npath = \"x\"\n"),
        ["fastpq_isi"]
    );
    assert_eq!(
        dependencies("[dependencies]\nprover = { package = \"fastpq_prover\", path = \"x\" }\n"),
        ["fastpq_prover"]
    );
    assert_eq!(
        dependencies("[dependencies.prover]\npackage = \"fastpq_isi\"\npath = \"x\"\n"),
        ["fastpq_isi"]
    );
    // Other keys, other tables, comments and longer names are not.
    assert!(dependencies("[package]\nname = \"fastpq_prover\"\n").is_empty());
    assert!(dependencies("[features]\nfastpq_isi = []\n").is_empty());
    assert!(
        dependencies("[dependencies]\n# fastpq_isi = \"1\"\nfastpq_isi_extra = \"1\"\n").is_empty()
    );
    assert!(dependencies("[dependencies.other]\npath = \"x\"\n[lib]\nfastpq_isi = 1\n").is_empty());
    assert!(names_stark_or_fri("crates/a/src/zk_stark_network.rs"));
    assert!(names_stark_or_fri("crates/a/src/stark/mod.rs"));
    assert!(names_stark_or_fri("crates/a/src/fri_fold.rs"));
    assert!(!names_stark_or_fri("crates/a/src/friend.rs"));
    assert!(!names_stark_or_fri("crates/a/src/starkness.rs"));
}

#[test]
fn inventory_is_complete_for_the_plan_and_keeps_distinct_families_apart() {
    let document = inventory();
    assert_eq!(
        text(document, "schema"),
        "fastpq.shared_backend_inventory.v1"
    );
    let engines = list(document, "engines");
    let mut classes = BTreeMap::new();
    for engine in engines {
        assert!(
            classes
                .insert(text(engine, "id"), text(engine, "class"))
                .is_none(),
            "duplicate engine id {}",
            text(engine, "id")
        );
        assert!(
            ["canonical", "canonical-support", "duplicate", "conditional"]
                .contains(&text(engine, "class")),
            "unknown engine class"
        );
        assert!(!strings(engine, "owner_paths").is_empty());
        assert!(field(engine, "components").as_object().is_some());
    }
    assert_eq!(
        classes
            .values()
            .filter(|&&class| class == "canonical")
            .count(),
        1,
        "exactly one canonical owner"
    );
    assert_eq!(
        text(field(document, "canonical_owner"), "engine"),
        "fastpq-q77"
    );
    assert_eq!(classes.get("fastpq-q77"), Some(&"canonical"));
    // ZK-AMS qPCS is conditional on the PCS selection, never unconditional.
    assert_eq!(classes.get("zk-ams-rns-qpcs"), Some(&"conditional"));

    let consumers = list(document, "consumers");
    let ids: Vec<&str> = consumers
        .iter()
        .map(|consumer| text(consumer, "id"))
        .collect();
    assert_eq!(ids, REQUIRED_CONSUMERS, "plan roles in plan order");
    for consumer in consumers {
        let used = strings(consumer, "engines");
        assert!(!used.is_empty());
        for engine in &used {
            assert!(
                classes.contains_key(engine.as_str()),
                "consumer {} names unknown engine {engine}",
                text(consumer, "id")
            );
        }
        assert!(!list(consumer, "entry_points").is_empty());
        assert!(!text(consumer, "migration").is_empty());
    }
    // A duplicate or conditional owner is used by a listed production role, or
    // says why none uses it.
    let used: BTreeSet<String> = consumers
        .iter()
        .flat_map(|consumer| strings(consumer, "engines"))
        .collect();
    for engine in engines {
        if ["duplicate", "conditional"].contains(&text(engine, "class")) {
            assert!(
                used.contains(text(engine, "id"))
                    || optional(engine, "note").is_some_and(|note| note.as_str() != Some("")),
                "engine {} has no consumer and no note",
                text(engine, "id")
            );
        }
    }
    // The SoraCloud role names both of its duplicate owners.
    let soracloud = consumers
        .iter()
        .find(|consumer| text(consumer, "id") == "soracloud-vk-bfv")
        .expect("SoraCloud role");
    assert_eq!(
        strings(soracloud, "engines"),
        ["core-zk-native-stark", "soracloud-bfv-native-stark"]
    );
    // Roles the plan names that have no FASTPQ-style engine today are recorded
    // as such, not left out.
    let absent: Vec<&str> = list(document, "roles_without_an_engine")
        .iter()
        .map(|role| text(role, "id"))
        .collect();
    assert_eq!(absent, ["ram-lfe"]);

    let families = list(document, "distinct_families");
    let family_ids: Vec<&str> = families.iter().map(|family| text(family, "id")).collect();
    assert_eq!(family_ids, DISTINCT_FAMILIES);
    let engine_owned: Vec<String> = engines
        .iter()
        .flat_map(|engine| strings(engine, "owner_paths"))
        .collect();
    for family in families {
        assert_eq!(
            field(family, "migration_target").as_bool(),
            Some(false),
            "{} must not be a migration target",
            text(family, "id")
        );
        assert!(!classes.contains_key(text(family, "id")));
        for path in strings(family, "owner_paths") {
            assert!(
                !under(&path, &engine_owned) && !engine_owned.iter().any(|owned| owned == &path),
                "distinct family path {path} is listed as a FASTPQ-style owner"
            );
        }
    }
}

#[test]
fn every_listed_path_and_symbol_exists() {
    let root = workspace_root();
    let document = inventory();
    let problems = path_problems(&root, &listed_paths());
    assert!(
        problems.is_empty(),
        "update {INVENTORY}:\n{}",
        problems.join("\n")
    );
    let stripped = |path: &str| {
        strip_comments(&fs::read_to_string(root.join(path)).expect("read listed source"))
    };
    for consumer in list(document, "consumers") {
        for entry in list(consumer, "entry_points") {
            let path = text(entry, "path");
            let Some(symbol) = optional(entry, "symbol") else {
                // Only a whole directory may stand without a symbol.
                assert!(
                    path.ends_with('/'),
                    "consumer {}: entry point {path} names no symbol",
                    text(consumer, "id")
                );
                continue;
            };
            let symbol = symbol.as_str().expect("symbol is a string");
            // A symbol that survives only in a comment or as part of a longer
            // identifier is gone.
            assert!(
                has_token(&stripped(path), symbol),
                "consumer {}: `{symbol}` is gone from {path}",
                text(consumer, "id")
            );
        }
    }
    for section in ["retained_utilities", "engine_utilities"] {
        for utility in list(document, section) {
            let path = text(utility, "defined_in");
            let symbol = text(utility, "symbol");
            if field(utility, "module_file").as_bool() == Some(true) {
                assert!(
                    path.ends_with(&format!("/{symbol}.rs")),
                    "utility module {symbol} is not {path}"
                );
            } else {
                // A group lists its members in `symbols`; otherwise the entry
                // is the one symbol.
                let members = strings(utility, "symbols");
                let members = if members.is_empty() {
                    vec![symbol.to_owned()]
                } else {
                    members
                };
                let source = stripped(path);
                for member in members {
                    assert!(
                        has_token(&source, &member),
                        "utility `{member}` is gone from {path}"
                    );
                }
            }
            assert!(
                UTILITY_KINDS.contains(&text(utility, "kind")),
                "unknown utility kind"
            );
        }
    }
    let engines: BTreeSet<&str> = list(document, "engines")
        .iter()
        .map(|engine| text(engine, "id"))
        .collect();
    for utility in list(document, "engine_utilities") {
        assert!(
            engines.contains(text(utility, "engine")),
            "engine utility {} names an unknown engine",
            text(utility, "symbol")
        );
    }
    for role in list(document, "roles_without_an_engine") {
        for evidence in list(role, "evidence") {
            let (path, symbol) = (text(evidence, "path"), text(evidence, "symbol"));
            assert!(
                has_token(&stripped(path), symbol),
                "role {}: `{symbol}` is gone from {path}",
                text(role, "id")
            );
        }
    }
}

/// Differences between the importers found in the source and the listed ones.
///
/// A file below a listed tree belongs to that tree's own file list; every other
/// file is listed on its own with its role.
fn importer_problems(
    label: &str,
    found: &BTreeSet<String>,
    listed: &BTreeMap<String, String>,
    trees: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let tree_of = |path: &str| {
        trees
            .iter()
            .find(|(tree, _)| under(path, core::slice::from_ref(tree)))
    };
    for path in found {
        if let Some((tree, covered)) = tree_of(path) {
            if !covered.contains(path) {
                problems.push(format!(
                    "{label}: new importer {path} is not listed under tree {tree}"
                ));
            }
            continue;
        }
        match listed.get(path) {
            None => problems.push(format!(
                "{label}: new importer {path} (role {}) is not listed",
                role_of(path)
            )),
            Some(role) if role != role_of(path) => problems.push(format!(
                "{label}: importer {path} is listed as `{role}` but is `{}`",
                role_of(path)
            )),
            Some(_) => {}
        }
    }
    for path in listed.keys() {
        if !found.contains(path) {
            problems.push(format!("{label}: listed importer {path} no longer matches"));
        }
        if tree_of(path).is_some() {
            problems.push(format!(
                "{label}: importer {path} is already covered by a tree"
            ));
        }
    }
    for (tree, covered) in trees {
        for path in covered {
            if !under(path, core::slice::from_ref(tree)) {
                problems.push(format!(
                    "{label}: {path} is listed under tree {tree} but lies outside it"
                ));
            } else if !found.contains(path) {
                problems.push(format!(
                    "{label}: listed importer {path} of tree {tree} no longer matches"
                ));
            }
        }
        if !found
            .iter()
            .any(|path| under(path, core::slice::from_ref(tree)))
        {
            problems.push(format!(
                "{label}: importer tree {tree} has no importer left"
            ));
        }
    }
    problems
}

/// Listed paths that are gone or whose directory marker is wrong.
fn path_problems(root: &Path, paths: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    for path in paths {
        let full = root.join(path);
        if !full.exists() {
            problems.push(format!("listed path is gone: {path}"));
        } else if full.is_dir() != path.ends_with('/') {
            problems.push(format!("directory entries end with `/`: {path}"));
        }
    }
    problems
}

#[test]
fn tripwires_report_new_stale_and_missing_entries() {
    let found: BTreeSet<String> = [
        "crates/a/src/known.rs",
        "crates/a/src/new_consumer.rs",
        "crates/tree/src/inner.rs",
        "crates/a/src/known_tests.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let listed: BTreeMap<String, String> = [
        ("crates/a/src/known.rs", "source"),
        ("crates/a/src/removed.rs", "source"),
        ("crates/a/src/known_tests.rs", "source"),
    ]
    .into_iter()
    .map(|(path, role)| (path.to_owned(), role.to_owned()))
    .collect();
    let tree = |name: &str, files: &[&str]| {
        (
            name.to_owned(),
            files
                .iter()
                .map(|&file| file.to_owned())
                .collect::<BTreeSet<String>>(),
        )
    };
    let trees: BTreeMap<String, BTreeSet<String>> = [
        tree("crates/tree/", &["crates/tree/src/inner.rs"]),
        tree("crates/empty/", &[]),
    ]
    .into();
    assert_eq!(
        importer_problems("engine x", &found, &listed, &trees),
        [
            "engine x: importer crates/a/src/known_tests.rs is listed as `source` but is `test`",
            "engine x: new importer crates/a/src/new_consumer.rs (role source) is not listed",
            "engine x: listed importer crates/a/src/removed.rs no longer matches",
            "engine x: importer tree crates/empty/ has no importer left",
        ]
    );
    // Inside a tree every file is compared as well: a new importer, a file
    // that stopped matching and a file listed under the wrong tree are reported.
    let stale: BTreeMap<String, BTreeSet<String>> = [tree(
        "crates/tree/",
        &["crates/tree/src/gone.rs", "crates/other/src/elsewhere.rs"],
    )]
    .into();
    assert_eq!(
        importer_problems("engine x", &found, &BTreeMap::new(), &stale),
        [
            "engine x: new importer crates/a/src/known.rs (role source) is not listed",
            "engine x: new importer crates/a/src/known_tests.rs (role test) is not listed",
            "engine x: new importer crates/a/src/new_consumer.rs (role source) is not listed",
            "engine x: new importer crates/tree/src/inner.rs is not listed under tree crates/tree/",
            "engine x: crates/other/src/elsewhere.rs is listed under tree crates/tree/ but lies outside it",
            "engine x: listed importer crates/tree/src/gone.rs of tree crates/tree/ no longer matches",
        ]
    );
    // A file below a tree cannot also be listed on its own.
    let doubled: BTreeMap<String, String> =
        [("crates/tree/src/inner.rs".to_owned(), "source".to_owned())].into();
    assert!(
        importer_problems("engine x", &found, &doubled, &trees).contains(
            &"engine x: importer crates/tree/src/inner.rs is already covered by a tree".to_owned()
        )
    );
    // An exact inventory reports nothing.
    let exact: BTreeMap<String, String> = [
        ("crates/a/src/known.rs", "source"),
        ("crates/a/src/new_consumer.rs", "source"),
        ("crates/a/src/known_tests.rs", "test"),
    ]
    .into_iter()
    .map(|(path, role)| (path.to_owned(), role.to_owned()))
    .collect();
    let exact_trees: BTreeMap<String, BTreeSet<String>> =
        [tree("crates/tree/", &["crates/tree/src/inner.rs"])].into();
    assert!(importer_problems("engine x", &found, &exact, &exact_trees).is_empty());

    let root = workspace_root();
    let paths: BTreeSet<String> = [
        "crates/fastpq_prover/src/lib.rs",
        "crates/fastpq_prover/src/removed_engine.rs",
        "crates/fastpq_prover/src",
        "crates/fastpq_prover/src/",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        path_problems(&root, &paths),
        [
            "directory entries end with `/`: crates/fastpq_prover/src",
            "listed path is gone: crates/fastpq_prover/src/removed_engine.rs",
        ]
    );
}

#[test]
fn importers_of_every_owner_match_the_workspace() {
    let scan = Scan::get();
    let mut problems = Vec::new();
    for (label, block, owned) in detect_blocks() {
        let found = scan.detect(&rules(block), &owned);
        let (listed, trees) = listed_importers(block);
        problems.extend(importer_problems(&label, &found, &listed, &trees));
    }
    assert!(
        problems.is_empty(),
        "update {INVENTORY}:\n{}",
        problems.join("\n")
    );
}

#[test]
fn goldilocks_modulus_definitions_are_all_listed() {
    let scan = Scan::get();
    let document = inventory();
    let engines: BTreeSet<&str> = list(document, "engines")
        .iter()
        .map(|engine| text(engine, "id"))
        .collect();
    let mut listed = BTreeSet::new();
    for site in list(document, "goldilocks_modulus_definitions") {
        listed.insert(text(site, "path").to_owned());
        match optional(site, "engine") {
            Some(engine) => assert!(
                engines.contains(engine.as_str().expect("engine id")),
                "modulus site names an unknown engine"
            ),
            None => assert!(
                !text(site, "note").is_empty(),
                "a modulus site without an engine needs a note"
            ),
        }
    }
    let mut found = BTreeSet::new();
    for (file, path) in scan.files.iter().enumerate() {
        if scan.tokens[file].modulus_candidate
            && !modulus_definitions(&scan.stripped(file)).is_empty()
        {
            found.insert(path.clone());
        }
    }
    assert_eq!(
        found, listed,
        "Goldilocks modulus definition sites changed; update {INVENTORY}"
    );
    // The SoraCloud BFV owner spells the prime as a two-line `MAX` difference.
    assert!(listed.contains("crates/iroha_crypto/src/fhe_bfv.rs"));
}

#[test]
fn stark_and_fri_named_sources_are_covered() {
    let scan = Scan::get();
    let listed: Vec<String> = listed_paths().into_iter().collect();
    let uncovered: Vec<&String> = scan
        .files
        .iter()
        .filter(|path| names_stark_or_fri(path) && !under(path, &listed))
        .collect();
    assert!(
        uncovered.is_empty(),
        "STARK/FRI-named sources not covered by {INVENTORY}: {uncovered:#?}"
    );
}

#[test]
fn ivm_step_chip_modules_match_the_listed_sets() {
    let root = workspace_root();
    let chips = list(inventory(), "consumers")
        .iter()
        .find(|consumer| text(consumer, "id") == "ivm-step-chips")
        .expect("IVM step chips role");
    let listed: BTreeMap<String, BTreeSet<String>> = list(chips, "module_sets")
        .iter()
        .map(|modules| {
            (
                text(modules, "declared_in").to_owned(),
                strings(modules, "modules").into_iter().collect(),
            )
        })
        .collect();
    assert_eq!(
        listed.len(),
        list(chips, "module_sets").len(),
        "a chip module file is listed twice"
    );
    // Every module file below the chip root that declares non-test modules, at
    // any depth, derived from the source.
    let chip_root = text(chips, "module_root");
    let mut derived = BTreeMap::new();
    module_tree(&root, chip_root, &mut derived);
    assert!(derived.contains_key(chip_root));
    assert_eq!(
        derived, listed,
        "chip modules below {chip_root} changed; update {INVENTORY}"
    );
    // The two sets B.1 first listed stay non-empty anchors of the tree.
    for anchor in [
        "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air.rs",
        "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/machine_bus.rs",
        "crates/iroha_core_privacy/src/execution_proofs/ivm_step_air/trace.rs",
    ] {
        assert!(
            listed.get(anchor).is_some_and(|modules| modules.len() >= 8),
            "{anchor}"
        );
    }
    assert!(listed.len() > 2, "nested chip module sets are listed");
}

#[test]
fn sealed_relations_match_the_deep_relation_implementors() {
    use fastpq_prover::air::q77::SEALED_RELATIONS;

    let scan = Scan::get();
    let root = workspace_root();
    let listed = list(field(inventory(), "canonical_owner"), "sealed_relations");
    // Every `impl ... DeepRelation for Type` of the canonical crate.
    let mut found = BTreeSet::new();
    for (file, path) in scan.files.iter().enumerate() {
        if !path.starts_with("crates/fastpq_prover/src/") {
            continue;
        }
        let source = scan.stripped(file);
        let mut rest = source.as_str();
        while let Some(offset) = rest.find("DeepRelation for ") {
            rest = &rest[offset + "DeepRelation for ".len()..];
            let name: String = rest
                .bytes()
                .take_while(|&byte| is_identifier(byte))
                .map(char::from)
                .collect();
            found.insert((path.clone(), name));
        }
    }
    let named: BTreeSet<(String, String)> = listed
        .iter()
        .map(|relation| {
            (
                text(relation, "implemented_in").to_owned(),
                text(relation, "type").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        found, named,
        "sealed q77 relation types changed; update {INVENTORY} and air::q77::SEALED_RELATIONS"
    );
    // Production implementors carry exactly the roles the interface publishes;
    // test-only implementors are really gated and publish nothing.
    let mut roles = BTreeSet::new();
    for relation in listed {
        let path = text(relation, "implemented_in");
        let role = optional(relation, "role");
        if field(relation, "test_only").as_bool() == Some(true) {
            assert!(role.is_none(), "{path}: a test relation has no role");
            let gated = role_of(path) == "test"
                || optional(relation, "test_gate").is_some_and(|gate| {
                    let parent = strip_comments(
                        &fs::read_to_string(root.join(text(gate, "declared_in")))
                            .expect("read gate"),
                    );
                    is_test_gated_module(&parent, text(gate, "module"))
                });
            assert!(gated, "{path} is listed as test-only but is not test-gated");
        } else {
            assert_ne!(role_of(path), "test", "{path}");
            let role = role
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{path}: a production relation needs a role"));
            assert!(roles.insert(role.to_owned()), "role {role} is listed twice");
        }
    }
    let published: BTreeSet<String> = SEALED_RELATIONS
        .iter()
        .map(|relation| format!("{:?}", relation.role))
        .collect();
    assert_eq!(roles, published);
    // Each published identity is distinct and names the compact protocol.
    let identities: BTreeSet<&str> = SEALED_RELATIONS
        .iter()
        .map(|relation| relation.identity)
        .collect();
    assert_eq!(identities.len(), SEALED_RELATIONS.len());
}

#[test]
fn roles_without_an_engine_import_none() {
    let scan = Scan::get();
    let document = inventory();
    // Importers of every proof engine. The parameter and hash crate is not a
    // proof engine: using its digests proves nothing.
    let mut importers: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for engine in list(document, "engines") {
        if text(engine, "class") == "canonical-support" {
            continue;
        }
        let mut owned = strings(engine, "owner_paths");
        owned.extend(strings(engine, "shared_owner_paths"));
        for path in scan.detect(&rules(engine), &owned) {
            importers.entry(path).or_default().push(text(engine, "id"));
        }
    }
    for role in list(document, "roles_without_an_engine") {
        let checked = strings(role, "checked_paths");
        assert!(!checked.is_empty());
        assert!(!strings(role, "planned_tasks").is_empty());
        assert!(!text(role, "finding").is_empty());
        let mut covered = 0;
        for path in &scan.files {
            if !under(path, &checked) {
                continue;
            }
            covered += 1;
            assert!(
                !importers.contains_key(path),
                "role {}: {path} now imports {:?}; list the role as a consumer in {INVENTORY}",
                text(role, "id"),
                importers.get(path)
            );
        }
        assert!(covered >= checked.len(), "every checked path holds source");
    }
}

#[test]
fn crate_dependents_match_the_manifests() {
    let root = workspace_root();
    let document = inventory();
    let skip: BTreeSet<String> = strings(field(document, "scan"), "skip_directories")
        .into_iter()
        .collect();
    let mut manifests = Vec::new();
    collect(&root, &root, &skip, "Cargo.toml", &mut manifests);
    let mut found = BTreeMap::new();
    for manifest in manifests {
        let source = fs::read_to_string(root.join(&manifest)).expect("read manifest");
        let crates = manifest_dependencies(&source, &["fastpq_prover", "fastpq_isi"]);
        if !crates.is_empty() {
            found.insert(manifest, crates.into_iter().collect::<Vec<_>>());
        }
    }
    let listed: BTreeMap<String, Vec<String>> = list(document, "crate_dependents")
        .iter()
        .map(|dependent| {
            (
                text(dependent, "manifest").to_owned(),
                strings(dependent, "depends_on"),
            )
        })
        .collect();
    assert_eq!(
        found, listed,
        "crates depending on fastpq_prover/fastpq_isi changed; update {INVENTORY}"
    );
}
