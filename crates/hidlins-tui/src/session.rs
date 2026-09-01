//! Valid-state ownership for the registry and unlocked vault.

use hidlins_core::{Vault, VaultRegistry};

/// Resources owned by the UI thread across locked, ready, and syncing states.
pub(crate) enum SessionResources {
    /// No vault is unlocked; registry access remains available.
    Locked { registry: VaultRegistry },
    /// The UI owns both the unlocked vault and its registry.
    Ready {
        vault: Box<Vault>,
        registry: VaultRegistry,
    },
    /// The background sync worker owns both resources.
    Syncing,
}

impl SessionResources {
    pub(crate) fn locked(registry: VaultRegistry) -> Self {
        Self::Locked { registry }
    }

    pub(crate) fn registry(&self) -> Option<&VaultRegistry> {
        match self {
            Self::Locked { registry } | Self::Ready { registry, .. } => Some(registry),
            Self::Syncing => None,
        }
    }

    pub(crate) fn registry_mut(&mut self) -> Option<&mut VaultRegistry> {
        match self {
            Self::Locked { registry } | Self::Ready { registry, .. } => Some(registry),
            Self::Syncing => None,
        }
    }

    pub(crate) fn vault(&self) -> Option<&Vault> {
        match self {
            Self::Ready { vault, .. } => Some(vault.as_ref()),
            Self::Locked { .. } | Self::Syncing => None,
        }
    }

    pub(crate) fn vault_mut(&mut self) -> Option<&mut Vault> {
        match self {
            Self::Ready { vault, .. } => Some(vault.as_mut()),
            Self::Locked { .. } | Self::Syncing => None,
        }
    }

    pub(crate) fn is_syncing(&self) -> bool {
        matches!(self, Self::Syncing)
    }

    pub(crate) fn unlock(&mut self, vault: Vault) -> bool {
        let previous = std::mem::replace(self, Self::Syncing);
        match previous {
            Self::Locked { registry } => {
                *self = Self::Ready {
                    vault: Box::new(vault),
                    registry,
                };
                true
            }
            other => {
                *self = other;
                drop(vault);
                false
            }
        }
    }

    pub(crate) fn begin_sync(&mut self) -> Option<(Vault, VaultRegistry)> {
        let previous = std::mem::replace(self, Self::Syncing);
        match previous {
            Self::Ready { vault, registry } => Some((*vault, registry)),
            other => {
                *self = other;
                None
            }
        }
    }

    pub(crate) fn finish_sync(&mut self, vault: Vault, registry: VaultRegistry) {
        debug_assert!(self.is_syncing());
        *self = Self::Ready {
            vault: Box::new(vault),
            registry,
        };
    }

    pub(crate) fn lock(&mut self) {
        let previous = std::mem::replace(self, Self::Syncing);
        *self = match previous {
            Self::Ready { registry, .. } | Self::Locked { registry } => Self::Locked { registry },
            Self::Syncing => Self::Syncing,
        };
    }

    pub(crate) fn recover_locked(&mut self, registry: VaultRegistry) {
        *self = Self::Locked { registry };
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use hidlins_core::{HidlinsPaths, VaultRegistry};

    use super::SessionResources;

    fn registry(name: &str) -> VaultRegistry {
        VaultRegistry::with_paths(HidlinsPaths::with_registry_file(PathBuf::from(name)))
    }

    #[test]
    fn locked_state_cannot_begin_sync_or_lose_its_registry() {
        let mut resources = SessionResources::locked(registry("locked.toml"));
        assert!(resources.begin_sync().is_none());
        assert!(resources.registry().is_some());
        assert!(resources.vault().is_none());
        assert!(!resources.is_syncing());
    }

    #[test]
    fn recovery_replaces_syncing_with_one_locked_owner() {
        let mut resources = SessionResources::Syncing;
        resources.recover_locked(registry("after.toml"));
        assert!(resources.registry().is_some());
        assert!(resources.vault().is_none());
        assert!(!resources.is_syncing());
    }
}
