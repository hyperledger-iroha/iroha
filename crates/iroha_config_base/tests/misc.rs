//! Miscellaneous config-reading tests for `iroha_config_base`.
#![allow(clippy::needless_raw_string_hashes)]

use std::{backtrace::Backtrace, panic::Location, path::PathBuf};

use error_stack::{Report, fmt::ColorMode};
use expect_test::expect;
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use toml::toml;

/// Sample configuration types used by tests to validate the reader.
pub mod sample_config {
    use std::{net::SocketAddr, path::PathBuf};

    use iroha_config_base::{
        WithOrigin,
        read::{ConfigReader, FinalWrap, ReadConfig},
    };
    use norito::json::{self, JsonDeserialize, JsonSerialize};

    /// Root configuration container aggregating all subsections.
    #[derive(Debug)]
    pub struct Root {
        /// Identifier of the blockchain network (chain ID).
        pub chain: String,
        /// Torii (HTTP API) configuration.
        pub torii: Torii,
        /// Kura (block storage) configuration.
        pub kura: Kura,
        /// Telemetry output configuration.
        pub telemetry: Telemetry,
        /// Logger configuration.
        pub logger: Logger,
    }

    impl ReadConfig for Root {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            let chain_id = reader
                .read_parameter::<String>(["chain"])
                .env("CHAIN")
                .value_required()
                .finish();

            let torii = reader.read_nested("torii");

            let kura = reader.read_nested("kura");

            let telemetry = reader.read_nested("telemetry");

            let logger = reader.read_nested("logger");

            FinalWrap::value_fn(move || Self {
                chain: chain_id.unwrap(),
                torii: torii.unwrap(),
                kura: kura.unwrap(),
                telemetry: telemetry.unwrap(),
                logger: logger.unwrap(),
            })
        }
    }

    /// Torii (HTTP API) configuration values.
    #[derive(Debug)]
    pub struct Torii {
        /// Socket address to bind the Torii HTTP server to.
        pub address: WithOrigin<SocketAddr>,
        /// Maximum allowed content length for POST bodies (bytes).
        pub max_content_len: u64,
    }

    impl ReadConfig for Torii {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            let address = reader
                .read_parameter::<String>(["address"])
                .env("API_ADDRESS")
                .value_or_else(|| "128.0.0.1:8080".to_string())
                .finish_with_origin();

            let max_content_len = reader
                .read_parameter::<u64>(["max_content_len"])
                .value_or_else(|| 1024)
                .finish();

            FinalWrap::value_fn(|| Self {
                address: address
                    .unwrap()
                    .map(|addr| addr.parse::<SocketAddr>().expect("invalid socket addr")),
                max_content_len: max_content_len.unwrap(),
            })
        }
    }

    /// Kura (block storage) configuration values.
    #[derive(Debug)]
    pub struct Kura {
        /// Directory where Kura stores its data.
        pub store_dir: WithOrigin<PathBuf>,
        /// Force debug behavior in Kura-related code paths.
        pub debug_force: bool,
    }

    impl ReadConfig for Kura {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            // origin needed so that we can resolve the path relative to the origin
            let store_dir = reader
                .read_parameter::<PathBuf>(["store_dir"])
                .env("KURA_STORE_DIR")
                .value_or_else(|| PathBuf::from("./storage"))
                .finish_with_origin();

            let debug_force = reader
                .read_parameter::<bool>(["debug_force"])
                .value_or_else(|| false)
                .finish();

            FinalWrap::value_fn(|| Self {
                store_dir: store_dir.unwrap(),
                debug_force: debug_force.unwrap(),
            })
        }
    }

    /// Telemetry configuration values.
    #[derive(Debug)]
    pub struct Telemetry {
        /// Optional file to write telemetry output to.
        pub out_file: Option<WithOrigin<PathBuf>>,
    }

    impl ReadConfig for Telemetry {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            // origin needed so that we can resolve the path relative to the origin
            let out_file = reader
                .read_parameter::<PathBuf>(["dev", "out_file"])
                .value_optional()
                .finish_with_origin();

            FinalWrap::value_fn(|| Self {
                out_file: out_file.unwrap(),
            })
        }
    }

    /// Logger configuration values.
    #[derive(Debug, Copy, Clone)]
    pub struct Logger {
        /// Logging verbosity level.
        pub level: LogLevel,
    }

    impl ReadConfig for Logger {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            let level = reader
                .read_parameter::<LogLevel>(["level"])
                .env("LOG_LEVEL")
                .value_or_default()
                .finish();

            FinalWrap::value_fn(|| Self {
                level: level.unwrap(),
            })
        }
    }

    /// Verbosity of log output.
    #[derive(Debug, Default, strum::Display, strum::EnumString, Copy, Clone)]
    pub enum LogLevel {
        /// Debug-level logging.
        Debug,
        /// Info-level logging.
        #[default]
        Info,
        /// Warning-level logging.
        Warning,
        /// Error-level logging.
        Error,
    }

    impl JsonSerialize for LogLevel {
        fn json_serialize(&self, out: &mut String) {
            let text = self.to_string();
            norito::json::write_json_string(&text.to_lowercase(), out);
        }
    }

    impl JsonDeserialize for LogLevel {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let text = parser.parse_string()?;
            match text.to_lowercase().as_str() {
                "debug" => Ok(LogLevel::Debug),
                "info" => Ok(LogLevel::Info),
                "warning" => Ok(LogLevel::Warning),
                "error" => Ok(LogLevel::Error),
                other => Err(json::Error::InvalidField {
                    field: "LogLevel".into(),
                    message: format!("unknown log level `{other}`"),
                }),
            }
        }
    }
}

fn format_report<C: ?Sized>(report: &Report<C>) -> String {
    Report::install_debug_hook::<Backtrace>(|_value, _context| {
        // noop
    });

    Report::install_debug_hook::<Location>(|_value, _context| {
        // noop
    });

    Report::set_color_mode(ColorMode::None);

    format!("{report:#?}")
}

trait ExpectExt {
    fn assert_eq_report<C>(&self, report: &Report<C>)
    where
        C: ?Sized;
}

impl ExpectExt for expect_test::Expect {
    fn assert_eq_report<C>(&self, report: &Report<C>)
    where
        C: ?Sized,
    {
        self.assert_eq(&format_report(report));
    }
}

#[test]
fn error_when_no_file() {
    let report = ConfigReader::new()
        .read_toml_with_extends("/path/to/non/existing...")
        .expect_err("the path doesn't exist");

    expect![[r#"
        Failed to read configuration from file
        │
        ├─▶ File system error
        │   ╰╴file path: /path/to/non/existing...
        │
        ╰─▶ No such file or directory (os error 2)"#]]
    .assert_eq_report(&report);
}

#[test]
fn error_invalid_extends() {
    let report = ConfigReader::new()
        .read_toml_with_extends("./tests/bad.invalid-extends.toml")
        .expect_err("extends is invalid, should fail");

    expect![[r#"
        Invalid `extends` field
        │
        ╰─▶ expected a string or an array of strings
            ├╴expected: a single path ("./file.toml") or an array of paths (["a.toml", "b.toml", "c.toml"])
            ╰╴actual value: 1234"#]]
        .assert_eq_report(&report);
}

#[test]
fn error_extends_depth_2_leads_to_nowhere() {
    let report = ConfigReader::new()
        .read_toml_with_extends("./tests/bad.invalid-nested-extends.toml")
        .expect_err("extends is invalid, should fail");

    expect![[r#"
        Failed to read configuration from file
        ├╴extending (2): `./tests/bad.invalid-nested-extends.base.toml` -> `./tests/non-existing.toml`
        │
        ├─▶ File system error
        │   ╰╴file path: ./tests/non-existing.toml
        │
        ╰─▶ No such file or directory (os error 2)"#]]
        .assert_eq_report(&report);
}

#[test]
fn extends_chain_applies_in_order() {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("extends_chain_{unique}"));
    fs::create_dir_all(&dir).unwrap();

    fs::write(dir.join("base.toml"), "chain = \"base\"").unwrap();
    fs::write(
        dir.join("middle.toml"),
        "extends = \"base.toml\"\nchain = \"middle\"",
    )
    .unwrap();
    fs::write(dir.join("top.toml"), "extends = \"middle.toml\"").unwrap();

    let mut reader = ConfigReader::new()
        .read_toml_with_extends(dir.join("top.toml"))
        .expect("valid chain");
    let chain = reader
        .read_parameter::<String>(["chain"])
        .value_required()
        .finish();
    reader.into_result().expect("config is valid");

    assert_eq!(chain.unwrap(), "middle");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn error_reading_empty_config() {
    let report = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml::Table::new(),
        ))
        .read_and_complete::<sample_config::Root>()
        .expect_err("should miss required fields");

    expect![[r#"
        Some required parameters are missing
        ╰╴missing parameter: `chain`"#]]
    .assert_eq_report(&report);
}

#[test]
fn error_extra_fields_in_multiple_files() {
    let report = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml! {
                extra_1 = 42
                extra_2 = false
            },
        ))
        .with_toml_source(TomlSource::new(
            PathBuf::from("./base.toml"),
            toml! {
                chain = "412"

                [torii]
                bar = false
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect_err("there are unknown fields");

    expect![[r#"
        Errors occurred while reading from file: `./base.toml`
        │
        ╰─▶ Found unrecognised parameters
            ╰╴unknown parameter: `torii.bar`

        Errors occurred while reading from file: `./config.toml`
        │
        ╰─▶ Found unrecognised parameters
            ├╴unknown parameter: `extra_1`
            ╰╴unknown parameter: `extra_2`"#]]
    .assert_eq_report(&report);
}

#[test]
fn multiple_parsing_errors_in_multiple_sources() {
    let report = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./base.toml"),
            toml! {
                chain = "ok"
                torii.address = "is it socket addr?"
            },
        ))
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml! {
                [torii]
                address = false
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect_err("invalid config");

    expect![[r#"
        Errors occurred while reading from file: `./config.toml`
        │
        ├─▶ Failed to parse parameter `torii.address`
        │
        ╰─▶ failed to deserialize config value: unexpected character `f` at byte 0 (line 1, col 1)
            ╰╴actual value: false"#]]
    .assert_eq_report(&report);
}

#[test]
fn minimal_config_ok() {
    let value = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml! {
                chain = "whatever"
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect("config is valid");

    expect![[r#"
        Root {
            chain: "whatever",
            torii: Torii {
                address: WithOrigin {
                    value: 128.0.0.1:8080,
                    origin: Default {
                        id: ParameterId(torii.address),
                    },
                },
                max_content_len: 1024,
            },
            kura: Kura {
                store_dir: WithOrigin {
                    value: "./storage",
                    origin: Default {
                        id: ParameterId(kura.store_dir),
                    },
                },
                debug_force: false,
            },
            telemetry: Telemetry {
                out_file: None,
            },
            logger: Logger {
                level: Info,
            },
        }"#]]
    .assert_eq(&format!("{value:#?}"));
}

#[test]
fn full_config_ok() {
    let value = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml! {
                chain = "whatever"

                [torii]
                address = "127.0.0.2:1337"
                max_content_len = 19

                [kura]
                store_dir = "./my-storage"
                debug_force = true

                [telemetry.dev]
                out_file = "./telemetry.json"

                [logger]
                level = "Error"
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect("config is valid");

    expect![[r#"
        Root {
            chain: "whatever",
            torii: Torii {
                address: WithOrigin {
                    value: 127.0.0.2:1337,
                    origin: File {
                        id: ParameterId(torii.address),
                        path: "./config.toml",
                    },
                },
                max_content_len: 19,
            },
            kura: Kura {
                store_dir: WithOrigin {
                    value: "./my-storage",
                    origin: File {
                        id: ParameterId(kura.store_dir),
                        path: "./config.toml",
                    },
                },
                debug_force: true,
            },
            telemetry: Telemetry {
                out_file: Some(
                    WithOrigin {
                        value: "./telemetry.json",
                        origin: File {
                            id: ParameterId(telemetry.dev.out_file),
                            path: "./config.toml",
                        },
                    },
                ),
            },
            logger: Logger {
                level: Error,
            },
        }"#]]
    .assert_eq(&format!("{value:#?}"));
}

#[test]
fn env_overwrites_toml() {
    let root = ConfigReader::new()
        .with_env(MockEnv::from(vec![("CHAIN", "in env")]))
        .with_toml_source(TomlSource::new(
            PathBuf::from("config.toml"),
            toml! {
                chain = "in file"
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect("config is valid");

    assert_eq!(root.chain, "in env");
}

#[test]
fn full_from_env() {
    use sample_config::{LogLevel, Root};

    let env = MockEnv::from([
        ("CHAIN", "from env"),
        ("API_ADDRESS", "127.0.0.1:3030"),
        ("KURA_STORE_DIR", "/var/lib/iroha"),
        ("LOG_LEVEL", "Error"),
    ]);

    let value = ConfigReader::new()
        .with_env(env)
        .read_and_complete::<Root>()
        .expect("config is valid");

    assert_eq!(value.chain, "from env");
    assert_eq!(
        *value.torii.address.value(),
        "127.0.0.1:3030".parse::<std::net::SocketAddr>().unwrap()
    );
    // `max_content_len` has no env -> default from reader
    assert_eq!(value.torii.max_content_len, 1024);
    assert_eq!(
        value.kura.store_dir.value().as_path(),
        std::path::Path::new("/var/lib/iroha")
    );
    // `debug_force` has no env -> default from reader
    assert!(!value.kura.debug_force);
    // telemetry.out_file has no env -> remains None
    assert!(value.telemetry.out_file.is_none());
    assert!(matches!(value.logger.level, LogLevel::Error));
}

#[test]
fn multiple_env_parsing_errors() {
    let report = ConfigReader::new()
        .with_env(MockEnv::from([
            ("CHAIN", "just to set"),
            ("API_ADDRESS", "i am not socket addr"),
            ("LOG_LEVEL", "error or whatever"),
        ]))
        .read_and_complete::<sample_config::Root>()
        .expect_err("invalid config");

    expect![[r#"
        Errors occurred while reading from environment variables
        │
        ├─▶ Failed to parse parameter `logger.level` from `LOG_LEVEL`
        │
        ╰─▶ Matching variant not found
            ╰╴value: LOG_LEVEL=error or whatever"#]]
    .assert_eq_report(&report);
}
