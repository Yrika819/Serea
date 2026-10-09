//! Deterministic provider registry and advertisement matching (ADR-0034).
//!
//! Providers confirm, withdraw, or report health. They never create
//! descriptor authority: every advertised descriptor must already exist in
//! the manifest, byte-semantically.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serea_protocol::provider::CapabilityProvider;

use crate::CapabilityManifestV1;

/// Provider registry construction/validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    DuplicateProviderId,
    HostProvider,
    AdvertisementNamespaceMismatch,
    UnmanifestedAdvertisement,
    AdvertisementMismatch,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ProviderError {}

/// Immutable deterministic registry of `Arc<dyn CapabilityProvider>`.
#[derive(Clone)]
//

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn CapabilityProvider>>,
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("len", &self.providers.len())
            .finish()
    }
}

impl ProviderRegistry {
    pub fn build(providers: Vec<Arc<dyn CapabilityProvider>>) -> Result<Self, ProviderError> {
        let mut seen: HashMap<String, ()> = HashMap::new();
        for provider in &providers {
            if provider.provider_id().as_str() == "host" {
                return Err(ProviderError::HostProvider);
            }
            if seen
                .insert(provider.provider_id().as_str().to_string(), ())
                .is_some()
            {
                return Err(ProviderError::DuplicateProviderId);
            }
        }
        let mut sorted = providers;
        sorted.sort_by(|a, b| a.provider_id().as_str().cmp(b.provider_id().as_str()));
        Ok(Self { providers: sorted })
    }

    pub fn providers(&self) -> &[Arc<dyn CapabilityProvider>] {
        &self.providers
    }

    /// All advertised descriptors must match exactly one manifest entry
    /// (or be rejected). Missing provider entries are allowed and become
    /// unavailable candidates, never manifest failures.
    pub fn validate_advertisements(
        &self,
        manifest: &CapabilityManifestV1,
    ) -> Result<(), ProviderError> {
        for provider in &self.providers {
            for descriptor in provider.capabilities() {
                if descriptor.provider_id().as_str() != provider.provider_id().as_str() {
                    return Err(ProviderError::AdvertisementNamespaceMismatch);
                }
                // identity-based lookup: (id, version, implementation_id)
                let identity_exists = manifest.entries().iter().any(|e| {
                    e.descriptor.id() == descriptor.id()
                        && e.descriptor.version() == descriptor.version()
                        && e.descriptor.implementation_id() == descriptor.implementation_id()
                });
                if !identity_exists {
                    return Err(ProviderError::UnmanifestedAdvertisement);
                }
                let exact = manifest.entries().iter().any(|e| {
                    e.descriptor.id() == descriptor.id()
                        && e.descriptor.version() == descriptor.version()
                        && e.descriptor.implementation_id() == descriptor.implementation_id()
                        && e.descriptor == descriptor
                });
                if !exact {
                    return Err(ProviderError::AdvertisementMismatch);
                }
            }
        }
        Ok(())
    }
}
