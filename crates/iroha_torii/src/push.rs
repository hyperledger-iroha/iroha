#[cfg(test)]
use std::sync::Mutex;
use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use iroha_config::parameters::actual;
use iroha_data_model::account::AccountId;
use nonzero_ext::nonzero;
/// Logical push notification target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Fcm,
    Apns,
}

impl Platform {
    fn from_label(label: &str) -> Result<Self, PushError> {
        match label.trim().to_ascii_uppercase().as_str() {
            "FCM" => Ok(Self::Fcm),
            "APNS" => Ok(Self::Apns),
            other => Err(PushError::InvalidPlatform(other.to_string())),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fcm => "FCM",
            Self::Apns => "APNS",
        }
    }
}

/// Request payload for `/v1/notify/devices`.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct RegisterDeviceRequest {
    pub account_id: String,
    pub platform: String,
    pub token: String,
    pub topics: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct RegisteredDevice {
    pub account_id: String,
    pub platform: Platform,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PushError {
    Disabled,
    InvalidAccount(String),
    InvalidPlatform(String),
    MissingCredentials { platform: Platform },
    TooManyTopics { max: usize },
    EmptyToken,
}

/// Dispatch settings derived from configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchSettings {
    /// HTTP connect timeout.
    pub connect_timeout: Duration,
    /// HTTP request timeout.
    pub request_timeout: Duration,
}

/// Provider-specific credentials derived from configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderCredentials {
    Fcm {
        api_key: String,
    },
    Apns {
        endpoint: String,
        auth_token: String,
    },
}

/// Minimal dispatcher trait so push publishing can be mocked in tests or wired
/// to real transports in follow-up work.
pub trait PushDispatcher: Send + Sync {
    /// Dispatch a payload to the given platform/token.
    fn send(
        &self,
        platform: Platform,
        token: &str,
        payload: &str,
        settings: &DispatchSettings,
        credentials: &ProviderCredentials,
    ) -> Result<(), PushError>;
}

#[derive(Clone, Default)]
struct NoopDispatcher;

impl PushDispatcher for NoopDispatcher {
    fn send(
        &self,
        _platform: Platform,
        _token: &str,
        _payload: &str,
        _settings: &DispatchSettings,
        _credentials: &ProviderCredentials,
    ) -> Result<(), PushError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct PushBridge {
    config: actual::Push,
    settings: DispatchSettings,
    dispatcher: Arc<dyn PushDispatcher>,
    devices: Arc<DashMap<String, RegisteredDevice>>,
}

impl PushBridge {
    pub fn new(config: actual::Push) -> Self {
        Self::with_dispatcher(config, Arc::new(NoopDispatcher))
    }

    pub fn with_dispatcher(config: actual::Push, dispatcher: Arc<dyn PushDispatcher>) -> Self {
        let settings = DispatchSettings {
            connect_timeout: config.connect_timeout,
            request_timeout: config.request_timeout,
        };
        Self {
            config,
            settings,
            dispatcher,
            devices: Arc::new(DashMap::new()),
        }
    }

    pub fn register_device(&self, request: RegisterDeviceRequest) -> Result<(), PushError> {
        if !self.config.enabled {
            return Err(PushError::Disabled);
        }
        let account_id = request.account_id.trim();
        AccountId::parse_encoded(account_id)
            .map_err(|_| PushError::InvalidAccount(request.account_id.clone()))?;
        let platform = Platform::from_label(&request.platform)?;
        if !self.has_credentials(platform) {
            return Err(PushError::MissingCredentials { platform });
        }
        if request.token.trim().is_empty() {
            return Err(PushError::EmptyToken);
        }
        let topics = request.topics.unwrap_or_default();
        let max_topics = self.config.max_topics_per_device.get();
        if topics.len() > max_topics {
            return Err(PushError::TooManyTopics { max: max_topics });
        }
        let device = RegisteredDevice {
            account_id: account_id.to_string(),
            platform,
            topics,
        };
        self.devices.insert(request.token, device);
        Ok(())
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    #[cfg(test)]
    pub(crate) fn registered_device(&self, token: &str) -> Option<RegisteredDevice> {
        self.devices.get(token).map(|entry| entry.clone())
    }

    fn has_credentials(&self, platform: Platform) -> bool {
        match platform {
            Platform::Fcm => self.config.fcm_api_key.is_some(),
            Platform::Apns => {
                self.config.apns_endpoint.is_some() && self.config.apns_auth_token.is_some()
            }
        }
    }

    /// Dispatch a payload to the given platform/token using the configured dispatcher.
    pub fn send_notification(
        &self,
        platform: Platform,
        token: &str,
        payload: &str,
    ) -> Result<(), PushError> {
        if !self.config.enabled {
            return Err(PushError::Disabled);
        }
        let credentials = self.credentials_for(platform)?;
        self.dispatcher
            .send(platform, token, payload, &self.settings, &credentials)
    }

    fn credentials_for(&self, platform: Platform) -> Result<ProviderCredentials, PushError> {
        match platform {
            Platform::Fcm => self
                .config
                .fcm_api_key
                .as_ref()
                .map(|api_key| ProviderCredentials::Fcm {
                    api_key: api_key.clone(),
                })
                .ok_or(PushError::MissingCredentials { platform }),
            Platform::Apns => match (
                self.config.apns_endpoint.as_ref(),
                self.config.apns_auth_token.as_ref(),
            ) {
                (Some(endpoint), Some(auth_token)) => Ok(ProviderCredentials::Apns {
                    endpoint: endpoint.clone(),
                    auth_token: auth_token.clone(),
                }),
                _ => Err(PushError::MissingCredentials { platform }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ACCOUNT_I105: &str = "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ";

    #[test]
    fn disabled_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: false,
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t0".to_string(),
                topics: None,
            })
            .expect_err("push disabled");
        matches!(err, PushError::Disabled);
    }

    #[test]
    fn missing_credentials_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: true,
            fcm_api_key: None,
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t1".to_string(),
                topics: None,
            })
            .expect_err("missing creds");
        assert!(matches!(err, PushError::MissingCredentials { .. }));
    }

    #[test]
    fn too_many_topics_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: true,
            fcm_api_key: Some("k".to_string()),
            max_topics_per_device: nonzero!(2_usize),
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "t2".to_string(),
                topics: Some(vec!["a".into(), "b".into(), "c".into()]),
            })
            .expect_err("too many topics");
        assert!(matches!(err, PushError::TooManyTopics { max: 2 }));
    }

    #[test]
    fn stores_device_on_success() {
        let bridge = PushBridge::new(actual::Push {
            enabled: true,
            fcm_api_key: Some("k".to_string()),
            ..Default::default()
        });
        bridge
            .register_device(RegisterDeviceRequest {
                account_id: TEST_ACCOUNT_I105.to_string(),
                platform: "FCM".to_string(),
                token: "token-123".to_string(),
                topics: Some(vec!["incoming".into()]),
            })
            .expect("should store device");
        assert_eq!(bridge.device_count(), 1);
    }

    #[test]
    fn alias_account_id_is_rejected() {
        let bridge = PushBridge::new(actual::Push {
            enabled: true,
            fcm_api_key: Some("k".to_string()),
            ..Default::default()
        });
        let err = bridge
            .register_device(RegisterDeviceRequest {
                account_id: "alice@wallets".to_string(),
                platform: "FCM".to_string(),
                token: "token-123".to_string(),
                topics: None,
            })
            .expect_err("aliases must be rejected");
        assert!(matches!(err, PushError::InvalidAccount(account) if account == "alice@wallets"));
    }

    #[test]
    fn dispatch_rejected_when_disabled() {
        let dispatcher = Arc::new(MockDispatcher::default());
        let bridge = PushBridge::with_dispatcher(
            actual::Push {
                enabled: false,
                fcm_api_key: Some("k".to_string()),
                ..Default::default()
            },
            dispatcher.clone(),
        );

        let err = bridge
            .send_notification(Platform::Fcm, "token-1", "payload")
            .expect_err("push disabled");
        assert!(matches!(err, PushError::Disabled));
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 0);
    }

    #[test]
    fn dispatch_rejected_without_credentials() {
        let dispatcher = Arc::new(MockDispatcher::default());
        let bridge = PushBridge::with_dispatcher(
            actual::Push {
                enabled: true,
                ..Default::default()
            },
            dispatcher.clone(),
        );

        let err = bridge
            .send_notification(Platform::Fcm, "token-2", "payload")
            .expect_err("missing creds");
        assert!(matches!(err, PushError::MissingCredentials { .. }));
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 0);
    }

    #[test]
    fn dispatch_uses_configured_timeouts_and_credentials() {
        let dispatcher = Arc::new(MockDispatcher::default());
        let bridge = PushBridge::with_dispatcher(
            actual::Push {
                enabled: true,
                connect_timeout: Duration::from_millis(1_500),
                request_timeout: Duration::from_millis(2_500),
                fcm_api_key: Some("secret-key".to_string()),
                ..Default::default()
            },
            dispatcher.clone(),
        );

        bridge
            .send_notification(Platform::Fcm, "token-3", "payload")
            .expect("dispatch should succeed");

        let calls = dispatcher.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one call recorded").clone();
        assert_eq!(call.platform, Platform::Fcm);
        assert_eq!(call.token, "token-3");
        assert_eq!(call.payload, "payload");
        assert_eq!(
            call.settings,
            DispatchSettings {
                connect_timeout: Duration::from_millis(1_500),
                request_timeout: Duration::from_millis(2_500),
            }
        );
        assert!(matches!(
            call.credentials,
            ProviderCredentials::Fcm { ref api_key } if api_key == "secret-key"
        ));
    }

    #[derive(Default)]
    struct MockDispatcher {
        calls: Arc<Mutex<Vec<DispatchCall>>>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DispatchCall {
        platform: Platform,
        token: String,
        payload: String,
        settings: DispatchSettings,
        credentials: ProviderCredentials,
    }

    impl PushDispatcher for MockDispatcher {
        fn send(
            &self,
            platform: Platform,
            token: &str,
            payload: &str,
            settings: &DispatchSettings,
            credentials: &ProviderCredentials,
        ) -> Result<(), PushError> {
            self.calls.lock().unwrap().push(DispatchCall {
                platform,
                token: token.to_string(),
                payload: payload.to_string(),
                settings: settings.clone(),
                credentials: credentials.clone(),
            });
            Ok(())
        }
    }
}
