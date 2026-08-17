//! Allocation-free size accounting for canonical Musubi encodings.
use super::*;
pub(super) fn canonical_frame_len<T: norito::core::NoritoSerialize>(
    value: &T,
) -> Result<usize, norito::core::Error> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::encoded_frame_len(value)
}
pub(super) fn validate_semantic_release_fields(
    release: &MusubiReleaseIdV1,
    abi: &MusubiAbiBindingV1,
    dependencies: &[MusubiDependencyReqV1],
    exports: &[Name],
    interface_digest: MusubiContentDigestV1,
    metadata: &MusubiReleaseMetadataV1,
    verification_lock_digest: MusubiVerificationLockDigestV1,
) -> Result<(), ParseError> {
    release.validate()?;
    abi.validate()?;
    metadata.validate()?;
    if dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
        || exports.len() > MUSUBI_MAX_EXPORTS_V1
        || dependencies.windows(2).any(|pair| pair[0] >= pair[1])
        || dependencies
            .windows(2)
            .any(|pair| pair[0].alias >= pair[1].alias)
        || exports.windows(2).any(|pair| pair[0] >= pair[1])
        || interface_digest.is_zero()
        || verification_lock_digest.is_zero()
    {
        return Err(ParseError::new(
            "Musubi semantic release manifest is invalid or noncanonical",
        ));
    }
    for dependency in dependencies {
        dependency.validate()?;
        if dependency.package == release.package {
            return Err(ParseError::new(
                "Musubi release cannot depend on its own package",
            ));
        }
    }
    Ok(())
}
pub(super) fn validate_semantic_release_lock(
    release: &MusubiReleaseIdV1,
    abi: &MusubiAbiBindingV1,
    dependencies: &[MusubiDependencyReqV1],
    exports: &[Name],
    metadata: &MusubiReleaseMetadataV1,
    digests: (MusubiContentDigestV1, MusubiVerificationLockDigestV1),
    verification_lock: &MusubiVerificationLockV1,
) -> Result<(), ParseError> {
    let (interface_digest, verification_lock_digest) = digests;
    validate_semantic_release_fields(
        release,
        abi,
        dependencies,
        exports,
        interface_digest,
        metadata,
        verification_lock_digest,
    )?;
    verification_lock.validate()?;
    if &verification_lock.root != release || verification_lock.digest() != verification_lock_digest
    {
        return Err(ParseError::new(
            "Musubi semantic release and verification lock do not bind the same root",
        ));
    }
    if dependencies.len() != verification_lock.root_dependencies.len() {
        return Err(ParseError::new(
            "Musubi semantic release and verification lock dependency counts differ",
        ));
    }
    for (requirement, exact) in dependencies
        .iter()
        .zip(&verification_lock.root_dependencies)
    {
        if exact.kind != MusubiDependencyKindV1::Normal
            || exact.alias != requirement.alias
            || exact.package != requirement.package
            || exact.requirement != requirement.requirement
        {
            return Err(ParseError::new(
                "Musubi semantic release does not exactly bind a verification-lock dependency",
            ));
        }
    }
    Ok(())
}
struct SemanticReleaseSource<'a>(&'a MusubiReleaseManifestV1);
impl norito::core::NoritoSerialize for SemanticReleaseSource<'_> {
    fn schema_hash() -> [u8; 16] {
        <MusubiSemanticReleaseManifestV1 as norito::core::NoritoSerialize>::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        if norito::core::use_packed_struct() {
            return Err(norito::core::Error::UnsupportedFeature(
                "borrowed Musubi semantic release packed struct",
            ));
        }
        let fields: [&dyn norito::core::NoritoSerialize; 8] = [
            &self.0.release,
            &self.0.edition,
            &self.0.abi,
            &self.0.dependencies,
            &self.0.exports,
            &self.0.interface_digest,
            &self.0.metadata,
            &self.0.verification_lock_digest,
        ];
        let mut scratch = norito::core::DeriveSmallBuf::new();
        for field in fields {
            norito::core::write_len_prefixed(writer, field, &mut scratch)?;
        }
        Ok(())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        if norito::core::use_packed_struct() {
            return None;
        }
        let fields: [&dyn norito::core::NoritoSerialize; 8] = [
            &self.0.release,
            &self.0.edition,
            &self.0.abi,
            &self.0.dependencies,
            &self.0.exports,
            &self.0.interface_digest,
            &self.0.metadata,
            &self.0.verification_lock_digest,
        ];
        fields.into_iter().try_fold(0_usize, |total, field| {
            let field_len = field.encoded_len_exact()?;
            total
                .checked_add(norito::core::len_prefix_len(field_len))?
                .checked_add(field_len)
        })
    }
}
pub(super) fn semantic_release_digest(
    manifest: &MusubiReleaseManifestV1,
) -> MusubiSemanticReleaseDigestV1 {
    MusubiSemanticReleaseDigestV1(domain_hash_value(
        MUSUBI_SEMANTIC_RELEASE_DIGEST_DOMAIN_V1,
        &SemanticReleaseSource(manifest),
    ))
}
#[cfg(feature = "json")]
struct JsonCountingSink {
    bytes: usize,
    maximum: usize,
    depth: usize,
}
#[cfg(feature = "json")]
impl JsonCountingSink {
    const fn new(maximum: usize) -> Self {
        Self {
            bytes: 0,
            maximum,
            depth: 0,
        }
    }
    fn add(&mut self, bytes: usize) -> Result<(), norito::json::BoundedJsonError> {
        let next = self
            .bytes
            .checked_add(bytes)
            .ok_or(norito::json::BoundedJsonError::BodyTooLarge)?;
        if next > self.maximum {
            return Err(norito::json::BoundedJsonError::BodyTooLarge);
        }
        self.bytes = next;
        Ok(())
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonWriteSink for JsonCountingSink {
    fn push(&mut self, value: char) -> Result<(), norito::json::BoundedJsonError> {
        self.add(value.len_utf8())
    }
    fn push_str(&mut self, value: &str) -> Result<(), norito::json::BoundedJsonError> {
        self.add(value.len())
    }
    fn begin_container(&mut self) -> Result<(), norito::json::BoundedJsonError> {
        let next = self
            .depth
            .checked_add(1)
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        if next >= 64 {
            return Err(norito::json::BoundedJsonError::Unsupported);
        }
        self.depth = next;
        Ok(())
    }
    fn end_container(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}
#[cfg(feature = "json")]
fn json_field<T: norito::json::JsonSerialize + ?Sized>(
    counter: &mut JsonCountingSink,
    name: &str,
    value: &T,
    first: bool,
) -> Result<(), norito::json::BoundedJsonError> {
    counter.add(usize::from(!first))?;
    counter.add(name.len().saturating_add(3))?;
    value.json_serialize_to(counter)
}
#[cfg(feature = "json")]
fn resolver_row_json(
    counter: &mut JsonCountingSink,
    row: &MusubiResolverReleaseRowV1,
) -> Result<(), norito::json::BoundedJsonError> {
    norito::json::JsonWriteSink::begin_container(counter)?;
    counter.add(1)?;
    json_field(counter, "release", &row.release, true)?;
    json_field(counter, "release_digest", &row.release_digest, false)?;
    json_field(counter, "archive_id", &row.archive_id, false)?;
    json_field(counter, "source_digest", &row.source_digest, false)?;
    json_field(counter, "interface_digest", &row.interface_digest, false)?;
    json_field(counter, "abi", &row.abi, false)?;
    json_field(counter, "dependencies", &row.dependencies, false)?;
    counter.add("\"selection\":".len() + 1)?;
    norito::json::JsonWriteSink::begin_container(counter)?;
    counter.add(1)?;
    counter.add("\"yank\":".len())?;
    norito::json::JsonWriteSink::begin_container(counter)?;
    counter.add(1)?;
    json_field(counter, "release", &row.selection.yank.release, true)?;
    json_field(counter, "yanked", &row.selection.yank.yanked, false)?;
    json_field(counter, "reason", &row.selection.yank.reason, false)?;
    counter.add(",\"changed_by\":".len())?;
    account_i105_json::serialize_bounded(&row.selection.yank.changed_by, counter)?;
    json_field(
        counter,
        "changed_at_height",
        &row.selection.yank.changed_at_height,
        false,
    )?;
    json_field(counter, "revision", &row.selection.yank.revision, false)?;
    counter.add(1)?;
    norito::json::JsonWriteSink::end_container(counter);
    json_field(counter, "storage", &row.selection.storage, false)?;
    json_field(counter, "governance", &row.selection.governance, false)?;
    counter.add(1)?;
    norito::json::JsonWriteSink::end_container(counter);
    json_field(counter, "index_revision", &row.index_revision, false)?;
    counter.add(1)?;
    norito::json::JsonWriteSink::end_container(counter);
    Ok(())
}
#[cfg(feature = "json")]
pub(super) fn musubi_resolver_row_json_len_bounded(
    row: &MusubiResolverReleaseRowV1,
    maximum: usize,
) -> Result<usize, norito::json::BoundedJsonError> {
    let mut counter = JsonCountingSink::new(maximum);
    resolver_row_json(&mut counter, row)?;
    Ok(counter.bytes)
}
#[cfg(feature = "json")]
pub(super) fn musubi_json_len_bounded(
    page: &MusubiResolverIndexPageV1,
    maximum: usize,
) -> Result<usize, norito::json::BoundedJsonError> {
    let mut counter = JsonCountingSink::new(maximum);
    norito::json::JsonWriteSink::begin_container(&mut counter)?;
    counter.add(1)?;
    json_field(&mut counter, "query", &page.query, true)?;
    json_field(&mut counter, "network_id", &page.network_id, false)?;
    counter.add(",\"items\":[".len())?;
    norito::json::JsonWriteSink::begin_container(&mut counter)?;
    for (index, row) in page.items.iter().enumerate() {
        counter.add(usize::from(index != 0))?;
        resolver_row_json(&mut counter, row)?;
    }
    counter.add(1)?;
    norito::json::JsonWriteSink::end_container(&mut counter);
    json_field(&mut counter, "next_cursor", &page.next_cursor, false)?;
    json_field(&mut counter, "snapshot", &page.snapshot, false)?;
    counter.add(1)?;
    norito::json::JsonWriteSink::end_container(&mut counter);
    Ok(counter.bytes)
}
#[cfg(feature = "json")]
struct FixedAccountAddress {
    bytes: [u8; MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1],
    len: usize,
}
#[cfg(feature = "json")]
impl FixedAccountAddress {
    const fn new() -> Self {
        Self {
            bytes: [0; MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1],
            len: 0,
        }
    }
    fn push(&mut self, byte: u8) -> Result<(), norito::json::BoundedJsonError> {
        let slot = self
            .bytes
            .get_mut(self.len)
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        *slot = byte;
        self.len += 1;
        Ok(())
    }
    fn extend(&mut self, bytes: &[u8]) -> Result<(), norito::json::BoundedJsonError> {
        let end = self
            .len
            .checked_add(bytes.len())
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        let destination = self
            .bytes
            .get_mut(self.len..end)
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        destination.copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
}
#[cfg(feature = "json")]
fn write_account_i105_json(
    account: &AccountId,
    out: &mut dyn norito::json::JsonWriteSink,
) -> Result<(), norito::json::BoundedJsonError> {
    let mut canonical = FixedAccountAddress::new();
    match account.controller() {
        AccountController::Single(key) => {
            canonical.push(0b0000_0010)?;
            let (algorithm, payload) = key
                .try_to_bytes()
                .map_err(|_| norito::json::BoundedJsonError::Unsupported)?;
            if let Ok(length) = u8::try_from(payload.len()) {
                canonical.extend(&[0, musubi_curve_id(algorithm)?, length])?;
            } else {
                let length = u16::try_from(payload.len())
                    .map_err(|_| norito::json::BoundedJsonError::Unsupported)?;
                canonical.extend(&[2, musubi_curve_id(algorithm)?])?;
                canonical.extend(&length.to_be_bytes())?;
            }
            canonical.extend(payload)?;
        }
        AccountController::Multisig(policy) => {
            canonical.extend(&[0b0000_1010, 1, policy.version()])?;
            canonical.extend(&policy.threshold().to_be_bytes())?;
            let member_count = u16::try_from(policy.members().len())
                .map_err(|_| norito::json::BoundedJsonError::Unsupported)?;
            canonical.extend(&member_count.to_be_bytes())?;
            for member in policy.members() {
                let (algorithm, payload) = member
                    .public_key()
                    .try_to_bytes()
                    .map_err(|_| norito::json::BoundedJsonError::Unsupported)?;
                let length = u16::try_from(payload.len())
                    .map_err(|_| norito::json::BoundedJsonError::Unsupported)?;
                canonical.push(musubi_curve_id(algorithm)?)?;
                canonical.extend(&member.weight().to_be_bytes())?;
                canonical.extend(&length.to_be_bytes())?;
                canonical.extend(payload)?;
            }
        }
    }
    let canonical_len = canonical.len;
    let canonical = &mut canonical.bytes[..canonical_len];
    let checksum = musubi_i105_checksum_digits(canonical);
    let leading_zeros = canonical.iter().take_while(|&&byte| byte == 0).count();
    // Base 105 needs fewer than two digits per input byte; including canonical leading zeroes,
    // twice the bounded account-address capacity is a strict fixed upper bound.
    let mut digits = [0_u8; 2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1];
    let mut digit_len = 0_usize;
    let mut start = leading_zeros;
    while start < canonical.len() {
        let mut remainder = 0_u32;
        for byte in &mut canonical[start..] {
            let accumulator = (remainder << 8) | u32::from(*byte);
            *byte = u8::try_from(accumulator / 105).expect("base-105 quotient fits in one byte");
            remainder = accumulator % 105;
        }
        let slot = digits
            .get_mut(digit_len)
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        *slot = u8::try_from(remainder).expect("base-105 remainder fits in one byte");
        digit_len += 1;
        while start < canonical.len() && canonical[start] == 0 {
            start += 1;
        }
    }
    for _ in 0..leading_zeros {
        let slot = digits
            .get_mut(digit_len)
            .ok_or(norito::json::BoundedJsonError::Unsupported)?;
        *slot = 0;
        digit_len += 1;
    }
    if digit_len == 0 {
        digits[0] = 0;
        digit_len = 1;
    }
    out.push('"')?;
    match crate::account::address::chain_discriminant() {
        0x02f1 => out.push_str("sora")?,
        0x0171 => out.push_str("test")?,
        0 => out.push_str("dev")?,
        discriminant => {
            out.push('n')?;
            write_musubi_u16_decimal(discriminant, out)?;
        }
    }
    for &digit in digits[..digit_len].iter().rev() {
        write_musubi_i105_symbol(digit, out)?;
    }
    for digit in checksum {
        write_musubi_i105_symbol(digit, out)?;
    }
    out.push('"')
}
#[cfg(feature = "json")]
pub(super) mod account_i105_json {
    use super::*;
    pub fn serialize(account: &AccountId, out: &mut String) {
        let literal = account
            .canonical_i105()
            .expect("AccountId JSON serialization requires canonical I105 encoding");
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
    pub fn serialize_bounded(
        account: &AccountId,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        write_account_i105_json(account, out)
    }
    pub fn deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<AccountId, norito::json::Error> {
        <AccountId as norito::json::JsonDeserialize>::json_deserialize(parser)
    }
}
#[cfg(feature = "json")]
fn musubi_curve_id(
    algorithm: iroha_crypto::Algorithm,
) -> Result<u8, norito::json::BoundedJsonError> {
    crate::account::curve::CurveId::try_from_algorithm(algorithm)
        .map(crate::account::curve::CurveId::as_u8)
        .map_err(|_| norito::json::BoundedJsonError::Unsupported)
}
#[cfg(feature = "json")]
fn write_musubi_u16_decimal(
    mut value: u16,
    out: &mut dyn norito::json::JsonWriteSink,
) -> Result<(), norito::json::BoundedJsonError> {
    let mut digits = [0_u8; 5];
    let mut cursor = digits.len();
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + u8::try_from(value % 10).expect("decimal digit fits in one byte");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for &digit in &digits[cursor..] {
        out.push(char::from(digit))?;
    }
    Ok(())
}
#[cfg(feature = "json")]
fn write_musubi_i105_symbol(
    digit: u8,
    out: &mut dyn norito::json::JsonWriteSink,
) -> Result<(), norito::json::BoundedJsonError> {
    const ASCII: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    const KANA: [&str; 47] = [
        "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ",
        "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ",
        "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
    ];
    if let Some(&symbol) = ASCII.get(usize::from(digit)) {
        out.push(char::from(symbol))
    } else if let Some(symbol) = digit
        .checked_sub(58)
        .and_then(|index| KANA.get(usize::from(index)))
    {
        out.push_str(symbol)
    } else {
        Err(norito::json::BoundedJsonError::Unsupported)
    }
}
#[cfg(feature = "json")]
fn musubi_i105_checksum_digits(canonical: &[u8]) -> [u8; 6] {
    fn step(mut checksum: u32, value: u8) -> u32 {
        const GENERATORS: [u32; 5] = [
            0x3b6a_57b2,
            0x2650_8e6d,
            0x1ea1_19fa,
            0x3d42_33dd,
            0x2a14_62b3,
        ];
        let top = checksum >> 25;
        checksum = ((checksum & 0x01ff_ffff) << 5) ^ u32::from(value);
        for (index, generator) in GENERATORS.iter().enumerate() {
            if (top >> index) & 1 == 1 {
                checksum ^= generator;
            }
        }
        checksum
    }
    let mut checksum = 1_u32;
    for &byte in b"snx" {
        checksum = step(checksum, byte >> 5);
    }
    checksum = step(checksum, 0);
    for &byte in b"snx" {
        checksum = step(checksum, byte & 0x1f);
    }
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for &byte in canonical {
        accumulator = (accumulator << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            checksum = step(
                checksum,
                u8::try_from((accumulator >> bits) & 0x1f)
                    .expect("five-bit checksum word fits in one byte"),
            );
        }
    }
    if bits > 0 {
        checksum = step(
            checksum,
            u8::try_from((accumulator << (5 - bits)) & 0x1f)
                .expect("five-bit checksum word fits in one byte"),
        );
    }
    for _ in 0..6 {
        checksum = step(checksum, 0);
    }
    checksum ^= 0x2bc8_30a3;
    let mut result = [0_u8; 6];
    for (index, slot) in result.iter_mut().enumerate() {
        let shift = 5 * (5 - index);
        *slot = u8::try_from((checksum >> shift) & 0x1f)
            .expect("five-bit checksum word fits in one byte");
    }
    result
}
