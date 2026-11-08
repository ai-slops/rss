use live_production_media::{
    preview::{PreviewOutputs, SOURCE_AUDIO_SLOTS},
    preview_network::{IceTransportPolicy, PreviewNetworkCapability, TurnTransport, UdpPortRange},
};
use serde::Serialize;

use crate::routes_media::MAXIMUM_MEDIA_CLIENTS;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaNetworkCapability {
    available: bool,
    native_backend: NativeBackend,
    authenticated_same_origin_signalling: bool,
    maximum_media_clients: usize,
    maximum_source_audio_monitors: usize,
    browser_ice_servers_configured: bool,
    native_ice: NativeIceCapability,
    host_requirement: HostRequirement,
    control_https_proves_media: bool,
    remote_laptop_verified: bool,
}

impl MediaNetworkCapability {
    pub fn from_outputs(outputs: Option<&PreviewOutputs>) -> Self {
        let (available, native_backend, native_ice) = outputs.map_or_else(
            || {
                (
                    false,
                    NativeBackend {
                        name: "rswebrtc",
                        version: "unavailable".into(),
                    },
                    NativeIceCapability::default(),
                )
            },
            |outputs| {
                (
                    true,
                    NativeBackend {
                        name: "rswebrtc",
                        version: outputs.plugin_version().into(),
                    },
                    NativeIceCapability::from(outputs.network_capability()),
                )
            },
        );
        let host_requirement = HostRequirement::from(&native_ice);
        Self {
            available,
            native_backend,
            authenticated_same_origin_signalling: true,
            maximum_media_clients: MAXIMUM_MEDIA_CLIENTS,
            maximum_source_audio_monitors: SOURCE_AUDIO_SLOTS,
            browser_ice_servers_configured: false,
            native_ice,
            host_requirement,
            control_https_proves_media: false,
            remote_laptop_verified: false,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeBackend {
    name: &'static str,
    version: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeIceCapability {
    stun_configured: bool,
    turn_configured: bool,
    secure_turn_configured: bool,
    turn_transports: Vec<TurnTransport>,
    native_udp_range: Option<UdpPortRange>,
    native_udp_range_enforced: bool,
    ice_transport_policy: Option<IceTransportPolicy>,
}

impl From<PreviewNetworkCapability> for NativeIceCapability {
    fn from(value: PreviewNetworkCapability) -> Self {
        Self {
            stun_configured: value.stun_configured,
            turn_configured: value.turn_configured,
            secure_turn_configured: value.secure_turn_configured,
            turn_transports: value.turn_transports,
            native_udp_range: value.native_udp_range,
            native_udp_range_enforced: value.native_udp_range.is_some(),
            ice_transport_policy: Some(value.ice_transport_policy),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum HostRequirement {
    UseConfiguredTurnRelay,
    ForwardDeclaredUdpRangeOrProvideTurn,
    DiscoverDynamicUdpOrProvideTurn,
}

impl From<&NativeIceCapability> for HostRequirement {
    fn from(value: &NativeIceCapability) -> Self {
        if value.turn_configured {
            Self::UseConfiguredTurnRelay
        } else if value.native_udp_range.is_some() {
            Self::ForwardDeclaredUdpRangeOrProvideTurn
        } else {
            Self::DiscoverDynamicUdpOrProvideTurn
        }
    }
}
