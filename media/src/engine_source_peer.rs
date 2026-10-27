use std::{fmt, net::SocketAddr};

use gio::prelude::*;
use gstreamer::glib::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerKey(SocketAddr);

impl PeerKey {
    pub fn from_signal(values: &[Value], index: usize) -> Option<Self> {
        let value = values.get(index)?;
        let address = value
            .get::<gio::SocketAddress>()
            .ok()
            .or_else(|| value.get::<Option<gio::SocketAddress>>().ok().flatten())?;
        let inet = address.downcast::<gio::InetSocketAddress>().ok()?;
        Some(Self(inet.into()))
    }

    #[cfg(test)]
    pub fn test(value: &str) -> Result<Self, std::net::AddrParseError> {
        value.parse().map(Self)
    }
}

impl fmt::Display for PeerKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
