    fn parse_key_description<'a>(
        cert: &'a X509Certificate<'a>,
    ) -> Result<KeyDescription, InstructionExecutionError> {
        let extensions = cert.extensions().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "android attestation certificate is missing extensions".into(),
            )
        })?;
        let key_desc_oid = oid!(1.3.6.1.4.1.11129.2.1.17);
        let ext = extensions
            .iter()
            .find(|ext| ext.oid == key_desc_oid)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "android attestation certificate does not contain keyDescription extension"
                        .into(),
                )
            })?;
        let mut reader = DerReader::new(ext.value);
        let attestation_version = reader.read_integer("attestationVersion")?;
        if attestation_version == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android attestationVersion must be positive".into(),
            )
            .into());
        }
        let attestation_security_level =
            SecurityLevel::try_from(reader.read_enumerated("attestationSecurityLevel")?)?;
        let keymaster_version = reader.read_integer("keymasterVersion")?;
        if keymaster_version == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "android keymasterVersion must be positive".into(),
            )
            .into());
        }
        let keymaster_security_level =
            SecurityLevel::try_from(reader.read_enumerated("keymasterSecurityLevel")?)?;
        let attestation_challenge = reader
            .read_octet_string("attestationChallenge")?
            .to_vec();
        let _unique_id = reader.read_octet_string("uniqueId")?;
        let software_bytes = reader.read_sequence_bytes("softwareEnforced")?;
        let tee_bytes = reader.read_sequence_bytes("teeEnforced")?;
        let strongbox_bytes = if reader.has_remaining() {
            Some(reader.read_sequence_bytes("strongBoxEnforced")?)
        } else {
            None
        };
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "android keyDescription contained trailing data".into(),
            )
            .into());
        }
        let software = parse_authorization_list(software_bytes)?;
        let tee = parse_authorization_list(tee_bytes)?;
        let strongbox = if let Some(bytes) = strongbox_bytes {
            Some(parse_authorization_list(bytes)?)
        } else {
            None
        };
        Ok(KeyDescription {
            attestation_security_level,
            keymaster_security_level,
            attestation_challenge,
            software,
            tee,
            strongbox,
        })
    }

    fn parse_authorization_list(
        data: &[u8],
    ) -> Result<AuthorizationList, InstructionExecutionError> {
        let mut cursor = DerReader::new(data);
        let mut list = AuthorizationList::default();
        while cursor.has_remaining() {
            let tlv = cursor.read_tlv()?;
            if tlv.class != TagClass::ContextSpecific {
                return Err(InstructionExecutionError::InvariantViolation(
                    "authorization entry must use a context-specific tag".into(),
                )
                .into());
            }
            match tlv.tag {
                KM_TAG_PURPOSE => {
                    list.purposes = parse_set_of_integers(tlv.value, "purpose")?;
                }
                KM_TAG_ALGORITHM => {
                    list.algorithm = Some(parse_explicit_integer(tlv.value, "algorithm")?);
                }
                KM_TAG_KEY_SIZE => {
                    list.key_size_bits = Some(parse_explicit_integer(tlv.value, "keySize")?);
                }
                KM_TAG_EC_CURVE => {
                    list.ec_curve = Some(parse_explicit_integer(tlv.value, "ecCurve")?);
                }
                KM_TAG_ORIGIN => {
                    list.origin = Some(parse_explicit_integer(tlv.value, "origin")?);
                }
                KM_TAG_ATTESTATION_APPLICATION_ID => {
                    list.attestation_app_id =
                        Some(parse_attestation_application_id(tlv.value)?);
                }
                KM_TAG_ROLLBACK_RESISTANCE => {
                    list.rollback_resistance = parse_explicit_bool(
                        tlv.value,
                        "rollbackResistance",
                    )?;
                }
                KM_TAG_ALL_APPLICATIONS => {
                    list.all_applications =
                        parse_explicit_bool(tlv.value, "allApplications")?;
                }
                KM_TAG_ROOT_OF_TRUST => {
                    list.root_of_trust = Some(parse_root_of_trust(tlv.value)?);
                }
                _ => {}
            }
        }
        Ok(list)
    }

    fn parse_attestation_application_id(
        value: &[u8],
    ) -> Result<AttestationApplicationId, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let seq = reader.expect_universal(TagClass::Universal, true, 16, "attestationApplicationId")?;
        let mut seq_reader = DerReader::new(seq);
        let packages_set = seq_reader.expect_universal(TagClass::Universal, true, 17, "packageInfos")?;
        let mut package_reader = DerReader::new(packages_set);
        let mut package_names = Vec::new();
        while package_reader.has_remaining() {
            let entry = package_reader.expect_universal(TagClass::Universal, true, 16, "packageInfo")?;
            let mut entry_reader = DerReader::new(entry);
            let name_bytes = entry_reader.expect_universal(TagClass::Universal, false, 4, "packageName")?;
            let name = String::from_utf8(name_bytes.to_vec()).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "package_name must be valid UTF-8".into(),
                )
            })?;
            package_names.push(name);
            let _version = entry_reader.expect_universal(TagClass::Universal, false, 2, "packageVersion")?;
            if entry_reader.has_remaining() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "packageInfo contained trailing data".into(),
                )
                .into());
            }
        }

        let digests_set =
            seq_reader.expect_universal(TagClass::Universal, true, 17, "signature_digests")?;
        let mut digests_reader = DerReader::new(digests_set);
        let mut signature_digests = Vec::new();
        while digests_reader.has_remaining() {
            let digest =
                digests_reader.expect_universal(TagClass::Universal, false, 4, "signatureDigest")?;
            signature_digests.push(digest.to_vec());
        }
        if seq_reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId contained trailing fields".into(),
            )
            .into());
        }
        if package_names.is_empty() || signature_digests.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId must include at least one package and digest".into(),
            )
            .into());
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "attestationApplicationId wrapper contained trailing data".into(),
            )
            .into());
        }
        Ok(AttestationApplicationId {
            package_names,
            signature_digests,
        })
    }

    fn parse_root_of_trust(
        value: &[u8],
    ) -> Result<RootOfTrust, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let seq = reader.expect_universal(TagClass::Universal, true, 16, "rootOfTrust")?;
        let mut seq_reader = DerReader::new(seq);
        let _verified_boot_key =
            seq_reader.expect_universal(TagClass::Universal, false, 4, "verifiedBootKey")?;
        let locked_bytes =
            seq_reader.expect_universal(TagClass::Universal, false, 1, "deviceLocked")?;
        let device_locked = parse_bool(locked_bytes, "deviceLocked")?;
        let verified_state =
            parse_unsigned_integer_bytes(seq_reader.expect_universal(TagClass::Universal, false, 10, "verifiedBootState")?, "verifiedBootState")?;
        if seq_reader.has_remaining() {
            let _ = seq_reader.expect_universal(TagClass::Universal, false, 4, "verifiedBootHash")?;
        }
        if seq_reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "rootOfTrust contained trailing data".into(),
            )
            .into());
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                "rootOfTrust wrapper contained trailing data".into(),
            )
            .into());
        }
        Ok(RootOfTrust {
            device_locked,
            verified_boot_state: verified_state
                .try_into()
                .map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        "verifiedBootState exceeds supported range".into(),
                    )
                })?,
        })
    }

    fn parse_set_of_integers(
        value: &[u8],
        label: &str,
    ) -> Result<Vec<u32>, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let set = reader.expect_universal(TagClass::Universal, true, 17, label)?;
        let mut set_reader = DerReader::new(set);
        let mut values = Vec::new();
        while set_reader.has_remaining() {
            let bytes =
                set_reader.expect_universal(TagClass::Universal, false, 2, label)?;
            let num = parse_unsigned_integer_bytes(bytes, label)?;
            values.push(
                num.try_into().map_err(|_| {
                    InstructionExecutionError::InvariantViolation(
                        format!("`{label}` value does not fit in u32").into(),
                    )
                })?,
            );
        }
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            )
            .into());
        }
        Ok(values)
    }

    fn parse_explicit_integer(
        value: &[u8],
        label: &str,
    ) -> Result<u32, InstructionExecutionError> {
        let mut reader = DerReader::new(value);
        let num = reader.read_integer(label)?;
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            )
            .into());
        }
        num.try_into().map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                format!("`{label}` exceeds supported range").into(),
            )
            .into()
        })
    }

    fn parse_explicit_bool(value: &[u8], label: &str) -> Result<bool, InstructionExecutionError> {
        if value.is_empty() {
            return Ok(true);
        }
        let mut reader = DerReader::new(value);
        let bytes = reader.expect_universal(TagClass::Universal, false, 1, label)?;
        if reader.has_remaining() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` contained trailing data").into(),
            )
            .into());
        }
        parse_bool(bytes, label)
    }

    fn parse_unsigned_integer_bytes(
        bytes: &[u8],
        label: &str,
    ) -> Result<u64, InstructionExecutionError> {
        if bytes.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` is missing its integer payload").into(),
            )
            .into());
        }
        if bytes.len() > 9 {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` integer is too large").into(),
            )
            .into());
        }
        let mut value = 0u64;
        for &b in bytes {
            value = (value << 8) | b as u64;
        }
        Ok(value)
    }

    fn parse_bool(bytes: &[u8], label: &str) -> Result<bool, InstructionExecutionError> {
        if bytes.len() != 1 {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("`{label}` must be a single-byte boolean").into(),
            )
            .into());
        }
        Ok(bytes[0] != 0)
    }

    struct KeyDescription {
        attestation_security_level: SecurityLevel,
        keymaster_security_level: SecurityLevel,
        attestation_challenge: Vec<u8>,
        software: AuthorizationList,
        tee: AuthorizationList,
        strongbox: Option<AuthorizationList>,
    }

    #[derive(Default, Clone)]
    struct AuthorizationList {
        purposes: Vec<u32>,
        algorithm: Option<u32>,
        key_size_bits: Option<u32>,
        ec_curve: Option<u32>,
        origin: Option<u32>,
        attestation_app_id: Option<AttestationApplicationId>,
        rollback_resistance: bool,
        all_applications: bool,
        root_of_trust: Option<RootOfTrust>,
    }

    #[derive(Clone)]
    struct AttestationApplicationId {
        package_names: Vec<String>,
        signature_digests: Vec<Vec<u8>>,
    }

    #[derive(Clone)]
    struct RootOfTrust {
        device_locked: bool,
        verified_boot_state: u32,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum SecurityLevel {
        Software,
        TrustedEnvironment,
        StrongBox,
    }

    impl TryFrom<u64> for SecurityLevel {
        type Error = InstructionExecutionError;

        fn try_from(value: u64) -> Result<Self, Self::Error> {
            match value as u32 {
                KM_SECURITY_LEVEL_SOFTWARE => Ok(SecurityLevel::Software),
                KM_SECURITY_LEVEL_TRUSTED_ENVIRONMENT => Ok(SecurityLevel::TrustedEnvironment),
                KM_SECURITY_LEVEL_STRONG_BOX => Ok(SecurityLevel::StrongBox),
                other => Err(InstructionExecutionError::InvariantViolation(
                    format!("unknown android security level `{other}`").into(),
                )
                .into()),
            }
        }
    }

    struct DerReader<'a> {
        data: &'a [u8],
        offset: usize,
    }

    impl<'a> DerReader<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self { data, offset: 0 }
        }

        fn has_remaining(&self) -> bool {
            self.offset < self.data.len()
        }

        fn read_integer(&mut self, label: &str) -> Result<u64, InstructionExecutionError> {
            let value = self.expect_universal(TagClass::Universal, false, 2, label)?;
            parse_unsigned_integer_bytes(value, label)
        }

        fn read_enumerated(&mut self, label: &str) -> Result<u64, InstructionExecutionError> {
            let value = self.expect_universal(TagClass::Universal, false, 10, label)?;
            parse_unsigned_integer_bytes(value, label)
        }

        fn read_octet_string(
            &mut self,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            self.expect_universal(TagClass::Universal, false, 4, label)
        }

        fn read_sequence_bytes(
            &mut self,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            self.expect_universal(TagClass::Universal, true, 16, label)
        }

        fn expect_universal(
            &mut self,
            class: TagClass,
            constructed: bool,
            tag: u32,
            label: &str,
        ) -> Result<&'a [u8], InstructionExecutionError> {
            let tlv = self.read_tlv()?;
            if tlv.class != class || tlv.constructed != constructed || tlv.tag != tag {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!("unexpected DER tag while parsing `{label}`").into(),
                )
                .into());
            }
            Ok(tlv.value)
        }

        fn read_tlv(&mut self) -> Result<Tlv<'a>, InstructionExecutionError> {
            if self.offset >= self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unexpected end of DER input".into(),
                )
                .into());
            }
            let tag_byte = self.data[self.offset];
            self.offset += 1;
            let class = match tag_byte >> 6 {
                0 => TagClass::Universal,
                1 => TagClass::Application,
                2 => TagClass::ContextSpecific,
                _ => TagClass::Private,
            };
            let constructed = (tag_byte & 0x20) != 0;
            let mut tag_number = (tag_byte & 0x1F) as u32;
            if tag_number == 0x1F {
                tag_number = 0;
                loop {
                    if self.offset >= self.data.len() {
                        return Err(InstructionExecutionError::InvariantViolation(
                            "invalid DER tag encoding".into(),
                        )
                        .into());
                    }
                    let byte = self.data[self.offset];
                    self.offset += 1;
                    tag_number = (tag_number << 7) | (byte & 0x7F) as u32;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
            }
            let length = self.read_length()?;
            if self.offset + length > self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "DER value exceeds available input".into(),
                )
                .into());
            }
            let value = &self.data[self.offset..self.offset + length];
            self.offset += length;
            Ok(Tlv {
                class,
                constructed,
                tag: tag_number,
                value,
            })
        }

        fn read_length(&mut self) -> Result<usize, InstructionExecutionError> {
            if self.offset >= self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid DER length encoding".into(),
                )
                .into());
            }
            let first = self.data[self.offset];
            self.offset += 1;
            if first & 0x80 == 0 {
                return Ok(first as usize);
            }
            let octets = (first & 0x7F) as usize;
            if octets == 0 || octets > 4 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "unsupported DER length encoding".into(),
                )
                .into());
            }
            if self.offset + octets > self.data.len() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "invalid DER length encoding".into(),
                )
                .into());
            }
            let mut length = 0usize;
            for _ in 0..octets {
                length = (length << 8) | self.data[self.offset] as usize;
                self.offset += 1;
            }
            Ok(length)
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TagClass {
        Universal,
        Application,
        ContextSpecific,
        Private,
    }

    struct Tlv<'a> {
        class: TagClass,
        constructed: bool,
        tag: u32,
        value: &'a [u8],
    }