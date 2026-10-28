use std::{fmt, path::Path};

use serde::Serialize;
use thiserror::Error;

use crate::preview_network_file::{self, TurnServer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IceTransportPolicy {
    All,
    Relay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpPortRange {
    pub min: u16,
    pub max: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnTransport {
    Udp,
    Tcp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewNetworkCapability {
    pub stun_configured: bool,
    pub turn_configured: bool,
    pub secure_turn_configured: bool,
    pub turn_transports: Vec<TurnTransport>,
    pub native_udp_range: Option<UdpPortRange>,
    pub ice_transport_policy: IceTransportPolicy,
}

#[derive(Clone)]
pub struct PreviewNetworkConfig {
    stun_server: Option<String>,
    turn_servers: Vec<TurnServer>,
    native_udp_range: Option<UdpPortRange>,
    ice_transport_policy: IceTransportPolicy,
}

impl fmt::Debug for PreviewNetworkConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PreviewNetworkConfig")
            .field(&self.capability())
            .finish()
    }
}

impl PreviewNetworkConfig {
    /// Loads the private network configuration and preserves the legacy STUN setting.
    ///
    /// # Errors
    /// Returns a redacted configuration error for insecure, malformed, or invalid input.
    pub fn load(
        path: Option<&Path>,
        legacy_stun: Option<String>,
    ) -> Result<Self, PreviewNetworkError> {
        let parsed = preview_network_file::load(path, legacy_stun)?;
        if parsed.ice_transport_policy == IceTransportPolicy::Relay
            && parsed.turn_servers.is_empty()
        {
            return Err(PreviewNetworkError::Invalid);
        }
        Ok(Self {
            stun_server: parsed.stun_server,
            turn_servers: parsed.turn_servers,
            native_udp_range: parsed.native_udp_range,
            ice_transport_policy: parsed.ice_transport_policy,
        })
    }

    /// Loads configuration from the private file path and legacy environment variable.
    ///
    /// # Errors
    /// Returns a redacted configuration error when the boundary cannot be parsed.
    pub fn from_environment() -> Result<Self, PreviewNetworkError> {
        let path = std::env::var_os("LIVE_PRODUCTION_PREVIEW_NETWORK_CONFIG")
            .map(std::path::PathBuf::from);
        let legacy_stun = std::env::var("LIVE_PRODUCTION_STUN_SERVER").ok();
        Self::load(path.as_deref(), legacy_stun)
    }

    #[must_use]
    pub fn capability(&self) -> PreviewNetworkCapability {
        PreviewNetworkCapability {
            stun_configured: self.stun_server.is_some(),
            turn_configured: !self.turn_servers.is_empty(),
            secure_turn_configured: self.turn_servers.iter().any(TurnServer::secure),
            turn_transports: self
                .turn_servers
                .iter()
                .map(TurnServer::transport)
                .collect(),
            native_udp_range: self.native_udp_range,
            ice_transport_policy: self.ice_transport_policy,
        }
    }

    pub(crate) fn stun_server(&self) -> Option<&str> {
        self.stun_server.as_deref()
    }

    pub(crate) fn turn_server_uris(&self) -> impl Iterator<Item = &str> {
        self.turn_servers.iter().map(TurnServer::uri)
    }

    pub(crate) const fn native_udp_range(&self) -> Option<UdpPortRange> {
        self.native_udp_range
    }

    pub(crate) const fn ice_transport_policy(&self) -> IceTransportPolicy {
        self.ice_transport_policy
    }
}

#[derive(Debug, Error)]
pub enum PreviewNetworkError {
    #[error("Cannot read the private preview network configuration")]
    Read(#[source] std::io::Error),
    #[error("Preview network configuration must be a private regular file")]
    Insecure,
    #[error("Preview network configuration is invalid")]
    Invalid,
    #[error("The installed WebRTC backend cannot enforce the configured native UDP range")]
    UnsupportedUdpRange,
}
