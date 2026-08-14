// Allocation-free syntax and allocation-profile scan for generic fanout JSON.
fn preflight_torii_fanout_json(
    bytes: &[u8],
    limits: ToriiFanoutJsonLimits,
) -> Result<ToriiFanoutJsonProfile, ToriiAppFanoutMemoryError> {
    ensure_fanout_limit(
        ToriiAppFanoutResource::RawBytes,
        bytes.len(),
        limits.raw_bytes,
    )?;
    core::str::from_utf8(bytes)
        .map_err(|error| ToriiAppFanoutMemoryError::syntax(error.valid_up_to(), "invalid UTF-8"))?;
    let mut scanner = ToriiFanoutJsonScanner {
        bytes,
        offset: 0,
        limits,
        profile: ToriiFanoutJsonProfile {
            raw_bytes: bytes.len(),
            ..ToriiFanoutJsonProfile::default()
        },
    };
    scanner.skip_whitespace();
    scanner.parse_value(1)?;
    scanner.skip_whitespace();
    if scanner.offset != bytes.len() {
        return Err(scanner.syntax("trailing characters"));
    }
    scanner.profile.finish(limits)
}
struct ToriiFanoutJsonScanner<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: ToriiFanoutJsonLimits,
    profile: ToriiFanoutJsonProfile,
}
impl ToriiFanoutJsonScanner<'_> {
    fn syntax(&self, detail: &'static str) -> ToriiAppFanoutMemoryError {
        ToriiAppFanoutMemoryError::syntax(self.offset, detail)
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.offset += 1;
        Some(byte)
    }
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }
    fn expect(
        &mut self,
        expected: u8,
        detail: &'static str,
    ) -> Result<(), ToriiAppFanoutMemoryError> {
        if self.bump() != Some(expected) {
            return Err(self.syntax(detail));
        }
        Ok(())
    }
    fn add_counter(
        counter: &mut usize,
        resource: ToriiAppFanoutResource,
        limit: usize,
    ) -> Result<(), ToriiAppFanoutMemoryError> {
        let next = counter
            .checked_add(1)
            .ok_or_else(|| ToriiAppFanoutMemoryError::overflow("JSON resource counter overflow"))?;
        ensure_fanout_limit(resource, next, limit)?;
        *counter = next;
        Ok(())
    }
    fn add_bytes(
        counter: &mut usize,
        bytes: usize,
        resource: ToriiAppFanoutResource,
        limit: usize,
    ) -> Result<(), ToriiAppFanoutMemoryError> {
        let next = counter
            .checked_add(bytes)
            .ok_or_else(|| ToriiAppFanoutMemoryError::overflow("JSON byte counter overflow"))?;
        ensure_fanout_limit(resource, next, limit)?;
        *counter = next;
        Ok(())
    }
    fn parse_value(&mut self, depth: usize) -> Result<(), ToriiAppFanoutMemoryError> {
        let depth_limit = self
            .limits
            .nesting_depth
            .min(norito::json::MAX_JSON_VALUE_NESTING_DEPTH);
        ensure_fanout_limit(ToriiAppFanoutResource::NestingDepth, depth, depth_limit)?;
        self.profile.max_nesting_depth = self.profile.max_nesting_depth.max(depth);
        Self::add_counter(
            &mut self.profile.values,
            ToriiAppFanoutResource::Values,
            self.limits.values,
        )?;
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => self.parse_string(),
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b't') => self.parse_literal(b"true", "invalid boolean"),
            Some(b'f') => self.parse_literal(b"false", "invalid boolean"),
            Some(b'n') => self.parse_literal(b"null", "invalid null"),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(self.syntax("unexpected JSON token")),
            None => Err(self.syntax("unexpected end of JSON")),
        }
    }
    fn parse_literal(
        &mut self,
        literal: &[u8],
        detail: &'static str,
    ) -> Result<(), ToriiAppFanoutMemoryError> {
        let end = self
            .offset
            .checked_add(literal.len())
            .ok_or_else(|| ToriiAppFanoutMemoryError::overflow("JSON literal offset overflow"))?;
        if self.bytes.get(self.offset..end) != Some(literal) {
            return Err(self.syntax(detail));
        }
        self.offset = end;
        Ok(())
    }
    fn parse_array(&mut self, depth: usize) -> Result<(), ToriiAppFanoutMemoryError> {
        self.profile.arrays =
            self.profile.arrays.checked_add(1).ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON array counter overflow")
            })?;
        self.expect(b'[', "expected array")?;
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.offset += 1;
            return Ok(());
        }
        loop {
            Self::add_counter(
                &mut self.profile.array_entries,
                ToriiAppFanoutResource::ArrayEntries,
                self.limits.array_entries,
            )?;
            let child_depth = depth.checked_add(1).ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON nesting depth overflow")
            })?;
            self.parse_value(child_depth)?;
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => {
                    self.skip_whitespace();
                    if self.peek() == Some(b']') {
                        return Err(self.syntax("trailing array comma"));
                    }
                }
                Some(b']') => return Ok(()),
                _ => return Err(self.syntax("expected comma or array end")),
            }
        }
    }
    fn parse_object(&mut self, depth: usize) -> Result<(), ToriiAppFanoutMemoryError> {
        self.profile.objects =
            self.profile.objects.checked_add(1).ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON object counter overflow")
            })?;
        self.expect(b'{', "expected object")?;
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.offset += 1;
            return Ok(());
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.syntax("object key must be a string"));
            }
            self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':', "expected colon after object key")?;
            Self::add_counter(
                &mut self.profile.object_entries,
                ToriiAppFanoutResource::ObjectEntries,
                self.limits.object_entries,
            )?;
            let child_depth = depth.checked_add(1).ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON nesting depth overflow")
            })?;
            self.parse_value(child_depth)?;
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => {
                    self.skip_whitespace();
                    if self.peek() == Some(b'}') {
                        return Err(self.syntax("trailing object comma"));
                    }
                }
                Some(b'}') => return Ok(()),
                _ => return Err(self.syntax("expected comma or object end")),
            }
        }
    }
    fn parse_string(&mut self) -> Result<(), ToriiAppFanoutMemoryError> {
        let token_start = self.offset;
        self.expect(b'"', "expected string")?;
        let content_start = self.offset;
        let mut decoded_bytes = 0usize;
        let mut escaped = false;
        loop {
            let byte = self
                .bump()
                .ok_or_else(|| self.syntax("unterminated string"))?;
            let added = match byte {
                b'"' => break,
                b'\\' => {
                    escaped = true;
                    match self
                        .bump()
                        .ok_or_else(|| self.syntax("unterminated string escape"))?
                    {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => 1,
                        b'u' => self.parse_unicode_escape()?,
                        _ => return Err(self.syntax("invalid string escape")),
                    }
                }
                0x00..=0x1f => return Err(self.syntax("control byte in string")),
                _ => 1,
            };
            decoded_bytes = decoded_bytes.checked_add(added).ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("decoded JSON string length overflow")
            })?;
            let aggregate = self
                .profile
                .decoded_string_bytes
                .checked_add(decoded_bytes)
                .ok_or_else(|| {
                    ToriiAppFanoutMemoryError::overflow("decoded JSON string budget overflow")
                })?;
            ensure_fanout_limit(
                ToriiAppFanoutResource::DecodedStringBytes,
                aggregate,
                self.limits.decoded_string_bytes,
            )?;
        }
        let token_bytes = self.offset.checked_sub(token_start).ok_or_else(|| {
            ToriiAppFanoutMemoryError::overflow("JSON string token length underflow")
        })?;
        let content_bytes = self
            .offset
            .checked_sub(1)
            .ok_or_else(|| ToriiAppFanoutMemoryError::overflow("JSON string end offset underflow"))?
            .checked_sub(content_start)
            .ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON string content length underflow")
            })?;
        Self::add_bytes(
            &mut self.profile.encoded_string_bytes,
            token_bytes,
            ToriiAppFanoutResource::EncodedStringBytes,
            self.limits.encoded_string_bytes,
        )?;
        Self::add_bytes(
            &mut self.profile.decoded_string_bytes,
            decoded_bytes,
            ToriiAppFanoutResource::DecodedStringBytes,
            self.limits.decoded_string_bytes,
        )?;
        // The fast path owns exactly the source content. The escaped path's
        // capacity starts at 16 and grows geometrically; decoded UTF-8 never
        // exceeds its encoded source spelling.
        let capacity = if escaped {
            checked_fanout_mul(content_bytes.max(TORII_FANOUT_JSON_STRING_MIN_CAPACITY), 2)?
        } else {
            content_bytes
        };
        self.profile.string_capacity_bytes = self
            .profile
            .string_capacity_bytes
            .checked_add(capacity)
            .ok_or_else(|| {
                ToriiAppFanoutMemoryError::overflow("JSON string capacity sum overflow")
            })?;
        if escaped {
            self.profile.max_escaped_string_capacity_bytes =
                self.profile.max_escaped_string_capacity_bytes.max(capacity);
        }
        Ok(())
    }
    fn parse_unicode_escape(&mut self) -> Result<usize, ToriiAppFanoutMemoryError> {
        let high = self.parse_hex_quad()?;
        if (0xd800..=0xdbff).contains(&high) {
            self.expect(b'\\', "expected low surrogate escape")?;
            self.expect(b'u', "expected low surrogate escape")?;
            let low = self.parse_hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err(self.syntax("invalid low surrogate"));
            }
            return Ok(4);
        }
        if (0xdc00..=0xdfff).contains(&high) {
            return Err(self.syntax("unexpected low surrogate"));
        }
        char::from_u32(high)
            .map(char::len_utf8)
            .ok_or_else(|| self.syntax("invalid Unicode scalar"))
    }
    fn parse_hex_quad(&mut self) -> Result<u32, ToriiAppFanoutMemoryError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let digit = self
                .bump()
                .ok_or_else(|| self.syntax("unterminated Unicode escape"))?;
            let nibble = match digit {
                b'0'..=b'9' => u32::from(digit - b'0'),
                b'a'..=b'f' => u32::from(digit - b'a' + 10),
                b'A'..=b'F' => u32::from(digit - b'A' + 10),
                _ => return Err(self.syntax("invalid Unicode escape")),
            };
            value = (value << 4) | nibble;
        }
        Ok(value)
    }
    fn parse_number(&mut self) -> Result<(), ToriiAppFanoutMemoryError> {
        if self.peek() == Some(b'-') {
            self.offset += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.offset += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.syntax("leading zero in number"));
                }
            }
            Some(b'1'..=b'9') => {
                self.offset += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.offset += 1;
                }
            }
            _ => return Err(self.syntax("expected number digits")),
        }
        if self.peek() == Some(b'.') {
            self.offset += 1;
            let start = self.offset;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            if self.offset == start {
                return Err(self.syntax("expected fractional digits"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let start = self.offset;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            if self.offset == start {
                return Err(self.syntax("expected exponent digits"));
            }
        }
        Ok(())
    }
}
