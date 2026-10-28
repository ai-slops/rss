use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
};

#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

use serde::Deserialize;

use crate::preview_network::{
    IceTransportPolicy, PreviewNetworkError, TurnTransport, UdpPortRange,
};

const MAX_CONFIG_BYTES: u64 = 65_536;
const MAX_CONFIG_READ: usize = 65_537;
const MAX_TURN_SERVERS: usize = 8;

#[cfg(target_os = "linux")]
const O_NOFOLLOW: i32 = 0o400_000;

pub struct ParsedPreviewNetwork {
    pub stun_server: Option<String>,
    pub turn_servers: Vec<TurnServer>,
    pub native_udp_range: Option<UdpPortRange>,
    pub ice_transport_policy: IceTransportPolicy,
}

#[derive(Clone)]
pub struct TurnServer {
    uri: String,
    secure: bool,
    transport: TurnTransport,
}

impl TurnServer {
    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn transport(&self) -> TurnTransport {
        self.transport
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn parse(value: TurnServerFile) -> Result<Self, PreviewNetworkError> {
        if value.host.is_empty()
            || value
                .host
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'@' | b'/' | b'?' | b'#'))
            || value.username.is_empty()
            || value.credential.is_empty()
        {
            return Err(PreviewNetworkError::Invalid);
        }
        let secure = value.scheme == TurnScheme::Turns;
        if secure && value.transport != TurnTransport::Tcp {
            return Err(PreviewNetworkError::Invalid);
        }
        let host = if value.host.contains(':') {
            format!("[{}]", value.host)
        } else {
            value.host
        };
        let uri = format!(
            "{}://{}:{}@{}:{}?transport={}",
            value.scheme.as_str(),
            encode_userinfo(&value.username),
            encode_userinfo(&value.credential),
            host,
            value.port,
            value.transport.as_str(),
        );
        Ok(Self {
            uri,
            secure,
            transport: value.transport,
        })
    }
}

pub fn load(
    path: Option<&Path>,
    legacy_stun: Option<String>,
) -> Result<ParsedPreviewNetwork, PreviewNetworkError> {
    let file = path.map(load_file).transpose()?.unwrap_or_default();
    let stun_server = file.stun_server.or(legacy_stun);
    if stun_server.as_ref().is_some_and(|value| !valid_stun(value))
        || file.turn_servers.len() > MAX_TURN_SERVERS
    {
        return Err(PreviewNetworkError::Invalid);
    }
    let turn_servers = file
        .turn_servers
        .into_iter()
        .map(TurnServer::parse)
        .collect::<Result<Vec<_>, _>>()?;
    let native_udp_range = file.native_udp_range.map(UdpPortRange::parse).transpose()?;
    Ok(ParsedPreviewNetwork {
        stun_server,
        turn_servers,
        native_udp_range,
        ice_transport_policy: file.ice_transport_policy.unwrap_or(IceTransportPolicy::All),
    })
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewNetworkFile {
    stun_server: Option<String>,
    #[serde(default)]
    turn_servers: Vec<TurnServerFile>,
    native_udp_range: Option<UdpPortRangeFile>,
    ice_transport_policy: Option<IceTransportPolicy>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TurnServerFile {
    scheme: TurnScheme,
    host: String,
    port: u16,
    transport: TurnTransport,
    username: String,
    credential: String,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TurnScheme {
    Turn,
    Turns,
}

impl TurnScheme {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Turns => "turns",
        }
    }
}

impl<'de> Deserialize<'de> for IceTransportPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match <&str>::deserialize(deserializer)? {
            "all" => Ok(Self::All),
            "relay" => Ok(Self::Relay),
            _ => Err(serde::de::Error::custom("invalid ICE transport policy")),
        }
    }
}

impl<'de> Deserialize<'de> for TurnTransport {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match <&str>::deserialize(deserializer)? {
            "udp" => Ok(Self::Udp),
            "tcp" => Ok(Self::Tcp),
            _ => Err(serde::de::Error::custom("invalid TURN transport")),
        }
    }
}

impl TurnTransport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct UdpPortRangeFile {
    min: u16,
    max: u16,
}

impl UdpPortRange {
    const fn parse(value: UdpPortRangeFile) -> Result<Self, PreviewNetworkError> {
        if value.min == 0 || value.min > value.max {
            return Err(PreviewNetworkError::Invalid);
        }
        Ok(Self {
            min: value.min,
            max: value.max,
        })
    }
}

fn load_file(path: &Path) -> Result<PreviewNetworkFile, PreviewNetworkError> {
    let file = open_private(path)?;
    let metadata = file.metadata().map_err(PreviewNetworkError::Read)?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG_BYTES || !private_permissions(&metadata) {
        return Err(PreviewNetworkError::Insecure);
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(PreviewNetworkError::Read)?;
    if bytes.len() >= MAX_CONFIG_READ {
        return Err(PreviewNetworkError::Insecure);
    }
    serde_json::from_slice(&bytes).map_err(|_| PreviewNetworkError::Invalid)
}

#[cfg(target_os = "linux")]
fn open_private(path: &Path) -> Result<File, PreviewNetworkError> {
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(path)
        .map_err(PreviewNetworkError::Read)
}

#[cfg(not(target_os = "linux"))]
fn open_private(_path: &Path) -> Result<File, PreviewNetworkError> {
    Err(PreviewNetworkError::Insecure)
}

#[cfg(unix)]
fn private_permissions(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode().trailing_zeros() >= 6
}

#[cfg(not(unix))]
const fn private_permissions(_metadata: &fs::Metadata) -> bool {
    true
}

fn valid_stun(value: &str) -> bool {
    value.starts_with("stun://")
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b'@')
}

fn encode_userinfo(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}
