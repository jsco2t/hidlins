//! Untrusted local-service discovery policy and platform-neutral port.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::IpAddr,
    str::FromStr,
    time::{Duration, Instant},
};

use crate::{
    address::{AddressError, LocalEndpoint},
    protocol::{MAX_DISCOVERY_ENDPOINTS, PAIRING_WINDOW},
};

#[cfg(any(
    all(
        feature = "desktop-discovery",
        any(target_os = "macos", target_os = "linux")
    ),
    all(feature = "android-scenario-authority", target_os = "android")
))]
pub mod desktop;

/// Normal trusted-server DNS-SD service type.
pub const TRUSTED_SERVICE_TYPE: &str = "_hidlins-sync._tcp.local.";
/// Explicit pairing-window DNS-SD service type.
pub const PAIRING_SERVICE_TYPE: &str = "_hidlins-pair._tcp.local.";
/// The complete V1 TXT record. No other key is valid.
pub const PROTOCOL_TXT: &[(&str, &str)] = &[("v", "1")];
/// Conservative lifetime for a desktop event whose backend omits source TTL.
pub const DESKTOP_CANDIDATE_TTL: Duration = Duration::from_secs(120);
/// Untrusted TTLs are clamped so one record cannot live indefinitely.
pub const MAX_CANDIDATE_TTL: Duration = Duration::from_secs(4_500);

const INSTANCE_LABEL_LEN: usize = 26;
const INSTANCE_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Discovery failures are deliberately free of attacker-controlled input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DiscoveryError {
    /// A service record does not match the exact V1 metadata contract.
    #[error("invalid local discovery record")]
    InvalidRecord,
    /// No address in a resolved record passed the local-only policy.
    #[error("discovery record has no allowed local endpoint")]
    NoAllowedEndpoint,
    /// The fixed discovery cache or event-batch limit was exceeded.
    #[error("local discovery resource limit exceeded")]
    Capacity,
    /// The user has not yet answered the platform permission prompt.
    #[error("local-network discovery permission is required")]
    PermissionRequired,
    /// The platform denied or restricted local-network discovery.
    #[error("local-network discovery permission was denied")]
    PermissionDenied,
    /// Discovery is unavailable on this platform or process.
    #[error("local-network discovery is unavailable")]
    Unavailable,
    /// The OS-backed desktop discovery service failed.
    #[error("local-network discovery backend failed")]
    Backend,
    /// Secure random generation failed.
    #[error("local-network discovery entropy unavailable")]
    EntropyUnavailable,
    /// The discovery source has already stopped.
    #[error("local-network discovery has stopped")]
    Stopped,
}

/// V1 service categories. Neither carries identity or authorization data.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ServiceKind {
    /// A normally available, Noise-IK-authenticated sync server.
    Trusted,
    /// A server inside an explicit Noise-XX pairing window.
    Pairing,
}

impl ServiceKind {
    /// Return the exact DNS-SD service type.
    #[must_use]
    pub const fn service_type(self) -> &'static str {
        match self {
            Self::Trusted => TRUSTED_SERVICE_TYPE,
            Self::Pairing => PAIRING_SERVICE_TYPE,
        }
    }

    fn parse(value: &str) -> Result<Self, DiscoveryError> {
        match value {
            TRUSTED_SERVICE_TYPE => Ok(Self::Trusted),
            PAIRING_SERVICE_TYPE => Ok(Self::Pairing),
            _ => Err(DiscoveryError::InvalidRecord),
        }
    }
}

/// One untrusted address returned by a desktop or native discovery backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawEndpoint {
    ip: IpAddr,
    scope_id: u32,
}

impl RawEndpoint {
    /// Construct an untrusted address/scope pair for policy validation.
    #[must_use]
    pub const fn new(ip: IpAddr, scope_id: u32) -> Self {
        Self { ip, scope_id }
    }
}

/// One complete but untrusted resolved DNS-SD service record.
#[derive(Clone, Eq, PartialEq)]
pub struct RawDiscoveryRecord {
    service_type: String,
    fullname: String,
    port: u16,
    txt: Vec<(String, Vec<u8>)>,
    endpoints: Vec<RawEndpoint>,
    ttl: Duration,
}

impl RawDiscoveryRecord {
    /// Capture native/backend input behind fixed allocation boundaries.
    pub fn try_from_untrusted(
        service_type: String,
        fullname: String,
        port: u16,
        txt: Vec<(String, Vec<u8>)>,
        endpoints: Vec<RawEndpoint>,
        ttl: Duration,
    ) -> Result<Self, DiscoveryError> {
        if service_type.len() > 64
            || fullname.len() > 96
            || txt.len() > 4
            || txt
                .iter()
                .any(|(key, value)| key.len() > 32 || value.len() > 64)
            || endpoints.len() > MAX_DISCOVERY_ENDPOINTS
        {
            return Err(DiscoveryError::Capacity);
        }
        Ok(Self {
            service_type,
            fullname,
            port,
            txt,
            endpoints,
            ttl,
        })
    }
}

impl fmt::Debug for RawDiscoveryRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RawDiscoveryRecord([UNTRUSTED REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RecordKey {
    kind: ServiceKind,
    label: String,
}

#[derive(Clone)]
struct CachedRecord {
    endpoints: BTreeSet<LocalEndpoint>,
    expires_at: Instant,
}

/// Bounded, expiring routing candidates. Possession never implies trust.
#[derive(Clone, Default)]
pub struct CandidateCache {
    records: BTreeMap<RecordKey, CachedRecord>,
}

impl CandidateCache {
    /// Create an empty candidate cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Validate and atomically apply one resolved untrusted record.
    pub fn apply_resolved(
        &mut self,
        record: RawDiscoveryRecord,
        now: Instant,
    ) -> Result<(), DiscoveryError> {
        if record.endpoints.is_empty()
            || record.endpoints.len() > MAX_DISCOVERY_ENDPOINTS
            || record.txt.len() != 1
            || record.txt[0].0 != "v"
            || record.txt[0].1.as_slice() != b"1"
            || record.ttl.is_zero()
        {
            return Err(DiscoveryError::InvalidRecord);
        }

        let kind = ServiceKind::parse(&record.service_type)?;
        let label = parse_fullname(&record.fullname, kind)?;
        let mut endpoints = BTreeSet::new();
        for candidate in record.endpoints {
            if let Ok(endpoint) = LocalEndpoint::new(candidate.ip, record.port, candidate.scope_id)
            {
                endpoints.insert(endpoint);
            }
        }
        if endpoints.is_empty() {
            return Err(DiscoveryError::NoAllowedEndpoint);
        }

        let ttl = record.ttl.min(MAX_CANDIDATE_TTL);
        let expires_at = now.checked_add(ttl).ok_or(DiscoveryError::InvalidRecord)?;
        let mut updated = self.clone();
        updated.purge_expired(now);
        updated.records.insert(
            RecordKey { kind, label },
            CachedRecord {
                endpoints,
                expires_at,
            },
        );
        if updated.records.len() > MAX_DISCOVERY_ENDPOINTS
            || updated.all_candidate_count() > MAX_DISCOVERY_ENDPOINTS
        {
            return Err(DiscoveryError::Capacity);
        }
        *self = updated;
        Ok(())
    }

    /// Remove the exact service instance named by a backend expiry/removal event.
    pub fn remove(&mut self, service_type: &str, fullname: &str) {
        let Ok(kind) = ServiceKind::parse(service_type) else {
            return;
        };
        let Ok(label) = parse_fullname(fullname, kind) else {
            return;
        };
        self.records.remove(&RecordKey { kind, label });
    }

    /// Evict every record whose monotonic lifetime has ended.
    pub fn purge_expired(&mut self, now: Instant) {
        self.records.retain(|_, record| record.expires_at > now);
    }

    /// Return sorted, deduplicated, policy-approved candidates for one operation.
    #[must_use]
    pub fn candidates(&self, kind: ServiceKind) -> Vec<LocalEndpoint> {
        self.records
            .iter()
            .filter(|(key, _)| key.kind == kind)
            .flat_map(|(_, record)| record.endpoints.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn all_candidate_count(&self) -> usize {
        self.records
            .values()
            .flat_map(|record| record.endpoints.iter().copied())
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// Parse the deliberately restricted manual IP-literal fallback.
pub fn manual_endpoint(input: &str) -> Result<LocalEndpoint, AddressError> {
    LocalEndpoint::from_str(input)
}

/// Metadata-minimal advertisement description shared by simulation and desktop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Advertisement {
    kind: ServiceKind,
    instance_label: String,
    hostname: String,
    port: u16,
}

impl Advertisement {
    /// Return the advertised service kind.
    #[must_use]
    pub const fn kind(&self) -> ServiceKind {
        self.kind
    }

    /// Return the exact service type.
    #[must_use]
    pub const fn service_type(&self) -> &'static str {
        self.kind.service_type()
    }

    /// Return the per-process random instance label.
    #[must_use]
    pub fn instance_label(&self) -> &str {
        &self.instance_label
    }

    /// Return the equally ephemeral hostname, never a device name.
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    /// Return the nonzero TCP listener port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Return the complete fixed V1 TXT payload.
    #[must_use]
    pub const fn txt(&self) -> &'static [(&'static str, &'static str)] {
        PROTOCOL_TXT
    }

    /// Return the exact full DNS-SD instance name.
    #[must_use]
    pub fn fullname(&self) -> String {
        format!("{}.{}", self.instance_label, self.kind.service_type())
    }
}

/// Process-lifetime advertisement policy, independent of a network backend.
pub struct AdvertisementSet {
    instance_label: String,
    hostname: String,
    port: u16,
    running: bool,
    pairing_until: Option<Instant>,
}

impl AdvertisementSet {
    /// Start a server with a fresh CSPRNG instance label.
    pub fn start(port: u16) -> Result<Self, DiscoveryError> {
        let mut entropy = [0_u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| DiscoveryError::EntropyUnavailable)?;
        Self::from_entropy(port, entropy)
    }

    /// Deterministic construction for the simulated discovery environment.
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn start_with_entropy(port: u16, entropy: [u8; 16]) -> Result<Self, DiscoveryError> {
        Self::from_entropy(port, entropy)
    }

    fn from_entropy(port: u16, entropy: [u8; 16]) -> Result<Self, DiscoveryError> {
        if port == 0 {
            return Err(DiscoveryError::InvalidRecord);
        }
        let instance_label = encode_instance_label(entropy);
        let hostname = format!("{instance_label}.local.");
        Ok(Self {
            instance_label,
            hostname,
            port,
            running: true,
            pairing_until: None,
        })
    }

    /// Return the randomized label shared by this start's service records.
    #[must_use]
    pub fn instance_label(&self) -> &str {
        &self.instance_label
    }

    /// Open or restart the explicit three-minute pairing advertisement.
    pub fn open_pairing_at(&mut self, now: Instant) -> Result<(), DiscoveryError> {
        if !self.running {
            return Err(DiscoveryError::Stopped);
        }
        self.pairing_until = Some(
            now.checked_add(PAIRING_WINDOW)
                .ok_or(DiscoveryError::InvalidRecord)?,
        );
        Ok(())
    }

    /// Withdraw only the pairing-window advertisement.
    pub fn close_pairing(&mut self) {
        self.pairing_until = None;
    }

    /// Return currently permitted advertisements and expire pairing if needed.
    pub fn active(&mut self, now: Instant) -> Vec<Advertisement> {
        if !self.running {
            return Vec::new();
        }
        if self.pairing_until.is_some_and(|deadline| now >= deadline) {
            self.pairing_until = None;
        }
        let mut records = vec![self.record(ServiceKind::Trusted)];
        if self.pairing_until.is_some() {
            records.push(self.record(ServiceKind::Pairing));
        }
        records
    }

    /// Withdraw every advertisement when the vault locks.
    pub fn lock(&mut self) {
        self.stop();
    }

    /// Withdraw every advertisement when service operation stops.
    pub fn stop(&mut self) {
        self.running = false;
        self.pairing_until = None;
    }

    fn record(&self, kind: ServiceKind) -> Advertisement {
        Advertisement {
            kind,
            instance_label: self.instance_label.clone(),
            hostname: self.hostname.clone(),
            port: self.port,
        }
    }
}

/// Native local-network permission state surfaced without platform details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryPermission {
    /// The platform has not yet received a user decision.
    NotDetermined,
    /// Discovery is currently allowed.
    Granted,
    /// The user denied local-network discovery.
    Denied,
    /// Device policy restricts local-network discovery.
    Restricted,
    /// The platform cannot provide local discovery.
    Unavailable,
}

/// One platform-neutral discovery update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryEvent {
    /// A complete service resolution replaces the same instance's routes.
    Resolved(RawDiscoveryRecord),
    /// An expiry/removal withdraws the exact service instance.
    Removed {
        /// Exact service type supplied by the backend.
        service_type: String,
        /// Exact full instance name supplied by the backend.
        fullname: String,
    },
}

/// Bounded result of one native or desktop discovery poll.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryBatch {
    permission: DiscoveryPermission,
    events: Vec<DiscoveryEvent>,
}

impl DiscoveryBatch {
    /// Enforce the boundary before events enter the core cache.
    pub fn new(
        permission: DiscoveryPermission,
        events: Vec<DiscoveryEvent>,
    ) -> Result<Self, DiscoveryError> {
        if events.len() > MAX_DISCOVERY_ENDPOINTS {
            return Err(DiscoveryError::Capacity);
        }
        Ok(Self { permission, events })
    }

    fn apply(self, cache: &mut CandidateCache, now: Instant) -> Result<(), DiscoveryError> {
        match self.permission {
            DiscoveryPermission::Granted => {}
            DiscoveryPermission::NotDetermined => return Err(DiscoveryError::PermissionRequired),
            DiscoveryPermission::Denied | DiscoveryPermission::Restricted => {
                return Err(DiscoveryError::PermissionDenied);
            }
            DiscoveryPermission::Unavailable => return Err(DiscoveryError::Unavailable),
        }

        let mut updated = cache.clone();
        for event in self.events {
            match event {
                DiscoveryEvent::Resolved(record) => match updated.apply_resolved(record, now) {
                    Ok(())
                    | Err(DiscoveryError::InvalidRecord | DiscoveryError::NoAllowedEndpoint) => {}
                    Err(error) => return Err(error),
                },
                DiscoveryEvent::Removed {
                    service_type,
                    fullname,
                } => updated.remove(&service_type, &fullname),
            }
        }
        updated.purge_expired(now);
        *cache = updated;
        Ok(())
    }
}

/// Narrow boundary implemented by native mobile and desktop adapters.
pub trait DiscoveryPort {
    /// Return one already-bounded batch without authorizing its contents.
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError>;

    /// Stop callbacks, browsing, and advertisements owned by the adapter.
    fn shutdown(&mut self) -> Result<(), DiscoveryError>;
}

/// Poll one adapter and atomically apply the batch after permission checks.
pub fn poll_into(
    port: &mut (impl DiscoveryPort + ?Sized),
    cache: &mut CandidateCache,
    now: Instant,
) -> Result<(), DiscoveryError> {
    port.poll()?.apply(cache, now)
}

/// Deterministic native-port stand-in for security and lifecycle tests.
#[cfg(any(test, feature = "test-helpers"))]
pub struct SimulatedDiscoveryPort {
    batches: std::collections::VecDeque<DiscoveryBatch>,
    stopped: bool,
}

#[cfg(any(test, feature = "test-helpers"))]
impl SimulatedDiscoveryPort {
    /// Queue deterministic batches in poll order.
    #[must_use]
    pub fn new(batches: Vec<DiscoveryBatch>) -> Self {
        Self {
            batches: batches.into(),
            stopped: false,
        }
    }

    /// Apply the next batch through the same production boundary.
    pub fn poll_into(
        &mut self,
        cache: &mut CandidateCache,
        now: Instant,
    ) -> Result<(), DiscoveryError> {
        poll_into(self, cache, now)
    }
}

#[cfg(any(test, feature = "test-helpers"))]
impl DiscoveryPort for SimulatedDiscoveryPort {
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
        if self.stopped {
            return Err(DiscoveryError::Stopped);
        }
        self.batches.pop_front().ok_or(DiscoveryError::Unavailable)
    }

    fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        self.stopped = true;
        self.batches.clear();
        Ok(())
    }
}

fn parse_fullname(fullname: &str, kind: ServiceKind) -> Result<String, DiscoveryError> {
    let suffix = kind.service_type();
    let label = fullname
        .strip_suffix(suffix)
        .and_then(|prefix| prefix.strip_suffix('.'))
        .ok_or(DiscoveryError::InvalidRecord)?;
    if label.len() != INSTANCE_LABEL_LEN
        || !label.bytes().all(|byte| INSTANCE_ALPHABET.contains(&byte))
    {
        return Err(DiscoveryError::InvalidRecord);
    }
    Ok(label.to_string())
}

fn encode_instance_label(entropy: [u8; 16]) -> String {
    let mut value = u128::from_be_bytes(entropy);
    let mut encoded = [b'0'; INSTANCE_LABEL_LEN];
    for slot in encoded.iter_mut().rev() {
        *slot = INSTANCE_ALPHABET[(value & 0x1f) as usize];
        value >>= 5;
    }
    String::from_utf8(encoded.to_vec()).expect("Crockford alphabet is ASCII")
}
