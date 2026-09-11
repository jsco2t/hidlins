//! Desktop DNS-SD/mDNS adapter.
//!
//! `mdns-sd` is routing convenience only. Every event is converted back into
//! the platform-neutral untrusted DTO and revalidated by
//! [`CandidateCache`](crate::discovery::CandidateCache).

use std::{collections::BTreeSet, time::Instant};

#[cfg(target_os = "android")]
use android_if_addrs as if_addrs;
#[cfg(target_os = "android")]
use android_mdns_sd as mdns_sd;
use mdns_sd::{IfKind, IfPredicate, Receiver, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};

use super::{
    Advertisement, AdvertisementSet, DiscoveryBatch, DiscoveryError, DiscoveryEvent,
    DiscoveryPermission, DiscoveryPort, RawDiscoveryRecord, RawEndpoint, DESKTOP_CANDIDATE_TTL,
    MAX_DISCOVERY_ENDPOINTS, PAIRING_SERVICE_TYPE, PROTOCOL_TXT, TRUSTED_SERVICE_TYPE,
};
use crate::address::{LocalBindEndpoint, LocalEndpoint};

/// Enumerate active, policy-approved local interface addresses for binding.
/// Non-loopback IPv4 routes sort first; loopback remains a deterministic
/// offline/test fallback when no LAN interface is available.
pub fn allowed_interface_endpoints(port: u16) -> Result<Vec<LocalEndpoint>, DiscoveryError> {
    let mut endpoints = BTreeSet::new();
    for interface in if_addrs::get_if_addrs().map_err(|_| DiscoveryError::Backend)? {
        if !interface.is_oper_up() {
            continue;
        }
        let ip = interface.addr.ip();
        let scope_id = match ip {
            std::net::IpAddr::V6(value) if value.segments()[0] & 0xffc0 == 0xfe80 => {
                interface.index.unwrap_or_default()
            }
            _ => 0,
        };
        if let Ok(endpoint) = LocalEndpoint::new(ip, port, scope_id) {
            endpoints.insert(endpoint);
        }
    }
    let mut endpoints: Vec<_> = endpoints.into_iter().collect();
    endpoints.sort_by_key(|endpoint| {
        let loopback = endpoint.ip().is_loopback();
        let ipv6 = endpoint.ip().is_ipv6();
        (loopback, ipv6, *endpoint)
    });
    if endpoints.is_empty() {
        return Err(DiscoveryError::Unavailable);
    }
    Ok(endpoints)
}

/// Enumerate active, policy-approved interface addresses for an ephemeral
/// listener bind. These values cannot be serialized or used as destinations.
pub fn allowed_interface_bind_endpoints() -> Result<Vec<LocalBindEndpoint>, DiscoveryError> {
    let mut endpoints = BTreeSet::new();
    for interface in if_addrs::get_if_addrs().map_err(|_| DiscoveryError::Backend)? {
        if !interface.is_oper_up() {
            continue;
        }
        let ip = interface.addr.ip();
        let scope_id = match ip {
            std::net::IpAddr::V6(value) if value.segments()[0] & 0xffc0 == 0xfe80 => {
                interface.index.unwrap_or_default()
            }
            _ => 0,
        };
        if let Ok(endpoint) = LocalBindEndpoint::ephemeral(ip, scope_id) {
            endpoints.insert(endpoint);
        }
    }
    let mut endpoints: Vec<_> = endpoints.into_iter().collect();
    endpoints.sort_by_key(|endpoint| {
        let loopback = endpoint.ip().is_loopback();
        let ipv6 = endpoint.ip().is_ipv6();
        (loopback, ipv6, *endpoint)
    });
    if endpoints.is_empty() {
        return Err(DiscoveryError::Unavailable);
    }
    Ok(endpoints)
}

/// OS-backed browser for both V1 service types on macOS and Linux.
pub struct DesktopBrowser {
    daemon: ServiceDaemon,
    trusted: Receiver<ServiceEvent>,
    pairing: Receiver<ServiceEvent>,
    stopped: bool,
}

impl DesktopBrowser {
    /// Start browsing both normal and pairing service types.
    pub fn start() -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|_| DiscoveryError::Backend)?;
        let Ok(trusted) = daemon.browse(TRUSTED_SERVICE_TYPE) else {
            let _ = daemon.shutdown();
            return Err(DiscoveryError::Backend);
        };
        let Ok(pairing) = daemon.browse(PAIRING_SERVICE_TYPE) else {
            let _ = daemon.stop_browse(TRUSTED_SERVICE_TYPE);
            let _ = daemon.shutdown();
            return Err(DiscoveryError::Backend);
        };
        Ok(Self {
            daemon,
            trusted,
            pairing,
            stopped: false,
        })
    }

    fn drain_events(&self) -> Result<Vec<DiscoveryEvent>, DiscoveryError> {
        let mut events = Vec::new();
        for receiver in [&self.trusted, &self.pairing] {
            while let Ok(event) = receiver.try_recv() {
                if let Some(event) = convert_event(event)? {
                    if events.len() == MAX_DISCOVERY_ENDPOINTS {
                        return Err(DiscoveryError::Capacity);
                    }
                    events.push(event);
                }
            }
        }
        Ok(events)
    }
}

impl DiscoveryPort for DesktopBrowser {
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
        if self.stopped {
            return Err(DiscoveryError::Stopped);
        }
        DiscoveryBatch::new(DiscoveryPermission::Granted, self.drain_events()?)
    }

    fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        let trusted = self.daemon.stop_browse(TRUSTED_SERVICE_TYPE);
        let pairing = self.daemon.stop_browse(PAIRING_SERVICE_TYPE);
        let shutdown = self.daemon.shutdown();
        if trusted.is_err() || pairing.is_err() || shutdown.is_err() {
            return Err(DiscoveryError::Backend);
        }
        Ok(())
    }
}

impl Drop for DesktopBrowser {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// OS-backed advertisement owner. Dropping, stopping, or locking withdraws all
/// records and shuts down its daemon thread.
pub struct DesktopAdvertiser {
    daemon: ServiceDaemon,
    policy: AdvertisementSet,
    endpoint: LocalEndpoint,
    registered: BTreeSet<String>,
    stopped: bool,
}

impl DesktopAdvertiser {
    /// Start the normal trusted-service advertisement with fresh entropy.
    pub fn start(endpoint: LocalEndpoint, now: Instant) -> Result<Self, DiscoveryError> {
        let daemon = ServiceDaemon::new().map_err(|_| DiscoveryError::Backend)?;
        let policy = AdvertisementSet::start(endpoint.port())?;
        let mut advertiser = Self {
            daemon,
            policy,
            endpoint,
            registered: BTreeSet::new(),
            stopped: false,
        };
        if advertiser.reconcile(now).is_err() {
            let _ = advertiser.shutdown();
            return Err(DiscoveryError::Backend);
        }
        Ok(advertiser)
    }

    /// Advertise the pairing service for exactly the explicit V1 window.
    pub fn open_pairing_at(&mut self, now: Instant) -> Result<(), DiscoveryError> {
        self.policy.open_pairing_at(now)?;
        if let Err(error) = self.reconcile(now) {
            self.policy.close_pairing();
            return Err(error);
        }
        Ok(())
    }

    /// Withdraw the pairing advertisement before its deadline.
    pub fn close_pairing(&mut self, now: Instant) -> Result<(), DiscoveryError> {
        self.policy.close_pairing();
        self.reconcile(now)
    }

    /// Apply monotonic pairing expiry and interface-address refresh state.
    pub fn tick(&mut self, now: Instant) -> Result<(), DiscoveryError> {
        self.reconcile(now)
    }

    /// Withdraw the old route and publish the same service policy on a newly
    /// bound allowed endpoint.
    pub fn replace_endpoint(
        &mut self,
        endpoint: LocalEndpoint,
        now: Instant,
    ) -> Result<(), DiscoveryError> {
        if endpoint == self.endpoint {
            return self.reconcile(now);
        }
        let pairing_open = self
            .policy
            .active(now)
            .iter()
            .any(|advertisement| advertisement.kind() == super::ServiceKind::Pairing);
        self.unregister_all()?;
        self.endpoint = endpoint;
        self.policy = AdvertisementSet::start(endpoint.port())?;
        if pairing_open {
            self.policy.open_pairing_at(now)?;
        }
        self.reconcile(now)
    }

    /// Withdraw every route while keeping the daemon available for recovery.
    pub fn suspend(&mut self) -> Result<(), DiscoveryError> {
        self.unregister_all()
    }

    /// Withdraw all records and stop the daemon because the vault locked.
    pub fn lock(&mut self) -> Result<(), DiscoveryError> {
        self.policy.lock();
        self.shutdown()
    }

    /// Withdraw all records and stop the daemon.
    pub fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        if self.stopped {
            return Ok(());
        }
        self.policy.stop();
        let mut failed = self.unregister_all().is_err();
        if self.daemon.shutdown().is_err() {
            failed = true;
        }
        self.stopped = true;
        if failed {
            Err(DiscoveryError::Backend)
        } else {
            Ok(())
        }
    }

    fn reconcile(&mut self, now: Instant) -> Result<(), DiscoveryError> {
        if self.stopped {
            return Err(DiscoveryError::Stopped);
        }
        let desired = self.policy.active(now);
        let desired_names: BTreeSet<_> = desired.iter().map(Advertisement::fullname).collect();

        let obsolete: Vec<_> = self
            .registered
            .difference(&desired_names)
            .cloned()
            .collect();
        for fullname in obsolete {
            self.daemon
                .unregister(&fullname)
                .map_err(|_| DiscoveryError::Backend)?;
            self.registered.remove(&fullname);
        }

        for advertisement in desired {
            let fullname = advertisement.fullname();
            if self.registered.contains(&fullname) {
                continue;
            }
            self.daemon
                .register(service_info(&advertisement, self.endpoint)?)
                .map_err(|_| DiscoveryError::Backend)?;
            self.registered.insert(fullname);
        }
        Ok(())
    }

    fn unregister_all(&mut self) -> Result<(), DiscoveryError> {
        let names: Vec<_> = self.registered.iter().cloned().collect();
        let mut failed = false;
        for fullname in names {
            if self.daemon.unregister(&fullname).is_err() {
                failed = true;
            }
            self.registered.remove(&fullname);
        }
        if failed {
            Err(DiscoveryError::Backend)
        } else {
            Ok(())
        }
    }
}

impl Drop for DesktopAdvertiser {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn service_info(
    advertisement: &Advertisement,
    endpoint: LocalEndpoint,
) -> Result<ServiceInfo, DiscoveryError> {
    let mut service = ServiceInfo::new(
        advertisement.service_type(),
        advertisement.instance_label(),
        advertisement.hostname(),
        (),
        advertisement.port(),
        PROTOCOL_TXT,
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(|_| DiscoveryError::Backend)?;
    service.set_interfaces(vec![IfKind::Predicate(IfPredicate::new(
        move |interface| {
            interface.ip() == endpoint.ip()
                && (endpoint.scope_id() == 0
                    || interface.index.unwrap_or_default() == endpoint.scope_id())
        },
    ))]);
    Ok(service)
}

fn convert_event(event: ServiceEvent) -> Result<Option<DiscoveryEvent>, DiscoveryError> {
    match event {
        ServiceEvent::ServiceResolved(service) => {
            let mut endpoints = Vec::new();
            for address in service.get_addresses() {
                match address {
                    ScopedIp::V4(address) => {
                        endpoints.push(RawEndpoint::new((*address.addr()).into(), 0));
                    }
                    ScopedIp::V6(address) => {
                        let ip = *address.addr();
                        let scope_id = if ip.segments()[0] & 0xffc0 == 0xfe80 {
                            address.scope_id().index
                        } else {
                            0
                        };
                        endpoints.push(RawEndpoint::new(ip.into(), scope_id));
                    }
                    _ => return Err(DiscoveryError::InvalidRecord),
                }
                if endpoints.len() > MAX_DISCOVERY_ENDPOINTS {
                    return Err(DiscoveryError::Capacity);
                }
            }
            let txt = service
                .get_properties()
                .iter()
                .map(|property| {
                    (
                        property.key().to_string(),
                        property.val().unwrap_or_default().to_vec(),
                    )
                })
                .collect();
            Ok(Some(DiscoveryEvent::Resolved(
                RawDiscoveryRecord::try_from_untrusted(
                    service.ty_domain.clone(),
                    service.get_fullname().to_string(),
                    service.get_port(),
                    txt,
                    endpoints,
                    DESKTOP_CANDIDATE_TTL,
                )?,
            )))
        }
        ServiceEvent::ServiceRemoved(service_type, fullname) => Ok(Some(DiscoveryEvent::Removed {
            service_type,
            fullname,
        })),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{allowed_interface_endpoints, service_info, DesktopAdvertiser, DesktopBrowser};
    use crate::{
        address::LocalEndpoint,
        discovery::{
            AdvertisementSet, DiscoveryEvent, DiscoveryPort, PROTOCOL_TXT, TRUSTED_SERVICE_TYPE,
        },
    };
    use std::{thread, time::Duration, time::Instant};

    #[test]
    fn service_info_contains_only_ephemeral_v1_metadata_and_auto_addresses() {
        let now = Instant::now();
        let mut set = AdvertisementSet::start_with_entropy(48101, [0x11; 16]).unwrap();
        let advertisement = set.active(now).remove(0);
        let endpoint = LocalEndpoint::new("127.0.0.1".parse().unwrap(), 48101, 0).unwrap();
        let info = service_info(&advertisement, endpoint).unwrap();

        assert_eq!(info.get_type(), TRUSTED_SERVICE_TYPE);
        assert_eq!(info.get_hostname(), advertisement.hostname());
        assert_eq!(info.get_port(), 48101);
        assert!(info.is_addr_auto());
        assert_eq!(info.get_properties().len(), PROTOCOL_TXT.len());
        assert_eq!(info.get_property_val_str("v"), Some("1"));
    }

    #[test]
    fn real_desktop_publisher_and_browser_interoperate() {
        // LNS-DISC-006: exercise the actual mdns-sd daemon boundary, not only
        // record construction and simulated discovery events.
        let endpoint = allowed_interface_endpoints(48_173)
            .expect("enumerate an allowed desktop interface")
            .into_iter()
            .next()
            .expect("at least one allowed desktop interface");
        let mut browser = DesktopBrowser::start().expect("start real desktop browser");
        let mut advertiser =
            DesktopAdvertiser::start(endpoint, Instant::now()).expect("start real publisher");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut resolved = false;
        while Instant::now() < deadline {
            let batch = browser.poll().expect("poll real desktop browser");
            resolved = batch.events.iter().any(|event| {
                matches!(
                    event,
                    DiscoveryEvent::Resolved(record)
                        if record.service_type == TRUSTED_SERVICE_TYPE
                            && record.port == endpoint.port()
                            && record.endpoints.iter().any(|candidate| {
                                candidate.ip == endpoint.ip()
                                    && candidate.scope_id == endpoint.scope_id()
                            })
                )
            });
            if resolved {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        advertiser.shutdown().expect("withdraw real advertisement");
        browser.shutdown().expect("stop real desktop browser");
        assert!(
            resolved,
            "real desktop browser did not resolve its publisher"
        );
    }
}
