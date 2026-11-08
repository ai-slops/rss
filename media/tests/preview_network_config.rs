use std::{fs, path::Path};

use live_production_media::preview_network::{IceTransportPolicy, PreviewNetworkConfig};
use tempfile::TempDir;

#[test]
fn private_config_parses_without_exposing_turn_credentials()
-> Result<(), Box<dyn std::error::Error>> {
    // Given
    let directory = TempDir::new()?;
    let path = directory.path().join("preview-network.json");
    write_private(
        &path,
        r#"{
            "stunServer":"stun://stun.example.test:3478",
            "turnServers":[{
                "scheme":"turns",
                "host":"relay.example.test",
                "port":5349,
                "transport":"tcp",
                "username":"operator:name",
                "credential":"secret+/="
            }],
            "nativeUdpRange":{"min":49160,"max":49200},
            "iceTransportPolicy":"relay"
        }"#,
    )?;

    // When
    let config = PreviewNetworkConfig::load(Some(&path), None)?;
    let capability = config.capability();

    // Then
    assert!(capability.stun_configured);
    assert!(capability.turn_configured);
    assert!(capability.secure_turn_configured);
    assert_eq!(
        capability
            .native_udp_range
            .map(|range| (range.min, range.max)),
        Some((49_160, 49_200))
    );
    assert_eq!(capability.ice_transport_policy, IceTransportPolicy::Relay);
    let serialized = serde_json::to_string(&capability)?;
    assert!(!serialized.contains("operator"));
    assert!(!serialized.contains("secret"));
    let debug = format!("{config:?}");
    assert!(!debug.contains("operator"));
    assert!(!debug.contains("secret"));
    Ok(())
}

#[test]
fn permissive_private_config_is_rejected_generically() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let directory = TempDir::new()?;
    let path = directory.path().join("preview-network.json");
    fs::write(
        &path,
        r#"{"stunServer":"stun://private.example.test:3478"}"#,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
    }

    // When
    let error =
        PreviewNetworkConfig::load(Some(&path), None).expect_err("insecure config must fail");

    // Then
    let message = error.to_string();
    assert!(!message.contains(path.to_string_lossy().as_ref()));
    assert!(!message.contains("private.example.test"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_to_private_config_is_rejected_without_secret_disclosure()
-> Result<(), Box<dyn std::error::Error>> {
    // Given
    use std::os::unix::fs::symlink;

    let directory = TempDir::new()?;
    let target = directory.path().join("private-target.json");
    let link = directory.path().join("preview-network.json");
    write_private(
        &target,
        r#"{"turnServers":[{"scheme":"turn","host":"relay.example.test","port":3478,"transport":"udp","username":"private-user","credential":"private-secret"}]}"#,
    )?;
    symlink(&target, &link)?;

    // When
    let error = PreviewNetworkConfig::load(Some(&link), None).expect_err("symlink must fail");

    // Then
    let message = error.to_string();
    assert!(!message.contains(link.to_string_lossy().as_ref()));
    assert!(!message.contains("private-user"));
    assert!(!message.contains("private-secret"));
    Ok(())
}

#[test]
fn legacy_stun_environment_remains_supported_without_a_config_file()
-> Result<(), Box<dyn std::error::Error>> {
    // Given / When
    let config = PreviewNetworkConfig::load(None, Some("stun://legacy.example.test:3478".into()))?;

    // Then
    assert!(config.capability().stun_configured);
    assert!(!config.capability().turn_configured);
    Ok(())
}

fn write_private(path: &Path, contents: &str) -> Result<(), std::io::Error> {
    fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
