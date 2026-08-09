// Closed V1 governance and ministry-agenda request/response normalizers.

export const VERIFYING_KEY_PRIVATE_KEY_FIELDS = new Set([
  "private_key",
  "privateKey",
  "private_key_hex",
  "privateKeyHex",
  "private_key_bytes",
  "privateKeyBytes",
  "private_key_seed",
  "privateKeySeed",
  "private_key_multihash",
  "privateKeyMultihash",
  "private_key_algorithm",
  "privateKeyAlgorithm",
]);

export function createToriiGovernanceNormalizers({
  ToriiClient,
  ValidationErrorCode,
  assertSupportedOptionKeys,
  createValidationError,
  ensureCanonicalAccountId,
  ensureRecord,
  isPlainObject,
  normalizeArbitraryHex,
  normalizeErrorPath,
  normalizeGovernanceUint64Integer,
  normalizeHex32String,
  normalizeManifestProvenancePayload,
  normalizeQuantityInput,
  normalizeRequiredBase64Payload,
  normalizeUint64DecimalString,
  requireExactLowerHex32String,
  requireExactNonEmptyString,
  requireExactTokenString,
  requireGovernanceSelectorString,
  requireNonEmptyString,
}) {
  const GOVERNANCE_WINDOW_KEYS = new Set(["lower", "upper"]);
  const GOVERNANCE_FINALIZE_REQUEST_KEYS = new Set([
    "referendumId",
    "proposalId",
  ]);
  const GOVERNANCE_ENACT_REQUEST_KEYS = new Set(["proposalId"]);
  const GOVERNANCE_DEPLOY_CONTRACT_REQUEST_KEYS = new Set([
    "contractAddress",
    "contractAlias",
    "abiVersion",
    "codeHash",
    "abiHash",
    "window",
    "mode",
    "manifestProvenance",
  ]);
  const GOVERNANCE_MANIFEST_PROVENANCE_KEYS = new Set(["signer", "signature"]);
  const GOVERNANCE_PLAIN_BALLOT_REQUEST_KEYS = new Set([
    "authority",
    "chainId",
    "referendumId",
    "owner",
    "amount",
    "durationBlocks",
    "direction",
  ]);
  const GOVERNANCE_PARLIAMENT_BALLOT_REQUEST_KEYS = new Set([
    "authority",
    "chainId",
    "proposalId",
    "body",
    "decision",
  ]);
  const GOVERNANCE_ZK_BALLOT_V1_REQUEST_KEYS = new Set([
    "authority",
    "chainId",
    "electionId",
    "backend",
    "envelope",
    "rootHint",
    "owner",
    "amount",
    "durationBlocks",
    "direction",
    "nullifier",
  ]);
  const GOVERNANCE_ZK_BALLOT_PROOF_REQUEST_KEYS = new Set([
    "authority",
    "chainId",
    "electionId",
    "ballot",
  ]);
  const GOVERNANCE_BALLOT_PROOF_KEYS = new Set([
    "backend",
    "envelopeBytes",
    "rootHint",
    "owner",
    "nullifier",
    "amount",
    "durationBlocks",
    "direction",
  ]);
  const GOVERNANCE_PARLIAMENT_BODIES = new Set([
    "rules-committee",
    "agenda-council",
    "interest-panel",
    "review-panel",
    "policy-jury",
    "oversight-committee",
    "fma-committee",
  ]);
  const GOVERNANCE_PARLIAMENT_DECISIONS = new Set([
    "approve",
    "reject",
    "abstain",
  ]);

  function rejectGovernancePrivateKeyFieldsDeep(value, context) {
    const pending = [{ value, path: context }];
    const visited = new WeakSet();
    while (pending.length > 0) {
      const current = pending.pop();
      const candidate = current.value;
      if (candidate === null || typeof candidate !== "object") {
        continue;
      }
      if (visited.has(candidate)) {
        continue;
      }
      visited.add(candidate);
      if (Array.isArray(candidate)) {
        for (let index = candidate.length - 1; index >= 0; index -= 1) {
          pending.push({ value: candidate[index], path: `${current.path}[${index}]` });
        }
        continue;
      }
      if (!isPlainObject(candidate)) {
        continue;
      }
      const fields = Object.keys(candidate).filter((key) =>
        VERIFYING_KEY_PRIVATE_KEY_FIELDS.has(key),
      );
      if (fields.length > 0) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${current.path} does not accept private-key fields (${fields.join(", ")}); sign the returned transaction draft locally`,
          `${current.path}.${fields[0]}`,
        );
      }
      for (const [key, nested] of Object.entries(candidate)) {
        pending.push({ value: nested, path: `${current.path}.${key}` });
      }
    }
  }

  function normalizeGovernanceFinalizePayload(input) {
    const context = "governanceFinalizeReferendum payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_FINALIZE_REQUEST_KEYS, context);
    const referendumId = record.referendumId;
    if (referendumId === undefined || referendumId === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "governanceFinalizeReferendum.referendumId is required",
        "governanceFinalizeReferendum.referendumId",
      );
    }
    const proposalId = record.proposalId;
    if (proposalId === undefined || proposalId === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "governanceFinalizeReferendum.proposalId is required",
        "governanceFinalizeReferendum.proposalId",
      );
    }
    const exactReferendumId = requireExactLowerHex32String(
      referendumId,
      "governanceFinalizeReferendum.referendumId",
    );
    const exactProposalId = requireExactLowerHex32String(
      proposalId,
      "governanceFinalizeReferendum.proposalId",
    );
    if (exactReferendumId !== exactProposalId) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "governanceFinalizeReferendum.referendumId must equal proposalId",
        "governanceFinalizeReferendum.referendumId",
      );
    }
    return {
      referendum_id: exactReferendumId,
      proposal_id: exactProposalId,
    };
  }

  function normalizeGovernanceEnactPayload(input) {
    const context = "governanceEnactProposal payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_ENACT_REQUEST_KEYS, context);
    const proposalId = record.proposalId;
    if (proposalId === undefined || proposalId === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "governanceEnactProposal.proposalId is required",
        "governanceEnactProposal.proposalId",
      );
    }
    return {
      proposal_id: requireExactLowerHex32String(
        proposalId,
        "governanceEnactProposal.proposalId",
      ),
    };
  }

  const MINISTRY_AGENDA_DRAFT_REQUEST_KEYS = new Set(["proposal", "authority"]);
  const MINISTRY_AGENDA_PROPOSAL_KEYS = new Set([
    "version",
    "proposal_id",
    "submitted_at_unix_ms",
    "language",
    "action",
    "summary",
    "tags",
    "targets",
    "evidence",
    "submitter",
    "duplicates",
  ]);
  const MINISTRY_AGENDA_SUMMARY_KEYS = new Set([
    "title",
    "motivation",
    "expected_impact",
  ]);
  const MINISTRY_AGENDA_TARGET_KEYS = new Set([
    "label",
    "hash_family",
    "hash_hex",
    "reason",
  ]);
  const MINISTRY_AGENDA_EVIDENCE_KEYS = new Set([
    "kind",
    "uri",
    "digest_blake3_hex",
    "description",
  ]);
  const MINISTRY_AGENDA_SUBMITTER_KEYS = new Set([
    "name",
    "contact",
    "organization",
    "pgp_fingerprint",
  ]);
  const MINISTRY_AGENDA_ACTIONS = new Set([
    "add-to-denylist",
    "remove-from-denylist",
    "amend-policy",
  ]);
  const MINISTRY_AGENDA_TAGS = new Set([
    "csam",
    "malware",
    "fraud",
    "harassment",
    "impersonation",
    "policy-escalation",
    "terrorism",
    "spam",
  ]);
  const MINISTRY_AGENDA_EVIDENCE_KINDS = new Set([
    "url",
    "torii-case",
    "sorafs-cid",
    "attachment",
  ]);

  function requireMinistryAgendaArray(value, context, { nonEmpty = false } = {}) {
    if (!Array.isArray(value) || (nonEmpty && value.length === 0)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} must be ${nonEmpty ? "a non-empty array" : "an array"}`,
        context,
      );
    }
    return value;
  }

  function normalizeMinistryAgendaOptionalText(value, context) {
    if (value === undefined) return undefined;
    if (value === null) return null;
    if (typeof value !== "string") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a string or null`,
        context,
      );
    }
    return value;
  }

  function requireMinistryAgendaText(value, context) {
    if (typeof value !== "string" || value.trim().length === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must contain non-whitespace text`,
        context,
      );
    }
    return value;
  }

  function normalizeMinistryAgendaProposal(input, context) {
    const proposal = ensureRecord(input, context);
    assertSupportedOptionKeys(proposal, MINISTRY_AGENDA_PROPOSAL_KEYS, context);

    const version = normalizeGovernanceUint64Integer(
      proposal.version,
      `${context}.version`,
    );
    if (version !== 1) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${context}.version must be exactly 1`,
        `${context}.version`,
      );
    }

    const proposalId = requireExactNonEmptyString(
      proposal.proposal_id,
      `${context}.proposal_id`,
    );
    if (!/^AC-[0-9]{4}-[0-9]{3}$/u.test(proposalId)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context}.proposal_id must follow AC-YYYY-###`,
        `${context}.proposal_id`,
      );
    }

    const submittedAt = normalizeGovernanceUint64Integer(
      proposal.submitted_at_unix_ms,
      `${context}.submitted_at_unix_ms`,
    );
    if (submittedAt === 0) {
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${context}.submitted_at_unix_ms must be positive`,
        `${context}.submitted_at_unix_ms`,
      );
    }

    const language = requireExactNonEmptyString(
      proposal.language,
      `${context}.language`,
    );
    if (
      language.length < 2 ||
      language.length > 32 ||
      !/^[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*$/u.test(language)
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context}.language must be an exact BCP-47 language tag`,
        `${context}.language`,
      );
    }

    const action = requireExactNonEmptyString(proposal.action, `${context}.action`);
    if (!MINISTRY_AGENDA_ACTIONS.has(action)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context}.action is unsupported`,
        `${context}.action`,
      );
    }

    const summaryContext = `${context}.summary`;
    const summary = ensureRecord(proposal.summary, summaryContext);
    assertSupportedOptionKeys(summary, MINISTRY_AGENDA_SUMMARY_KEYS, summaryContext);
    const normalizedSummary = {
      title: requireMinistryAgendaText(summary.title, `${summaryContext}.title`),
      motivation: requireMinistryAgendaText(
        summary.motivation,
        `${summaryContext}.motivation`,
      ),
      expected_impact: requireMinistryAgendaText(
        summary.expected_impact,
        `${summaryContext}.expected_impact`,
      ),
    };

    const tags = requireMinistryAgendaArray(
      proposal.tags ?? [],
      `${context}.tags`,
    ).map((tag, index) => {
      const normalized = requireExactNonEmptyString(tag, `${context}.tags[${index}]`);
      if (!MINISTRY_AGENDA_TAGS.has(normalized)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${context}.tags[${index}] is unsupported`,
          `${context}.tags[${index}]`,
        );
      }
      return normalized;
    });

    const fingerprints = new Set();
    const targets = requireMinistryAgendaArray(
      proposal.targets,
      `${context}.targets`,
      { nonEmpty: true },
    ).map((entry, index) => {
      const targetContext = `${context}.targets[${index}]`;
      const target = ensureRecord(entry, targetContext);
      assertSupportedOptionKeys(target, MINISTRY_AGENDA_TARGET_KEYS, targetContext);
      const hashFamily = requireExactNonEmptyString(
        target.hash_family,
        `${targetContext}.hash_family`,
      );
      if (
        hashFamily.length > 48 ||
        !/^[A-Za-z0-9._-]+$/u.test(hashFamily)
      ) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${targetContext}.hash_family is invalid`,
          `${targetContext}.hash_family`,
        );
      }
      const hashHex = requireExactNonEmptyString(
        target.hash_hex,
        `${targetContext}.hash_hex`,
      );
      if (hashHex.length < 32 || !/^[0-9a-fA-F]+$/u.test(hashHex)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${targetContext}.hash_hex must contain at least 16 bytes of hexadecimal`,
          `${targetContext}.hash_hex`,
        );
      }
      const fingerprint = `${hashFamily.toLowerCase()}:${hashHex.toLowerCase()}`;
      if (fingerprints.has(fingerprint)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${targetContext} duplicates an earlier target`,
          targetContext,
        );
      }
      fingerprints.add(fingerprint);
      return {
        label: requireMinistryAgendaText(target.label, `${targetContext}.label`),
        hash_family: hashFamily,
        hash_hex: hashHex,
        reason: requireMinistryAgendaText(target.reason, `${targetContext}.reason`),
      };
    });

    const evidence = requireMinistryAgendaArray(
      proposal.evidence,
      `${context}.evidence`,
      { nonEmpty: true },
    ).map((entry, index) => {
      const evidenceContext = `${context}.evidence[${index}]`;
      const attachment = ensureRecord(entry, evidenceContext);
      assertSupportedOptionKeys(attachment, MINISTRY_AGENDA_EVIDENCE_KEYS, evidenceContext);
      const kind = requireExactNonEmptyString(
        attachment.kind,
        `${evidenceContext}.kind`,
      );
      if (!MINISTRY_AGENDA_EVIDENCE_KINDS.has(kind)) {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${evidenceContext}.kind is unsupported`,
          `${evidenceContext}.kind`,
        );
      }
      const normalized = {
        kind,
        uri: requireMinistryAgendaText(attachment.uri, `${evidenceContext}.uri`),
      };
      if (attachment.digest_blake3_hex !== undefined && attachment.digest_blake3_hex !== null) {
        const digest = requireExactNonEmptyString(
          attachment.digest_blake3_hex,
          `${evidenceContext}.digest_blake3_hex`,
        );
        if (!/^[0-9a-fA-F]{64}$/u.test(digest)) {
          throw createValidationError(
            ValidationErrorCode.INVALID_STRING,
            `${evidenceContext}.digest_blake3_hex must be exactly 32 bytes of hexadecimal`,
            `${evidenceContext}.digest_blake3_hex`,
          );
        }
        normalized.digest_blake3_hex = digest;
      } else if (kind === "sorafs-cid" || kind === "attachment") {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${evidenceContext}.digest_blake3_hex is required for ${kind}`,
          `${evidenceContext}.digest_blake3_hex`,
        );
      } else if (attachment.digest_blake3_hex === null) {
        normalized.digest_blake3_hex = null;
      }
      if (attachment.description !== undefined) {
        normalized.description = normalizeMinistryAgendaOptionalText(
          attachment.description,
          `${evidenceContext}.description`,
        );
      }
      return normalized;
    });

    const submitterContext = `${context}.submitter`;
    const submitter = ensureRecord(proposal.submitter, submitterContext);
    assertSupportedOptionKeys(submitter, MINISTRY_AGENDA_SUBMITTER_KEYS, submitterContext);
    const normalizedSubmitter = {
      name: requireMinistryAgendaText(submitter.name, `${submitterContext}.name`),
      contact: requireMinistryAgendaText(
        submitter.contact,
        `${submitterContext}.contact`,
      ),
    };
    for (const field of ["organization", "pgp_fingerprint"]) {
      if (submitter[field] !== undefined) {
        normalizedSubmitter[field] = normalizeMinistryAgendaOptionalText(
          submitter[field],
          `${submitterContext}.${field}`,
        );
      }
    }

    const duplicates = requireMinistryAgendaArray(
      proposal.duplicates ?? [],
      `${context}.duplicates`,
    ).map((duplicate, index) => {
      if (typeof duplicate !== "string") {
        throw createValidationError(
          ValidationErrorCode.INVALID_STRING,
          `${context}.duplicates[${index}] must be a string`,
          `${context}.duplicates[${index}]`,
        );
      }
      return duplicate;
    });

    return {
      version,
      proposal_id: proposalId,
      submitted_at_unix_ms: submittedAt,
      language,
      action,
      summary: normalizedSummary,
      tags,
      targets,
      evidence,
      submitter: normalizedSubmitter,
      duplicates,
    };
  }

  function normalizeMinistryAgendaProposalDraftRequest(input) {
    const context = "draftMinistryAgendaProposal payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, MINISTRY_AGENDA_DRAFT_REQUEST_KEYS, context);
    return {
      proposal: normalizeMinistryAgendaProposal(
        record.proposal,
        `${context}.proposal`,
      ),
      authority: requireExactTokenString(
        record.authority,
        `${context}.authority`,
      ),
    };
  }

  function normalizeMinistryAgendaProposalRecord(
    payload,
    context = "ministry agenda proposal record",
  ) {
    const record = ensureRecord(payload, context);
    return {
      proposal: ensureRecord(record.proposal, `${context}.proposal`),
      authority: requireNonEmptyString(record.authority, `${context}.authority`),
      submitted_tx_hash_hex: normalizeHex32String(
        record.submitted_tx_hash_hex,
        `${context}.submitted_tx_hash_hex`,
      ),
      submitted_height: ToriiClient._normalizeUnsignedInteger(
        record.submitted_height,
        `${context}.submitted_height`,
        { allowZero: true },
      ),
    };
  }

  function normalizeMinistryAgendaProposalDraftResponse(
    payload,
    context = "ministry agenda proposal draft response",
  ) {
    const record = ensureRecord(payload, context);
    const base = normalizeGovernanceDraftResponse(
      {
        ok: record.ok,
        tx_instructions: record.tx_instructions ?? [],
      },
      context,
    );
    return {
      ok: base.ok,
      agenda_proposal_id: requireNonEmptyString(
        record.agenda_proposal_id,
        `${context}.agenda_proposal_id`,
      ),
      authority: requireNonEmptyString(record.authority, `${context}.authority`),
      tx_instructions: base.tx_instructions,
      signable_transaction_b64: requireNonEmptyString(
        record.signable_transaction_b64,
        `${context}.signable_transaction_b64`,
      ),
    };
  }

  function normalizeMinistryAgendaProposalGetResponse(
    payload,
    context = "ministry agenda proposal lookup response",
  ) {
    const record = ensureRecord(payload, context);
    const found = Boolean(record.found);
    const proposalRecord =
      record.record === undefined || record.record === null
        ? null
        : normalizeMinistryAgendaProposalRecord(record.record, `${context}.record`);
    return {
      found,
      record: proposalRecord,
    };
  }

  function normalizeGovernanceWindow(value, name) {
    const record = ensureRecord(value, name);
    assertSupportedOptionKeys(record, GOVERNANCE_WINDOW_KEYS, name);
    const lowerValue = record.lower;
    const upperValue = record.upper;
    if (lowerValue === undefined || upperValue === undefined) {
      const basePath = normalizeErrorPath(name);
      if (lowerValue === undefined) {
        throw createValidationError(
          ValidationErrorCode.INVALID_NUMERIC,
          `${name}.lower is required`,
          `${basePath}.lower`,
        );
      }
      throw createValidationError(
        ValidationErrorCode.INVALID_NUMERIC,
        `${name}.upper is required`,
        `${basePath}.upper`,
      );
    }
    const lower = normalizeGovernanceUint64Integer(lowerValue, `${name}.lower`);
    const upper = normalizeGovernanceUint64Integer(upperValue, `${name}.upper`);
    if (upper < lower) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name}.upper must be greater than or equal to lower`,
        `${normalizeErrorPath(name)}.upper`,
      );
    }
    return { lower, upper };
  }

  function normalizeGovernanceVotingMode(value, name) {
    const canonical = requireExactNonEmptyString(value, name);
    if (canonical === "Zk" || canonical === "Plain") {
      return canonical;
    }
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be either 'Zk' or 'Plain'`,
      normalizeErrorPath(name),
    );
  }

  function normalizeGovernanceDraftResponse(
    payload,
    context = "governance draft response",
  ) {
    const record = ensureRecord(payload, context);
    const instructionsValue = record.tx_instructions ?? [];
    if (!Array.isArray(instructionsValue)) {
      throw new TypeError(`${context}.tx_instructions must be an array`);
    }
    const txInstructions = instructionsValue.map((entry, index) => {
      const item = ensureRecord(entry, `${context}.tx_instructions[${index}]`);
      const wireId = requireNonEmptyString(
        item.wire_id,
        `${context}.tx_instructions[${index}].wire_id`,
      );
      const payloadHexValue = item.payload_hex;
      const normalizedPayload =
        payloadHexValue === undefined || payloadHexValue === null
          ? null
          : normalizeArbitraryHex(
              payloadHexValue,
              `${context}.tx_instructions[${index}].payload_hex`,
            );
      return normalizedPayload === null
        ? { wire_id: wireId }
        : { wire_id: wireId, payload_hex: normalizedPayload };
    });
    let proposalId = record.proposal_id ?? null;
    if (proposalId !== null && proposalId !== undefined) {
      proposalId = normalizeHex32String(proposalId, `${context}.proposal_id`);
    } else {
      proposalId = null;
    }
    const normalized = {
      ok: Boolean(record.ok),
      proposal_id: proposalId,
      tx_instructions: txInstructions,
    };
    if (record.accepted !== undefined) {
      normalized.accepted = Boolean(record.accepted);
    }
    if (record.reason !== undefined) {
      normalized.reason =
        record.reason === null || record.reason === undefined
          ? null
          : requireNonEmptyString(record.reason, `${context}.reason`);
    }
    return normalized;
  }

  function createEmptyGovernanceDraftResponse(context) {
    return normalizeGovernanceDraftResponse(
      {
        ok: true,
        tx_instructions: [],
      },
      context,
    );
  }

  function normalizeTriggerMutationResponse(
    payload,
    context = "trigger mutation response",
  ) {
    const record = ensureRecord(payload, context);
    const base = normalizeGovernanceDraftResponse(record, context);
    let triggerId = record.trigger_id ?? null;
    if (triggerId !== null && triggerId !== undefined) {
      triggerId = requireNonEmptyString(triggerId, `${context}.trigger_id`);
    } else {
      triggerId = null;
    }
    const messageValue = record.message ?? null;
    const message =
      messageValue === null || messageValue === undefined
        ? null
        : String(messageValue);
    const normalized = {
      ok: base.ok,
      trigger_id: triggerId,
      tx_instructions: base.tx_instructions,
    };
    if (base.accepted !== undefined) {
      normalized.accepted = base.accepted;
    }
    if (message !== null && message.length > 0) {
      normalized.message = message;
    }
    return normalized;
  }

  function normalizeGovernanceBallotResponse(payload, context) {
    const record = ensureRecord(payload, context);
    if (record.accepted === undefined) {
      throw new TypeError(`${context}.accepted is required`);
    }
    const base = normalizeGovernanceDraftResponse(record, context);
    const reason =
      record.reason === undefined || record.reason === null
        ? null
        : requireNonEmptyString(record.reason, `${context}.reason`);
    return {
      ok: base.ok,
      proposal_id: base.proposal_id,
      tx_instructions: base.tx_instructions,
      accepted: Boolean(record.accepted),
      reason,
    };
  }

  function normalizeGovernanceDeployContractProposalPayload(input) {
    const context = "governanceProposeDeployContract payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_DEPLOY_CONTRACT_REQUEST_KEYS, context);
    const contractAddressValue = record.contractAddress ?? null;
    const contractAliasValue = record.contractAlias ?? null;
    if ((contractAddressValue == null) === (contractAliasValue == null)) {
      throw new TypeError(
        "governanceProposeDeployContract requires exactly one of contractAddress or contractAlias",
      );
    }
    const abiVersionValue =
      record.abiVersion === undefined ? "1" : record.abiVersion;
    const abiVersion = requireExactNonEmptyString(
      abiVersionValue,
      "governanceProposeDeployContract.abiVersion",
    );
    if (abiVersion !== "1") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        "governanceProposeDeployContract.abiVersion must be exactly '1'",
        "governanceProposeDeployContract.abiVersion",
      );
    }
    const codeHashValue = record.codeHash;
    if (codeHashValue === undefined || codeHashValue === null) {
      throw new TypeError("governanceProposeDeployContract.codeHash is required");
    }
    const abiHashValue = record.abiHash;
    if (abiHashValue === undefined || abiHashValue === null) {
      throw new TypeError("governanceProposeDeployContract.abiHash is required");
    }
    const payload = {
      abi_version: abiVersion,
      code_hash: normalizeHex32String(
        codeHashValue,
        "governanceProposeDeployContract.codeHash",
        { allowScheme: true, scheme: "blake2b32", exactString: true },
      ),
      abi_hash: normalizeHex32String(
        abiHashValue,
        "governanceProposeDeployContract.abiHash",
        { allowScheme: true, scheme: "blake2b32", exactString: true },
      ),
    };
    if (contractAddressValue != null) {
      payload.contract_address = requireNonEmptyString(
        contractAddressValue,
        "governanceProposeDeployContract.contractAddress",
      );
    } else {
      payload.contract_alias = requireNonEmptyString(
        contractAliasValue,
        "governanceProposeDeployContract.contractAlias",
      );
    }
    const windowValue =
      record.window;
    if (windowValue !== undefined && windowValue !== null) {
      payload.window = normalizeGovernanceWindow(
        windowValue,
        "governanceProposeDeployContract.window",
      );
    }
    const modeValue = record.mode;
    if (modeValue !== undefined && modeValue !== null) {
      payload.mode = normalizeGovernanceVotingMode(
        modeValue,
        "governanceProposeDeployContract.mode",
      );
    }
    const manifestProvenance = record.manifestProvenance;
    if (manifestProvenance !== undefined) {
      payload.manifest_provenance =
        manifestProvenance === null
          ? null
          : normalizeGovernanceManifestProvenancePayload(
              manifestProvenance,
              "governanceProposeDeployContract.manifestProvenance",
            );
    }
    return payload;
  }

  function normalizeGovernanceManifestProvenancePayload(value, context) {
    const record = ensureRecord(value, context);
    assertSupportedOptionKeys(record, GOVERNANCE_MANIFEST_PROVENANCE_KEYS, context);
    return normalizeManifestProvenancePayload(record, context);
  }

  function normalizeGovernancePlainBallotPayload(input) {
    const context = "governanceSubmitPlainBallot payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_PLAIN_BALLOT_REQUEST_KEYS, context);
    const direction = record.direction;
    const payload = {
      authority: ToriiClient._normalizeAccountId(
        record.authority,
        "governanceSubmitPlainBallot.authority",
      ),
      chain_id: requireExactTokenString(
        record.chainId,
        "governanceSubmitPlainBallot.chainId",
      ),
      referendum_id: requireGovernanceSelectorString(
        record.referendumId,
        "governanceSubmitPlainBallot.referendumId",
      ),
      owner: ToriiClient._normalizeAccountId(
        record.owner,
        "governanceSubmitPlainBallot.owner",
      ),
      amount: normalizeQuantityInput(
        record.amount,
        "governanceSubmitPlainBallot.amount",
      ),
      duration_blocks: normalizeUint64DecimalString(
        record.durationBlocks,
        "governanceSubmitPlainBallot.durationBlocks",
        { allowZero: true },
      ),
      direction: normalizeGovernanceBallotDirection(
        direction,
        "governanceSubmitPlainBallot.direction",
      ),
    };
    return payload;
  }

  function normalizeGovernanceParliamentBallotPayload(input) {
    const context = "governanceSubmitParliamentBallot payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_PARLIAMENT_BALLOT_REQUEST_KEYS, context);
    return {
      authority: ToriiClient._normalizeAccountId(
        record.authority,
        "governanceSubmitParliamentBallot.authority",
      ),
      chain_id: requireExactTokenString(
        record.chainId,
        "governanceSubmitParliamentBallot.chainId",
      ),
      proposal_id: normalizeHex32String(
        record.proposalId,
        "governanceSubmitParliamentBallot.proposalId",
        { allowScheme: true, scheme: "blake2b32", exactString: true },
      ),
      body: normalizeGovernanceParliamentBody(
        record.body,
        "governanceSubmitParliamentBallot.body",
      ),
      decision: normalizeGovernanceParliamentDecision(
        record.decision,
        "governanceSubmitParliamentBallot.decision",
      ),
    };
  }

  function normalizeGovernanceParliamentBody(value, name) {
    const body = requireExactTokenString(value, name);
    if (!GOVERNANCE_PARLIAMENT_BODIES.has(body)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must name a canonical Parliament body`,
        name,
      );
    }
    return body;
  }

  function normalizeGovernanceParliamentDecision(value, name) {
    const decision = requireExactTokenString(value, name);
    if (!GOVERNANCE_PARLIAMENT_DECISIONS.has(decision)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be approve, reject, or abstain`,
        name,
      );
    }
    return decision;
  }

  function normalizeGovernanceBallotDirection(value, name) {
    const canonical = requireExactNonEmptyString(value, name);
    if (canonical === "Aye" || canonical === "Nay" || canonical === "Abstain") {
      return canonical;
    }
    throw new TypeError(`${name} must be one of Aye, Nay, or Abstain`);
  }

  function ensureGovernanceLockHintsComplete(source, name) {
    const hasOwner = source.owner !== undefined && source.owner !== null;
    const hasAmount = source.amount !== undefined && source.amount !== null;
    const hasDuration =
      source.durationBlocks !== undefined && source.durationBlocks !== null;
    const hasAnyLockHint = hasOwner || hasAmount || hasDuration;
    if (hasAnyLockHint && !(hasOwner && hasAmount && hasDuration)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${name} must include owner, amount, and durationBlocks when providing lock hints`,
        name,
      );
    }
  }

  function normalizeGovernanceZkBallotV1Payload(input) {
    const context = "governanceSubmitZkBallotV1 payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_ZK_BALLOT_V1_REQUEST_KEYS, context);
    const payload = {
      authority: ToriiClient._normalizeAccountId(
        record.authority,
        "governanceSubmitZkBallotV1.authority",
      ),
      chain_id: requireExactTokenString(
        record.chainId,
        "governanceSubmitZkBallotV1.chainId",
      ),
      election_id: requireGovernanceSelectorString(
        record.electionId,
        "governanceSubmitZkBallotV1.electionId",
      ),
      backend: requireExactTokenString(
        record.backend,
        "governanceSubmitZkBallotV1.backend",
      ),
      envelope_b64: normalizeRequiredBase64Payload(
        record.envelope,
        "governanceSubmitZkBallotV1.envelope",
      ),
    };
    ensureGovernanceLockHintsComplete(record, "governanceSubmitZkBallotV1");
    const rootHint = record.rootHint;
    if (rootHint !== undefined && rootHint !== null) {
      payload.root_hint = normalizeHex32String(
        rootHint,
        "governanceSubmitZkBallotV1.rootHint",
        { allowScheme: true, scheme: "blake2b32", exactString: true },
      );
    }
    if (record.owner !== undefined && record.owner !== null) {
      payload.owner = requireNonEmptyString(
        record.owner,
        "governanceSubmitZkBallotV1.owner",
      );
    }
    if (record.amount !== undefined && record.amount !== null) {
      payload.amount = normalizeQuantityInput(
        record.amount,
        "governanceSubmitZkBallotV1.amount",
      );
    }
    const durationBlocks = record.durationBlocks;
    if (durationBlocks !== undefined && durationBlocks !== null) {
      payload.duration_blocks = normalizeGovernanceUint64Integer(
        durationBlocks,
        "governanceSubmitZkBallotV1.durationBlocks",
      );
    }
    if (record.direction !== undefined && record.direction !== null) {
      payload.direction = normalizeGovernanceBallotDirection(
        record.direction,
        "governanceSubmitZkBallotV1.direction",
      );
    }
    const nullifier = record.nullifier;
    if (nullifier !== undefined && nullifier !== null) {
      payload.nullifier = normalizeHex32String(
        nullifier,
        "governanceSubmitZkBallotV1.nullifier",
        { allowScheme: true, scheme: "blake2b32", exactString: true },
      );
    }
    if (payload.owner !== undefined && payload.owner !== null) {
      payload.owner = ensureCanonicalAccountId(
        payload.owner,
        "governanceSubmitZkBallotV1.owner",
      );
    }
    return payload;
  }

  function normalizeGovernanceZkBallotProofPayload(input) {
    const context = "governanceSubmitZkBallotProofV1 payload";
    const record = ensureRecord(input, context);
    rejectGovernancePrivateKeyFieldsDeep(record, context);
    assertSupportedOptionKeys(record, GOVERNANCE_ZK_BALLOT_PROOF_REQUEST_KEYS, context);
    const ballot = ensureRecord(
      record.ballot,
      "governanceSubmitZkBallotProofV1.ballot",
    );
    const ballotContext = "governanceSubmitZkBallotProofV1.ballot";
    rejectGovernancePrivateKeyFieldsDeep(ballot, ballotContext);
    assertSupportedOptionKeys(ballot, GOVERNANCE_BALLOT_PROOF_KEYS, ballotContext);
    ensureGovernanceLockHintsComplete(ballot, ballotContext);
    if (ballot.envelopeBytes === undefined || ballot.envelopeBytes === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${ballotContext}.envelopeBytes is required`,
        `${ballotContext}.envelopeBytes`,
      );
    }
    const normalizedBallot = {
      backend: requireExactTokenString(ballot.backend, `${ballotContext}.backend`),
      envelope_bytes: normalizeRequiredBase64Payload(
        ballot.envelopeBytes,
        `${ballotContext}.envelopeBytes`,
      ),
    };
    if (Object.prototype.hasOwnProperty.call(ballot, "rootHint")) {
      normalizedBallot.root_hint =
        ballot.rootHint === null
          ? null
          : normalizeHex32String(ballot.rootHint, `${ballotContext}.rootHint`, {
              allowScheme: true,
              scheme: "blake2b32",
              exactString: true,
            });
    }
    if (Object.prototype.hasOwnProperty.call(ballot, "nullifier")) {
      normalizedBallot.nullifier =
        ballot.nullifier === null
          ? null
          : normalizeHex32String(ballot.nullifier, `${ballotContext}.nullifier`, {
              allowScheme: true,
              scheme: "blake2b32",
              exactString: true,
            });
    }
    if (Object.prototype.hasOwnProperty.call(ballot, "amount")) {
      normalizedBallot.amount =
        ballot.amount === null
          ? null
          : normalizeQuantityInput(ballot.amount, `${ballotContext}.amount`);
    }
    if (Object.prototype.hasOwnProperty.call(ballot, "owner")) {
      normalizedBallot.owner =
        ballot.owner === null
          ? null
          : ensureCanonicalAccountId(ballot.owner, `${ballotContext}.owner`);
    }
    if (Object.prototype.hasOwnProperty.call(ballot, "durationBlocks")) {
      normalizedBallot.duration_blocks =
        ballot.durationBlocks === null
          ? null
          : normalizeGovernanceUint64Integer(
              ballot.durationBlocks,
              `${ballotContext}.durationBlocks`,
            );
    }
    if (Object.prototype.hasOwnProperty.call(ballot, "direction")) {
      normalizedBallot.direction =
        ballot.direction === null
          ? null
          : normalizeGovernanceBallotDirection(
              ballot.direction,
              `${ballotContext}.direction`,
            );
    }
    const payload = {
      authority: ToriiClient._normalizeAccountId(
        record.authority,
        "governanceSubmitZkBallotProofV1.authority",
      ),
      chain_id: requireExactTokenString(
        record.chainId,
        "governanceSubmitZkBallotProofV1.chainId",
      ),
      election_id: requireGovernanceSelectorString(
        record.electionId,
        "governanceSubmitZkBallotProofV1.electionId",
      ),
      ballot: normalizedBallot,
    };
    return payload;
  }

  return {
    createEmptyGovernanceDraftResponse,
    normalizeGovernanceBallotResponse,
    normalizeGovernanceDeployContractProposalPayload,
    normalizeGovernanceDraftResponse,
    normalizeGovernanceEnactPayload,
    normalizeGovernanceFinalizePayload,
    normalizeGovernanceParliamentBallotPayload,
    normalizeGovernancePlainBallotPayload,
    normalizeGovernanceZkBallotProofPayload,
    normalizeGovernanceZkBallotV1Payload,
    normalizeMinistryAgendaProposalDraftRequest,
    normalizeMinistryAgendaProposalDraftResponse,
    normalizeMinistryAgendaProposalGetResponse,
    normalizeTriggerMutationResponse,
  };
}
