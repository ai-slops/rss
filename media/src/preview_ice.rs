use gst::prelude::*;
use gstreamer as gst;

use crate::preview_network::{
    IceTransportPolicy, PreviewNetworkConfig, PreviewNetworkError, UdpPortRange,
};

pub fn configure_sink(
    sink: &gst::Element,
    config: &PreviewNetworkConfig,
) -> Result<(), PreviewNetworkError> {
    sink.set_property("stun-server", config.stun_server().map(str::to_owned));
    sink.set_property(
        "turn-servers",
        gst::Array::new(config.turn_server_uris().map(str::to_owned)),
    );
    sink.set_property_from_str(
        "ice-transport-policy",
        match config.ice_transport_policy() {
            IceTransportPolicy::All => "all",
            IceTransportPolicy::Relay => "relay",
        },
    );
    if let Some(range) = config.native_udp_range() {
        validate_udp_range_support(range)?;
        connect_udp_range(sink, range);
    }
    Ok(())
}

fn validate_udp_range_support(range: UdpPortRange) -> Result<(), PreviewNetworkError> {
    let webrtcbin = gst::ElementFactory::make("webrtcbin")
        .build()
        .map_err(|_| PreviewNetworkError::UnsupportedUdpRange)?;
    apply_udp_range(&webrtcbin, range)
}

fn connect_udp_range(sink: &gst::Element, range: UdpPortRange) {
    sink.connect("consumer-added", false, move |values| {
        if let Some(webrtcbin) = values
            .get(2)
            .and_then(|value| value.get::<gst::Element>().ok())
            && apply_udp_range(&webrtcbin, range).is_err()
        {
            let _ = webrtcbin.set_state(gst::State::Null);
        }
        None
    });
}

fn apply_udp_range(
    webrtcbin: &gst::Element,
    range: UdpPortRange,
) -> Result<(), PreviewNetworkError> {
    let agent = webrtcbin.property::<gst::glib::Object>("ice-agent");
    if agent.find_property("min-rtp-port").is_none()
        || agent.find_property("max-rtp-port").is_none()
    {
        return Err(PreviewNetworkError::UnsupportedUdpRange);
    }
    agent.set_property("min-rtp-port", u32::from(range.min));
    agent.set_property("max-rtp-port", u32::from(range.max));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use gstreamer::prelude::*;
    use tempfile::TempDir;

    use crate::preview_network::PreviewNetworkConfig;

    use super::{UdpPortRange, configure_sink, validate_udp_range_support};

    #[test]
    fn installed_ice_agent_accepts_a_fixed_udp_range() -> Result<(), Box<dyn std::error::Error>> {
        // Given
        gstreamer::init()?;
        let range = UdpPortRange {
            min: 49_160,
            max: 49_200,
        };

        // When / Then
        validate_udp_range_support(range)?;
        Ok(())
    }

    #[test]
    fn installed_sink_accepts_private_stun_turn_and_relay_policy()
    -> Result<(), Box<dyn std::error::Error>> {
        // Given
        gstreamer::init()?;
        let directory = TempDir::new()?;
        let path = directory.path().join("preview-network.json");
        fs::write(
            &path,
            r#"{"stunServer":"stun://stun.example.test:3478","turnServers":[{"scheme":"turns","host":"relay.example.test","port":5349,"transport":"tcp","username":"test","credential":"private"}],"iceTransportPolicy":"relay"}"#,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        let config = PreviewNetworkConfig::load(Some(&path), None)?;
        let sink = gstreamer::ElementFactory::make("webrtcsink").build()?;

        // When
        configure_sink(&sink, &config)?;

        // Then
        assert_eq!(
            sink.property::<Option<String>>("stun-server").as_deref(),
            Some("stun://stun.example.test:3478")
        );
        assert_eq!(sink.property::<gstreamer::Array>("turn-servers").len(), 1);
        Ok(())
    }
}
