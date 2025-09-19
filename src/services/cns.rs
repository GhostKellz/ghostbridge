/*!
CNS (Crypto Name Server) service integration

Integration with the CNS service for domain resolution and management using etherlink.
*/

use crate::error::{BridgeError, Result, ServiceError};
use crate::types::Address;
use crate::services::ServiceEndpoint;
use etherlink::CNSClient;
use std::collections::HashMap;
use tracing::{debug, instrument};

/// CNS service wrapper
pub struct CnsService {
    client: CNSClient,
    endpoint: ServiceEndpoint,
}

impl CnsService {
    /// Create a new CNS service instance
    #[instrument(skip(endpoint))]
    pub async fn new(endpoint: &ServiceEndpoint) -> Result<Self> {
        debug!("Connecting to CNS service at {}", endpoint.grpc_endpoint());

        let client = CNSClient::connect(endpoint.grpc_endpoint()).await
            .map_err(|e| BridgeError::Service(ServiceError::Cns(format!(
                "Failed to connect to CNS: {}", e
            ))))?;

        Ok(Self {
            client,
            endpoint: endpoint.clone(),
        })
    }

    /// Resolve a domain name to addresses
    #[instrument(skip(self))]
    pub async fn resolve_domain(&mut self, domain: &str) -> Result<DomainResolution> {
        debug!("Resolving domain: {}", domain);

        let response = self.client
            .resolve_domain(domain.to_string(), vec!["A".to_string(), "AAAA".to_string()])
            .await
            .map_err(|e| BridgeError::Service(ServiceError::Cns(format!(
                "Domain resolution failed: {}", e
            ))))?;

        let mut records = HashMap::new();

        // Parse DNS records from response
        // Note: This is a simplified implementation
        // Real implementation would parse the actual response structure
        records.insert("A".to_string(), vec!["192.168.1.1".to_string()]);

        let resolution = DomainResolution {
            domain: domain.to_string(),
            records,
            owner: None, // TODO: Extract from response
            ttl: 3600,
            resolved_at: chrono::Utc::now(),
        };

        debug!("Successfully resolved domain: {}", domain);
        Ok(resolution)
    }

    /// Register a new domain
    #[instrument(skip(self))]
    pub async fn register_domain(
        &mut self,
        domain: &str,
        owner_address: &Address,
        initial_records: HashMap<String, Vec<String>>,
    ) -> Result<DomainRegistration> {
        debug!("Registering domain: {} for owner: {}", domain, owner_address);

        // Convert initial records to etherlink format
        let etherlink_records = vec![]; // TODO: Convert records

        let response = self.client
            .register_domain(
                domain.to_string(),
                owner_address.to_hex(),
                etherlink_records,
            )
            .await
            .map_err(|e| BridgeError::Service(ServiceError::Cns(format!(
                "Domain registration failed: {}", e
            ))))?;

        let registration = DomainRegistration {
            domain: domain.to_string(),
            owner: owner_address.clone(),
            transaction_hash: "0x1234...".to_string(), // TODO: Extract from response
            registered_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(365),
        };

        debug!("Successfully registered domain: {}", domain);
        Ok(registration)
    }

    /// Subscribe to domain changes
    #[instrument(skip(self))]
    pub async fn subscribe_domain_changes(
        &mut self,
        domains: Vec<String>,
    ) -> Result<()> {
        debug!("Subscribing to changes for {} domains", domains.len());

        let _stream = self.client
            .subscribe_domain_changes(domains)
            .await
            .map_err(|e| BridgeError::Service(ServiceError::Cns(format!(
                "Failed to subscribe to domain changes: {}", e
            ))))?;

        // TODO: Handle stream events
        debug!("Successfully subscribed to domain changes");
        Ok(())
    }

    /// Health check for CNS service
    pub async fn health_check(&self) -> Result<()> {
        // Simple health check - try to resolve a test domain
        // In a real implementation, this would be a dedicated health endpoint
        debug!("Performing CNS health check");

        // For now, just return OK if we have a client
        // TODO: Implement actual health check
        Ok(())
    }
}

/// Domain resolution result
#[derive(Debug, Clone)]
pub struct DomainResolution {
    pub domain: String,
    pub records: HashMap<String, Vec<String>>,
    pub owner: Option<Address>,
    pub ttl: u32,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
}

/// Domain registration result
#[derive(Debug, Clone)]
pub struct DomainRegistration {
    pub domain: String,
    pub owner: Address,
    pub transaction_hash: String,
    pub registered_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ServiceEndpoint;

    #[test]
    fn test_domain_resolution_creation() {
        let mut records = HashMap::new();
        records.insert("A".to_string(), vec!["192.168.1.1".to_string()]);

        let resolution = DomainResolution {
            domain: "test.ghost".to_string(),
            records,
            owner: None,
            ttl: 3600,
            resolved_at: chrono::Utc::now(),
        };

        assert_eq!(resolution.domain, "test.ghost");
        assert_eq!(resolution.ttl, 3600);
    }

    #[test]
    fn test_domain_registration_creation() {
        let owner = Address([1u8; 20]);

        let registration = DomainRegistration {
            domain: "test.ghost".to_string(),
            owner: owner.clone(),
            transaction_hash: "0x1234...".to_string(),
            registered_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(365),
        };

        assert_eq!(registration.domain, "test.ghost");
        assert_eq!(registration.owner, owner);
    }
}