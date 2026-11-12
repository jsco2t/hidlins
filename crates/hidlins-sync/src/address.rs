//! Non-bypassable local-address policy for sync and discovery sockets.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
};

/// Failure to construct a canonical local endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AddressError {
    /// The input is not an unambiguous IP-literal socket address.
    #[error("invalid local endpoint")]
    InvalidLiteral,
    /// Port zero cannot identify a sync service.
    #[error("invalid local endpoint port")]
    InvalidPort,
    /// The address is outside the fixed local-only allowlist.
    #[error("endpoint is outside the local address policy")]
    NotLocal,
    /// An IPv6 scope is missing, forbidden, or malformed.
    #[error("invalid IPv6 interface scope")]
    InvalidScope,
}

/// A canonical, policy-approved sync socket endpoint.
///
/// Construction is private to the checked constructors, so later network
/// paths cannot accidentally carry a public or malformed socket address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalEndpoint(SocketAddr);

impl LocalEndpoint {
    /// Validate and canonicalize an IP address, port, and numeric IPv6 scope.
    pub fn new(ip: IpAddr, port: u16, scope_id: u32) -> Result<Self, AddressError> {
        if port == 0 {
            return Err(AddressError::InvalidPort);
        }

        match ip {
            IpAddr::V4(ip) => {
                if scope_id != 0 {
                    return Err(AddressError::InvalidScope);
                }
                if !allowed_v4(ip) {
                    return Err(AddressError::NotLocal);
                }
                Ok(Self(SocketAddr::V4(SocketAddrV4::new(ip, port))))
            }
            IpAddr::V6(ip) => {
                if let Some(mapped) = ip.to_ipv4_mapped() {
                    if scope_id != 0 {
                        return Err(AddressError::InvalidScope);
                    }
                    return Self::new(IpAddr::V4(mapped), port, 0);
                }

                let link_local = is_ipv6_link_local(ip);
                if (link_local && scope_id == 0) || (!link_local && scope_id != 0) {
                    return Err(AddressError::InvalidScope);
                }
                if !(is_ipv6_unique_local(ip) || link_local || ip.is_loopback()) {
                    return Err(AddressError::NotLocal);
                }
                Ok(Self(SocketAddr::V6(SocketAddrV6::new(
                    ip, port, 0, scope_id,
                ))))
            }
        }
    }

    /// Return the normalized IP address.
    #[must_use]
    pub const fn ip(self) -> IpAddr {
        self.0.ip()
    }

    /// Return the service port.
    #[must_use]
    pub const fn port(self) -> u16 {
        self.0.port()
    }

    /// Return the numeric IPv6 interface scope, or zero for unscoped forms.
    #[must_use]
    pub const fn scope_id(self) -> u32 {
        match self.0 {
            SocketAddr::V4(_) => 0,
            SocketAddr::V6(address) => address.scope_id(),
        }
    }

    /// Return the validated standard-library socket address.
    #[must_use]
    pub const fn socket_addr(self) -> SocketAddr {
        self.0
    }
}

/// A policy-approved interface address reserved for `bind(2)` with an
/// operating-system-assigned port.
///
/// This type is intentionally neither serializable nor parseable and cannot
/// be used as a client destination. Port zero exists only at this narrow bind
/// boundary; a successfully bound listener must be converted back into a
/// nonzero [`LocalEndpoint`] before it is advertised or returned.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LocalBindEndpoint(SocketAddr);

impl LocalBindEndpoint {
    /// Validate a local interface address and construct an ephemeral bind
    /// request for it.
    pub fn ephemeral(ip: IpAddr, scope_id: u32) -> Result<Self, AddressError> {
        let checked = LocalEndpoint::new(ip, 1, scope_id)?;
        let mut address = checked.socket_addr();
        address.set_port(0);
        Ok(Self(address))
    }

    /// Return the policy-approved socket address for `TcpListener::bind`.
    #[must_use]
    pub const fn socket_addr(self) -> SocketAddr {
        self.0
    }

    /// Return the validated interface address.
    #[must_use]
    pub const fn ip(self) -> IpAddr {
        self.0.ip()
    }
}

impl fmt::Display for LocalEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for LocalEndpoint {
    type Err = AddressError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.starts_with('[') {
            parse_ipv6_endpoint(input)
        } else {
            parse_ipv4_endpoint(input)
        }
    }
}

impl Serialize for LocalEndpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LocalEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let literal = String::deserialize(deserializer)?;
        literal.parse().map_err(serde::de::Error::custom)
    }
}

fn parse_ipv4_endpoint(input: &str) -> Result<LocalEndpoint, AddressError> {
    let (ip, port) = input.rsplit_once(':').ok_or(AddressError::InvalidLiteral)?;
    if ip.contains(':') {
        return Err(AddressError::InvalidLiteral);
    }
    LocalEndpoint::new(
        IpAddr::V4(ip.parse().map_err(|_| AddressError::InvalidLiteral)?),
        parse_canonical_u16(port)?,
        0,
    )
}

fn parse_ipv6_endpoint(input: &str) -> Result<LocalEndpoint, AddressError> {
    let closing = input.find(']').ok_or(AddressError::InvalidLiteral)?;
    let port = input
        .get(closing + 1..)
        .and_then(|suffix| suffix.strip_prefix(':'))
        .ok_or(AddressError::InvalidLiteral)?;
    let inside = input.get(1..closing).ok_or(AddressError::InvalidLiteral)?;
    let (ip, scope) = if let Some((ip, scope)) = inside.rsplit_once('%') {
        (ip, parse_canonical_u32(scope)?)
    } else {
        (inside, 0)
    };
    LocalEndpoint::new(
        IpAddr::V6(ip.parse().map_err(|_| AddressError::InvalidLiteral)?),
        parse_canonical_u16(port)?,
        scope,
    )
}

fn parse_canonical_u16(input: &str) -> Result<u16, AddressError> {
    if input.is_empty()
        || !input.bytes().all(|byte| byte.is_ascii_digit())
        || (input.len() > 1 && input.starts_with('0'))
    {
        return Err(AddressError::InvalidLiteral);
    }
    input.parse().map_err(|_| AddressError::InvalidLiteral)
}

fn parse_canonical_u32(input: &str) -> Result<u32, AddressError> {
    if input.is_empty()
        || !input.bytes().all(|byte| byte.is_ascii_digit())
        || (input.len() > 1 && input.starts_with('0'))
    {
        return Err(AddressError::InvalidScope);
    }
    input.parse().map_err(|_| AddressError::InvalidScope)
}

const fn allowed_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 10
        || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 169 && octets[1] == 254)
        || octets[0] == 127
}

const fn is_ipv6_unique_local(ip: Ipv6Addr) -> bool {
    ip.octets()[0] & 0xfe == 0xfc
}

const fn is_ipv6_link_local(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xffc0 == 0xfe80
}

/// A multicast destination usable only by the discovery adapter.
///
/// This type deliberately cannot be converted into [`LocalEndpoint`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscoveryDestination(SocketAddr);

impl DiscoveryDestination {
    /// Return the standard IPv4 mDNS multicast destination.
    #[must_use]
    pub const fn mdns_v4() -> Self {
        Self(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(224, 0, 0, 251),
            5353,
        )))
    }

    /// Return the link-scoped IPv6 mDNS multicast destination.
    pub fn mdns_v6(scope_id: u32) -> Result<Self, AddressError> {
        if scope_id == 0 {
            return Err(AddressError::InvalidScope);
        }
        Ok(Self(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x00fb),
            5353,
            0,
            scope_id,
        ))))
    }

    /// Return the destination for a discovery-only datagram socket.
    #[must_use]
    pub const fn socket_addr(self) -> SocketAddr {
        self.0
    }
}
