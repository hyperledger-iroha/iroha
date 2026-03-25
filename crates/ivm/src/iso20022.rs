//! Basic scaffolding for ISO 20022 message handling opcodes.
//!
//! The real implementation will rely on generated schema information for every
//! ISO 20022 message type.  For now we keep the design deliberately simple so
//! that other parts of the VM can start integrating with the API.  The
//! functions below maintain a very small in-memory *stack* of messages which
//! allows opcodes like `MSG_CLONE` to duplicate the current message.  This keeps
//! the API lightweight while still letting tests and prototypes exercise opcode
//! behaviour without bringing in heavy dependencies or parsing logic.

use core::fmt;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    io::Write,
    str::FromStr,
};

use ed25519_dalek::{Signer as _, SigningKey};
use iroha_crypto::{Algorithm, EcdsaSecp256k1Sha256};

use crate::signature::{SignatureScheme, verify_signature};

/// Extremely small ISO 20022 message representation used for testing.
#[derive(Clone, Default)]
struct IsoMessage {
    /// Identifier such as `pacs.008`.
    message_type: String,
    /// Flat map of field name to encoded value.
    fields: HashMap<String, Vec<u8>>,
    /// Counters for `MSG_ADD` to emulate repeating fields.
    repeats: HashMap<String, usize>,
}

thread_local! {
    /// Thread-local stack of ISO 20022 messages.  The most recently created
    /// or parsed message lives at the top of the stack.
    static MESSAGE_STACK: RefCell<Vec<IsoMessage>> = const { RefCell::new(Vec::new()) };
    /// Last validation failure recorded by [`msg_validate`].
    static LAST_VALIDATION_FAILURE: RefCell<Option<ValidationFailure>> =
        const { RefCell::new(None) };
}

/// Return the list of fields that must be present for the given message type.
///
/// This is a tiny stand-in for schema driven validation. Only a handful of
/// message types are recognised and each lists a couple of representative
/// mandatory fields. The map can be extended over time as more messages are
/// supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Requirement {
    Required,
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierKind {
    /// International Securities Identification Number (ISO 6166)
    Isin,
    /// Committee on Uniform Securities Identification Procedures
    Cusip,
    /// Legal Entity Identifier (ISO 17442)
    Lei,
    /// Business Identifier Code (ISO 9362)
    Bic,
    /// Market Identifier Code (ISO 10383)
    Mic,
    /// International Bank Account Number (ISO 13616 / 7064 checksum)
    Iban,
    /// ISO 4217 currency code
    Currency,
}

impl IdentifierKind {
    fn label(self) -> &'static str {
        match self {
            IdentifierKind::Isin => "ISIN",
            IdentifierKind::Cusip => "CUSIP",
            IdentifierKind::Lei => "LEI",
            IdentifierKind::Bic => "BIC",
            IdentifierKind::Mic => "MIC",
            IdentifierKind::Iban => "IBAN",
            IdentifierKind::Currency => "ISO 4217 currency",
        }
    }
}

impl fmt::Display for IdentifierKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Text,
    Numeric,
    Amount,
    Identifier(IdentifierKind),
    Instrument,
    Date,
    DateTime,
    Enum(&'static [&'static str]),
}

#[derive(Clone, Debug)]
enum InvalidReason {
    Empty,
    Numeric,
    Amount,
    Identifier(IdentifierKind),
    Instrument,
    Date,
    DateTime,
    Enum,
    Utf8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidValueKind {
    Empty,
    Numeric,
    Amount,
    Date,
    DateTime,
    Enum,
    Utf8,
}

impl InvalidValueKind {
    fn label(self) -> &'static str {
        match self {
            InvalidValueKind::Empty => "empty",
            InvalidValueKind::Numeric => "numeric",
            InvalidValueKind::Amount => "amount",
            InvalidValueKind::Date => "date",
            InvalidValueKind::DateTime => "date-time",
            InvalidValueKind::Enum => "enumerated",
            InvalidValueKind::Utf8 => "UTF-8",
        }
    }
}

impl fmt::Display for InvalidValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
enum ValidationFailure {
    MissingField(&'static str),
    TooManyOccurrences {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    InvalidField {
        field: String,
        reason: InvalidReason,
    },
}

#[derive(Clone, Copy, Debug)]
struct AliasSpec {
    alias: &'static str,
    canonical: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct FieldSpec {
    pattern: &'static str,
    requirement: Requirement,
    max_occurs: Option<usize>,
    kind: FieldKind,
}

impl FieldSpec {
    const fn required(pattern: &'static str, kind: FieldKind) -> Self {
        Self {
            pattern,
            requirement: Requirement::Required,
            max_occurs: None,
            kind,
        }
    }

    const fn optional(pattern: &'static str, kind: FieldKind) -> Self {
        Self {
            pattern,
            requirement: Requirement::Optional,
            max_occurs: None,
            kind,
        }
    }

    const fn limited(
        pattern: &'static str,
        min_required: bool,
        max: usize,
        kind: FieldKind,
    ) -> Self {
        Self {
            pattern,
            requirement: if min_required {
                Requirement::Required
            } else {
                Requirement::Optional
            },
            max_occurs: Some(max),
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MessageSchema {
    fields: &'static [FieldSpec],
    aliases: &'static [AliasSpec],
}

impl MessageSchema {
    fn field_specs(&self) -> &'static [FieldSpec] {
        self.fields
    }

    fn aliases(&self) -> &'static [AliasSpec] {
        self.aliases
    }
}

fn canonical_message_type(message_type: &str) -> Cow<'_, str> {
    let parts: Vec<&str> = message_type.split('.').collect();
    if parts.len() >= 4
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        Cow::Owned(format!("{}.{}", parts[0], parts[1]))
    } else {
        Cow::Borrowed(message_type)
    }
}

fn record_validation_failure(failure: ValidationFailure) {
    LAST_VALIDATION_FAILURE.with(|cell| {
        *cell.borrow_mut() = Some(failure);
    });
}

fn take_validation_failure() -> Option<ValidationFailure> {
    LAST_VALIDATION_FAILURE.with(|cell| cell.borrow_mut().take())
}

fn clear_validation_failure() {
    LAST_VALIDATION_FAILURE.with(|cell| {
        cell.borrow_mut().take();
    });
}

fn schema_for(message_type: &str) -> Option<&'static MessageSchema> {
    match canonical_message_type(message_type).as_ref() {
        "head.001" => Some(&HEAD001_SCHEMA),
        "colr.007" => Some(&COLR007_SCHEMA),
        "pacs.008" => Some(&PACS008_SCHEMA),
        "pacs.002" => Some(&PACS002_SCHEMA),
        "pacs.004" => Some(&PACS004_SCHEMA),
        "pacs.028" => Some(&PACS028_SCHEMA),
        "pacs.029" => Some(&PACS029_SCHEMA),
        "camt.052" => Some(&CAMT052_SCHEMA),
        "camt.053" => Some(&CAMT053_SCHEMA),
        "camt.054" => Some(&CAMT054_SCHEMA),
        "camt.056" => Some(&CAMT056_SCHEMA),
        "pain.001" => Some(&PAIN001_SCHEMA),
        "pain.002" => Some(&PAIN002_SCHEMA),
        "pacs.007" => Some(&PACS007_SCHEMA),
        "pacs.009" => Some(&PACS009_SCHEMA),
        "sese.023" => Some(&SESE023_SCHEMA),
        "sese.025" => Some(&SESE025_SCHEMA),
        _ => None,
    }
}

const HEAD001_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("AppHdr/BizMsgIdr", FieldKind::Text),
    FieldSpec::required("AppHdr/MsgDefIdr", FieldKind::Text),
    FieldSpec::required("AppHdr/CreDt", FieldKind::DateTime),
    FieldSpec::optional(
        "AppHdr/Fr/FIId/FinInstnId/BICFI",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::optional(
        "AppHdr/To/FIId/FinInstnId/BICFI",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::optional("AppHdr/BizSvc", FieldKind::Text),
    FieldSpec::optional(
        "AppHdr/Fr/FIId/FinInstnId/ClrSysMmbId/MmbId",
        FieldKind::Text,
    ),
    FieldSpec::optional(
        "AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId",
        FieldKind::Text,
    ),
];

const HEAD001_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "DataPDU/AppHdr/BizMsgIdr",
        canonical: "AppHdr/BizMsgIdr",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/MsgDefIdr",
        canonical: "AppHdr/MsgDefIdr",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/CreDt",
        canonical: "AppHdr/CreDt",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/Fr/FIId/FinInstnId/BICFI",
        canonical: "AppHdr/Fr/FIId/FinInstnId/BICFI",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/To/FIId/FinInstnId/BICFI",
        canonical: "AppHdr/To/FIId/FinInstnId/BICFI",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/BizSvc",
        canonical: "AppHdr/BizSvc",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/Fr/FIId/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "AppHdr/Fr/FIId/FinInstnId/ClrSysMmbId/MmbId",
    },
    AliasSpec {
        alias: "DataPDU/AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId",
    },
];

const HEAD001_SCHEMA: MessageSchema = MessageSchema {
    fields: HEAD001_FIELDS,
    aliases: HEAD001_ALIASES,
};

const PACS008_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("MsgId", FieldKind::Text),
    FieldSpec::required(
        "IntrBkSttlmCcy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::required("IntrBkSttlmAmt", FieldKind::Amount),
    FieldSpec::required("IntrBkSttlmDt", FieldKind::Date),
    FieldSpec::required("DbtrAcct", FieldKind::Identifier(IdentifierKind::Iban)),
    FieldSpec::required("CdtrAcct", FieldKind::Identifier(IdentifierKind::Iban)),
    FieldSpec::required("DbtrAgt", FieldKind::Identifier(IdentifierKind::Bic)),
    FieldSpec::required("CdtrAgt", FieldKind::Identifier(IdentifierKind::Bic)),
    FieldSpec::optional("DbtrAcct/Prxy/Id", FieldKind::Text),
    FieldSpec::optional("DbtrAcct/Prxy/Tp/Cd", FieldKind::Text),
    FieldSpec::optional("DbtrAcct/Prxy/Tp/Prtry", FieldKind::Text),
    FieldSpec::optional("CdtrAcct/Prxy/Id", FieldKind::Text),
    FieldSpec::optional("CdtrAcct/Prxy/Tp/Cd", FieldKind::Text),
    FieldSpec::optional("CdtrAcct/Prxy/Tp/Prtry", FieldKind::Text),
    FieldSpec::optional("ChrgBr", FieldKind::Enum(&["DEBT", "CRED", "SHAR", "SLEV"])),
    FieldSpec::limited("RmtInf/Ustrd[*]", false, 10, FieldKind::Text),
    FieldSpec::optional("SplmtryData/LedgerId", FieldKind::Text),
    FieldSpec::optional("SplmtryData/SourceAccountId", FieldKind::Text),
    FieldSpec::optional("SplmtryData/SourceAccountAddress", FieldKind::Text),
    FieldSpec::optional("SplmtryData/TargetAccountId", FieldKind::Text),
    FieldSpec::optional("SplmtryData/TargetAccountAddress", FieldKind::Text),
    FieldSpec::optional("SplmtryData/AssetDefinitionId", FieldKind::Text),
];

const PACS008_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/GrpHdr/MsgId",
        canonical: "MsgId",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/IntrBkSttlmAmt",
        canonical: "IntrBkSttlmAmt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/IntrBkSttlmAmt/@Ccy",
        canonical: "IntrBkSttlmCcy",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/IntrBkSttlmDt",
        canonical: "IntrBkSttlmDt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAcct/Id/IBAN",
        canonical: "DbtrAcct",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAcct/Id/IBAN",
        canonical: "CdtrAcct",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "DbtrAgt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAgt/FinInstnId/BICFI",
        canonical: "DbtrAgt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "CdtrAgt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAgt/FinInstnId/BICFI",
        canonical: "CdtrAgt",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAcct/Prxy/Id",
        canonical: "DbtrAcct/Prxy/Id",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAcct/Prxy/Tp/Cd",
        canonical: "DbtrAcct/Prxy/Tp/Cd",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/DbtrAcct/Prxy/Tp/Prtry",
        canonical: "DbtrAcct/Prxy/Tp/Prtry",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAcct/Prxy/Id",
        canonical: "CdtrAcct/Prxy/Id",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAcct/Prxy/Tp/Cd",
        canonical: "CdtrAcct/Prxy/Tp/Cd",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/CdtrAcct/Prxy/Tp/Prtry",
        canonical: "CdtrAcct/Prxy/Tp/Prtry",
    },
    AliasSpec {
        alias: "Document/FIToFICstmrCdtTrf/CdtTrfTxInf/RmtInf/Ustrd[*]",
        canonical: "RmtInf/Ustrd[*]",
    },
];

const PACS008_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS008_FIELDS,
    aliases: PACS008_ALIASES,
};

const PACS002_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("OrgnlMsgId", FieldKind::Text),
    FieldSpec::optional(
        "TxSts",
        FieldKind::Enum(&["ACTC", "ACSC", "ACSP", "RJCT", "PDNG", "PART"]),
    ),
    FieldSpec::optional("RsnCd", FieldKind::Text),
    FieldSpec::limited("AddtlInf[*]", false, 8, FieldKind::Text),
];

const PACS002_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/OrgnlMsgId",
        canonical: "OrgnlMsgId",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/TxInfAndSts/TxSts",
        canonical: "TxSts",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/TxInfAndSts/StsRsnInf/Rsn/Cd",
        canonical: "RsnCd",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/TxInfAndSts/StsRsnInf/Rsn/Prtry",
        canonical: "RsnCd",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/StsRsnInf/Rsn/Cd",
        canonical: "RsnCd",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/StsRsnInf/Rsn/Prtry",
        canonical: "RsnCd",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/GrpSts",
        canonical: "TxSts",
    },
    AliasSpec {
        alias: "Document/FIToFIPmtStsRpt/TxInfAndSts/AddtlInf[*]",
        canonical: "AddtlInf[*]",
    },
];

const PACS002_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS002_FIELDS,
    aliases: PACS002_ALIASES,
};

const PACS004_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("MsgId", FieldKind::Text),
    FieldSpec::required("CreDtTm", FieldKind::DateTime),
    FieldSpec::required("OrgnlGrpInf/OrgnlMsgId", FieldKind::Text),
    FieldSpec::limited("TxInf[*]/OrgnlInstrId", true, 1024, FieldKind::Text),
    FieldSpec::limited("TxInf[*]/RtrdInstdAmt", true, 1024, FieldKind::Amount),
    FieldSpec::limited(
        "TxInf[*]/RtrdInstdAmtCcy",
        true,
        1024,
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::optional(
        "TxInf[*]/ChrgBr",
        FieldKind::Enum(&["DEBT", "CRED", "SHAR", "SLEV"]),
    ),
    FieldSpec::optional("TxInf[*]/RtrdRsn/Cd", FieldKind::Text),
    FieldSpec::optional("TxInf[*]/RtrdRsn/Prtry", FieldKind::Text),
];

const PACS004_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS004_FIELDS,
    aliases: &[
        AliasSpec {
            alias: "Document/PmtRtr/GrpHdr/MsgId",
            canonical: "MsgId",
        },
        AliasSpec {
            alias: "Document/PmtRtr/GrpHdr/CreDtTm",
            canonical: "CreDtTm",
        },
        AliasSpec {
            alias: "Document/PmtRtr/OrgnlGrpInf/OrgnlMsgId",
            canonical: "OrgnlGrpInf/OrgnlMsgId",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/OrgnlInstrId",
            canonical: "TxInf[*]/OrgnlInstrId",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/RtrdIntrBkSttlmAmt",
            canonical: "TxInf[*]/RtrdInstdAmt",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/RtrdIntrBkSttlmAmt/@Ccy",
            canonical: "TxInf[*]/RtrdInstdAmtCcy",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/TxRtrdInstdAmtCcy",
            canonical: "TxInf[*]/RtrdInstdAmtCcy",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/ChrgBr",
            canonical: "TxInf[*]/ChrgBr",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/RtrRsnInf/Rsn/Cd",
            canonical: "TxInf[*]/RtrdRsn/Cd",
        },
        AliasSpec {
            alias: "Document/PmtRtr/TxInf[*]/RtrRsnInf/Rsn/Prtry",
            canonical: "TxInf[*]/RtrdRsn/Prtry",
        },
    ],
};

const PACS028_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("MsgId", FieldKind::Text),
    FieldSpec::required("CreDtTm", FieldKind::DateTime),
    FieldSpec::required("OrgnlGrpInf/OrgnlMsgId", FieldKind::Text),
    FieldSpec::optional("OrgnlGrpInf/OrgnlCreDtTm", FieldKind::DateTime),
];

const PACS028_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS028_FIELDS,
    aliases: &[],
};

const PACS029_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("MsgId", FieldKind::Text),
    FieldSpec::required("CreDtTm", FieldKind::DateTime),
    FieldSpec::required("OrgnlGrpInf/OrgnlMsgId", FieldKind::Text),
    FieldSpec::limited(
        "TxInfAndSts[*]/TxSts",
        true,
        1024,
        FieldKind::Enum(&["ACSC", "RJCT", "PDNG", "ACSP", "MS02", "MS03"]),
    ),
    FieldSpec::optional("TxInfAndSts[*]/StsRsnInf/Rsn/Cd", FieldKind::Text),
];

const PACS029_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS029_FIELDS,
    aliases: &[],
};

const CAMT052_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("Rpt/Id", FieldKind::Text),
    FieldSpec::required("Rpt/CreDtTm", FieldKind::DateTime),
    FieldSpec::required("Rpt/Acct/Id", FieldKind::Text),
    FieldSpec::required(
        "Rpt/Acct/Ccy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::limited("Rpt/Ntry[*]/Amt", false, 256, FieldKind::Amount),
    FieldSpec::limited(
        "Rpt/Ntry[*]/CdtDbtInd",
        false,
        256,
        FieldKind::Enum(&["CRDT", "DBIT"]),
    ),
    FieldSpec::limited("Rpt/Ntry[*]/BookgDt", false, 256, FieldKind::Date),
];

const CAMT052_SCHEMA: MessageSchema = MessageSchema {
    fields: CAMT052_FIELDS,
    aliases: &[
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Id",
            canonical: "Rpt/Id",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/CreDtTm",
            canonical: "Rpt/CreDtTm",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Acct/Id/IBAN",
            canonical: "Rpt/Acct/Id",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Acct/Id/Othr/Id",
            canonical: "Rpt/Acct/Id",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Acct/Ccy",
            canonical: "Rpt/Acct/Ccy",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Ntry[*]/Amt",
            canonical: "Rpt/Ntry[*]/Amt",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Ntry[*]/CdtDbtInd",
            canonical: "Rpt/Ntry[*]/CdtDbtInd",
        },
        AliasSpec {
            alias: "Document/BkToCstmrAcctRpt/Rpt/Ntry[*]/BookgDt/Dt",
            canonical: "Rpt/Ntry[*]/BookgDt",
        },
    ],
};

const CAMT053_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("Stmt/Id", FieldKind::Text),
    FieldSpec::required("Stmt/Acct/Id", FieldKind::Identifier(IdentifierKind::Iban)),
    FieldSpec::required(
        "Stmt/Acct/Ccy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::limited("Stmt/Bal[*]/Amt", true, 64, FieldKind::Amount),
    FieldSpec::limited(
        "Stmt/Bal[*]/Ccy",
        true,
        64,
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::limited(
        "Stmt/Bal[*]/Cd",
        true,
        64,
        FieldKind::Enum(&["CRDT", "DBIT", "PRCD", "OPBD"]),
    ),
    FieldSpec::optional("Stmt/Bal[*]/Dt", FieldKind::Date),
];

const CAMT053_SCHEMA: MessageSchema = MessageSchema {
    fields: CAMT053_FIELDS,
    aliases: &[],
};

const CAMT054_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("Ntfctn/Id", FieldKind::Text),
    FieldSpec::required(
        "Ntfctn/Acct/Id",
        FieldKind::Identifier(IdentifierKind::Iban),
    ),
    FieldSpec::limited("Ntfctn/Ntry[*]/Amt", true, 256, FieldKind::Amount),
    FieldSpec::limited(
        "Ntfctn/Ntry[*]/Ccy",
        true,
        256,
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::limited(
        "Ntfctn/Ntry[*]/CdtDbtInd",
        true,
        256,
        FieldKind::Enum(&["CRDT", "DBIT"]),
    ),
    FieldSpec::optional("Ntfctn/Ntry[*]/BookgDt", FieldKind::Date),
];

const CAMT054_SCHEMA: MessageSchema = MessageSchema {
    fields: CAMT054_FIELDS,
    aliases: &[],
};

const CAMT056_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("Assgnmt/Id", FieldKind::Text),
    FieldSpec::required("Assgnmt/CreDtTm", FieldKind::DateTime),
    FieldSpec::required("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId", FieldKind::Text),
    FieldSpec::optional("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgNmId", FieldKind::Text),
    FieldSpec::optional("Undrlyg/TxInf/OrgnlCreDtTm", FieldKind::DateTime),
    FieldSpec::limited("Undrlyg/TxInf/OrgnlInstrId", false, 1024, FieldKind::Text),
    FieldSpec::limited(
        "Undrlyg/TxInf/OrgnlEndToEndId",
        false,
        1024,
        FieldKind::Text,
    ),
    FieldSpec::limited("Undrlyg/TxInf/OrgnlTxId", false, 1024, FieldKind::Text),
    FieldSpec::limited(
        "Undrlyg/TxInf/CxlRsnInf/Rsn/Cd",
        false,
        1024,
        FieldKind::Text,
    ),
    FieldSpec::limited(
        "Undrlyg/TxInf/CxlRsnInf/Rsn/Prtry",
        false,
        1024,
        FieldKind::Text,
    ),
    FieldSpec::limited(
        "Undrlyg/TxInf/CxlRsnInf/AddtlInf",
        false,
        1024,
        FieldKind::Text,
    ),
];

const CAMT056_SCHEMA: MessageSchema = MessageSchema {
    fields: CAMT056_FIELDS,
    aliases: &[
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Assgnmt/Id",
            canonical: "Assgnmt/Id",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Assgnmt/CreDtTm",
            canonical: "Assgnmt/CreDtTm",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId",
            canonical: "Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgNmId",
            canonical: "Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgNmId",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlCreDtTm",
            canonical: "Undrlyg/TxInf/OrgnlCreDtTm",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlInstrId",
            canonical: "Undrlyg/TxInf/OrgnlInstrId",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlEndToEndId",
            canonical: "Undrlyg/TxInf/OrgnlEndToEndId",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlTxId",
            canonical: "Undrlyg/TxInf/OrgnlTxId",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/CxlRsnInf/Rsn/Cd",
            canonical: "Undrlyg/TxInf/CxlRsnInf/Rsn/Cd",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/CxlRsnInf/Rsn/Prtry",
            canonical: "Undrlyg/TxInf/CxlRsnInf/Rsn/Prtry",
        },
        AliasSpec {
            alias: "Document/FIToFIPmtCxlReq/Undrlyg/TxInf/CxlRsnInf/AddtlInf",
            canonical: "Undrlyg/TxInf/CxlRsnInf/AddtlInf",
        },
    ],
};

const PAIN001_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("GrpHdr/MsgId", FieldKind::Text),
    FieldSpec::required("GrpHdr/CreDtTm", FieldKind::DateTime),
    FieldSpec::required("GrpHdr/NbOfTxs", FieldKind::Numeric),
    FieldSpec::required("GrpHdr/InitgPty/Nm", FieldKind::Text),
    FieldSpec::limited("PmtInf[*]/PmtInfId", true, 64, FieldKind::Text),
    FieldSpec::limited("PmtInf[*]/ReqdExctnDt", true, 64, FieldKind::Date),
    FieldSpec::limited(
        "PmtInf[*]/DbtrAcct/Id",
        true,
        64,
        FieldKind::Identifier(IdentifierKind::Iban),
    ),
    FieldSpec::limited(
        "PmtInf[*]/CdtTrfTxInf[*]/Amt",
        true,
        1024,
        FieldKind::Amount,
    ),
    FieldSpec::limited(
        "PmtInf[*]/CdtTrfTxInf[*]/Ccy",
        true,
        1024,
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::limited(
        "PmtInf[*]/CdtTrfTxInf[*]/CdtrAcct/Id",
        true,
        1024,
        FieldKind::Identifier(IdentifierKind::Iban),
    ),
    FieldSpec::limited(
        "PmtInf[*]/CdtTrfTxInf[*]/CdtrAgt",
        true,
        1024,
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::limited(
        "PmtInf[*]/CdtTrfTxInf[*]/EndToEndId",
        true,
        1024,
        FieldKind::Text,
    ),
];

const PAIN001_SCHEMA: MessageSchema = MessageSchema {
    fields: PAIN001_FIELDS,
    aliases: &[],
};

const PAIN002_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("GrpHdr/MsgId", FieldKind::Text),
    FieldSpec::required("GrpHdr/CreDtTm", FieldKind::DateTime),
    FieldSpec::required("OrgnlGrpInfAndSts/OrgnlMsgId", FieldKind::Text),
    FieldSpec::required(
        "OrgnlGrpInfAndSts/GrpSts",
        FieldKind::Enum(&["ACSP", "RJCT", "ACSC", "PDNG", "PART"]),
    ),
    FieldSpec::optional("OrgnlGrpInfAndSts/StsRsnInf/Rsn/Cd", FieldKind::Text),
];

const PAIN002_SCHEMA: MessageSchema = MessageSchema {
    fields: PAIN002_FIELDS,
    aliases: &[],
};

const PACS007_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("MsgId", FieldKind::Text),
    FieldSpec::required("CreDtTm", FieldKind::DateTime),
    FieldSpec::required("OrgnlGrpInf/OrgnlMsgId", FieldKind::Text),
    FieldSpec::limited("TxInf[*]/OrgnlInstrId", true, 1024, FieldKind::Text),
    FieldSpec::limited("TxInf[*]/OrgnlEndToEndId", false, 1024, FieldKind::Text),
    FieldSpec::limited("TxInf[*]/OrgnlTxId", false, 1024, FieldKind::Text),
    FieldSpec::limited("TxInf[*]/CxlRsnInf/Rsn/Cd", false, 1024, FieldKind::Text),
    FieldSpec::optional("SplmtryData/LedgerInstruction", FieldKind::Text),
];

const PACS007_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS007_FIELDS,
    aliases: &[],
};

const PACS009_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("BizMsgIdr", FieldKind::Text),
    FieldSpec::required("MsgDefIdr", FieldKind::Text),
    FieldSpec::required("CreDtTm", FieldKind::DateTime),
    FieldSpec::required("IntrBkSttlmAmt", FieldKind::Amount),
    FieldSpec::required(
        "IntrBkSttlmCcy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::required("IntrBkSttlmDt", FieldKind::Date),
    FieldSpec::required("InstgAgt", FieldKind::Identifier(IdentifierKind::Bic)),
    FieldSpec::required("InstdAgt", FieldKind::Identifier(IdentifierKind::Bic)),
    FieldSpec::required("DbtrAcct", FieldKind::Identifier(IdentifierKind::Iban)),
    FieldSpec::required("CdtrAcct", FieldKind::Identifier(IdentifierKind::Iban)),
    FieldSpec::optional("Purp", FieldKind::Text),
];

const PACS009_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "AppHdr/BizMsgIdr",
        canonical: "BizMsgIdr",
    },
    AliasSpec {
        alias: "AppHdr/MsgDefIdr",
        canonical: "MsgDefIdr",
    },
    AliasSpec {
        alias: "AppHdr/CreDt",
        canonical: "CreDtTm",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/MsgId",
        canonical: "BizMsgIdr",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/BizMsgIdr",
        canonical: "BizMsgIdr",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/MsgDefIdr",
        canonical: "MsgDefIdr",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/CreDtTm",
        canonical: "CreDtTm",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/InstgAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "InstgAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/InstgAgt/FinInstnId/BICFI",
        canonical: "InstgAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/InstdAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "InstdAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/GrpHdr/InstdAgt/FinInstnId/BICFI",
        canonical: "InstdAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/IntrBkSttlmAmt",
        canonical: "IntrBkSttlmAmt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/IntrBkSttlmAmt/@Ccy",
        canonical: "IntrBkSttlmCcy",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/IntrBkSttlmDt",
        canonical: "IntrBkSttlmDt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/InstgAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "InstgAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/InstgAgt/FinInstnId/BICFI",
        canonical: "InstgAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/InstdAgt/FinInstnId/ClrSysMmbId/MmbId",
        canonical: "InstdAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/InstdAgt/FinInstnId/BICFI",
        canonical: "InstdAgt",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/DbtrAcct/Id/IBAN",
        canonical: "DbtrAcct",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/DbtrAcct/Id/Othr/Id",
        canonical: "DbtrAcct",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/CdtrAcct/Id/IBAN",
        canonical: "CdtrAcct",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/CdtrAcct/Id/Othr/Id",
        canonical: "CdtrAcct",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/Purp/Cd",
        canonical: "Purp",
    },
    AliasSpec {
        alias: "Document/FICdtTrf/CdtTrfTxInf/Purp/Prtry",
        canonical: "Purp",
    },
];

const PACS009_SCHEMA: MessageSchema = MessageSchema {
    fields: PACS009_FIELDS,
    aliases: PACS009_ALIASES,
};

// Settlement schemas cover the DvP/PvP/collateral fields exercised by the bridge
// (movement/payment qualifiers, execution plans, instruments, currencies, and parties).
const SESE023_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("TxId", FieldKind::Text),
    FieldSpec::required("SttlmDt", FieldKind::Date),
    FieldSpec::required(
        "SttlmTpAndAddtlParams/SctiesMvmntTp",
        FieldKind::Enum(&["DELI", "RECE"]),
    ),
    FieldSpec::required(
        "SttlmTpAndAddtlParams/Pmt",
        FieldKind::Enum(&["APMT", "FREE"]),
    ),
    FieldSpec::optional("SttlmParams/HldInd", FieldKind::Enum(&["true", "false"])),
    FieldSpec::optional(
        "SttlmParams/PrtlSttlmInd",
        FieldKind::Enum(&["NPAR", "PART", "PARQ", "PARC"]),
    ),
    FieldSpec::optional("SttlmParams/SttlmTxCond/Cd", FieldKind::Text),
    FieldSpec::optional(
        "PlcOfSttlm/MktId",
        FieldKind::Identifier(IdentifierKind::Mic),
    ),
    FieldSpec::limited(
        "Lnkgs/Lnkg[*]/Tp/Cd",
        false,
        16,
        FieldKind::Enum(&["WITH", "BEFO", "AFTE"]),
    ),
    FieldSpec::limited("Lnkgs/Lnkg[*]/Ref/Prtry", false, 16, FieldKind::Text),
    FieldSpec::required("SctiesLeg/FinInstrmId", FieldKind::Instrument),
    FieldSpec::required("SctiesLeg/Qty", FieldKind::Numeric),
    FieldSpec::required("CashLeg/Amt", FieldKind::Amount),
    FieldSpec::required(
        "CashLeg/Ccy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::required(
        "DlvrgSttlmPties/Pty/Bic",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::required("DlvrgSttlmPties/Acct", FieldKind::Text),
    FieldSpec::required(
        "RcvgSttlmPties/Pty/Bic",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::required("RcvgSttlmPties/Acct", FieldKind::Text),
    FieldSpec::required(
        "Plan/ExecutionOrder",
        FieldKind::Enum(&["DELIVERY_THEN_PAYMENT", "PAYMENT_THEN_DELIVERY"]),
    ),
    FieldSpec::required(
        "Plan/Atomicity",
        FieldKind::Enum(&["ALL_OR_NOTHING", "COMMIT_FIRST_LEG", "COMMIT_SECOND_LEG"]),
    ),
    FieldSpec::optional("SctiesLeg/Metadata", FieldKind::Text),
    FieldSpec::optional("CashLeg/Metadata", FieldKind::Text),
];

const SESE023_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/TxId",
        canonical: "TxId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/FctvSttlmDt",
        canonical: "SttlmDt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmDt",
        canonical: "SttlmDt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmTpAndAddtlParams/SctiesMvmntTp",
        canonical: "SttlmTpAndAddtlParams/SctiesMvmntTp",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmTpAndAddtlParams/Pmt",
        canonical: "SttlmTpAndAddtlParams/Pmt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmParams/HldInd",
        canonical: "SttlmParams/HldInd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmParams/PrtlSttlmInd",
        canonical: "SttlmParams/PrtlSttlmInd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmParams/PlcOfSttlm/MktId",
        canonical: "PlcOfSttlm/MktId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmParams/SttlmTxCond/Cd",
        canonical: "SttlmParams/SttlmTxCond/Cd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmParams/SttlmTxCond/Prtry",
        canonical: "SttlmParams/SttlmTxCond/Cd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/Lnkgs/Lnkg[*]/Tp/Cd",
        canonical: "Lnkgs/Lnkg[*]/Tp/Cd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/Lnkgs/Lnkg[*]/Ref/Prtry",
        canonical: "Lnkgs/Lnkg[*]/Ref/Prtry",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SctiesMvmntDtls/SctiesId/ISIN",
        canonical: "SctiesLeg/FinInstrmId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SctiesMvmntDtls/SctiesId/OthrId/Id",
        canonical: "SctiesLeg/FinInstrmId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/PlcOfSttlm/MktId",
        canonical: "PlcOfSttlm/MktId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/PlcOfTrad/MktId",
        canonical: "PlcOfSttlm/MktId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SctiesMvmntDtls/Qty/QtyChc/Unit",
        canonical: "SctiesLeg/Qty",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmAmt",
        canonical: "CashLeg/Amt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmAmt/@Ccy",
        canonical: "CashLeg/Ccy",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/DlvrgSttlmPties/Pty/Id/AnyBIC",
        canonical: "DlvrgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/DlvrgSttlmPties/Pty/Id/OrgId/AnyBIC",
        canonical: "DlvrgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/DlvrgSttlmPties/Acct/Id/Othr/Id",
        canonical: "DlvrgSttlmPties/Acct",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/RcvgSttlmPties/Pty/Id/AnyBIC",
        canonical: "RcvgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/RcvgSttlmPties/Pty/Id/OrgId/AnyBIC",
        canonical: "RcvgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SttlmPties/RcvgSttlmPties/Acct/Id/Othr/Id",
        canonical: "RcvgSttlmPties/Acct",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SplmtryData/Plan/ExecutionOrder",
        canonical: "Plan/ExecutionOrder",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SplmtryData/Plan/Atomicity",
        canonical: "Plan/Atomicity",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SplmtryData/SctiesLeg/Metadata",
        canonical: "SctiesLeg/Metadata",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxInstr/SplmtryData/CashLeg/Metadata",
        canonical: "CashLeg/Metadata",
    },
];

const SESE023_SCHEMA: MessageSchema = MessageSchema {
    fields: SESE023_FIELDS,
    aliases: SESE023_ALIASES,
};

const SESE025_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("TxId", FieldKind::Text),
    FieldSpec::required("SttlmDt", FieldKind::Date),
    FieldSpec::required(
        "SttlmTpAndAddtlParams/SctiesMvmntTp",
        FieldKind::Enum(&["DELI", "RECE"]),
    ),
    FieldSpec::required(
        "SttlmTpAndAddtlParams/Pmt",
        FieldKind::Enum(&["APMT", "FREE"]),
    ),
    FieldSpec::optional("SttlmParams/HldInd", FieldKind::Enum(&["true", "false"])),
    FieldSpec::optional(
        "SttlmParams/PrtlSttlmInd",
        FieldKind::Enum(&["NPAR", "PART", "PARQ", "PARC"]),
    ),
    FieldSpec::optional("SttlmParams/SttlmTxCond/Cd", FieldKind::Text),
    FieldSpec::optional(
        "PlcOfSttlm/MktId",
        FieldKind::Identifier(IdentifierKind::Mic),
    ),
    FieldSpec::required(
        "ConfSts",
        FieldKind::Enum(&["ACCP", "PEND", "PART", "RJCT"]),
    ),
    FieldSpec::required("SttlmQty", FieldKind::Numeric),
    FieldSpec::required("SttlmAmt", FieldKind::Amount),
    FieldSpec::required("SttlmCcy", FieldKind::Identifier(IdentifierKind::Currency)),
    FieldSpec::optional("SctiesLeg/FinInstrmId", FieldKind::Instrument),
    FieldSpec::optional("SctiesLeg/Qty", FieldKind::Numeric),
    FieldSpec::optional(
        "DlvrgSttlmPties/Pty/Bic",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::optional("DlvrgSttlmPties/Acct", FieldKind::Text),
    FieldSpec::optional(
        "RcvgSttlmPties/Pty/Bic",
        FieldKind::Identifier(IdentifierKind::Bic),
    ),
    FieldSpec::optional("RcvgSttlmPties/Acct", FieldKind::Text),
    FieldSpec::required(
        "Plan/ExecutionOrder",
        FieldKind::Enum(&["DELIVERY_THEN_PAYMENT", "PAYMENT_THEN_DELIVERY"]),
    ),
    FieldSpec::required(
        "Plan/Atomicity",
        FieldKind::Enum(&["ALL_OR_NOTHING", "COMMIT_FIRST_LEG", "COMMIT_SECOND_LEG"]),
    ),
    FieldSpec::optional("RsnCd", FieldKind::Text),
    FieldSpec::optional("AddtlInf", FieldKind::Text),
];

const SESE025_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/TxId",
        canonical: "TxId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmDt",
        canonical: "SttlmDt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/FctvSttlmDt",
        canonical: "SttlmDt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmTpAndAddtlParams/SctiesMvmntTp",
        canonical: "SttlmTpAndAddtlParams/SctiesMvmntTp",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmTpAndAddtlParams/Pmt",
        canonical: "SttlmTpAndAddtlParams/Pmt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmParams/HldInd",
        canonical: "SttlmParams/HldInd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmParams/PrtlSttlmInd",
        canonical: "SttlmParams/PrtlSttlmInd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmParams/PlcOfSttlm/MktId",
        canonical: "PlcOfSttlm/MktId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/PlcOfSttlm/MktId",
        canonical: "PlcOfSttlm/MktId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmParams/SttlmTxCond/Cd",
        canonical: "SttlmParams/SttlmTxCond/Cd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmParams/SttlmTxCond/Prtry",
        canonical: "SttlmParams/SttlmTxCond/Cd",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/TxSts/ConfSts",
        canonical: "ConfSts",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/ConfSts",
        canonical: "ConfSts",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmQty/Qty/Unit",
        canonical: "SttlmQty",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/QtyAndAcctDtls/SctiesQty/QtyChc/Unit",
        canonical: "SttlmQty",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmAmt",
        canonical: "SttlmAmt",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmAmt/@Ccy",
        canonical: "SttlmCcy",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SctiesMvmntDtls/SctiesId/ISIN",
        canonical: "SctiesLeg/FinInstrmId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SctiesMvmntDtls/SctiesId/OthrId/Id",
        canonical: "SctiesLeg/FinInstrmId",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SctiesMvmntDtls/Qty/QtyChc/Unit",
        canonical: "SctiesLeg/Qty",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/DlvrgSttlmPties/Pty/Id/AnyBIC",
        canonical: "DlvrgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/DlvrgSttlmPties/Pty/Id/OrgId/AnyBIC",
        canonical: "DlvrgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/DlvrgSttlmPties/Acct/Id/Othr/Id",
        canonical: "DlvrgSttlmPties/Acct",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/RcvgSttlmPties/Pty/Id/AnyBIC",
        canonical: "RcvgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/RcvgSttlmPties/Pty/Id/OrgId/AnyBIC",
        canonical: "RcvgSttlmPties/Pty/Bic",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SttlmPties/RcvgSttlmPties/Acct/Id/Othr/Id",
        canonical: "RcvgSttlmPties/Acct",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SplmtryData/Plan/ExecutionOrder",
        canonical: "Plan/ExecutionOrder",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/SplmtryData/Plan/Atomicity",
        canonical: "Plan/Atomicity",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/AddtlInf",
        canonical: "AddtlInf",
    },
    AliasSpec {
        alias: "Document/SctiesSttlmTxConf/Rsn/Cd",
        canonical: "RsnCd",
    },
];

const SESE025_SCHEMA: MessageSchema = MessageSchema {
    fields: SESE025_FIELDS,
    aliases: SESE025_ALIASES,
};

const COLR007_FIELDS: &[FieldSpec] = &[
    FieldSpec::required("TxId", FieldKind::Text),
    FieldSpec::required("OblgtnId", FieldKind::Text),
    FieldSpec::required("Substitution/OriginalAmt", FieldKind::Amount),
    FieldSpec::required(
        "Substitution/OriginalCcy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::required("Substitution/SubstituteAmt", FieldKind::Amount),
    FieldSpec::required(
        "Substitution/SubstituteCcy",
        FieldKind::Identifier(IdentifierKind::Currency),
    ),
    FieldSpec::required("Substitution/EffectiveDt", FieldKind::Date),
    FieldSpec::required("Substitution/Type", FieldKind::Enum(&["FULL", "PARTIAL"])),
    FieldSpec::optional("Substitution/Haircut", FieldKind::Numeric),
    FieldSpec::optional("Substitution/OriginalFinInstrmId", FieldKind::Instrument),
    FieldSpec::optional("Substitution/SubstituteFinInstrmId", FieldKind::Instrument),
    FieldSpec::optional("Substitution/ReasonCd", FieldKind::Text),
];

const COLR007_ALIASES: &[AliasSpec] = &[
    AliasSpec {
        alias: "Document/CollSbstitnConf/TxId",
        canonical: "TxId",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/OblgtnId",
        canonical: "OblgtnId",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/OrgnlColl/Amt",
        canonical: "Substitution/OriginalAmt",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/OrgnlColl/Amt/@Ccy",
        canonical: "Substitution/OriginalCcy",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/SubstnColl/Amt",
        canonical: "Substitution/SubstituteAmt",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/SubstnColl/Amt/@Ccy",
        canonical: "Substitution/SubstituteCcy",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/EffctvDt",
        canonical: "Substitution/EffectiveDt",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/Tp",
        canonical: "Substitution/Type",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/Rsn/Cd",
        canonical: "Substitution/ReasonCd",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/Hrct",
        canonical: "Substitution/Haircut",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/OrgnlColl/SctiesId/ISIN",
        canonical: "Substitution/OriginalFinInstrmId",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/OrgnlColl/SctiesId/OthrId/Id",
        canonical: "Substitution/OriginalFinInstrmId",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/SubstnColl/SctiesId/ISIN",
        canonical: "Substitution/SubstituteFinInstrmId",
    },
    AliasSpec {
        alias: "Document/CollSbstitnConf/CollSbstitn/SubstnColl/SctiesId/OthrId/Id",
        canonical: "Substitution/SubstituteFinInstrmId",
    },
];

const COLR007_SCHEMA: MessageSchema = MessageSchema {
    fields: COLR007_FIELDS,
    aliases: COLR007_ALIASES,
};

/// Errors that can occur when preparing or sending ISO 20022 messages.
#[derive(Debug)]
pub enum MsgError {
    NoActiveMessage,
    UnknownMessageType,
    ValidationFailed,
    MissingField(&'static str),
    TooManyOccurrences {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    InvalidIdentifier {
        field: String,
        kind: IdentifierKind,
    },
    InvalidInstrument {
        field: String,
    },
    InvalidValue {
        field: String,
        kind: InvalidValueKind,
    },
    InvalidFormat,
    UnsupportedChannel,
    HttpStatus(u16),
    Io(std::io::Error),
}

impl fmt::Display for MsgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MsgError::NoActiveMessage => f.write_str("no active ISO 20022 message"),
            MsgError::UnknownMessageType => f.write_str("unsupported ISO 20022 message type"),
            MsgError::ValidationFailed => f.write_str("ISO 20022 validation failed"),
            MsgError::MissingField(field) => {
                write!(f, "missing ISO 20022 field `{field}`")
            }
            MsgError::TooManyOccurrences { field, max, actual } => write!(
                f,
                "field `{field}` exceeds max occurrences ({actual} > {max})"
            ),
            MsgError::InvalidIdentifier { field, kind } => {
                write!(f, "invalid {} value for field `{field}`", kind.label())
            }
            MsgError::InvalidInstrument { field } => write!(
                f,
                "field `{field}` must contain a valid ISIN or CUSIP identifier"
            ),
            MsgError::InvalidValue { field, kind } => {
                write!(f, "invalid {} value for field `{field}`", kind.label())
            }
            MsgError::InvalidFormat => f.write_str("ISO 20022 message format is invalid"),
            MsgError::UnsupportedChannel => {
                f.write_str("ISO 20022 delivery channel is unsupported")
            }
            MsgError::HttpStatus(code) => {
                write!(f, "ISO 20022 transport returned HTTP status {code}")
            }
            MsgError::Io(err) => write!(f, "ISO 20022 transport error: {err}"),
        }
    }
}

impl From<std::io::Error> for MsgError {
    fn from(err: std::io::Error) -> Self {
        MsgError::Io(err)
    }
}

impl From<ValidationFailure> for MsgError {
    fn from(failure: ValidationFailure) -> Self {
        match failure {
            ValidationFailure::MissingField(field) => MsgError::MissingField(field),
            ValidationFailure::TooManyOccurrences { field, max, actual } => {
                MsgError::TooManyOccurrences { field, max, actual }
            }
            ValidationFailure::InvalidField { field, reason } => match reason {
                InvalidReason::Identifier(kind) => MsgError::InvalidIdentifier { field, kind },
                InvalidReason::Instrument => MsgError::InvalidInstrument { field },
                InvalidReason::Empty => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Empty,
                },
                InvalidReason::Numeric => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Numeric,
                },
                InvalidReason::Amount => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Amount,
                },
                InvalidReason::Date => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Date,
                },
                InvalidReason::DateTime => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::DateTime,
                },
                InvalidReason::Enum => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Enum,
                },
                InvalidReason::Utf8 => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Utf8,
                },
            },
        }
    }
}

/// Consume and return the most recent validation error recorded by [`msg_validate`].
///
/// The helper yields [`MsgError`] variants mirroring the validation failure and
/// clears the stored state so subsequent calls return `None` until
/// [`msg_validate`] runs again.
#[must_use]
pub fn take_validation_error() -> Option<MsgError> {
    take_validation_failure().map(MsgError::from)
}

/// Materialised ISO 20022 message extracted from the VM stack.
#[derive(Clone, Debug)]
pub struct ParsedMessage {
    message_type: String,
    fields: BTreeMap<String, Vec<u8>>,
}

impl ParsedMessage {
    /// Return the ISO 20022 message code (e.g. `"pacs.008"`).
    pub fn message_type(&self) -> &str {
        &self.message_type
    }

    /// Retrieve the raw bytes stored under the canonical field path.
    pub fn field_bytes(&self, field: &str) -> Option<&[u8]> {
        self.fields.get(field).map(|v| v.as_slice())
    }

    /// Retrieve the UTF-8 string stored under the canonical field path.
    pub fn field_text(&self, field: &str) -> Option<&str> {
        self.field_bytes(field)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
    }

    /// Iterate over canonical field paths and their values.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.fields.iter()
    }
}

/// Parse, validate, and materialise an ISO 20022 message.
///
/// This helper wraps `msg_parse`/`msg_validate` and drains the temporary VM
/// stack entry, returning an owned [`ParsedMessage`] on success.
pub fn parse_message(message_type: &str, data: &[u8]) -> Result<ParsedMessage, MsgError> {
    msg_parse(message_type, data)?;
    let valid = msg_validate();
    MESSAGE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let maybe_msg = stack.pop();
        drop(stack);
        match (valid, maybe_msg) {
            (true, Some(msg)) => Ok(ParsedMessage {
                message_type: msg.message_type,
                fields: msg.fields.into_iter().collect(),
            }),
            (false, _) => {
                let err = take_validation_failure()
                    .map(MsgError::from)
                    .unwrap_or(MsgError::ValidationFailed);
                Err(err)
            }
            (true, None) => Err(MsgError::NoActiveMessage),
        }
    })
}

/// Norito-friendly projections of the ISO 20022 settlement messages covered by
/// this helper. These structs make it easy to encode/decode settlement payloads
/// alongside the VM message stack without reimplementing field mapping.
pub mod norito_schemas {
    use norito::codec::{Decode, Encode};

    use super::{InvalidValueKind, MsgError, ParsedMessage, msg_add, msg_create, msg_set};

    fn required_text(parsed: &ParsedMessage, field: &'static str) -> Result<String, MsgError> {
        parsed
            .field_text(field)
            .map(str::to_owned)
            .ok_or(MsgError::MissingField(field))
    }

    fn optional_text(parsed: &ParsedMessage, field: &str) -> Option<String> {
        parsed.field_text(field).map(str::to_owned)
    }

    fn optional_bool(
        parsed: &ParsedMessage,
        field: &'static str,
    ) -> Result<Option<bool>, MsgError> {
        let Some(text) = parsed.field_text(field) else {
            return Ok(None);
        };
        match text {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(MsgError::InvalidValue {
                field: field.to_owned(),
                kind: InvalidValueKind::Enum,
            }),
        }
    }

    fn bool_bytes(value: bool) -> &'static [u8] {
        if value { b"true" } else { b"false" }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Linkage {
        pub relation: String,
        pub reference: String,
    }

    /// Norito schema for `sese.023` DvP instructions.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Sese023 {
        pub tx_id: String,
        pub settlement_date: String,
        pub movement_type: String,
        pub payment_type: String,
        pub fin_instr_id: String,
        pub quantity: String,
        pub cash_amount: String,
        pub cash_currency: String,
        pub delivering_party_bic: String,
        pub delivering_account: String,
        pub receiving_party_bic: String,
        pub receiving_account: String,
        pub execution_order: String,
        pub atomicity: String,
        pub settlement_condition: Option<String>,
        pub partial_settlement_indicator: Option<String>,
        pub hold_indicator: Option<bool>,
        pub venue_mic: Option<String>,
        pub linkages: Vec<Linkage>,
        pub securities_metadata: Option<String>,
        pub cash_metadata: Option<String>,
    }

    impl Sese023 {
        /// Populate the VM message stack with this instruction.
        pub fn apply_to_stack(&self) {
            msg_create("sese.023");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("SttlmDt", self.settlement_date.as_bytes());
            msg_set(
                "SttlmTpAndAddtlParams/SctiesMvmntTp",
                self.movement_type.as_bytes(),
            );
            msg_set("SttlmTpAndAddtlParams/Pmt", self.payment_type.as_bytes());
            if let Some(condition) = &self.settlement_condition {
                msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
            }
            if let Some(indicator) = &self.partial_settlement_indicator {
                msg_set("SttlmParams/PrtlSttlmInd", indicator.as_bytes());
            }
            if let Some(hold) = self.hold_indicator {
                msg_set("SttlmParams/HldInd", bool_bytes(hold));
            }
            if let Some(mic) = &self.venue_mic {
                msg_set("PlcOfSttlm/MktId", mic.as_bytes());
            }
            msg_set("SctiesLeg/FinInstrmId", self.fin_instr_id.as_bytes());
            msg_set("SctiesLeg/Qty", self.quantity.as_bytes());
            msg_set("CashLeg/Amt", self.cash_amount.as_bytes());
            msg_set("CashLeg/Ccy", self.cash_currency.as_bytes());
            msg_set(
                "DlvrgSttlmPties/Pty/Bic",
                self.delivering_party_bic.as_bytes(),
            );
            msg_set("DlvrgSttlmPties/Acct", self.delivering_account.as_bytes());
            msg_set(
                "RcvgSttlmPties/Pty/Bic",
                self.receiving_party_bic.as_bytes(),
            );
            msg_set("RcvgSttlmPties/Acct", self.receiving_account.as_bytes());
            msg_set("Plan/ExecutionOrder", self.execution_order.as_bytes());
            msg_set("Plan/Atomicity", self.atomicity.as_bytes());
            for (idx, linkage) in self.linkages.iter().enumerate() {
                msg_add("Lnkgs/Lnkg");
                let prefix = format!("Lnkgs/Lnkg[{idx}]");
                msg_set(
                    format!("{prefix}/Tp/Cd").as_str(),
                    linkage.relation.as_bytes(),
                );
                msg_set(
                    format!("{prefix}/Ref/Prtry").as_str(),
                    linkage.reference.as_bytes(),
                );
            }
            if let Some(meta) = &self.securities_metadata {
                msg_set("SctiesLeg/Metadata", meta.as_bytes());
            }
            if let Some(meta) = &self.cash_metadata {
                msg_set("CashLeg/Metadata", meta.as_bytes());
            }
        }

        /// Build the Norito view from a parsed and validated message.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                settlement_date: required_text(parsed, "SttlmDt")?,
                movement_type: required_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp")?,
                payment_type: required_text(parsed, "SttlmTpAndAddtlParams/Pmt")?,
                fin_instr_id: required_text(parsed, "SctiesLeg/FinInstrmId")?,
                quantity: required_text(parsed, "SctiesLeg/Qty")?,
                cash_amount: required_text(parsed, "CashLeg/Amt")?,
                cash_currency: required_text(parsed, "CashLeg/Ccy")?,
                delivering_party_bic: required_text(parsed, "DlvrgSttlmPties/Pty/Bic")?,
                delivering_account: required_text(parsed, "DlvrgSttlmPties/Acct")?,
                receiving_party_bic: required_text(parsed, "RcvgSttlmPties/Pty/Bic")?,
                receiving_account: required_text(parsed, "RcvgSttlmPties/Acct")?,
                execution_order: required_text(parsed, "Plan/ExecutionOrder")?,
                atomicity: required_text(parsed, "Plan/Atomicity")?,
                settlement_condition: optional_text(parsed, "SttlmParams/SttlmTxCond/Cd"),
                partial_settlement_indicator: optional_text(parsed, "SttlmParams/PrtlSttlmInd"),
                hold_indicator: optional_bool(parsed, "SttlmParams/HldInd")?,
                venue_mic: optional_text(parsed, "PlcOfSttlm/MktId"),
                linkages: collect_linkages(parsed),
                securities_metadata: optional_text(parsed, "SctiesLeg/Metadata"),
                cash_metadata: optional_text(parsed, "CashLeg/Metadata"),
            })
        }
    }

    fn collect_linkages(parsed: &ParsedMessage) -> Vec<Linkage> {
        let mut linkages = Vec::new();
        let mut idx = 0usize;
        loop {
            let tp_field = format!("Lnkgs/Lnkg[{idx}]/Tp/Cd");
            let ref_field = format!("Lnkgs/Lnkg[{idx}]/Ref/Prtry");
            let Some(tp) = parsed.field_text(&tp_field) else {
                break;
            };
            let reference = parsed.field_text(&ref_field).unwrap_or_default().to_owned();
            linkages.push(Linkage {
                relation: tp.to_owned(),
                reference,
            });
            idx += 1;
        }
        linkages
    }

    /// Norito schema for `sese.025` PvP confirmations.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Sese025 {
        pub tx_id: String,
        pub settlement_date: String,
        pub movement_type: String,
        pub payment_type: String,
        pub confirmation_status: String,
        pub settlement_quantity: String,
        pub settlement_amount: String,
        pub settlement_currency: String,
        pub security_id: Option<String>,
        pub security_quantity: Option<String>,
        pub delivering_party_bic: Option<String>,
        pub delivering_account: Option<String>,
        pub receiving_party_bic: Option<String>,
        pub receiving_account: Option<String>,
        pub execution_order: String,
        pub atomicity: String,
        pub hold_indicator: Option<bool>,
        pub partial_settlement_indicator: Option<String>,
        pub settlement_condition: Option<String>,
        pub venue_mic: Option<String>,
        pub reason_code: Option<String>,
        pub additional_info: Option<String>,
    }

    impl Sese025 {
        /// Populate the VM stack with a confirmation message.
        pub fn apply_to_stack(&self) {
            msg_create("sese.025");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("SttlmDt", self.settlement_date.as_bytes());
            msg_set(
                "SttlmTpAndAddtlParams/SctiesMvmntTp",
                self.movement_type.as_bytes(),
            );
            msg_set("SttlmTpAndAddtlParams/Pmt", self.payment_type.as_bytes());
            msg_set("ConfSts", self.confirmation_status.as_bytes());
            msg_set("SttlmQty", self.settlement_quantity.as_bytes());
            msg_set("SttlmAmt", self.settlement_amount.as_bytes());
            msg_set("SttlmCcy", self.settlement_currency.as_bytes());
            if let Some(id) = &self.security_id {
                msg_set("SctiesLeg/FinInstrmId", id.as_bytes());
            }
            if let Some(qty) = &self.security_quantity {
                msg_set("SctiesLeg/Qty", qty.as_bytes());
            }
            if let Some(bic) = &self.delivering_party_bic {
                msg_set("DlvrgSttlmPties/Pty/Bic", bic.as_bytes());
            }
            if let Some(acct) = &self.delivering_account {
                msg_set("DlvrgSttlmPties/Acct", acct.as_bytes());
            }
            if let Some(bic) = &self.receiving_party_bic {
                msg_set("RcvgSttlmPties/Pty/Bic", bic.as_bytes());
            }
            if let Some(acct) = &self.receiving_account {
                msg_set("RcvgSttlmPties/Acct", acct.as_bytes());
            }
            msg_set("Plan/ExecutionOrder", self.execution_order.as_bytes());
            msg_set("Plan/Atomicity", self.atomicity.as_bytes());
            if let Some(hold) = self.hold_indicator {
                msg_set("SttlmParams/HldInd", bool_bytes(hold));
            }
            if let Some(indicator) = &self.partial_settlement_indicator {
                msg_set("SttlmParams/PrtlSttlmInd", indicator.as_bytes());
            }
            if let Some(condition) = &self.settlement_condition {
                msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
            }
            if let Some(mic) = &self.venue_mic {
                msg_set("PlcOfSttlm/MktId", mic.as_bytes());
            }
            if let Some(reason) = &self.reason_code {
                msg_set("RsnCd", reason.as_bytes());
            }
            if let Some(info) = &self.additional_info {
                msg_set("AddtlInf", info.as_bytes());
            }
        }

        /// Convert a parsed message into the Norito struct.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                settlement_date: required_text(parsed, "SttlmDt")?,
                movement_type: required_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp")?,
                payment_type: required_text(parsed, "SttlmTpAndAddtlParams/Pmt")?,
                confirmation_status: required_text(parsed, "ConfSts")?,
                settlement_quantity: required_text(parsed, "SttlmQty")?,
                settlement_amount: required_text(parsed, "SttlmAmt")?,
                settlement_currency: required_text(parsed, "SttlmCcy")?,
                security_id: optional_text(parsed, "SctiesLeg/FinInstrmId"),
                security_quantity: optional_text(parsed, "SctiesLeg/Qty"),
                delivering_party_bic: optional_text(parsed, "DlvrgSttlmPties/Pty/Bic"),
                delivering_account: optional_text(parsed, "DlvrgSttlmPties/Acct"),
                receiving_party_bic: optional_text(parsed, "RcvgSttlmPties/Pty/Bic"),
                receiving_account: optional_text(parsed, "RcvgSttlmPties/Acct"),
                execution_order: required_text(parsed, "Plan/ExecutionOrder")?,
                atomicity: required_text(parsed, "Plan/Atomicity")?,
                hold_indicator: optional_bool(parsed, "SttlmParams/HldInd")?,
                partial_settlement_indicator: optional_text(parsed, "SttlmParams/PrtlSttlmInd"),
                settlement_condition: optional_text(parsed, "SttlmParams/SttlmTxCond/Cd"),
                venue_mic: optional_text(parsed, "PlcOfSttlm/MktId"),
                reason_code: optional_text(parsed, "RsnCd"),
                additional_info: optional_text(parsed, "AddtlInf"),
            })
        }
    }

    /// Norito schema for `colr.007` collateral substitutions.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Colr007 {
        pub tx_id: String,
        pub obligation_id: String,
        pub original_amount: String,
        pub original_currency: String,
        pub substitute_amount: String,
        pub substitute_currency: String,
        pub haircut: Option<String>,
        pub effective_date: String,
        pub substitution_type: String,
        pub original_fin_instr_id: Option<String>,
        pub substitute_fin_instr_id: Option<String>,
        pub reason_code: Option<String>,
    }

    impl Colr007 {
        /// Populate the VM stack with a substitution confirmation.
        pub fn apply_to_stack(&self) {
            msg_create("colr.007");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("OblgtnId", self.obligation_id.as_bytes());
            msg_set("Substitution/OriginalAmt", self.original_amount.as_bytes());
            msg_set(
                "Substitution/OriginalCcy",
                self.original_currency.as_bytes(),
            );
            msg_set(
                "Substitution/SubstituteAmt",
                self.substitute_amount.as_bytes(),
            );
            msg_set(
                "Substitution/SubstituteCcy",
                self.substitute_currency.as_bytes(),
            );
            if let Some(haircut) = &self.haircut {
                msg_set("Substitution/Haircut", haircut.as_bytes());
            }
            msg_set("Substitution/EffectiveDt", self.effective_date.as_bytes());
            msg_set("Substitution/Type", self.substitution_type.as_bytes());
            if let Some(id) = &self.original_fin_instr_id {
                msg_set("Substitution/OriginalFinInstrmId", id.as_bytes());
            }
            if let Some(id) = &self.substitute_fin_instr_id {
                msg_set("Substitution/SubstituteFinInstrmId", id.as_bytes());
            }
            if let Some(reason) = &self.reason_code {
                msg_set("Substitution/ReasonCd", reason.as_bytes());
            }
        }

        /// Convert a parsed message into the Norito struct.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                obligation_id: required_text(parsed, "OblgtnId")?,
                original_amount: required_text(parsed, "Substitution/OriginalAmt")?,
                original_currency: required_text(parsed, "Substitution/OriginalCcy")?,
                substitute_amount: required_text(parsed, "Substitution/SubstituteAmt")?,
                substitute_currency: required_text(parsed, "Substitution/SubstituteCcy")?,
                haircut: optional_text(parsed, "Substitution/Haircut"),
                effective_date: required_text(parsed, "Substitution/EffectiveDt")?,
                substitution_type: required_text(parsed, "Substitution/Type")?,
                original_fin_instr_id: optional_text(parsed, "Substitution/OriginalFinInstrmId"),
                substitute_fin_instr_id: optional_text(
                    parsed,
                    "Substitution/SubstituteFinInstrmId",
                ),
                reason_code: optional_text(parsed, "Substitution/ReasonCd"),
            })
        }
    }
}

fn parse_index(segment: &str) -> Option<(&str, usize)> {
    let (name, rest) = segment.split_once('[')?;
    let idx_str = rest.strip_suffix(']')?;
    if idx_str.is_empty() {
        return None;
    }
    Some((name, idx_str.parse().ok()?))
}

fn pattern_matches(pattern: &str, field: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let field_parts: Vec<&str> = field.split('/').collect();
    if pattern_parts.len() != field_parts.len() {
        return false;
    }
    pattern_parts
        .iter()
        .zip(field_parts.iter())
        .all(|(pat, actual)| {
            if let Some(base) = pat.strip_suffix("[*]") {
                parse_index(actual).is_some_and(|(name, _)| name == base)
            } else {
                pat == actual
            }
        })
}

fn repeating_base(pattern: &'static str) -> Option<&'static str> {
    let idx = pattern.find("[*]")?;
    let base = &pattern[..idx];
    Some(base.strip_suffix('/').unwrap_or(base))
}

fn resolve_alias(schema: &MessageSchema, field: &str) -> Option<String> {
    if schema
        .field_specs()
        .iter()
        .any(|spec| pattern_matches(spec.pattern, field))
    {
        return Some(field.to_owned());
    }
    for alias in schema.aliases() {
        if alias.alias == field {
            return Some(alias.canonical.to_owned());
        }
        if pattern_matches(alias.alias, field) {
            let field_parts: Vec<&str> = field.split('/').collect();
            let alias_parts: Vec<&str> = alias.alias.split('/').collect();
            let filtered_parts: Vec<&str> = field_parts
                .iter()
                .copied()
                .filter(|part| !part.starts_with('@'))
                .collect();
            let canonical_parts: Vec<&str> = alias.canonical.split('/').collect();
            if canonical_parts.len() <= filtered_parts.len() {
                let offset = filtered_parts.len().saturating_sub(canonical_parts.len());
                let mut out = Vec::with_capacity(canonical_parts.len());
                for (i, canon) in canonical_parts.iter().enumerate() {
                    if let Some(base_for_index) = canon.strip_suffix("[*]") {
                        let field_part = alias_parts
                            .iter()
                            .position(|part| part.trim_end_matches("[*]") == base_for_index)
                            .and_then(|idx| filtered_parts.get(idx))
                            .or_else(|| filtered_parts.get(offset + i));
                        if let Some(field_part) = field_part
                            && let Some((_, idx)) = parse_index(field_part)
                        {
                            out.push(format!("{base_for_index}[{idx}]"));
                        } else {
                            out.push(base_for_index.to_owned());
                        }
                    } else {
                        out.push((*canon).to_owned());
                    }
                }
                return Some(out.join("/"));
            }
        }
        if alias.alias.ends_with("[*]") {
            let base = alias.alias.trim_end_matches("[*]");
            if let Some(rest) = field.strip_prefix(base) {
                let canonical_base = alias.canonical.trim_end_matches("[*]");
                return Some(format!("{canonical_base}{rest}"));
            }
        }
    }
    None
}

fn canonical_field_name(message_type: &str, field: &str) -> String {
    schema_for(message_type)
        .and_then(|schema| resolve_alias(schema, field))
        .unwrap_or_else(|| field.to_owned())
}

fn canonical_repeating_base(message_type: &str, base: &str) -> String {
    if let Some(schema) = schema_for(message_type) {
        if let Some(resolved) = resolve_alias(schema, base) {
            return resolved;
        }
        if schema
            .field_specs()
            .iter()
            .filter_map(|spec| repeating_base(spec.pattern))
            .any(|candidate| candidate == base)
        {
            return base.to_owned();
        }
    }
    base.to_owned()
}

#[derive(Clone, Copy)]
struct IbanSpec {
    code: [u8; 2],
    length: u8,
}

/// Source: IBAN Registry (June 2024). Keep alphabetically sorted so
/// `iban_length_for_country` can binary-search deterministically.
const IBAN_SPECS: &[IbanSpec] = &[
    IbanSpec {
        code: *b"AD",
        length: 24,
    },
    IbanSpec {
        code: *b"AE",
        length: 23,
    },
    IbanSpec {
        code: *b"AL",
        length: 28,
    },
    IbanSpec {
        code: *b"AT",
        length: 20,
    },
    IbanSpec {
        code: *b"AZ",
        length: 28,
    },
    IbanSpec {
        code: *b"BA",
        length: 20,
    },
    IbanSpec {
        code: *b"BE",
        length: 16,
    },
    IbanSpec {
        code: *b"BG",
        length: 22,
    },
    IbanSpec {
        code: *b"BH",
        length: 22,
    },
    IbanSpec {
        code: *b"BR",
        length: 29,
    },
    IbanSpec {
        code: *b"BY",
        length: 28,
    },
    IbanSpec {
        code: *b"CH",
        length: 21,
    },
    IbanSpec {
        code: *b"CR",
        length: 22,
    },
    IbanSpec {
        code: *b"CY",
        length: 28,
    },
    IbanSpec {
        code: *b"CZ",
        length: 24,
    },
    IbanSpec {
        code: *b"DE",
        length: 22,
    },
    IbanSpec {
        code: *b"DK",
        length: 18,
    },
    IbanSpec {
        code: *b"DO",
        length: 28,
    },
    IbanSpec {
        code: *b"EE",
        length: 20,
    },
    IbanSpec {
        code: *b"EG",
        length: 27,
    },
    IbanSpec {
        code: *b"ES",
        length: 24,
    },
    IbanSpec {
        code: *b"FI",
        length: 18,
    },
    IbanSpec {
        code: *b"FO",
        length: 18,
    },
    IbanSpec {
        code: *b"FR",
        length: 27,
    },
    IbanSpec {
        code: *b"GB",
        length: 22,
    },
    IbanSpec {
        code: *b"GE",
        length: 22,
    },
    IbanSpec {
        code: *b"GI",
        length: 23,
    },
    IbanSpec {
        code: *b"GL",
        length: 18,
    },
    IbanSpec {
        code: *b"GR",
        length: 27,
    },
    IbanSpec {
        code: *b"GT",
        length: 28,
    },
    IbanSpec {
        code: *b"HR",
        length: 21,
    },
    IbanSpec {
        code: *b"HU",
        length: 28,
    },
    IbanSpec {
        code: *b"IE",
        length: 22,
    },
    IbanSpec {
        code: *b"IL",
        length: 23,
    },
    IbanSpec {
        code: *b"IQ",
        length: 23,
    },
    IbanSpec {
        code: *b"IR",
        length: 26,
    },
    IbanSpec {
        code: *b"IS",
        length: 26,
    },
    IbanSpec {
        code: *b"IT",
        length: 27,
    },
    IbanSpec {
        code: *b"JO",
        length: 30,
    },
    IbanSpec {
        code: *b"KW",
        length: 30,
    },
    IbanSpec {
        code: *b"KZ",
        length: 20,
    },
    IbanSpec {
        code: *b"LB",
        length: 28,
    },
    IbanSpec {
        code: *b"LC",
        length: 32,
    },
    IbanSpec {
        code: *b"LI",
        length: 21,
    },
    IbanSpec {
        code: *b"LT",
        length: 20,
    },
    IbanSpec {
        code: *b"LU",
        length: 20,
    },
    IbanSpec {
        code: *b"LV",
        length: 21,
    },
    IbanSpec {
        code: *b"MC",
        length: 27,
    },
    IbanSpec {
        code: *b"MD",
        length: 24,
    },
    IbanSpec {
        code: *b"ME",
        length: 22,
    },
    IbanSpec {
        code: *b"MK",
        length: 19,
    },
    IbanSpec {
        code: *b"MR",
        length: 27,
    },
    IbanSpec {
        code: *b"MT",
        length: 31,
    },
    IbanSpec {
        code: *b"MU",
        length: 30,
    },
    IbanSpec {
        code: *b"NL",
        length: 18,
    },
    IbanSpec {
        code: *b"NO",
        length: 15,
    },
    IbanSpec {
        code: *b"PK",
        length: 24,
    },
    IbanSpec {
        code: *b"PL",
        length: 28,
    },
    IbanSpec {
        code: *b"PS",
        length: 29,
    },
    IbanSpec {
        code: *b"PT",
        length: 25,
    },
    IbanSpec {
        code: *b"QA",
        length: 29,
    },
    IbanSpec {
        code: *b"RO",
        length: 24,
    },
    IbanSpec {
        code: *b"RS",
        length: 22,
    },
    IbanSpec {
        code: *b"SA",
        length: 24,
    },
    IbanSpec {
        code: *b"SC",
        length: 31,
    },
    IbanSpec {
        code: *b"SE",
        length: 24,
    },
    IbanSpec {
        code: *b"SI",
        length: 19,
    },
    IbanSpec {
        code: *b"SK",
        length: 24,
    },
    IbanSpec {
        code: *b"SM",
        length: 27,
    },
    IbanSpec {
        code: *b"ST",
        length: 25,
    },
    IbanSpec {
        code: *b"SV",
        length: 28,
    },
    IbanSpec {
        code: *b"TL",
        length: 23,
    },
    IbanSpec {
        code: *b"TN",
        length: 24,
    },
    IbanSpec {
        code: *b"TR",
        length: 26,
    },
    IbanSpec {
        code: *b"UA",
        length: 29,
    },
    IbanSpec {
        code: *b"VA",
        length: 22,
    },
    IbanSpec {
        code: *b"VG",
        length: 24,
    },
    IbanSpec {
        code: *b"XK",
        length: 20,
    },
];

fn iban_length_for_country(code: [u8; 2]) -> Option<usize> {
    IBAN_SPECS
        .binary_search_by(|spec| spec.code.cmp(&code))
        .ok()
        .map(|idx| IBAN_SPECS[idx].length as usize)
}

/// Validate an IBAN using the ISO 7064 mod 97-10 algorithm with
/// country-specific length checks and digit validation for the check byte pair.
fn validate_iban(value: &[u8]) -> bool {
    if value.len() < 4 {
        return false;
    }

    let mut normalized = Vec::with_capacity(value.len());
    for &byte in value {
        match byte {
            b'0'..=b'9' => normalized.push(byte),
            b'A'..=b'Z' => normalized.push(byte),
            b'a'..=b'z' => normalized.push(byte.to_ascii_uppercase()),
            _ => return false,
        }
    }

    if normalized.len() < 4 {
        return false;
    }
    let country = [normalized[0], normalized[1]];
    let expected_len = match iban_length_for_country(country) {
        Some(len) => len,
        None => return false,
    };
    if normalized.len() != expected_len {
        return false;
    }
    if !normalized[2].is_ascii_digit() || !normalized[3].is_ascii_digit() {
        return false;
    }

    // Rotate the country code and check digits to the end before running mod 97.
    normalized.rotate_left(4);
    let mut acc: u32 = 0;
    for byte in normalized {
        match byte {
            b'0'..=b'9' => acc = (acc * 10 + u32::from(byte - b'0')) % 97,
            b'A'..=b'Z' => acc = (acc * 100 + u32::from(byte - b'A' + 10)) % 97,
            _ => return false,
        }
    }
    acc == 1
}

/// Validate a BIC. The check is deliberately lightweight: it enforces
/// an uppercase alphanumeric string of length 8 or 11 as per ISO 9362 but
/// does not verify country codes or institution existence.
fn validate_bic(value: &[u8]) -> bool {
    core::str::from_utf8(value)
        .map(validate_bic_str)
        .unwrap_or(false)
}

fn validate_bic_str(value: &str) -> bool {
    let bytes = value.as_bytes();
    let len = bytes.len();
    if !(len == 8 || len == 11) {
        return false;
    }
    for &b in bytes {
        if !b.is_ascii_alphanumeric() {
            return false;
        }
    }
    if !bytes[..4].iter().all(|&b| b.is_ascii_uppercase()) {
        return false;
    }
    if !bytes[4..6].iter().all(|&b| b.is_ascii_uppercase()) {
        return false;
    }
    if !bytes[6..8]
        .iter()
        .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    if len == 11
        && !bytes[8..11]
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    true
}

fn validate_amount(value: &[u8]) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut digits = 0;
    let mut dot = 0;
    for b in value {
        match b {
            b'0'..=b'9' => digits += 1,
            b'.' => {
                dot += 1;
                if dot > 1 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    if digits == 0 {
        return false;
    }
    if dot == 1 {
        let s = match core::str::from_utf8(value) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let mut parts = s.split('.');
        let whole = parts.next().unwrap_or("");
        let frac = parts.next().unwrap_or("");
        if parts.next().is_some() {
            return false;
        }
        if whole.is_empty() {
            return false;
        }
        frac.len() <= 5
    } else {
        true
    }
}

const VALID_CURRENCY_CODES: &[&str] = &[
    "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
    "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BOV", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD",
    "CAD", "CDF", "CHE", "CHF", "CHW", "CLF", "CLP", "CNY", "COP", "COU", "CRC", "CUC", "CUP",
    "CVE", "CZK", "DJF", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP",
    "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG", "HUF", "IDR",
    "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW",
    "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA",
    "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR", "MWK", "MXN", "MXV", "MYR", "MZN", "NAD",
    "NGN", "NIO", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG",
    "QAR", "RON", "RSD", "RUB", "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLL",
    "SOS", "SRD", "SSP", "STN", "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY",
    "TTD", "TWD", "TZS", "UAH", "UGX", "USD", "USN", "UYI", "UYU", "UYW", "UZS", "VED", "VES",
    "VND", "VUV", "WST", "XAF", "XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XCD", "XDR", "XOF",
    "XPD", "XPF", "XPT", "XSU", "XTS", "XUA", "XXX", "YER", "ZAR", "ZMW", "ZWL",
];

fn validate_currency_str(value: &str) -> bool {
    if value.len() != 3 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    VALID_CURRENCY_CODES.binary_search(&value).is_ok()
}

fn validate_mic_str(value: &str) -> bool {
    if value.len() != 4 {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() || !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn luhn_sum_from_digits(digits: impl DoubleEndedIterator<Item = u32>) -> u32 {
    let mut sum = 0;
    let mut double = true;
    for mut value in digits {
        if double {
            value *= 2;
            if value >= 10 {
                sum += value / 10 + value % 10;
            } else {
                sum += value;
            }
        } else {
            sum += value;
        }
        double = !double;
    }
    sum
}

fn validate_isin_str(value: &str) -> bool {
    if value.len() != 12 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let mut digits = Vec::with_capacity(24);
    for ch in value.chars() {
        if let Some(d) = ch.to_digit(10) {
            digits.push(d);
        } else if ch.is_ascii_uppercase() {
            let mapped = 10 + (ch as u32 - 'A' as u32);
            if (10..36).contains(&mapped) {
                if mapped >= 20 {
                    digits.push(mapped / 10);
                } else {
                    digits.push(1);
                }
                digits.push(mapped % 10);
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    let check = digits.pop().unwrap_or(0);
    let sum = luhn_sum_from_digits(digits.into_iter().rev());
    (sum + check) % 10 == 0
}

fn cusip_char_value(ch: char) -> Option<u32> {
    match ch {
        '0'..='9' => Some(ch as u32 - '0' as u32),
        'A'..='Z' => Some(ch as u32 - 'A' as u32 + 10),
        '*' => Some(36),
        '@' => Some(37),
        '#' => Some(38),
        _ => None,
    }
}

fn validate_cusip_str(value: &str) -> bool {
    if value.len() != 9 {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let value = value.to_ascii_uppercase();
    let mut sum = 0u32;
    for (idx, ch) in value.chars().take(8).enumerate() {
        let mut val = match cusip_char_value(ch) {
            Some(v) => v,
            None => return false,
        };
        if idx % 2 == 1 {
            val *= 2;
        }
        sum += val / 10 + val % 10;
    }
    let check_char = value.chars().nth(8).unwrap_or('0');
    let check_digit = match check_char.to_digit(10) {
        Some(d) => d,
        None => return false,
    };
    (sum + check_digit).is_multiple_of(10)
}

fn validate_lei_str(value: &str) -> bool {
    if value.len() != 20 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let upper = value.to_ascii_uppercase();
    let mut remainder: u32 = 0;
    for ch in upper.chars() {
        if let Some(d) = ch.to_digit(10) {
            remainder = (remainder * 10 + d) % 97;
        } else if ch.is_ascii_uppercase() {
            let mapped = 10 + (ch as u32 - 'A' as u32);
            remainder = (remainder * 100 + mapped) % 97;
        } else {
            return false;
        }
    }
    remainder == 1
}

pub fn validate_identifier(kind: IdentifierKind, value: &str) -> bool {
    match kind {
        IdentifierKind::Isin => validate_isin_str(value),
        IdentifierKind::Cusip => validate_cusip_str(value),
        IdentifierKind::Lei => validate_lei_str(value),
        IdentifierKind::Bic => validate_bic_str(value),
        IdentifierKind::Mic => validate_mic_str(value),
        IdentifierKind::Iban => validate_iban(value.as_bytes()),
        IdentifierKind::Currency => validate_currency_str(value),
    }
}

pub fn validate_instrument_identifier(value: &str) -> bool {
    validate_identifier(IdentifierKind::Isin, value)
        || validate_identifier(IdentifierKind::Cusip, value)
}

fn validate_numeric(value: &[u8]) -> bool {
    !value.is_empty() && value.iter().all(|b| b.is_ascii_digit())
}

fn parse_ascii_u32(slice: &[u8]) -> Option<u32> {
    if slice.is_empty() {
        return None;
    }
    let mut acc = 0u32;
    for &b in slice {
        if !b.is_ascii_digit() {
            return None;
        }
        acc = acc * 10 + u32::from(b - b'0');
    }
    Some(acc)
}

fn validate_date(value: &[u8]) -> bool {
    if value.len() != 10 {
        return false;
    }
    if value[4] != b'-' || value[7] != b'-' {
        return false;
    }
    let year = match parse_ascii_u32(&value[0..4]) {
        Some(v) => v,
        None => return false,
    };
    let month = match parse_ascii_u32(&value[5..7]) {
        Some(v) => v,
        None => return false,
    };
    let day = match parse_ascii_u32(&value[8..10]) {
        Some(v) => v,
        None => return false,
    };
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap { 29 } else { 28 }
        }
        _ => return false,
    };
    day <= max_day
}

fn validate_offset(offset: &str) -> bool {
    if offset.len() != 6 {
        return false;
    }
    let mut chars = offset.chars();
    let sign = chars.next().unwrap_or('+');
    if sign != '+' && sign != '-' {
        return false;
    }
    let hour_tens = chars.next().and_then(|c| c.to_digit(10));
    let hour_ones = chars.next().and_then(|c| c.to_digit(10));
    if chars.next() != Some(':') {
        return false;
    }
    let min_tens = chars.next().and_then(|c| c.to_digit(10));
    let min_ones = chars.next().and_then(|c| c.to_digit(10));
    if chars.next().is_some() {
        return false;
    }
    let hour = match (hour_tens, hour_ones) {
        (Some(t), Some(o)) => t * 10 + o,
        _ => return false,
    };
    let minute = match (min_tens, min_ones) {
        (Some(t), Some(o)) => t * 10 + o,
        _ => return false,
    };
    hour <= 23 && minute <= 59
}

fn validate_datetime(value: &[u8]) -> bool {
    let s = match core::str::from_utf8(value) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let (date_part, rest) = match s.split_once('T') {
        Some(parts) => parts,
        None => return false,
    };
    if !validate_date(date_part.as_bytes()) {
        return false;
    }
    let (time_part, tz_part) = if let Some(v) = rest.strip_suffix('Z') {
        (v, None)
    } else if let Some(pos) = rest.rfind(['+', '-']) {
        (rest[..pos].trim_end_matches('Z'), Some(&rest[pos..]))
    } else {
        (rest, None)
    };
    let mut pieces = time_part.split(':');
    let hour = match pieces.next() {
        Some(v) if v.len() == 2 => match v.parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        },
        _ => return false,
    };
    let minute = match pieces.next() {
        Some(v) if v.len() == 2 => match v.parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        },
        _ => return false,
    };
    let sec_fragment = match pieces.next() {
        Some(v) => v,
        None => return false,
    };
    if pieces.next().is_some() {
        return false;
    }
    let (second_str, fraction_ok) = if let Some((sec, frac)) = sec_fragment.split_once('.') {
        (
            sec,
            !frac.is_empty() && frac.len() <= 6 && frac.chars().all(|c| c.is_ascii_digit()),
        )
    } else {
        (sec_fragment, true)
    };
    if !fraction_ok || second_str.len() != 2 {
        return false;
    }
    let second = match second_str.parse::<u32>() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let tz_ok = if let Some(offset) = tz_part {
        validate_offset(offset)
    } else {
        true
    };
    tz_ok && hour <= 23 && minute <= 59 && second <= 60
}

fn validate_identifier_bytes(kind: IdentifierKind, value: &[u8]) -> Result<(), InvalidReason> {
    let text = core::str::from_utf8(value).map_err(|_| InvalidReason::Utf8)?;
    if validate_identifier(kind, text) {
        Ok(())
    } else {
        Err(InvalidReason::Identifier(kind))
    }
}

fn validate_instrument_bytes(value: &[u8]) -> Result<(), InvalidReason> {
    let text = core::str::from_utf8(value).map_err(|_| InvalidReason::Utf8)?;
    if validate_identifier(IdentifierKind::Isin, text)
        || validate_identifier(IdentifierKind::Cusip, text)
    {
        Ok(())
    } else {
        Err(InvalidReason::Instrument)
    }
}

fn validate_field(kind: FieldKind, value: &[u8]) -> Result<(), InvalidReason> {
    match kind {
        FieldKind::Text => {
            if value.is_empty() {
                Err(InvalidReason::Empty)
            } else {
                Ok(())
            }
        }
        FieldKind::Numeric => {
            if validate_numeric(value) {
                Ok(())
            } else {
                Err(InvalidReason::Numeric)
            }
        }
        FieldKind::Amount => {
            if validate_amount(value) {
                Ok(())
            } else {
                Err(InvalidReason::Amount)
            }
        }
        FieldKind::Identifier(kind) => validate_identifier_bytes(kind, value),
        FieldKind::Instrument => validate_instrument_bytes(value),
        FieldKind::Date => {
            if validate_date(value) {
                Ok(())
            } else {
                Err(InvalidReason::Date)
            }
        }
        FieldKind::DateTime => {
            if validate_datetime(value) {
                Ok(())
            } else {
                Err(InvalidReason::DateTime)
            }
        }
        FieldKind::Enum(options) => {
            if options.iter().any(|opt| opt.as_bytes() == value) {
                Ok(())
            } else {
                Err(InvalidReason::Enum)
            }
        }
    }
}

fn proxy_fallback_match<'a>(
    pattern: &str,
    message: &'a IsoMessage,
) -> Option<(&'a String, &'a Vec<u8>, FieldKind)> {
    match pattern {
        "DbtrAcct" => message
            .fields
            .get_key_value("DbtrAcct/Prxy/Id")
            .map(|(field, value)| (field, value, FieldKind::Text)),
        "CdtrAcct" => message
            .fields
            .get_key_value("CdtrAcct/Prxy/Id")
            .map(|(field, value)| (field, value, FieldKind::Text)),
        _ => None,
    }
}

fn validate_message_against_schema(
    message: &IsoMessage,
    schema: &MessageSchema,
) -> Result<(), ValidationFailure> {
    for spec in schema.field_specs() {
        let mut matches: Vec<(&String, &Vec<u8>, FieldKind)> = message
            .fields
            .iter()
            .filter(|(field, _)| pattern_matches(spec.pattern, field))
            .map(|(field, value)| (field, value, spec.kind))
            .collect();
        if matches.is_empty()
            && let Some(fallback) = proxy_fallback_match(spec.pattern, message)
        {
            matches.push(fallback);
        }
        if matches.is_empty() && matches!(spec.requirement, Requirement::Required) {
            return Err(ValidationFailure::MissingField(spec.pattern));
        }
        if let Some(max) = spec.max_occurs
            && matches.len() > max
        {
            return Err(ValidationFailure::TooManyOccurrences {
                field: spec.pattern,
                max,
                actual: matches.len(),
            });
        }
        for (field, value, kind) in matches {
            if let Err(reason) = validate_field(kind, value) {
                return Err(ValidationFailure::InvalidField {
                    field: field.clone(),
                    reason,
                });
            }
        }
    }
    Ok(())
}

fn collect_fields_in_order<'a>(
    message: &'a IsoMessage,
    schema: Option<&'static MessageSchema>,
) -> Vec<(&'a String, &'a Vec<u8>, usize)> {
    let mut pairs: Vec<(&String, &Vec<u8>, usize)> = message
        .fields
        .iter()
        .map(|(key, value)| {
            let order = schema
                .and_then(|s| {
                    s.field_specs()
                        .iter()
                        .position(|spec| pattern_matches(spec.pattern, key))
                })
                .unwrap_or(usize::MAX);
            (key, value, order)
        })
        .collect();
    pairs.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(b.0)));
    pairs
}

fn escape_xml_text(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn escape_xml_attr(input: &str) -> String {
    escape_xml_text(input)
}

fn serialize_key_value(message: &IsoMessage, schema: Option<&'static MessageSchema>) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (key, value, _)) in collect_fields_in_order(message, schema)
        .into_iter()
        .enumerate()
    {
        if i != 0 {
            out.push(b'\n');
        }
        out.extend_from_slice(key.as_bytes());
        out.push(b'=');
        out.extend_from_slice(value);
    }
    out
}

fn serialize_xml(message: &IsoMessage, schema: Option<&'static MessageSchema>) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = write!(out, "<ISO20022 message=\"{}\">", message.message_type);
    for (key, value, _) in collect_fields_in_order(message, schema) {
        let path = escape_xml_attr(key);
        if let Ok(text) = core::str::from_utf8(value) {
            let escaped = escape_xml_text(text);
            let _ = write!(out, "<Field path=\"{path}\">{escaped}</Field>");
        } else {
            let encoded = encode_base64(value);
            let encoded_str = String::from_utf8(encoded).unwrap_or_default();
            let _ = write!(
                out,
                "<Field path=\"{path}\" encoding=\"base64\">{encoded_str}</Field>"
            );
        }
    }
    out.extend_from_slice(b"</ISO20022>");
    out
}

fn looks_like_xml(data: &[u8]) -> bool {
    data.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'<')
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn message_type_from_namespace(ns: &str) -> Option<String> {
    ns.rsplit(':').next().map(|s| s.to_owned())
}

fn repeating_bases_for(message_type: &str) -> Vec<String> {
    schema_for(message_type)
        .map(|schema| {
            schema
                .field_specs()
                .iter()
                .filter_map(|spec| repeating_base(spec.pattern))
                .map(|base| base.to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn should_index(path: &str, repeating_bases: &[String]) -> bool {
    repeating_bases.iter().any(|base| path.ends_with(base))
}

const SIGNATURE_IGNORED_VALUE: &[u8] = b"signature-block-ignored";

fn should_skip_element(name: &str) -> bool {
    matches!(
        local_name(name),
        "Sgntr"
            | "Signature"
            | "SignedInfo"
            | "KeyInfo"
            | "Object"
            | "QualifyingProperties"
            | "SignatureValue"
            | "SignedProperties"
    )
}

fn normalised_parts(stack: &[String]) -> Vec<String> {
    stack
        .iter()
        .map(|s| s.as_str())
        .filter(|s| {
            let name = local_name(s);
            !matches!(name, "DataPDU" | "DataEnvelope" | "Body")
        })
        .map(|s| local_name(s).to_owned())
        .collect()
}

fn current_path(stack: &[String]) -> Option<String> {
    let parts = normalised_parts(stack);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut in_quote = None;
    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) if b == q => in_quote = None,
            None if b == b'"' || b == b'\'' => in_quote = Some(b),
            None if b == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn parse_attributes(tag_body: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut cursor = tag_body.trim();
    if let Some((_, rest)) = cursor.split_once(char::is_whitespace) {
        cursor = rest.trim();
    } else {
        return attrs;
    }
    if cursor.ends_with('/') {
        cursor = cursor.trim_end_matches('/').trim_end();
    }
    while !cursor.is_empty() {
        let Some(eq_idx) = cursor.find('=') else {
            break;
        };
        let name = cursor[..eq_idx].trim().to_owned();
        let mut remainder = cursor[eq_idx + 1..].trim_start();
        if remainder.is_empty() {
            break;
        }
        let (value, consumed) = if remainder.starts_with('"') || remainder.starts_with('\'') {
            let quote = remainder.as_bytes()[0] as char;
            remainder = &remainder[1..];
            let Some(end_idx) = remainder.find(quote) else {
                break;
            };
            (&remainder[..end_idx], end_idx + 1)
        } else {
            let end_idx = remainder
                .find(char::is_whitespace)
                .unwrap_or(remainder.len());
            (&remainder[..end_idx], end_idx)
        };
        attrs.push((name, value.to_owned()));
        remainder = remainder.get(consumed..).unwrap_or("").trim_start();
        cursor = remainder;
    }
    attrs
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let name_bytes = name.as_bytes();
    let mut i = 0;
    while i + name_bytes.len() + 2 < bytes.len() {
        if &bytes[i..i + name_bytes.len()] == name_bytes
            && bytes[i + name_bytes.len()] == b'='
            && bytes[i + name_bytes.len() + 1] == b'"'
        {
            let mut end = i + name_bytes.len() + 2;
            while end < bytes.len() && bytes[end] != b'"' {
                end += 1;
            }
            if end < bytes.len() {
                return tag.get(i + name_bytes.len() + 2..end);
            } else {
                return None;
            }
        }
        i += 1;
    }
    None
}

fn unescape_xml_text(input: &str) -> Result<String, MsgError> {
    if !input.contains('&') {
        return Ok(input.to_owned());
    }
    let mut out = String::with_capacity(input.len());
    let mut remainder = input;
    while let Some(idx) = remainder.find('&') {
        out.push_str(&remainder[..idx]);
        remainder = &remainder[idx + 1..];
        let Some(end) = remainder.find(';') else {
            return Err(MsgError::InvalidFormat);
        };
        let entity = &remainder[..end];
        remainder = &remainder[end + 1..];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => return Err(MsgError::InvalidFormat),
        }
    }
    out.push_str(remainder);
    Ok(out)
}

fn parse_key_values(message_type: &str, text: &str) {
    for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
        msg_set(key.trim(), value.trim().as_bytes());
    }
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            // Ensure the message type was set correctly for empty inputs.
            if m.message_type != message_type {
                m.message_type = message_type.to_owned();
            }
        }
    });
}

fn parse_real_iso20022(message_type: &str, text: &str) -> Result<(), MsgError> {
    let mut stack: Vec<String> = Vec::new();
    let mut skip_stack: Vec<bool> = Vec::new();
    let mut skip_depth = 0usize;
    let repeating_bases = repeating_bases_for(message_type);
    let mut repeat_counters: HashMap<String, usize> = HashMap::new();
    let mut declared_message_type: Option<String> = None;

    let mut idx = 0usize;
    let bytes = text.as_bytes();
    let len = bytes.len();

    while idx < len {
        let next_lt = match text[idx..].find('<') {
            Some(offset) => idx + offset,
            None => {
                let tail = &text[idx..];
                if skip_depth == 0
                    && let Some(path) = current_path(&stack)
                {
                    let trimmed = tail.trim();
                    if !trimmed.is_empty() {
                        if stack
                            .last()
                            .is_some_and(|last| local_name(last) == "MsgDefIdr")
                        {
                            declared_message_type.get_or_insert_with(|| trimmed.to_owned());
                        }
                        msg_set(&path, trimmed.as_bytes());
                    }
                }
                break;
            }
        };
        if next_lt > idx && skip_depth == 0 {
            let body = &text[idx..next_lt];
            if let Some(path) = current_path(&stack) {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    if stack
                        .last()
                        .is_some_and(|last| local_name(last) == "MsgDefIdr")
                    {
                        declared_message_type.get_or_insert_with(|| trimmed.to_owned());
                    }
                    msg_set(&path, trimmed.as_bytes());
                }
            }
        }
        let Some(tag_end) = find_tag_end(bytes, next_lt + 1) else {
            return Err(MsgError::InvalidFormat);
        };
        let raw_tag = &text[next_lt + 1..tag_end];
        idx = tag_end + 1;

        let tag = raw_tag.trim();
        if tag.starts_with("!--") || tag.starts_with("?") {
            continue;
        }
        let closing = tag.starts_with('/');
        let tag_body = if closing {
            tag.trim_start_matches('/').trim()
        } else {
            tag
        };
        let self_closing = !closing && tag_body.ends_with('/');
        let tag_body = if self_closing {
            tag_body.trim_end_matches('/').trim_end()
        } else {
            tag_body
        };

        let (name_part, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        let lname = local_name(name_part);

        if closing {
            if let Some(skipped) = skip_stack.pop()
                && skipped
                && skip_depth > 0
            {
                skip_depth -= 1;
            }
            stack.pop();
            continue;
        }

        let attrs = parse_attributes(tag_body);
        let mut parts = normalised_parts(&stack);
        parts.push(lname.to_owned());
        let base_path = parts.join("/");
        let mut element_name = lname.to_owned();
        if should_index(&base_path, &repeating_bases) {
            let counter = repeat_counters.entry(base_path.clone()).or_insert(0);
            element_name = format!("{lname}[{counter}]");
            *counter += 1;
        }
        stack.push(element_name.clone());

        let is_skipped = should_skip_element(lname);
        if is_skipped && skip_depth == 0 {
            // Track that a signature subtree was present while deliberately ignoring its contents.
            if let Some(path) = current_path(&stack) {
                let marker_path = format!("{path}/@ignored");
                msg_set(&marker_path, SIGNATURE_IGNORED_VALUE);
            }
        }
        skip_stack.push(is_skipped);
        if is_skipped {
            skip_depth += 1;
        }

        if skip_depth == 0 {
            if lname == "Document" {
                for (attr_name, value) in &attrs {
                    if attr_name == "xmlns"
                        && let Some(mt) = message_type_from_namespace(value)
                    {
                        declared_message_type.get_or_insert(mt);
                    }
                }
            }
            if let Some(path) = current_path(&stack) {
                for (attr_name, value) in &attrs {
                    if attr_name.starts_with("xmlns") {
                        continue;
                    }
                    let attr_path = format!("{path}/@{}", local_name(attr_name));
                    msg_set(&attr_path, value.as_bytes());
                }
            }
        }

        if self_closing {
            if let Some(skipped) = skip_stack.pop()
                && skipped
                && skip_depth > 0
            {
                skip_depth -= 1;
            }
            stack.pop();
        }
    }

    if let Some(declared) = declared_message_type {
        let canonical_declared = canonical_message_type(&declared);
        let canonical_requested = canonical_message_type(message_type);
        if canonical_requested != "head.001" && canonical_declared != canonical_requested {
            return Err(MsgError::UnknownMessageType);
        }
    }
    Ok(())
}

fn parse_xml_into_current(message_type: &str, text: &str) -> Result<(), MsgError> {
    let trimmed = text.trim();
    let start = trimmed.find("<ISO20022").ok_or(MsgError::InvalidFormat)?;
    let after_start = &trimmed[start..];
    let tag_end = after_start.find('>').ok_or(MsgError::InvalidFormat)?;
    let tag = &after_start[..tag_end];
    let declared = attr_value(tag, "message").ok_or(MsgError::InvalidFormat)?;
    if declared != message_type {
        return Err(MsgError::UnknownMessageType);
    }
    let cursor = &after_start[tag_end + 1..];
    if let Some(close_idx) = cursor.find("</ISO20022>") {
        let mut fields = &cursor[..close_idx];
        while let Some(field_idx) = fields.find("<Field") {
            fields = &fields[field_idx + "<Field".len()..];
            let field_tag_end = fields.find('>').ok_or(MsgError::InvalidFormat)?;
            let field_tag = &fields[..field_tag_end];
            fields = &fields[field_tag_end + 1..];
            let path = attr_value(field_tag, "path").ok_or(MsgError::InvalidFormat)?;
            let encoding = attr_value(field_tag, "encoding");
            let end_idx = fields.find("</Field>").ok_or(MsgError::InvalidFormat)?;
            let value_text = fields[..end_idx].trim();
            fields = &fields[end_idx + "</Field>".len()..];
            let value = if encoding == Some("base64") {
                decode_base64(value_text.as_bytes()).ok_or(MsgError::InvalidFormat)?
            } else {
                unescape_xml_text(value_text)?.into_bytes()
            };
            msg_set(path, &value);
        }
    } else {
        return Err(MsgError::InvalidFormat);
    }
    Ok(())
}

#[allow(dead_code)]
struct HttpEndpoint {
    host: String,
    port: u16,
    path: String,
}

impl HttpEndpoint {
    fn parse(channel: &str) -> Option<Self> {
        let trimmed = channel.trim();
        let without_scheme = trimmed.strip_prefix("http://")?;
        let (host_part, path_part) = match without_scheme.split_once('/') {
            Some((host, path)) => (host, format!("/{path}")),
            None => (without_scheme, "/".to_owned()),
        };
        if host_part.is_empty() {
            return None;
        }
        let (host, port) = if let Some((name, port_str)) = host_part.rsplit_once(':') {
            let port = u16::from_str(port_str).ok()?;
            (name.to_owned(), port)
        } else {
            (host_part.to_owned(), 80)
        };
        Some(Self {
            host,
            port,
            path: path_part,
        })
    }
}

fn send_http(endpoint: &HttpEndpoint, payload: &[u8]) -> Result<(), MsgError> {
    #[cfg(test)]
    if let Some(override_cb) = HTTP_SENDER_OVERRIDE.with(|sender| sender.borrow().as_ref().copied())
    {
        return override_cb(endpoint, payload);
    }
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect((endpoint.host.as_str(), endpoint.port))?;
    let host_header = if endpoint.port == 80 {
        endpoint.host.clone()
    } else {
        format!("{}:{}", endpoint.host, endpoint.port)
    };
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n",
        endpoint.path,
        host_header,
        payload.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(payload)?;
    stream.flush()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let status = response
        .splitn(2, |b| *b == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or(MsgError::InvalidFormat)?;
    if !(200..300).contains(&status) {
        return Err(MsgError::HttpStatus(status));
    }
    Ok(())
}

/// Encode a numeric amount as an ASCII string.
pub fn encode_amount(value: u64) -> Vec<u8> {
    value.to_string().into_bytes()
}

/// Decode a numeric amount from an ASCII string.
///
/// Returns `None` if the input contains non-digit characters or does not fit
/// into a `u64`.
pub fn decode_amount(value: &[u8]) -> Option<u64> {
    if value.iter().all(|b| b.is_ascii_digit()) {
        core::str::from_utf8(value).ok()?.parse().ok()
    } else {
        None
    }
}

/// Base64 alphabet used by [`encode_base64`] and [`decode_base64`].
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Precomputed table mapping ASCII bytes to their 6-bit Base64 value.
/// Invalid bytes map to `0xFF`.
const fn build_b64_decode_table() -> [u8; 256] {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < 64 {
        table[BASE64_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    // Padding character is treated as zero during decoding.
    table[b'=' as usize] = 0;
    table
}

const BASE64_DECODE_TABLE: [u8; 256] = build_b64_decode_table();

/// Encode binary data as a Base64 ASCII string.
///
/// This lightweight helper is sufficient for tests and prototypes and uses
/// constant-time table lookups.
pub fn encode_base64(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(BASE64_TABLE[(b0 >> 2) as usize]);
        out.push(BASE64_TABLE[((b0 & 0x03) << 4 | (b1 >> 4)) as usize]);
        if chunk.len() > 1 {
            out.push(BASE64_TABLE[((b1 & 0x0F) << 2 | (b2 >> 6)) as usize]);
        } else {
            out.push(b'=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_TABLE[(b2 & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }
    }
    out
}

/// Decode a Base64 ASCII string into the provided output buffer.
///
/// The caller supplies the destination [`Vec`] which is extended with the
/// decoded bytes. This allows large payloads to be processed without
/// allocating a fresh buffer for every call.
///
/// Returns `None` if the input contains invalid characters or has the wrong
/// padding.
pub fn decode_base64_into(data: &[u8], out: &mut Vec<u8>) -> Option<()> {
    if !data.len().is_multiple_of(4) {
        return None;
    }
    out.reserve(data.len().div_ceil(4) * 3);
    let mut i = 0;
    while i < data.len() {
        let n0 = BASE64_DECODE_TABLE[data[i] as usize];
        let n1 = BASE64_DECODE_TABLE[data[i + 1] as usize];
        let n2 = BASE64_DECODE_TABLE[data[i + 2] as usize];
        let n3 = BASE64_DECODE_TABLE[data[i + 3] as usize];
        if (n0 | n1 | n2 | n3) == 0xFF {
            return None;
        }
        out.push((n0 << 2) | (n1 >> 4));
        if data[i + 2] != b'=' {
            out.push((n1 << 4) | (n2 >> 2));
            if data[i + 3] != b'=' {
                out.push((n2 << 6) | n3);
            } else if n3 != 0 {
                return None;
            }
        } else if n2 != 0 || n3 != 0 {
            return None;
        }
        i += 4;
    }
    Some(())
}

/// Decode a Base64 ASCII string back into binary data.
///
/// Returns `None` if the input contains invalid characters or has the wrong
/// padding.
pub fn decode_base64(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len().div_ceil(4) * 3);
    decode_base64_into(data, &mut out)?;
    Some(out)
}

/// Encode a byte slice according to a named format.
///
/// Currently only `"BASE64"` is supported.
pub fn encode_str(format: &str, value: &[u8]) -> Vec<u8> {
    match format {
        "BASE64" => encode_base64(value),
        _ => value.to_vec(),
    }
}

/// Decode a string according to a named format.
///
/// Currently only `"BASE64"` is supported.
pub fn decode_str(format: &str, value: &[u8]) -> Option<Vec<u8>> {
    match format {
        "BASE64" => decode_base64(value),
        _ => Some(value.to_vec()),
    }
}

/// Validate a value against a named pattern.
///
/// Supported patterns are `"IBAN"`, `"BIC"`, and `"NUMERIC"`.
pub fn validate_format(pattern: &str, value: &[u8]) -> bool {
    match pattern {
        "IBAN" => validate_iban(value),
        "BIC" => validate_bic(value),
        "NUMERIC" => value.iter().all(|b| b.is_ascii_digit()),
        _ => false,
    }
}

/// Create a new ISO 20022 message of the given type.
///
/// The real opcode will allocate structures based on schema definitions.  The
/// temporary implementation simply initialises an empty [`IsoMessage`] and
/// stores it in a thread-local slot.
pub fn msg_create(message_type: &str) {
    MESSAGE_STACK.with(|stack| {
        stack.borrow_mut().push(IsoMessage {
            message_type: message_type.to_owned(),
            ..IsoMessage::default()
        });
    });
}

/// Clone the current ISO 20022 message object.
///
/// This operation is cheap in the temporary implementation since the message is
/// stored in thread-local memory.  Cloning will duplicate the in-memory
/// structure which is sufficient for tests.
pub fn msg_clone() {
    MESSAGE_STACK.with(|stack| {
        let cloned = { stack.borrow().last().cloned() };
        if let Some(m) = cloned {
            stack.borrow_mut().push(m);
        }
    });
}

/// Set the value of an ISO 20022 field.
///
/// Values are stored verbatim; type checking is intentionally omitted but the
/// call site can at least observe storage behaviour.
pub fn msg_set(field: &str, value: &[u8]) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let key = canonical_field_name(&m.message_type, field);
            m.fields.insert(key, value.to_vec());
        }
    });
}

/// Retrieve the value of an ISO 20022 field.
pub fn msg_get(field: &str) -> Option<Vec<u8>> {
    MESSAGE_STACK.with(|stack| {
        let borrow = stack.borrow();
        let message = borrow.last()?;
        let key = canonical_field_name(&message.message_type, field);
        message.fields.get(&key).cloned()
    })
}

/// Append a repeating ISO 20022 sub-structure.
///
/// Each call creates an empty entry with an incremented index.  The entry can
/// later be populated using [`msg_set`] with the generated key.
pub fn msg_add(field: &str) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let base = canonical_repeating_base(&m.message_type, field);
            let count = m.repeats.entry(base.clone()).or_insert(0);
            let key = format!("{}[{}]", base, *count);
            m.fields.entry(key).or_default();
            *count += 1;
        }
    });
}

/// Remove a field or sub-structure from the current message.
pub fn msg_remove(field: &str) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let key = canonical_field_name(&m.message_type, field);
            m.fields.remove(&key);
        }
    });
}

/// Clear all fields of the current ISO 20022 message.
pub fn msg_clear() {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            m.fields.clear();
            m.repeats.clear();
        }
    });
}

/// Parse raw data into an ISO 20022 message.
///
/// The temporary parser understands a very small "key=value" line oriented
/// format so tests can interact with individual fields without pulling in a
/// full XML stack. Lines that don't contain an `=` sign are ignored. Values
/// are stored exactly as provided.
pub fn msg_parse(message_type: &str, data: &[u8]) -> Result<(), MsgError> {
    msg_create(message_type);
    let result = if looks_like_xml(data) {
        let text = core::str::from_utf8(data).map_err(|_| MsgError::InvalidFormat)?;
        if text.contains("<ISO20022") {
            parse_xml_into_current(message_type, text)
        } else {
            parse_real_iso20022(message_type, text)
        }
    } else {
        let text = core::str::from_utf8(data).map_err(|_| MsgError::InvalidFormat)?;
        parse_key_values(message_type, text);
        Ok(())
    };
    if result.is_err() {
        MESSAGE_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
    result
}

/// Serialize the current ISO 20022 message into a simple key=value format.
///
/// Fields are written one per line in lexicographic order of their keys.
pub fn msg_serialize(format: &str) -> Result<Vec<u8>, MsgError> {
    MESSAGE_STACK.with(|stack| {
        let borrow = stack.borrow();
        let message = borrow.last().ok_or(MsgError::NoActiveMessage)?;
        let schema = schema_for(&message.message_type);
        let normalized = if format.is_empty() {
            "KV".to_owned()
        } else {
            format.to_ascii_uppercase()
        };
        match normalized.as_str() {
            "XML" => Ok(serialize_xml(message, schema)),
            "KV" | "KEYVALUE" => Ok(serialize_key_value(message, schema)),
            _ => Err(MsgError::InvalidFormat),
        }
    })
}

/// Validate the current ISO 20022 message against schema rules.
///
/// Rather than a full schema engine we keep a tiny table of mandatory fields
/// for a handful of message types. Validation succeeds when the current message
/// exists **and** all required fields for its type are present.
pub fn msg_validate() -> bool {
    clear_validation_failure();
    MESSAGE_STACK.with(|stack| {
        stack.borrow().last().is_some_and(|m| {
            if let Some(schema) = schema_for(&m.message_type) {
                match validate_message_against_schema(m, schema) {
                    Ok(()) => true,
                    Err(err) => {
                        record_validation_failure(err);
                        false
                    }
                }
            } else {
                !m.fields.is_empty()
            }
        })
    })
}

/// Sign the current ISO 20022 message.
///
/// Uses Ed25519, secp256k1, or ML-DSA (Dilithium3) depending on the key
/// length. Secret keys may be prefixed with an `iroha_crypto::Algorithm` tag;
/// secp256k1 signing requires the `Algorithm::Secp256k1` tag to disambiguate
/// 32-byte secret keys. The function signs the serialized message bytes and
/// returns the signature or an empty vector if signing fails.
#[allow(unused_variables)]
pub fn msg_sign(key: &[u8]) -> Vec<u8> {
    let msg = match msg_serialize("XML") {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    use pqcrypto_mldsa::mldsa65 as dilithium;
    use pqcrypto_traits::sign::{DetachedSignature as _, SecretKey as _};

    if let Some((tag, rest)) = key.split_first() {
        if *tag == Algorithm::Ed25519 as u8 && rest.len() == 32 {
            let Ok(sk_bytes) = <[u8; 32]>::try_from(rest) else {
                return Vec::new();
            };
            let sk = SigningKey::from_bytes(&sk_bytes);
            return sk.sign(&msg).to_bytes().to_vec();
        }
        if *tag == Algorithm::Secp256k1 as u8 && rest.len() == 32 {
            let Ok(sk_bytes) = <[u8; 32]>::try_from(rest) else {
                return Vec::new();
            };
            let Ok(sk) = EcdsaSecp256k1Sha256::parse_private_key(&sk_bytes) else {
                return Vec::new();
            };
            return EcdsaSecp256k1Sha256::sign(&msg, &sk);
        }
        if *tag == Algorithm::MlDsa as u8 && rest.len() == dilithium::secret_key_bytes() {
            let Ok(sk) = dilithium::SecretKey::from_bytes(rest) else {
                return Vec::new();
            };
            let sig = dilithium::detached_sign(&msg, &sk);
            return sig.as_bytes().to_vec();
        }
    }

    if let Ok(sk_bytes) = <[u8; 32]>::try_from(key) {
        let sk = SigningKey::from_bytes(&sk_bytes);
        return sk.sign(&msg).to_bytes().to_vec();
    }
    if key.len() == dilithium::secret_key_bytes()
        && let Ok(sk) = dilithium::SecretKey::from_bytes(key)
    {
        let sig = dilithium::detached_sign(&msg, &sk);
        return sig.as_bytes().to_vec();
    }
    Vec::new()
}

/// Verify the signature on an ISO 20022 message.
///
/// Uses the [`verify_signature`] helper with Ed25519, secp256k1, or ML-DSA
/// depending on the key length. The serialized message bytes are used as the
/// signing payload. Secp256k1 expects a 33-byte compressed SEC1 public key.
#[allow(unused_variables)]
pub fn msg_verify_sig(sig: &[u8], key: &[u8]) -> bool {
    let msg = match msg_serialize("XML") {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    {
        if key.len() == 32 {
            return verify_signature(SignatureScheme::Ed25519, &msg, sig, key);
        }
    }
    {
        use pqcrypto_mldsa::mldsa65 as dilithium;
        if key.len() == dilithium::public_key_bytes() && sig.len() == dilithium::signature_bytes() {
            return verify_signature(SignatureScheme::MlDsa, &msg, sig, key);
        }
    }
    {
        if key.len() == 33 && sig.len() == 64 {
            return verify_signature(SignatureScheme::Secp256k1, &msg, sig, key);
        }
    }
    false
}

/// Callback type used for delivering messages.
pub type MsgSendCallback = fn(&str, &[u8]);

#[cfg(test)]
type TestHttpSender = fn(&HttpEndpoint, &[u8]) -> Result<(), MsgError>;

/// Set the backend used by [`msg_send`].
///
/// Passing `None` restores the default in-memory log based behaviour.
pub fn set_msg_sender(callback: Option<MsgSendCallback>) {
    MSG_SENDER.with(|s| *s.borrow_mut() = callback);
}

/// Send the current ISO 20022 message through a channel.
///
/// Before dispatching the message is validated using [`msg_validate`]; if
/// validation fails the function returns `false` and nothing is sent. When the
/// message is valid it is serialised using [`msg_serialize`] and either passed
/// to a custom backend registered via [`set_msg_sender`] or recorded in a
/// thread‑local log for inspection. HTTP channels are delivered via a simple
/// TCP client; tests can override delivery. The serialised bytes are sent and
/// the function returns `true` to indicate success.
pub fn msg_send(channel: &str) -> Result<(), MsgError> {
    if !msg_validate() {
        return Err(MsgError::ValidationFailed);
    }
    let data = msg_serialize("XML")?;
    if channel.trim_start().starts_with("http://") {
        let endpoint = HttpEndpoint::parse(channel).ok_or(MsgError::InvalidFormat)?;
        send_http(&endpoint, &data)?;
    } else {
        MSG_SENDER.with(|sender| {
            if let Some(cb) = *sender.borrow() {
                cb(channel, &data);
            } else {
                SENT_MSGS.with(|log| {
                    log.borrow_mut().push((channel.to_owned(), data.clone()));
                });
            }
        });
    }
    Ok(())
}

/// Drain the log of previously sent messages.
///
/// This helper is primarily intended for tests or prototype environments that
/// want to examine what would have been dispatched without installing a custom
/// backend. The log is cleared as part of this call.
pub fn take_sent_messages() -> Vec<(String, Vec<u8>)> {
    SENT_MSGS.with(|log| log.borrow_mut().drain(..).collect())
}

thread_local! {
    /// Collected messages sent via [`msg_send`].
    static SENT_MSGS: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    /// Optional callback used to deliver messages.
    static MSG_SENDER: RefCell<Option<MsgSendCallback>> = RefCell::new(None);
}

#[cfg(test)]
thread_local! {
    static HTTP_SENDER_OVERRIDE: RefCell<Option<TestHttpSender>> = const { RefCell::new(None) };
}

#[cfg(test)]
/// Register a test-only HTTP sender override, bypassing real network dispatch.
fn set_http_sender_override(callback: Option<TestHttpSender>) {
    HTTP_SENDER_OVERRIDE.with(|sender| *sender.borrow_mut() = callback);
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader, ErrorKind, Read, Write},
        net::TcpListener,
        thread,
    };

    use ed25519_dalek::SigningKey;
    use norito::codec::{Decode, Encode};

    use super::{
        norito_schemas::{Colr007, Linkage, Sese023, Sese025},
        *,
    };

    thread_local! {
        static HTTP_CALLS: RefCell<Vec<(String, u16, String, Vec<u8>)>> = const {
            RefCell::new(Vec::new())
        };
    }

    // Helper to reset the thread-local between tests.
    fn reset() {
        MESSAGE_STACK.with(|m| m.borrow_mut().clear());
        SENT_MSGS.with(|s| s.borrow_mut().clear());
        set_msg_sender(None);
        set_http_sender_override(None);
        HTTP_CALLS.with(|calls| calls.borrow_mut().clear());
    }

    fn populate_pacs008_minimal() {
        msg_set("MsgId", b"1");
        msg_set("IntrBkSttlmCcy", b"USD");
        msg_set("IntrBkSttlmAmt", b"100");
        msg_set("IntrBkSttlmDt", b"2024-01-01");
        msg_set("DbtrAcct", b"GB82WEST12345698765432");
        msg_set("CdtrAcct", b"GB33BUKB20201555555555");
        msg_set("DbtrAgt", b"DEUTDEFF");
        msg_set("CdtrAgt", b"DEUTDEFF");
    }

    fn populate_camt053_minimal() {
        msg_set("Stmt/Id", b"1");
        msg_set("Stmt/Acct/Id", b"GB82WEST12345698765432");
        msg_set("Stmt/Acct/Ccy", b"USD");
        msg_set("Stmt/Bal[0]/Amt", b"100");
        msg_set("Stmt/Bal[0]/Ccy", b"USD");
        msg_set("Stmt/Bal[0]/Cd", b"CRDT");
    }

    fn populate_camt052_minimal() {
        msg_set("Rpt/Id", b"RPT1");
        msg_set("Rpt/CreDtTm", b"2024-01-01T00:00:00Z");
        msg_set("Rpt/Acct/Id", b"GB82WEST12345698765432");
        msg_set("Rpt/Acct/Ccy", b"USD");
        msg_add("Rpt/Ntry");
        msg_set("Rpt/Ntry[0]/Amt", b"100.00");
        msg_set("Rpt/Ntry[0]/CdtDbtInd", b"CRDT");
        msg_set("Rpt/Ntry[0]/BookgDt", b"2024-01-01");
    }

    fn populate_pain001_minimal() {
        msg_set("GrpHdr/MsgId", b"MSG-1");
        msg_set("GrpHdr/CreDtTm", b"2024-01-01T10:00:00Z");
        msg_set("GrpHdr/NbOfTxs", b"1");
        msg_set("GrpHdr/InitgPty/Nm", b"Initiator");
        msg_add("PmtInf");
        msg_set("PmtInf[0]/PmtInfId", b"PMT1");
        msg_set("PmtInf[0]/ReqdExctnDt", b"2024-01-02");
        msg_set("PmtInf[0]/DbtrAcct/Id", b"GB82WEST12345698765432");
        msg_add("PmtInf[0]/CdtTrfTxInf");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/Amt", b"100");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/Ccy", b"USD");
        msg_set(
            "PmtInf[0]/CdtTrfTxInf[0]/CdtrAcct/Id",
            b"GB33BUKB20201555555555",
        );
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/CdtrAgt", b"DEUTDEFF");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/EndToEndId", b"E2E1");
    }

    fn populate_pacs009_minimal() {
        msg_set("BizMsgIdr", b"BMSG1");
        msg_set("MsgDefIdr", b"pacs.009.001.10");
        msg_set("CreDtTm", b"2024-01-01T12:00:00Z");
        msg_set("IntrBkSttlmAmt", b"5000");
        msg_set("IntrBkSttlmCcy", b"USD");
        msg_set("IntrBkSttlmDt", b"2024-01-03");
        msg_set("InstgAgt", b"DEUTDEFF");
        msg_set("InstdAgt", b"MARKDEFF");
        msg_set("DbtrAcct", b"GB82WEST12345698765432");
        msg_set("CdtrAcct", b"GB33BUKB20201555555555");
    }

    fn populate_head001_minimal() {
        msg_set("AppHdr/BizMsgIdr", b"HDR-123");
        msg_set("AppHdr/MsgDefIdr", b"pacs.008.001.08");
        msg_set("AppHdr/CreDt", b"2025-01-01T12:00:00Z");
        msg_set("AppHdr/Fr/FIId/FinInstnId/BICFI", b"DEUTDEFF");
        msg_set("AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId", b"123456");
    }

    fn populate_pacs004_minimal() {
        msg_set("MsgId", b"RTRN1");
        msg_set("CreDtTm", b"2024-01-05T10:00:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInf");
        msg_set("TxInf[0]/OrgnlInstrId", b"INST1");
        msg_set("TxInf[0]/RtrdInstdAmt", b"100.00");
        msg_set("TxInf[0]/RtrdInstdAmtCcy", b"USD");
    }

    fn populate_pacs028_minimal() {
        msg_set("MsgId", b"REQ1");
        msg_set("CreDtTm", b"2024-01-06T09:30:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
    }

    fn populate_pacs029_minimal() {
        msg_set("MsgId", b"STAT1");
        msg_set("CreDtTm", b"2024-01-06T09:45:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInfAndSts");
        msg_set("TxInfAndSts[0]/TxSts", b"ACSP");
    }

    fn populate_pain002_minimal() {
        msg_set("GrpHdr/MsgId", b"PAINSTAT1");
        msg_set("GrpHdr/CreDtTm", b"2024-01-07T08:00:00Z");
        msg_set("OrgnlGrpInfAndSts/OrgnlMsgId", b"PAIN1");
        msg_set("OrgnlGrpInfAndSts/GrpSts", b"ACSP");
    }

    fn populate_pacs007_minimal() {
        msg_set("MsgId", b"CXL1");
        msg_set("CreDtTm", b"2024-01-02T09:30:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInf");
        msg_set("TxInf[0]/OrgnlInstrId", b"INST1");
        msg_set("TxInf[0]/OrgnlEndToEndId", b"E2E1");
        msg_set("TxInf[0]/OrgnlTxId", b"TX1");
        msg_set("TxInf[0]/CxlRsnInf/Rsn/Cd", b"RR01");
    }

    fn populate_camt056_minimal() {
        msg_set("Assgnmt/Id", b"CXL2");
        msg_set("Assgnmt/CreDtTm", b"2024-01-03T11:15:00Z");
        msg_set("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId", b"ORIG2");
        msg_set("Undrlyg/TxInf/OrgnlInstrId", b"INST2");
        msg_set("Undrlyg/TxInf/OrgnlEndToEndId", b"E2E2");
        msg_set("Undrlyg/TxInf/OrgnlTxId", b"TX2");
        msg_set("Undrlyg/TxInf/CxlRsnInf/Rsn/Cd", b"RC01");
    }

    fn populate_sese023_minimal() {
        msg_set("TxId", b"DVP-SETTLEMENT-1");
        msg_set("SttlmDt", b"2024-01-02");
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
        msg_set("SctiesLeg/FinInstrmId", b"US0378331005");
        msg_set("SctiesLeg/Qty", b"1000");
        msg_set("CashLeg/Amt", b"1050000");
        msg_set("CashLeg/Ccy", b"USD");
        msg_set("DlvrgSttlmPties/Pty/Bic", b"DEUTDEFF");
        msg_set("DlvrgSttlmPties/Acct", b"DLVRY-ACC");
        msg_set("RcvgSttlmPties/Pty/Bic", b"MARKDEFF");
        msg_set("RcvgSttlmPties/Acct", b"RCVG-ACC");
        msg_set("Plan/ExecutionOrder", b"DELIVERY_THEN_PAYMENT");
        msg_set("Plan/Atomicity", b"ALL_OR_NOTHING");
    }

    fn populate_sese025_minimal() {
        msg_set("TxId", b"DVP-SETTLEMENT-1");
        msg_set("SttlmDt", b"2024-01-02");
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
        msg_set("ConfSts", b"ACCP");
        msg_set("SttlmQty", b"1000");
        msg_set("SttlmAmt", b"1050000");
        msg_set("SttlmCcy", b"USD");
        msg_set("Plan/ExecutionOrder", b"DELIVERY_THEN_PAYMENT");
        msg_set("Plan/Atomicity", b"ALL_OR_NOTHING");
        msg_set("RsnCd", b"SETTLED");
    }

    fn populate_colr007_minimal() {
        msg_set("TxId", b"COLLATERAL-EXCHANGE-1");
        msg_set("OblgtnId", b"REPO-DAILY-1");
        msg_set("Substitution/OriginalAmt", b"1000000");
        msg_set("Substitution/OriginalCcy", b"USD");
        msg_set("Substitution/SubstituteAmt", b"1005000");
        msg_set("Substitution/SubstituteCcy", b"USD");
        msg_set("Substitution/EffectiveDt", b"2024-01-05");
        msg_set("Substitution/Type", b"FULL");
        msg_set("Substitution/ReasonCd", b"HAIRCUT");
    }

    const SESE023_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/sese023_fixture.xml");
    const SESE025_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/sese025_fixture.xml");
    const COLR007_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/colr007_fixture.xml");

    fn expected_sese023_schema() -> Sese023 {
        Sese023 {
            tx_id: "DVP-FIXTURE-1".to_owned(),
            settlement_date: "2024-02-02".to_owned(),
            movement_type: "DELI".to_owned(),
            payment_type: "APMT".to_owned(),
            fin_instr_id: "US0378331005".to_owned(),
            quantity: "500".to_owned(),
            cash_amount: "1050000".to_owned(),
            cash_currency: "USD".to_owned(),
            delivering_party_bic: "DEUTDEFF".to_owned(),
            delivering_account: "DLVRY-ACC".to_owned(),
            receiving_party_bic: "MARKDEFF".to_owned(),
            receiving_account: "RCVG-ACC".to_owned(),
            execution_order: "DELIVERY_THEN_PAYMENT".to_owned(),
            atomicity: "ALL_OR_NOTHING".to_owned(),
            settlement_condition: Some("NOMC".to_owned()),
            partial_settlement_indicator: Some("NPAR".to_owned()),
            hold_indicator: Some(true),
            venue_mic: Some("XNAS".to_owned()),
            linkages: vec![
                Linkage {
                    relation: "WITH".to_owned(),
                    reference: "SUBST-PAIR-B".to_owned(),
                },
                Linkage {
                    relation: "BEFO".to_owned(),
                    reference: "PACS009-CLS".to_owned(),
                },
            ],
            securities_metadata: Some(r#"{"note":"delivery"}"#.to_owned()),
            cash_metadata: Some(r#"{"note":"cash"}"#.to_owned()),
        }
    }

    fn expected_sese025_schema() -> Sese025 {
        Sese025 {
            tx_id: "PVP-FIXTURE-1".to_owned(),
            settlement_date: "2024-03-01".to_owned(),
            movement_type: "RECE".to_owned(),
            payment_type: "APMT".to_owned(),
            confirmation_status: "ACCP".to_owned(),
            settlement_quantity: "250000".to_owned(),
            settlement_amount: "100000".to_owned(),
            settlement_currency: "USD".to_owned(),
            security_id: Some("US0378331005".to_owned()),
            security_quantity: Some("500".to_owned()),
            delivering_party_bic: Some("DEUTDEFF".to_owned()),
            delivering_account: Some("DLVRY-ACC".to_owned()),
            receiving_party_bic: Some("MARKDEFF".to_owned()),
            receiving_account: Some("RCVG-ACC".to_owned()),
            execution_order: "PAYMENT_THEN_DELIVERY".to_owned(),
            atomicity: "COMMIT_SECOND_LEG".to_owned(),
            hold_indicator: Some(false),
            partial_settlement_indicator: Some("NPAR".to_owned()),
            settlement_condition: Some("NOMC".to_owned()),
            venue_mic: None,
            reason_code: Some("MATCHED".to_owned()),
            additional_info: Some(r#"{"counter_ccy":"EUR"}"#.to_owned()),
        }
    }

    fn expected_colr007_schema() -> Colr007 {
        Colr007 {
            tx_id: "COLR-FIXTURE-1".to_owned(),
            obligation_id: "REPO-123".to_owned(),
            original_amount: "1000000".to_owned(),
            original_currency: "USD".to_owned(),
            substitute_amount: "1002000".to_owned(),
            substitute_currency: "USD".to_owned(),
            haircut: Some("50".to_owned()),
            effective_date: "2024-04-05".to_owned(),
            substitution_type: "PARTIAL".to_owned(),
            original_fin_instr_id: Some("US0378331005".to_owned()),
            substitute_fin_instr_id: Some("US5949181045".to_owned()),
            reason_code: Some("MARGIN".to_owned()),
        }
    }

    const GENERATED_MD: &str = include_str!(r"../../../generatediso20022.md");

    const SAMPLE_PACS008_XML: &str = r#"
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-SAMPLE-008</BizMsgIdr>
      <MsgDefIdr>pacs.008.001.08</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-11T09:34:09Z</CreDt>
    </AppHdr>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
      <FIToFICstmrCdtTrf>
        <GrpHdr>
          <MsgId>ISO-008-GRP</MsgId>
          <CreDtTm>2025-11-11T09:34:09</CreDtTm>
          <BtchBookg>false</BtchBookg>
          <NbOfTxs>1</NbOfTxs>
          <SttlmInf><SttlmMtd>CLRG</SttlmMtd></SttlmInf>
        </GrpHdr>
        <CdtTrfTxInf>
          <PmtId>
            <InstrId>SAMPLE-INSTR-008</InstrId>
            <EndToEndId>SAMPLE-E2E-008</EndToEndId>
            <TxId>SAMPLE-TX-008</TxId>
          </PmtId>
          <PmtTpInf>
            <ClrChanl>RTNS</ClrChanl>
            <SvcLvl><Prtry>0100</Prtry></SvcLvl>
            <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
            <CtgyPurp><Prtry>005</Prtry></CtgyPurp>
          </PmtTpInf>
          <IntrBkSttlmAmt Ccy="USD">1400.00</IntrBkSttlmAmt>
          <IntrBkSttlmDt>2025-11-11</IntrBkSttlmDt>
          <ChrgBr>SLEV</ChrgBr>
          <Dbtr><Nm>Example Debtor</Nm></Dbtr>
          <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
          <DbtrAgt><FinInstnId><BICFI>ALPHGB2L</BICFI></FinInstnId></DbtrAgt>
          <CdtrAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGGB2L</MmbId></ClrSysMmbId></FinInstnId></CdtrAgt>
          <Cdtr><Nm>Example Creditor</Nm></Cdtr>
          <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
          <InstrForNxtAgt><InstrInf>/INFO/Example note</InstrInf></InstrForNxtAgt>
        </CdtTrfTxInf>
      </FIToFICstmrCdtTrf>
    </Document>
  </Body>
</DataEnvelope>
"#;

    const SAMPLE_PACS004_XML: &str = r#"
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.004.001.09">
      <PmtRtr>
        <GrpHdr>
          <MsgId>ISO-PACS004-MSG</MsgId>
          <CreDtTm>2025-11-06T12:42:25.758Z</CreDtTm>
          <BtchBookg>false</BtchBookg>
          <NbOfTxs>1</NbOfTxs>
          <SttlmInf><SttlmMtd>CLRG</SttlmMtd></SttlmInf>
          <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
        </GrpHdr>
        <OrgnlGrpInf>
          <OrgnlMsgId>ORIGINAL-008</OrgnlMsgId>
          <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
          <OrgnlCreDtTm>2025-11-06T12:42:07.770Z</OrgnlCreDtTm>
        </OrgnlGrpInf>
        <TxInf>
          <RtrId>RETURN-5310</RtrId>
          <OrgnlInstrId>ORIGINAL-INSTR-008</OrgnlInstrId>
          <OrgnlEndToEndId>ORIGINAL-E2E-008</OrgnlEndToEndId>
          <OrgnlTxId>ORIGINAL-TX-008</OrgnlTxId>
          <OrgnlIntrBkSttlmDt>2025-11-06</OrgnlIntrBkSttlmDt>
          <RtrdIntrBkSttlmAmt Ccy="USD">10.00</RtrdIntrBkSttlmAmt>
          <IntrBkSttlmDt>2025-11-06</IntrBkSttlmDt>
          <ChrgBr>SLEV</ChrgBr>
          <InstgAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></InstgAgt>
          <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
          <RtrRsnInf><Rsn><Prtry>TechnicalProblem</Prtry></Rsn></RtrRsnInf>
          <OrgnlTxRef>
            <IntrBkSttlmAmt Ccy="USD">10.00</IntrBkSttlmAmt>
            <PmtTpInf>
              <ClrChanl>RTNS</ClrChanl>
              <SvcLvl><Prtry>0100</Prtry></SvcLvl>
              <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
              <CtgyPurp><Prtry>001</Prtry></CtgyPurp>
            </PmtTpInf>
            <Dbtr><Pty><Nm>Debtor One</Nm></Pty></Dbtr>
            <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
            <DbtrAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></DbtrAgt>
            <CdtrAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></CdtrAgt>
            <Cdtr><Pty><Nm>Creditor One</Nm></Pty></Cdtr>
            <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
          </OrgnlTxRef>
        </TxInf>
      </PmtRtr>
    </Document>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-PACS004-MSG</BizMsgIdr>
      <MsgDefIdr>pacs.004.001.09</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-06T07:42:25.771Z</CreDt>
      <Sgntr>
        <SignedInfo><SignatureMethod>sample</SignatureMethod></SignedInfo>
        <SignatureValue>ignored</SignatureValue>
      </Sgntr>
    </AppHdr>
  </Body>
</DataEnvelope>
"#;

    const SAMPLE_PACS009_XML: &str = r#"
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.10">
  <FICdtTrf>
    <GrpHdr>
      <MsgId>PACS009-GRP</MsgId>
      <CreDtTm>2024-01-01T12:00:00Z</CreDtTm>
      <BizMsgIdr>PACS009-BIZ</BizMsgIdr>
      <MsgDefIdr>pacs.009.001.10</MsgDefIdr>
    </GrpHdr>
    <CdtTrfTxInf>
      <IntrBkSttlmAmt Ccy="USD">2500</IntrBkSttlmAmt>
      <IntrBkSttlmDt>2024-01-03</IntrBkSttlmDt>
      <InstgAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></InstgAgt>
      <InstdAgt><FinInstnId><BICFI>MARKDEFF</BICFI></FinInstnId></InstdAgt>
      <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
      <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
      <Purp><Cd>SECU</Cd></Purp>
    </CdtTrfTxInf>
  </FICdtTrf>
</Document>
"#;

    const SAMPLE_PACS009_ENVELOPE_XML: &str = r#"
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <BizMsgIdr>BAH-PACS009-1</BizMsgIdr>
      <MsgDefIdr>pacs.009.001.10</MsgDefIdr>
      <CreDt>2025-11-12T09:34:09Z</CreDt>
    </AppHdr>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.10">
      <FICdtTrf>
        <GrpHdr>
          <InstgAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></InstgAgt>
          <InstdAgt><FinInstnId><BICFI>MARKDEFF</BICFI></FinInstnId></InstdAgt>
        </GrpHdr>
        <CdtTrfTxInf>
          <IntrBkSttlmAmt Ccy="USD">2500</IntrBkSttlmAmt>
          <IntrBkSttlmDt>2024-01-03</IntrBkSttlmDt>
          <DbtrAcct><Id><Othr><Id>GB82WEST12345698765432</Id></Othr></Id></DbtrAcct>
          <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
          <Purp><Cd>SECU</Cd></Purp>
        </CdtTrfTxInf>
      </FICdtTrf>
    </Document>
  </Body>
</DataEnvelope>
"#;

    const SAMPLE_PACS002_STATUS_XML: &str = r#"
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">
  <FIToFIPmtStsRpt>
    <GrpHdr>
      <MsgId>ISO-PACS002-STATUS</MsgId>
      <CreDtTm>2025-11-11T09:35:15.671Z</CreDtTm>
    </GrpHdr>
    <OrgnlGrpInfAndSts>
      <OrgnlMsgId>ISO-SAMPLE-008</OrgnlMsgId>
      <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
      <OrgnlCreDtTm>2025-11-11T09:34:09</OrgnlCreDtTm>
      <OrgnlNbOfTxs>1</OrgnlNbOfTxs>
      <GrpSts>ACSP</GrpSts>
    </OrgnlGrpInfAndSts>
    <TxInfAndSts>
      <StsId>ISO-PACS002-STATUS</StsId>
      <OrgnlInstrId>SAMPLE-INSTR-008</OrgnlInstrId>
      <OrgnlEndToEndId>SAMPLE-E2E-008</OrgnlEndToEndId>
      <OrgnlTxId>SAMPLE-TX-008</OrgnlTxId>
      <TxSts>ACSP</TxSts>
      <AcctSvcrRef>6762</AcctSvcrRef>
      <InstgAgt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></InstgAgt>
      <InstdAgt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></InstdAgt>
      <OrgnlTxRef>
        <IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2025-11-11</IntrBkSttlmDt>
        <PmtTpInf>
          <ClrChanl>RTNS</ClrChanl>
          <SvcLvl><Prtry>0100</Prtry></SvcLvl>
          <LclInstrm><Prtry>CTAA</Prtry></LclInstrm>
          <CtgyPurp><Prtry>005</Prtry></CtgyPurp>
        </PmtTpInf>
        <Purp><Prtry>005</Prtry></Purp>
      </OrgnlTxRef>
    </TxInfAndSts>
  </FIToFIPmtStsRpt>
</Document>
"#;

    const SAMPLE_PACS002_AUTH_XML: &str = r#"
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">
  <FIToFIPmtStsRpt>
    <GrpHdr>
      <MsgId>ISO-PACS002-AUTH</MsgId>
      <CreDtTm>2025-11-11T09:35:15.671Z</CreDtTm>
    </GrpHdr>
    <OrgnlGrpInfAndSts>
      <OrgnlMsgId>ISO-SAMPLE-008</OrgnlMsgId>
      <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
      <OrgnlCreDtTm>2025-11-11T09:34:09</OrgnlCreDtTm>
      <OrgnlNbOfTxs>1</OrgnlNbOfTxs>
    </OrgnlGrpInfAndSts>
    <TxInfAndSts>
      <StsId>ISO-PACS002-AUTH</StsId>
      <OrgnlInstrId>SAMPLE-INSTR-008</OrgnlInstrId>
      <OrgnlEndToEndId>SAMPLE-E2E-008</OrgnlEndToEndId>
      <OrgnlTxId>SAMPLE-TX-008</OrgnlTxId>
      <AcctSvcrRef>7001</AcctSvcrRef>
      <OrgnlTxRef>
        <IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2025-11-11</IntrBkSttlmDt>
      </OrgnlTxRef>
    </TxInfAndSts>
  </FIToFIPmtStsRpt>
</Document>
"#;

    const SAMPLE_CAMT052_XML: &str = r#"
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.052.001.08">
  <BkToCstmrAcctRpt>
    <GrpHdr>
      <MsgId>RPT-460052</MsgId>
      <CreDtTm>2025-11-10T15:17:49.578Z</CreDtTm>
    </GrpHdr>
    <Rpt>
      <Id>RPT-460052</Id>
      <ElctrncSeqNb>531</ElctrncSeqNb>
      <CreDtTm>2025-11-10T15:17:49.580Z</CreDtTm>
      <Acct>
        <Id><Othr><Id>ALTACCOUNT</Id></Othr></Id>
        <Ccy>USD</Ccy>
        <Ownr>
          <Id><OrgId><Othr><Id>OWNER001</Id><SchmeNm><Prtry>PCOD</Prtry></SchmeNm></Othr></OrgId></Id>
        </Ownr>
      </Acct>
      <Bal>
        <Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp>
        <Amt Ccy="USD">1000.00</Amt>
        <CdtDbtInd>DBIT</CdtDbtInd>
        <Dt><Dt>2025-11-06</Dt></Dt>
      </Bal>
      <TxsSummry>
        <TtlNtries>
          <NbOfNtries>41</NbOfNtries>
          <Sum>1011323.14</Sum>
          <TtlNetNtry><Amt>991491.14</Amt><CdtDbtInd>DBIT</CdtDbtInd></TtlNetNtry>
        </TtlNtries>
        <TtlCdtNtries><NbOfNtries>21</NbOfNtries><Sum>9916.00</Sum></TtlCdtNtries>
        <TtlDbtNtries><NbOfNtries>20</NbOfNtries><Sum>1001407.14</Sum></TtlDbtNtries>
      </TxsSummry>
      <AddtlRptInf>/SESSIONID/6761</AddtlRptInf>
    </Rpt>
  </BkToCstmrAcctRpt>
</Document>
"#;

    const SAMPLE_CAMT056_XML: &str = r#"
<DataEnvelope xmlns="urn:sample:iso">
  <Body>
    <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
      <Fr><FIId><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></Fr>
      <To><FIId><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></FIId></To>
      <BizMsgIdr>ISO-CAMT056-ASSIGN</BizMsgIdr>
      <MsgDefIdr>camt.056.001.08</MsgDefIdr>
      <BizSvc>IPS</BizSvc>
      <CreDt>2025-11-07T16:27:33Z</CreDt>
      <Sgntr><SignedInfo><SignatureMethod>sample</SignatureMethod></SignedInfo><SignatureValue>ignored</SignatureValue></Sgntr>
    </AppHdr>
    <Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.056.001.08">
      <FIToFIPmtCxlReq>
        <Assgnmt>
          <Id>ISO-CAMT056-ASSIGN</Id>
          <Assgnr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgnr>
          <Assgne><Agt><FinInstnId><ClrSysMmbId><MmbId>PAYFASTX</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgne>
          <CreDtTm>2025-11-07T16:27:33Z</CreDtTm>
        </Assgnmt>
        <Case>
          <Id>ISO-CAMT056-ASSIGN</Id>
          <Cretr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Cretr>
        </Case>
        <Undrlyg>
          <TxInf>
            <OrgnlGrpInf>
              <OrgnlMsgId>ISO-SAMPLE-008</OrgnlMsgId>
              <OrgnlMsgNmId>pacs.008.001.08</OrgnlMsgNmId>
              <OrgnlCreDtTm>2025-11-07T16:13:08.557</OrgnlCreDtTm>
            </OrgnlGrpInf>
            <OrgnlIntrBkSttlmDt>2025-11-07</OrgnlIntrBkSttlmDt>
            <Assgnr><Agt><FinInstnId><ClrSysMmbId><MmbId>ALPHBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgnr>
            <Assgne><Agt><FinInstnId><ClrSysMmbId><MmbId>OMEGBANK</MmbId></ClrSysMmbId></FinInstnId></Agt></Assgne>
          </TxInf>
        </Undrlyg>
      </FIToFIPmtCxlReq>
    </Document>
  </Body>
</DataEnvelope>
"#;

    fn assert_validated(message_type: &str, xml: &str) {
        reset();
        msg_parse(message_type, xml.as_bytes())
            .unwrap_or_else(|err| panic!("parse {message_type} sample: {err:?}"));
        let valid = msg_validate();
        let failure = take_validation_failure();
        assert!(valid, "validation failed: {failure:?}");
    }

    fn generated_sample(message_marker: &str) -> String {
        let needle = format!("<!-- {message_marker} -->");
        let all = GENERATED_MD;
        let marker_pos = all
            .find(&needle)
            .unwrap_or_else(|| panic!("marker not found: {message_marker}"));
        let before = &all[..marker_pos];
        let start_fence = before
            .rfind("```xml")
            .unwrap_or_else(|| panic!("xml fence missing for {message_marker}"));
        let after_fence = &all[start_fence + "```xml".len()..];
        let end = after_fence
            .find("```")
            .unwrap_or_else(|| panic!("closing fence missing for {message_marker}"));
        after_fence[..end].trim().to_owned()
    }

    #[test]
    fn msg_create_and_validate() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs008_requires_creditor_agent() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_remove("CdtrAgt");
        assert!(!msg_validate());
    }

    #[test]
    fn pacs008_requires_debtor_account() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_remove("DbtrAcct");
        assert!(!msg_validate());
    }

    #[test]
    fn msg_validate_rejects_non_numeric_amount() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmAmt", b"not-a-number");
        assert!(!msg_validate());
    }

    #[test]
    fn msg_validate_rejects_invalid_iban() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct", b"GB82WEST12345698765433");
        assert!(!msg_validate());
    }

    #[test]
    fn take_validation_error_reports_identifier_failure() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmCcy", b"ZZZ");
        assert!(!msg_validate());
        let err = take_validation_error().expect("validation error captured");
        match err {
            MsgError::InvalidIdentifier { field, kind } => {
                assert_eq!(field, "IntrBkSttlmCcy");
                assert_eq!(kind, IdentifierKind::Currency);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(take_validation_error().is_none(), "error should be drained");
    }

    #[test]
    fn msg_validate_accepts_valid_iban() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs008_validates_proxy_identifiers() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct/Prxy/Id", b"1233214568521");
        msg_set("DbtrAcct/Prxy/Tp/Cd", b"2100");
        msg_set("CdtrAcct/Prxy/Id", b"4210118604441");
        assert!(
            msg_validate(),
            "proxy identifiers should validate when populated alongside IBANs"
        );
        assert_eq!(
            msg_get("DbtrAcct/Prxy/Id").as_deref(),
            Some(&b"1233214568521"[..])
        );
    }

    #[test]
    fn pacs008_rejects_empty_proxy_identifier() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct/Prxy/Id", b"");
        assert!(!msg_validate(), "empty proxy ids must fail validation");
        let failure = take_validation_failure().expect("validation failure captured");
        match failure {
            ValidationFailure::InvalidField { field, reason } => {
                assert_eq!(field, "DbtrAcct/Prxy/Id");
                assert!(matches!(reason, InvalidReason::Empty));
            }
            other => panic!("unexpected validation failure: {other:?}"),
        }
    }

    #[test]
    fn head001_requires_core_fields() {
        reset();
        msg_create("head.001.001.03");
        populate_head001_minimal();
        assert!(msg_validate(), "baseline header should validate");

        msg_remove("AppHdr/CreDt");
        assert!(!msg_validate(), "missing CreDt must fail validation");
        let err = take_validation_error().expect("validation error captured");
        match err {
            MsgError::MissingField(field) => assert_eq!(field, "AppHdr/CreDt"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn msg_validate_rejects_invalid_bic() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAgt", b"deutdeff");
        assert!(!msg_validate());
    }

    #[test]
    fn msg_validate_accepts_valid_bic() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn msg_validate_rejects_invalid_isin() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"INVALID123456");
        assert!(!msg_validate());
    }

    #[test]
    fn msg_validate_accepts_cusip_instrument() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"037833100");
        assert!(msg_validate());
    }

    #[test]
    fn parse_message_reports_invalid_instrument() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"BADSIGN");
        let xml = msg_serialize("XML").expect("serialize");
        let err = parse_message("sese.023", &xml).expect_err("validation should fail");
        assert!(matches!(
            err,
            MsgError::InvalidInstrument { field } if field == "SctiesLeg/FinInstrmId"
        ));
    }

    #[test]
    fn validate_identifier_helpers() {
        assert!(validate_identifier(IdentifierKind::Isin, "US0378331005"));
        assert!(!validate_identifier(IdentifierKind::Isin, "US0378331004"));

        assert!(validate_identifier(IdentifierKind::Cusip, "037833100"));
        assert!(!validate_identifier(IdentifierKind::Cusip, "03783310X"));

        assert!(validate_identifier(
            IdentifierKind::Lei,
            "5493001KJTIIGC8Y1R12"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Lei,
            "5493001KJTIIGC8Y1R13"
        ));

        assert!(validate_identifier(IdentifierKind::Bic, "DEUTDEFF"));
        assert!(!validate_identifier(IdentifierKind::Bic, "deutDEFF"));

        assert!(validate_identifier(IdentifierKind::Mic, "XNAS"));
        assert!(!validate_identifier(IdentifierKind::Mic, "1NAS"));

        assert!(validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST12345698765432"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST12345698765433"
        ));
        assert!(validate_identifier(
            IdentifierKind::Iban,
            "de89370400440532013000"
        ));
        assert!(validate_identifier(IdentifierKind::Iban, "NO9386011117947"));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST1234569876543"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "ZZ82WEST12345698765432"
        ));

        assert!(validate_identifier(IdentifierKind::Currency, "USD"));
        assert!(!validate_identifier(IdentifierKind::Currency, "ZZZ"));
    }

    #[test]
    fn validate_instrument_identifier_helper() {
        assert!(validate_instrument_identifier("US0378331005"));
        assert!(validate_instrument_identifier("037833100"));
        assert!(!validate_instrument_identifier("INVALID"));
    }

    #[test]
    fn camt053_requires_account_id() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_remove("Stmt/Acct/Id");
        assert!(!msg_validate());
    }

    #[test]
    fn camt053_rejects_invalid_iban() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_set("Stmt/Acct/Id", b"GB82WEST12345698765433");
        assert!(!msg_validate());
    }

    #[test]
    fn camt053_rejects_non_numeric_balance() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_set("Stmt/Bal[0]/Amt", b"not-number");
        assert!(!msg_validate());
    }

    #[test]
    fn camt053_accepts_valid_message() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn camt052_accepts_valid_message() {
        reset();
        msg_create("camt.052");
        populate_camt052_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pain001_accepts_valid_message() {
        reset();
        msg_create("pain.001");
        populate_pain001_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pain001_rejects_missing_credit_transfer() {
        reset();
        msg_create("pain.001");
        populate_pain001_minimal();
        msg_remove("PmtInf[0]/CdtTrfTxInf[0]/Amt");
        assert!(!msg_validate());
    }

    #[test]
    fn pacs009_accepts_valid_message() {
        reset();
        msg_create("pacs.009");
        populate_pacs009_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs009_rejects_missing_agents() {
        reset();
        msg_create("pacs.009");
        populate_pacs009_minimal();
        msg_remove("InstgAgt");
        assert!(!msg_validate());
    }

    #[test]
    fn pacs004_accepts_valid_message() {
        reset();
        msg_create("pacs.004");
        populate_pacs004_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs028_accepts_valid_message() {
        reset();
        msg_create("pacs.028");
        populate_pacs028_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs029_accepts_valid_message() {
        reset();
        msg_create("pacs.029");
        populate_pacs029_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pain002_accepts_valid_message() {
        reset();
        msg_create("pain.002");
        populate_pain002_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs007_accepts_valid_message() {
        reset();
        msg_create("pacs.007");
        populate_pacs007_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn camt056_accepts_valid_message() {
        reset();
        msg_create("camt.056");
        populate_camt056_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn pacs002_accepts_valid_message() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        msg_set("TxSts", b"ACTC");
        assert!(msg_validate());
    }

    #[test]
    fn pacs002_accepts_missing_tx_status() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        assert!(msg_validate());
    }

    #[test]
    fn pacs002_rejects_unknown_status() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        msg_set("TxSts", b"XXXX");
        assert!(!msg_validate());
    }

    #[test]
    fn msg_clone_copies_fields() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_clone();
        assert_eq!(msg_get("field").as_deref(), Some(&b"value"[..]));
    }

    #[test]
    fn msg_set_and_get() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        assert_eq!(msg_get("field").as_deref(), Some(&b"value"[..]));
    }

    #[test]
    fn msg_clear_removes_all() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_clear();
        assert!(msg_get("field").is_none());
    }

    #[test]
    fn msg_add_creates_incrementing_keys() {
        reset();
        msg_create("pacs.008");
        msg_add("Entry");
        msg_add("Entry");
        assert!(msg_get("Entry[0]").is_some());
        assert!(msg_get("Entry[1]").is_some());
        assert!(msg_get("Entry[2]").is_none());
    }

    #[test]
    fn msg_remove_deletes_field() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_remove("field");
        assert!(msg_get("field").is_none());
    }

    #[test]
    fn msg_clone_pushes_copy_on_stack() {
        reset();
        msg_create("pacs.008");
        msg_set("MsgId", b"1");
        msg_clone();
        msg_set("MsgId", b"2");
        super::MESSAGE_STACK.with(|s| {
            let stack = s.borrow();
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[0].fields.get("MsgId").unwrap(), b"1");
            assert_eq!(stack[1].fields.get("MsgId").unwrap(), b"2");
        });
    }

    #[test]
    fn msg_parse_and_serialize_roundtrip() {
        reset();
        msg_parse("pacs.008", b"field=value\nfoo=bar").unwrap();
        assert_eq!(msg_get("foo").as_deref(), Some(&b"bar"[..]));
        assert_eq!(
            msg_serialize("KV").unwrap(),
            b"field=value\nfoo=bar".to_vec()
        );
    }

    #[test]
    fn parse_message_materialises_fields() {
        reset();
        let parsed = parse_message(
            "pacs.008",
            b"MsgId=abc\nIntrBkSttlmCcy=USD\nIntrBkSttlmAmt=10\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB33BUKB20201555555555\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("message parses");
        assert_eq!(parsed.message_type(), "pacs.008");
        assert_eq!(parsed.field_text("MsgId"), Some("abc"));
        assert_eq!(parsed.field_text("IntrBkSttlmCcy"), Some("USD"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }

    #[test]
    fn parse_message_validation_failure() {
        reset();
        let err = parse_message("pacs.008", b"MsgId=abc").unwrap_err();
        assert!(matches!(err, MsgError::MissingField("IntrBkSttlmCcy")));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }

    #[test]
    fn parse_head001_envelope_preserves_apphdr_fields() {
        reset();
        let xml = r#"
<DataPDU>
  <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.03">
    <Fr>
      <FIId>
        <FinInstnId>
          <BICFI>DEUTDEFF</BICFI>
        </FinInstnId>
      </FIId>
    </Fr>
    <To>
      <FIId>
        <FinInstnId>
          <ClrSysMmbId>
            <MmbId>654321</MmbId>
          </ClrSysMmbId>
        </FinInstnId>
      </FIId>
    </To>
    <BizMsgIdr>HDR-123</BizMsgIdr>
    <MsgDefIdr>pacs.008.001.08</MsgDefIdr>
    <BizSvc>swift.cbprplus.02</BizSvc>
    <CreDt>2025-01-01T12:00:00Z</CreDt>
  </AppHdr>
  <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
    <FIToFICstmrCdtTrf>
      <CdtTrfTxInf>
        <IntrBkSttlmAmt Ccy="USD">100</IntrBkSttlmAmt>
      </CdtTrfTxInf>
    </FIToFICstmrCdtTrf>
  </Document>
</DataPDU>
        "#;
        let parsed =
            parse_message("head.001.001.03", xml.as_bytes()).expect("header envelope parses");
        assert_eq!(parsed.message_type(), "head.001.001.03");
        assert_eq!(parsed.field_text("AppHdr/BizMsgIdr"), Some("HDR-123"));
        assert_eq!(
            parsed.field_text("AppHdr/MsgDefIdr"),
            Some("pacs.008.001.08")
        );
        assert_eq!(
            parsed.field_text("AppHdr/Fr/FIId/FinInstnId/BICFI"),
            Some("DEUTDEFF")
        );
        assert_eq!(
            parsed.field_text("AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId"),
            Some("654321")
        );
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }

    #[test]
    fn pacs008_accepts_proxy_accounts_without_iban() {
        reset();
        let xml = r#"
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
  <FIToFICstmrCdtTrf>
    <GrpHdr>
      <MsgId>ISO-PROXY</MsgId>
    </GrpHdr>
    <CdtTrfTxInf>
      <IntrBkSttlmAmt Ccy="USD">42</IntrBkSttlmAmt>
      <IntrBkSttlmDt>2025-02-02</IntrBkSttlmDt>
      <DbtrAcct>
        <Prxy>
          <Id>proxy-debtor</Id>
        </Prxy>
      </DbtrAcct>
      <CdtrAcct>
        <Prxy>
          <Id>proxy-creditor</Id>
        </Prxy>
      </CdtrAcct>
      <DbtrAgt>
        <FinInstnId>
          <ClrSysMmbId>
            <MmbId>DBTRCODE</MmbId>
          </ClrSysMmbId>
        </FinInstnId>
      </DbtrAgt>
      <CdtrAgt>
        <FinInstnId>
          <ClrSysMmbId>
            <MmbId>CDTRCODE</MmbId>
          </ClrSysMmbId>
        </FinInstnId>
      </CdtrAgt>
    </CdtTrfTxInf>
  </FIToFICstmrCdtTrf>
</Document>
"#;
        let parsed =
            parse_message("pacs.008.001.08", xml.as_bytes()).expect("proxy-only pacs.008 parses");
        assert_eq!(parsed.field_text("DbtrAcct/Prxy/Id"), Some("proxy-debtor"));
        assert_eq!(
            parsed.field_text("CdtrAcct/Prxy/Id"),
            Some("proxy-creditor")
        );
        assert!(parsed.field_text("DbtrAcct").is_none());
        assert!(parsed.field_text("CdtrAcct").is_none());
    }

    #[test]
    fn msg_validate_none() {
        reset();
        assert!(!msg_validate());
    }

    #[test]
    fn versioned_pacs008_supported() {
        reset();
        msg_create("pacs.008.001.10");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_pacs002_supported() {
        reset();
        msg_create("pacs.002.001.12");
        msg_set("OrgnlMsgId", b"ABC");
        msg_set("TxSts", b"ACSP");
        assert!(msg_validate());
    }

    #[test]
    fn versioned_camt053_supported() {
        reset();
        msg_create("camt.053.001.08");
        populate_camt053_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_camt054_supported() {
        reset();
        msg_create("camt.054.001.08");
        msg_set("Ntfctn/Id", b"NTF1");
        msg_set("Ntfctn/Acct/Id", b"GB82WEST12345698765432");
        msg_add("Ntfctn/Ntry");
        msg_set("Ntfctn/Ntry[0]/Amt", b"15.00");
        msg_set("Ntfctn/Ntry[0]/Ccy", b"USD");
        msg_set("Ntfctn/Ntry[0]/CdtDbtInd", b"CRDT");
        assert!(msg_validate());
    }

    #[test]
    fn versioned_camt052_supported() {
        reset();
        msg_create("camt.052.001.09");
        populate_camt052_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn parse_sample_pacs008() {
        assert_validated("pacs.008.001.08", SAMPLE_PACS008_XML);
        let msg_id = msg_get("MsgId");
        assert_eq!(msg_id.as_deref(), Some(b"ISO-008-GRP".as_ref()));
        assert_eq!(msg_get("IntrBkSttlmCcy").as_deref(), Some(b"USD".as_ref()));
        assert_eq!(
            msg_get("IntrBkSttlmAmt").as_deref(),
            Some(b"1400.00".as_ref())
        );
    }

    #[test]
    fn parse_sample_pacs009() {
        assert_validated("pacs.009.001.10", SAMPLE_PACS009_XML);
        assert_eq!(
            msg_get("BizMsgIdr").as_deref(),
            Some(b"PACS009-BIZ".as_ref())
        );
        assert_eq!(
            msg_get("MsgDefIdr").as_deref(),
            Some(b"pacs.009.001.10".as_ref())
        );
        assert_eq!(msg_get("IntrBkSttlmAmt").as_deref(), Some(b"2500".as_ref()));
        assert_eq!(msg_get("IntrBkSttlmCcy").as_deref(), Some(b"USD".as_ref()));
        assert_eq!(
            msg_get("DbtrAcct").as_deref(),
            Some(b"GB82WEST12345698765432".as_ref())
        );
        assert_eq!(
            msg_get("CdtrAcct").as_deref(),
            Some(b"GB33BUKB20201555555555".as_ref())
        );
        assert_eq!(msg_get("Purp").as_deref(), Some(b"SECU".as_ref()));
    }

    #[test]
    fn parse_sample_pacs009_envelope() {
        assert_validated("pacs.009.001.10", SAMPLE_PACS009_ENVELOPE_XML);
        assert_eq!(
            msg_get("BizMsgIdr").as_deref(),
            Some(b"BAH-PACS009-1".as_ref())
        );
        assert_eq!(
            msg_get("MsgDefIdr").as_deref(),
            Some(b"pacs.009.001.10".as_ref())
        );
        assert_eq!(
            msg_get("CreDtTm").as_deref(),
            Some(b"2025-11-12T09:34:09Z".as_ref())
        );
        assert_eq!(msg_get("InstgAgt").as_deref(), Some(b"DEUTDEFF".as_ref()));
        assert_eq!(msg_get("InstdAgt").as_deref(), Some(b"MARKDEFF".as_ref()));
        assert_eq!(
            msg_get("DbtrAcct").as_deref(),
            Some(b"GB82WEST12345698765432".as_ref())
        );
        assert_eq!(
            msg_get("CdtrAcct").as_deref(),
            Some(b"GB33BUKB20201555555555".as_ref())
        );
    }

    #[test]
    fn parse_sample_pacs002_auth_allows_missing_txsts() {
        assert_validated("pacs.002.001.10", SAMPLE_PACS002_AUTH_XML);
        assert_eq!(
            msg_get("OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
    }

    #[test]
    fn parse_sample_camt052_allows_other_account_id() {
        assert_validated("camt.052.001.08", SAMPLE_CAMT052_XML);
        assert_eq!(
            msg_get("Rpt/Acct/Id").as_deref(),
            Some(b"ALTACCOUNT".as_ref())
        );
    }

    #[test]
    fn parse_sample_camt056_tracks_assignment() {
        assert_validated("camt.056.001.08", SAMPLE_CAMT056_XML);
        assert_eq!(
            msg_get("Assgnmt/Id").as_deref(),
            Some(b"ISO-CAMT056-ASSIGN".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
    }

    #[test]
    fn parse_sample_pacs004_return() {
        assert_validated("pacs.004.001.09", SAMPLE_PACS004_XML);
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"ISO-PACS004-MSG".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmt").as_deref(),
            Some(b"10.00".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmtCcy").as_deref(),
            Some(b"USD".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/ChrgBr").as_deref(),
            Some(b"SLEV".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdRsn/Prtry").as_deref(),
            Some(b"TechnicalProblem".as_ref())
        );
    }

    #[test]
    fn parse_sample_pacs002_status() {
        assert_validated("pacs.002.001.10", SAMPLE_PACS002_STATUS_XML);
        assert_eq!(
            msg_get("OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
        assert_eq!(msg_get("TxSts").as_deref(), Some(b"ACSP".as_ref()));
    }

    #[test]
    fn parse_generated_md_pacs008() {
        let xml = generated_sample("pacs.008.001.08");
        let parsed = parse_message("pacs.008", xml.as_bytes()).expect("parse pacs.008 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("MsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Document/FIToFICstmrCdtTrf/CdtTrfTxInf/PmtId/UETR"),
            Some("123e4567-e89b-12d3-a456-426614174000"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Document/FIToFICstmrCdtTrf/CdtTrfTxInf/ChrgBr"),
            Some("SHAR"),
            "keys={keys:?}"
        );
    }

    #[test]
    fn parse_generated_md_pacs004() {
        let xml = generated_sample("pacs.004.001.09");
        let parsed = parse_message("pacs.004", xml.as_bytes()).expect("parse pacs.004 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("MsgId"),
            Some("ISO-SAMPLE-004"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("OrgnlGrpInf/OrgnlMsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("TxInf[0]/ChrgBr"),
            Some("SHAR"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("TxInf[0]/RtrdRsn/Prtry"),
            Some("PR01"),
            "keys={keys:?}"
        );
    }

    #[test]
    fn parse_generated_md_pacs002() {
        let xml = generated_sample("pacs.002.001.10");
        let parsed = parse_message("pacs.002", xml.as_bytes()).expect("parse pacs.002 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(parsed.field_text("TxSts"), Some("ACSP"), "keys={keys:?}");
        assert_eq!(
            parsed.field_text("Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/OrgnlMsgNmId"),
            Some("pacs.008.001.08"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("OrgnlMsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
    }

    #[test]
    fn parse_generated_md_camt052() {
        let xml = generated_sample("camt.052.001.08");
        reset();
        msg_parse("camt.052", xml.as_bytes()).expect("parse camt.052 sample");
        let valid = msg_validate();
        let failure = take_validation_failure();
        assert!(
            !valid,
            "camt.052 sample should miss Rpt/CreDtTm: {failure:?}"
        );
        assert_eq!(
            msg_get("Rpt/Acct/Id").as_deref(),
            Some(b"ALPHBANK-USD-ACCOUNT-001".as_ref())
        );
        assert_eq!(
            msg_get("Rpt/Ntry[0]/CdtDbtInd").as_deref(),
            Some(b"DBIT".as_ref())
        );
    }

    #[test]
    fn parse_generated_md_camt056() {
        let xml = generated_sample("camt.056.001.08");
        let parsed = parse_message("camt.056", xml.as_bytes()).expect("parse camt.056 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlUETR"),
            Some("123e4567-e89b-12d3-a456-426614174000"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Assgnmt/Id"),
            Some("ISO-SAMPLE-056-ASGMT-001"),
            "keys={keys:?}"
        );
    }

    #[test]
    fn versioned_pacs007_supported() {
        reset();
        msg_create("pacs.007.001.09");
        populate_pacs007_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_camt056_supported() {
        reset();
        msg_create("camt.056.001.09");
        populate_camt056_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_pacs004_supported() {
        reset();
        msg_create("pacs.004.001.10");
        populate_pacs004_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_pacs028_supported() {
        reset();
        msg_create("pacs.028.001.09");
        populate_pacs028_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_pacs029_supported() {
        reset();
        msg_create("pacs.029.001.09");
        populate_pacs029_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn sese023_roundtrip_and_norito_snapshot() {
        reset();
        let schema = expected_sese023_schema();
        schema.apply_to_stack();
        assert!(msg_validate());

        let xml = msg_serialize("XML").expect("serialize sese.023");
        let xml_str = String::from_utf8(xml.clone()).expect("utf8");
        assert!(xml_str.contains("ISO20022 message=\"sese.023\""));
        assert!(xml_str.contains("Field path=\"SttlmTpAndAddtlParams/SctiesMvmntTp\""));
        assert!(xml_str.contains("Field path=\"Plan/Atomicity\""));

        let parsed = parse_message("sese.023", &xml).expect("parse sese.023");
        assert_eq!(parsed.field_text("SttlmParams/PrtlSttlmInd"), Some("NPAR"));
        assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("true"));
        let materialized =
            Sese023::from_parsed(&parsed).expect("materialize sese.023 into Norito schema");
        assert_eq!(schema, materialized);

        let encoded = schema.encode();
        let mut cursor = encoded.as_slice();
        let decoded = Sese023::decode(&mut cursor).expect("decode");
        assert_eq!(schema, decoded);
    }

    #[test]
    fn sese023_requires_movement_and_payment_qualifiers() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_remove("SttlmTpAndAddtlParams/SctiesMvmntTp");
        assert!(!msg_validate());
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"INVALID");
        assert!(!msg_validate());
    }

    #[test]
    fn sese023_missing_execution_order_fails() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_remove("Plan/ExecutionOrder");
        assert!(!msg_validate());
        msg_set("Plan/ExecutionOrder", b"INVALID");
        assert!(!msg_validate());
    }

    #[test]
    fn sese023_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("sese.023", SESE023_FIXTURE.as_bytes()).expect("parse sese.023 fixture");
        let schema = Sese023::from_parsed(&parsed).expect("materialize sese.023 from fixture");
        assert_eq!(schema, expected_sese023_schema());
    }

    #[test]
    fn sese025_validation_and_serialization() {
        reset();
        let schema = expected_sese025_schema();
        schema.apply_to_stack();
        assert!(msg_validate());
        let xml = msg_serialize("XML").expect("serialize sese.025");
        let parsed = parse_message("sese.025", &xml).expect("parse sese.025");
        let materialized = Sese025::from_parsed(&parsed).expect("materialize sese.025 into schema");
        assert_eq!(materialized, schema);
        assert_eq!(parsed.field_text("ConfSts"), Some("ACCP"));
        assert_eq!(
            parsed.field_text("Plan/Atomicity"),
            Some("COMMIT_SECOND_LEG")
        );
        assert_eq!(
            parsed.field_text("SttlmTpAndAddtlParams/SctiesMvmntTp"),
            Some("RECE")
        );
        assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("false"));
    }

    #[test]
    fn sese025_requires_plan_fields() {
        reset();
        msg_create("sese.025");
        populate_sese025_minimal();
        msg_remove("Plan/ExecutionOrder");
        assert!(!msg_validate());
        msg_set("Plan/ExecutionOrder", b"PAYMENT_THEN_DELIVERY");
        msg_set("Plan/Atomicity", b"INVALID");
        assert!(!msg_validate());
    }

    #[test]
    fn sese025_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("sese.025", SESE025_FIXTURE.as_bytes()).expect("parse sese.025 fixture");
        let schema = Sese025::from_parsed(&parsed).expect("materialize sese.025 from fixture");
        assert_eq!(schema, expected_sese025_schema());
    }

    #[test]
    fn colr007_roundtrip_and_norito_snapshot() {
        reset();
        let schema = expected_colr007_schema();
        schema.apply_to_stack();
        assert!(msg_validate());

        let xml = msg_serialize("XML").expect("serialize colr.007");
        let parsed = parse_message("colr.007", &xml).expect("parse colr.007");
        assert_eq!(parsed.field_text("Substitution/Haircut"), Some("50"));
        let materialized = Colr007::from_parsed(&parsed).expect("materialize colr.007 into schema");
        assert_eq!(materialized, schema);

        let encoded = schema.encode();
        let mut cursor = encoded.as_slice();
        let decoded = Colr007::decode(&mut cursor).expect("decode");
        assert_eq!(schema, decoded);
    }

    #[test]
    fn colr007_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("colr.007", COLR007_FIXTURE.as_bytes()).expect("parse colr.007 fixture");
        let schema = Colr007::from_parsed(&parsed).expect("materialize colr.007 from fixture");
        assert_eq!(schema, expected_colr007_schema());
    }

    #[test]
    fn colr007_rejects_unknown_type() {
        reset();
        msg_create("colr.007");
        populate_colr007_minimal();
        msg_set("Substitution/Type", b"UNEXPECTED");
        assert!(!msg_validate());
    }

    #[test]
    fn versioned_sese023_supported() {
        reset();
        msg_create("sese.023.001.09");
        populate_sese023_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_sese025_supported() {
        reset();
        msg_create("sese.025.001.08");
        populate_sese025_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_colr007_supported() {
        reset();
        msg_create("colr.007.001.08");
        populate_colr007_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn versioned_pain002_supported() {
        reset();
        msg_create("pain.002.001.12");
        populate_pain002_minimal();
        assert!(msg_validate());
    }

    #[test]
    fn msg_sign_and_verify_roundtrip() {
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let sk_bytes = [7u8; 32];
        let sig = msg_sign(&sk_bytes);
        let pk = SigningKey::from_bytes(&sk_bytes).verifying_key();
        assert!(msg_verify_sig(&sig, pk.as_bytes()));
    }

    #[test]
    fn msg_sign_and_verify_roundtrip_dilithium() {
        use pqcrypto_mldsa::mldsa65 as dilithium;
        use pqcrypto_traits::sign::{PublicKey, SecretKey};
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let (pk, sk) = dilithium::keypair();
        let mut tagged = Vec::with_capacity(1 + sk.as_bytes().len());
        tagged.push(Algorithm::MlDsa as u8);
        tagged.extend_from_slice(sk.as_bytes());
        let sig = msg_sign(&tagged);
        assert!(msg_verify_sig(&sig, pk.as_bytes()));
    }

    #[test]
    fn msg_sign_and_verify_roundtrip_secp256k1() {
        use k256::ecdsa::{SigningKey, VerifyingKey};
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let sk = SigningKey::from_bytes(&[9u8; 32].into()).expect("sk");
        let sk_bytes = sk.to_bytes();
        let mut tagged = Vec::with_capacity(1 + sk_bytes.len());
        tagged.push(Algorithm::Secp256k1 as u8);
        tagged.extend_from_slice(sk_bytes.as_slice());
        let sig = msg_sign(&tagged);
        let pk = VerifyingKey::from(&sk);
        let pk_bytes = pk.to_encoded_point(true);
        assert!(msg_verify_sig(&sig, pk_bytes.as_bytes()));
    }

    #[test]
    fn msg_send_records_message() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmAmt", b"10");
        let expected = msg_serialize("XML").unwrap();
        msg_send("chan").unwrap();
        SENT_MSGS.with(|log| {
            let logged = log.borrow();
            assert_eq!(logged.len(), 1);
            assert_eq!(logged[0].0, "chan");
            assert_eq!(logged[0].1, expected);
        });
    }

    thread_local! {
        static CUSTOM: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    }

    fn custom_backend(chan: &str, data: &[u8]) {
        CUSTOM.with(|c| c.borrow_mut().push((chan.to_owned(), data.to_vec())));
    }

    #[test]
    fn msg_send_custom_backend() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmAmt", b"10");
        set_msg_sender(Some(custom_backend));
        msg_send("chan").unwrap();
        CUSTOM.with(|c| {
            let log = c.borrow();
            assert_eq!(log.len(), 1);
            assert_eq!(log[0].0, "chan");
            assert_eq!(log[0].1, msg_serialize("XML").unwrap());
        });
    }

    #[test]
    fn take_sent_messages_drains_log() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmAmt", b"10");
        msg_send("chan").unwrap();
        let msgs = take_sent_messages();
        assert_eq!(msgs.len(), 1);
        assert!(SENT_MSGS.with(|log| log.borrow().is_empty()));
    }

    #[test]
    fn msg_send_requires_valid_message() {
        reset();
        msg_create("pacs.008");
        assert!(matches!(msg_send("chan"), Err(MsgError::ValidationFailed)));
        SENT_MSGS.with(|log| assert!(log.borrow().is_empty()));
    }

    #[test]
    fn msg_parse_xml_roundtrip() {
        reset();
        msg_parse(
            "pacs.008",
            b"<ISO20022 message=\"pacs.008\"><Field path=\"MsgId\">1</Field><Field path=\"IntrBkSttlmCcy\">USD</Field><Field path=\"IntrBkSttlmAmt\">10</Field><Field path=\"IntrBkSttlmDt\">2024-01-01</Field><Field path=\"DbtrAcct\">GB82WEST12345698765432</Field><Field path=\"CdtrAcct\">GB33BUKB20201555555555</Field><Field path=\"DbtrAgt\">DEUTDEFF</Field><Field path=\"CdtrAgt\">DEUTDEFF</Field></ISO20022>"
        ).unwrap();
        assert!(msg_validate());
        let xml = msg_serialize("XML").unwrap();
        let xml_str = String::from_utf8(xml).unwrap();
        assert!(xml_str.contains("<ISO20022"));
        assert!(xml_str.contains("CdtrAgt"));
    }

    #[test]
    fn signature_blocks_marked_as_ignored() {
        reset();
        let xml = r#"
            <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
                <FIToFICstmrCdtTrf>
                    <GrpHdr>
                        <MsgId>sig-001</MsgId>
                    </GrpHdr>
                    <CdtTrfTxInf>
                        <IntrBkSttlmAmt Ccy="USD">10</IntrBkSttlmAmt>
                        <IntrBkSttlmDt>2024-01-01</IntrBkSttlmDt>
                        <DbtrAcct>
                            <Id><IBAN>GB82WEST12345698765432</IBAN></Id>
                        </DbtrAcct>
                        <CdtrAcct>
                            <Id><IBAN>GB33BUKB20201555555555</IBAN></Id>
                        </CdtrAcct>
                        <DbtrAgt>
                            <FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId>
                        </DbtrAgt>
                        <CdtrAgt>
                            <FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId>
                        </CdtrAgt>
                    </CdtTrfTxInf>
                    <Sgntr>
                        <SignedInfo>
                            <SignatureMethod>dummy</SignatureMethod>
                        </SignedInfo>
                        <SignatureValue>Zm9vYmFy</SignatureValue>
                    </Sgntr>
                </FIToFICstmrCdtTrf>
            </Document>
        "#;
        msg_parse("pacs.008", xml.as_bytes()).expect("parse pacs.008 with signature");
        assert_eq!(
            msg_get("Document/FIToFICstmrCdtTrf/Sgntr/@ignored").as_deref(),
            Some(SIGNATURE_IGNORED_VALUE),
        );
        assert!(msg_get("Document/FIToFICstmrCdtTrf/Sgntr/SignatureValue").is_none());
        assert!(msg_validate());
    }

    #[test]
    fn msg_send_http_invokes_override() {
        fn record_http(endpoint: &HttpEndpoint, payload: &[u8]) -> Result<(), MsgError> {
            HTTP_CALLS.with(|calls| {
                calls.borrow_mut().push((
                    endpoint.host.clone(),
                    endpoint.port,
                    endpoint.path.clone(),
                    payload.to_vec(),
                ));
            });
            Ok(())
        }

        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        let expected = msg_serialize("XML").unwrap();
        set_http_sender_override(Some(record_http));
        msg_send("http://example.com/submit").unwrap();
        HTTP_CALLS.with(|calls| {
            let log = calls.borrow();
            assert_eq!(log.len(), 1);
            let (host, port, path, body) = &log[0];
            assert_eq!(host, "example.com");
            assert_eq!(*port, 80);
            assert_eq!(path, "/submit");
            assert_eq!(body, &expected);
        });
    }

    #[test]
    fn msg_send_http_without_override_sends_payload() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping msg_send_http_without_override_sends_payload: {err}");
                return;
            }
            Err(err) => panic!("listener: {err}"),
        };
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = String::new();
            let mut line = String::new();
            let mut content_len = 0usize;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((_, value)) = line
                    .split_once(':')
                    .filter(|_| line.to_ascii_lowercase().starts_with("content-length"))
                {
                    content_len = value.trim().parse().unwrap_or(0);
                }
                headers.push_str(&line);
            }
            let mut body = vec![0u8; content_len];
            reader.read_exact(&mut body).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            (headers, body)
        });

        let url = format!("http://{addr}/submit");
        msg_send(&url).unwrap();
        let (headers, body) = handle.join().unwrap();
        assert!(headers.contains("POST /submit HTTP/1.1"));
        assert!(String::from_utf8_lossy(&body).contains("ISO20022"));
    }

    #[test]
    fn amount_encode_decode_roundtrip() {
        let enc = encode_amount(42);
        assert_eq!(enc, b"42".to_vec());
        assert_eq!(decode_amount(&enc), Some(42));
        assert!(decode_amount(b"12a").is_none());
    }

    #[test]
    fn validate_format_dispatches() {
        assert!(validate_format("IBAN", b"GB82WEST12345698765432"));
        assert!(!validate_format("IBAN", b"GB82WEST12345698765433"));
        assert!(!validate_format("IBAN", b"NO938601111794"));
        assert!(validate_format("BIC", b"DEUTDEFF"));
        assert!(validate_format("NUMERIC", b"12345"));
        assert!(!validate_format("NUMERIC", b"12a"));
    }

    #[test]
    fn base64_encode_decode_roundtrip() {
        let data = b"hello world";
        let enc = encode_base64(data);
        assert_eq!(enc, b"aGVsbG8gd29ybGQ=".to_vec());
        assert_eq!(decode_base64(&enc), Some(data.to_vec()));
    }

    #[test]
    fn decode_base64_rejects_invalid() {
        assert!(decode_base64(b"@@@=").is_none());
    }

    #[test]
    fn decode_base64_into_reuses_buffer() {
        let mut out = Vec::new();
        decode_base64_into(b"SGVsbG8=", &mut out).unwrap();
        assert_eq!(out, b"Hello");
    }

    #[test]
    fn decode_base64_into_rejects_invalid() {
        let mut out = Vec::new();
        assert!(decode_base64_into(b"@@@=", &mut out).is_none());
    }
}
