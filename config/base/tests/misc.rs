#![allow(clippy::needless_raw_string_hashes)]

use std::{backtrace::Backtrace, panic::Location, path::PathBuf};

use error_stack::{fmt::ColorMode, Context, Report};
use expect_test::expect;
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use toml::toml;

pub mod sample_config {
    use std::{net::SocketAddr, path::PathBuf};

    use iroha_config_base::{
        read::{ConfigReader, FinalWrap, ReadConfig},
        WithOrigin,
    };
    use serde::Deserialize;

    #[derive(Debug)]
    pub struct Root {
        pub chain_id: String,
        pub torii: Torii,
        pub kura: Kura,
        pub telemetry: Telemetry,
        pub logger: Logger,
    }

    impl ReadConfig for Root {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            let chain_id = reader
                .read_parameter::<String>(["chain_id"])
                .env("CHAIN_ID")
                .value_required()
                .finish();

            let torii = reader.read_nested("torii");

            let kura = reader.read_nested("kura");

            let telemetry = reader.read_nested("telemetry");

            let logger = reader.read_nested("logger");

            FinalWrap::value_fn(move || Self {
                chain_id: chain_id.unwrap(),
                torii: torii.unwrap(),
                kura: kura.unwrap(),
                telemetry: telemetry.unwrap(),
                logger: logger.unwrap(),
            })
        }
    }

    #[derive(Debug)]
    pub struct Torii {
        pub address: WithOrigin<SocketAddr>,
        pub max_content_len: u64,
    }

    impl ReadConfig for Torii {
        fn read(reader: &mut ConfigReader) -> FinalWrap<Self>
        where
            Self: Sized,
        {
            let address = reader
                .read_parameter::<SocketAddr>(["address"])
                .env("API_ADDRESS")
                .value_or_else(|| "128.0.0.1:8080".parse().unwrap())
                .finish_with_origin();

            let max_content_len = reader
                .read_parameter::<u64>(["max_content_length"])
                .value_or_else(|| 1024)
                .finish();

            FinalWrap::value_fn(|| Self {
                address: address.unwrap(),
                max_content_len: max_content_len.unwrap(),
            })
        }
    }

    #[derive(Debug)]
    pub struct Kura {
        pub store_dir: WithOrigin<PathBuf>,
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

    #[derive(Debug)]
    pub struct Telemetry {
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

    #[derive(Debug, Copy, Clone)]
    pub struct Logger {
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

    #[derive(Deserialize, Debug, Default, strum::Display, strum::EnumString, Copy, Clone)]
    pub enum LogLevel {
        Debug,
        #[default]
        Info,
        Warning,
        Error,
    }
}

fn format_report<C>(report: &Report<C>) -> String {
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
    fn assert_eq_report(&self, report: &Report<impl Context>);
}

impl ExpectExt for expect_test::Expect {
    fn assert_eq_report(&self, report: &Report<impl Context>) {
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
        ╰─▶ data did not match any variant of untagged enum ExtendsPaths
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
        ├╴extending (1): `./tests/bad.invalid-nested-extends.toml` -> `./tests/bad.invalid-nested-extends.base.toml`
        │
        ├─▶ File system error
        │   ╰╴file path: ./tests/non-existing.toml
        │
        ╰─▶ No such file or directory (os error 2)"#]]
    .assert_eq_report(&report);
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
        ╰╴missing parameter: `chain_id`"#]]
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
                chain_id = "412"

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
                chain_id = "ok"
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
        Errors occurred while reading from file: `./base.toml`
        │
        ├─▶ Failed to parse parameter `torii.address`
        │
        ╰─▶ invalid socket address syntax
            ╰╴value: "is it socket addr?"

        Errors occurred while reading from file: `./config.toml`
        │
        ├─▶ Failed to parse parameter `torii.address`
        │
        ╰─▶ invalid type: boolean `false`, expected socket address
            ╰╴value: false"#]]
    .assert_eq_report(&report);
}

#[test]
fn minimal_config_ok() {
    let value = ConfigReader::new()
        .with_toml_source(TomlSource::new(
            PathBuf::from("./config.toml"),
            toml! {
                chain_id = "whatever"
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect("config is valid");

    expect![[r#"
        Root {
            chain_id: "whatever",
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
                chain_id = "whatever"

                [torii]
                address = "127.0.0.2:1337"
                max_content_length = 19

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
            chain_id: "whatever",
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
        .with_env(MockEnv::from(vec![("CHAIN_ID", "in env")]))
        .with_toml_source(TomlSource::new(
            PathBuf::from("config.toml"),
            toml! {
                chain_id = "in file"
            },
        ))
        .read_and_complete::<sample_config::Root>()
        .expect("config is valid");

    assert_eq!(root.chain_id, "in env");
}

#[test]
#[ignore]
fn full_from_env() {
    todo!()
}

#[test]
fn multiple_env_parsing_errors() {
    let report = ConfigReader::new()
        .with_env(MockEnv::from([
            ("CHAIN_ID", "just to set"),
            ("API_ADDRESS", "i am not socket addr"),
            ("LOG_LEVEL", "error or whatever"),
        ]))
        .read_and_complete::<sample_config::Root>()
        .expect_err("invalid config");

    expect![[r#"
        Errors occurred while reading from environment variables
        │
        ╰┬▶ Failed to parse parameter `torii.address` from `API_ADDRESS`
         │  │
         │  ╰─▶ invalid socket address syntax
         │      ╰╴value: API_ADDRESS=i am not socket addr
         │
         ╰▶ Failed to parse parameter `logger.level` from `LOG_LEVEL`
            │
            ╰─▶ Matching variant not found
                ╰╴value: LOG_LEVEL=error or whatever"#]]
    .assert_eq_report(&report);
}
