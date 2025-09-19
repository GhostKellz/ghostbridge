/*!
Audit logging and security compliance system

Provides comprehensive audit trails, security event logging, and compliance
reporting for regulatory requirements.
*/

use crate::error::{BridgeError, Result, SecurityError};
use crate::types::{Address, Transaction};
use crate::security::GuardianConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use serde::{Deserialize, Serialize};

/// Audit logger for security events
pub struct AuditLogger {
    config: GuardianConfig,
    event_store: Arc<RwLock<AuditEventStore>>,
    compliance_tracker: ComplianceTracker,
    retention_manager: RetentionManager,
}

/// Audit event storage
#[derive(Debug, Clone)]
struct AuditEventStore {
    events: Vec<AuditEvent>,
    event_index: HashMap<String, usize>, // event_id -> index
    address_index: HashMap<Address, Vec<usize>>, // address -> event indices
    type_index: HashMap<String, Vec<usize>>, // event_type -> event indices
    daily_counts: HashMap<String, u64>, // date -> event count
}

/// Security audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub event_type: String,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
    pub transaction_id: Option<String>,
    pub address: Option<Address>,
    pub user_id: Option<String>,
    pub result: bool,
    pub details: String,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timestamp: SystemTime,
    pub source_system: String,
    pub correlation_id: Option<String>,
}

/// Audit event categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditCategory {
    Authentication,
    Authorization,
    DataAccess,
    DataModification,
    SystemAccess,
    SecurityEvent,
    ComplianceEvent,
    PolicyViolation,
    ConfigChange,
    AdminAction,
}

/// Audit severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Security audit summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAudit {
    pub audit_id: String,
    pub period_start: SystemTime,
    pub period_end: SystemTime,
    pub total_events: u64,
    pub events_by_category: HashMap<AuditCategory, u64>,
    pub events_by_severity: HashMap<AuditSeverity, u64>,
    pub security_incidents: Vec<SecurityIncidentSummary>,
    pub compliance_status: ComplianceStatus,
    pub recommendations: Vec<SecurityRecommendation>,
    pub generated_at: SystemTime,
}

/// Security incident summary for audits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIncidentSummary {
    pub incident_id: String,
    pub incident_type: String,
    pub severity: AuditSeverity,
    pub affected_addresses: Vec<Address>,
    pub event_count: u64,
    pub first_detected: SystemTime,
    pub last_detected: SystemTime,
    pub resolved: bool,
}

/// Compliance tracking
struct ComplianceTracker {
    compliance_frameworks: HashMap<String, ComplianceFramework>,
    audit_requirements: HashMap<String, AuditRequirement>,
}

/// Compliance framework definition
#[derive(Debug, Clone)]
struct ComplianceFramework {
    name: String,
    description: String,
    requirements: Vec<String>,
    audit_frequency: Duration,
    retention_period: Duration,
    reporting_requirements: Vec<ReportingRequirement>,
}

/// Audit requirement
#[derive(Debug, Clone)]
struct AuditRequirement {
    name: String,
    description: String,
    event_types: Vec<String>,
    retention_period: Duration,
    real_time_alerting: bool,
    reporting_frequency: Duration,
}

/// Reporting requirement
#[derive(Debug, Clone)]
struct ReportingRequirement {
    report_type: String,
    frequency: Duration,
    recipients: Vec<String>,
    format: ReportFormat,
}

/// Report formats
#[derive(Debug, Clone)]
enum ReportFormat {
    Json,
    Csv,
    Pdf,
    Html,
}

/// Compliance status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub framework: String,
    pub overall_compliance: bool,
    pub compliance_score: f64, // 0.0 to 1.0
    pub requirements_met: u32,
    pub requirements_total: u32,
    pub violations: Vec<ComplianceViolation>,
    pub last_assessment: SystemTime,
}

/// Compliance violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub requirement: String,
    pub description: String,
    pub severity: AuditSeverity,
    pub detected_at: SystemTime,
    pub resolved: bool,
    pub remediation_actions: Vec<String>,
}

/// Security recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRecommendation {
    pub category: String,
    pub priority: RecommendationPriority,
    pub title: String,
    pub description: String,
    pub remediation_steps: Vec<String>,
    pub estimated_effort: String,
    pub risk_reduction: f64,
}

/// Recommendation priority
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Retention management
struct RetentionManager {
    retention_policies: HashMap<AuditCategory, Duration>,
    archival_enabled: bool,
    archival_location: String,
}

/// Audit query parameters
#[derive(Debug, Clone)]
pub struct AuditQuery {
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
    pub event_types: Option<Vec<String>>,
    pub categories: Option<Vec<AuditCategory>>,
    pub severities: Option<Vec<AuditSeverity>>,
    pub addresses: Option<Vec<Address>>,
    pub transaction_ids: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl AuditLogger {
    /// Initialize audit logger
    #[instrument(skip(config))]
    pub async fn new(config: GuardianConfig) -> Result<Self> {
        info!("Initializing audit logger");

        let event_store = Arc::new(RwLock::new(AuditEventStore {
            events: Vec::new(),
            event_index: HashMap::new(),
            address_index: HashMap::new(),
            type_index: HashMap::new(),
            daily_counts: HashMap::new(),
        }));

        let compliance_tracker = ComplianceTracker {
            compliance_frameworks: Self::initialize_compliance_frameworks(),
            audit_requirements: Self::initialize_audit_requirements(),
        };

        let retention_manager = RetentionManager {
            retention_policies: Self::initialize_retention_policies(),
            archival_enabled: true,
            archival_location: "audit_archive".to_string(),
        };

        Ok(Self {
            config,
            event_store,
            compliance_tracker,
            retention_manager,
        })
    }

    /// Log security audit event
    #[instrument(skip(self, event))]
    pub async fn log_event(&self, mut event: AuditEvent) -> Result<()> {
        // Generate event ID if not provided
        if event.event_id.is_empty() {
            event.event_id = self.generate_event_id().await;
        }

        debug!("Logging audit event: {} ({})", event.event_id, event.event_type);

        let mut store = self.event_store.write().await;

        // Add to main storage
        let event_index = store.events.len();
        store.events.push(event.clone());

        // Update indices
        store.event_index.insert(event.event_id.clone(), event_index);

        if let Some(address) = &event.address {
            store.address_index
                .entry(address.clone())
                .or_insert_with(Vec::new)
                .push(event_index);
        }

        store.type_index
            .entry(event.event_type.clone())
            .or_insert_with(Vec::new)
            .push(event_index);

        // Update daily counts
        let date_key = self.format_date(event.timestamp);
        *store.daily_counts.entry(date_key).or_insert(0) += 1;

        // Check for real-time alerts
        if event.severity >= AuditSeverity::Error {
            self.trigger_alert(&event).await?;
        }

        debug!("Audit event logged successfully: {}", event.event_id);
        Ok(())
    }

    /// Query audit events
    #[instrument(skip(self, query))]
    pub async fn query_events(&self, query: AuditQuery) -> Result<Vec<AuditEvent>> {
        debug!("Querying audit events");

        let store = self.event_store.read().await;
        let mut results = Vec::new();

        for (index, event) in store.events.iter().enumerate() {
            // Apply filters
            if let Some(start_time) = query.start_time {
                if event.timestamp < start_time {
                    continue;
                }
            }

            if let Some(end_time) = query.end_time {
                if event.timestamp > end_time {
                    continue;
                }
            }

            if let Some(ref event_types) = query.event_types {
                if !event_types.contains(&event.event_type) {
                    continue;
                }
            }

            if let Some(ref categories) = query.categories {
                if !categories.contains(&event.category) {
                    continue;
                }
            }

            if let Some(ref severities) = query.severities {
                if !severities.contains(&event.severity) {
                    continue;
                }
            }

            if let Some(ref addresses) = query.addresses {
                if let Some(ref event_address) = event.address {
                    if !addresses.contains(event_address) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            // Apply offset and limit
            if let Some(offset) = query.offset {
                if index < offset {
                    continue;
                }
            }

            if let Some(limit) = query.limit {
                if results.len() >= limit {
                    break;
                }
            }

            results.push(event.clone());
        }

        debug!("Query returned {} events", results.len());
        Ok(results)
    }

    /// Generate security audit report
    #[instrument(skip(self))]
    pub async fn generate_security_audit(
        &self,
        period_start: SystemTime,
        period_end: SystemTime,
    ) -> Result<SecurityAudit> {
        debug!("Generating security audit for period");

        let query = AuditQuery {
            start_time: Some(period_start),
            end_time: Some(period_end),
            event_types: None,
            categories: None,
            severities: None,
            addresses: None,
            transaction_ids: None,
            limit: None,
            offset: None,
        };

        let events = self.query_events(query).await?;
        let total_events = events.len() as u64;

        // Categorize events
        let mut events_by_category = HashMap::new();
        let mut events_by_severity = HashMap::new();

        for event in &events {
            *events_by_category.entry(event.category.clone()).or_insert(0) += 1;
            *events_by_severity.entry(event.severity.clone()).or_insert(0) += 1;
        }

        // Analyze security incidents
        let security_incidents = self.analyze_security_incidents(&events).await?;

        // Check compliance status
        let compliance_status = self.assess_compliance(&events).await?;

        // Generate recommendations
        let recommendations = self.generate_recommendations(&events, &security_incidents).await?;

        let audit = SecurityAudit {
            audit_id: self.generate_audit_id().await,
            period_start,
            period_end,
            total_events,
            events_by_category,
            events_by_severity,
            security_incidents,
            compliance_status,
            recommendations,
            generated_at: SystemTime::now(),
        };

        info!("Security audit generated with {} events and {} incidents",
              total_events, audit.security_incidents.len());

        Ok(audit)
    }

    /// Get audit statistics
    pub async fn get_audit_statistics(&self) -> Result<AuditStatistics> {
        let store = self.event_store.read().await;

        let total_events = store.events.len();
        let events_today = self.count_events_today(&store).await;
        let critical_events = store.events.iter()
            .filter(|e| e.severity == AuditSeverity::Critical)
            .count();

        Ok(AuditStatistics {
            total_events,
            events_today,
            critical_events,
            storage_size_mb: (total_events * 1024) / (1024 * 1024), // Rough estimate
        })
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        let store = self.event_store.read().await;

        // Check if audit logger is functional
        store.events.len() < 10_000_000 // Reasonable upper bound
    }

    /// Cleanup expired events
    #[instrument(skip(self))]
    pub async fn cleanup_expired_events(&self) -> Result<u64> {
        debug!("Cleaning up expired audit events");

        let mut store = self.event_store.write().await;
        let mut removed_count = 0;

        // Find expired events
        let now = SystemTime::now();
        let mut indices_to_remove = Vec::new();

        for (index, event) in store.events.iter().enumerate() {
            if let Some(retention_period) = self.retention_manager.retention_policies.get(&event.category) {
                if let Ok(age) = now.duration_since(event.timestamp) {
                    if age > *retention_period {
                        indices_to_remove.push(index);
                    }
                }
            }
        }

        // Remove expired events (in reverse order to maintain indices)
        for &index in indices_to_remove.iter().rev() {
            if self.retention_manager.archival_enabled {
                // TODO: Archive event before removal
                self.archive_event(&store.events[index]).await?;
            }

            store.events.remove(index);
            removed_count += 1;
        }

        // Rebuild indices
        self.rebuild_indices(&mut store).await;

        info!("Cleaned up {} expired audit events", removed_count);
        Ok(removed_count)
    }

    async fn generate_event_id(&self) -> String {
        format!("audit-{}-{}",
                SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_millis(),
                rand::random::<u32>())
    }

    async fn generate_audit_id(&self) -> String {
        format!("security-audit-{}",
                SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs())
    }

    fn format_date(&self, timestamp: SystemTime) -> String {
        // Simple date formatting - in production, use proper date library
        let seconds = timestamp.duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs();
        format!("{}", seconds / (24 * 60 * 60))
    }

    async fn trigger_alert(&self, event: &AuditEvent) -> Result<()> {
        warn!("Security alert triggered for event: {} ({})", event.event_id, event.event_type);

        // TODO: Implement actual alerting mechanism
        // - Send notifications to security team
        // - Integrate with SIEM systems
        // - Trigger automated responses

        Ok(())
    }

    async fn analyze_security_incidents(&self, events: &[AuditEvent]) -> Result<Vec<SecurityIncidentSummary>> {
        let mut incidents = Vec::new();

        // Group events by type and analyze patterns
        let mut grouped_events: HashMap<String, Vec<&AuditEvent>> = HashMap::new();
        for event in events {
            grouped_events.entry(event.event_type.clone())
                .or_insert_with(Vec::new)
                .push(event);
        }

        for (event_type, type_events) in grouped_events {
            if type_events.len() > 10 { // Threshold for incident
                let first_detected = type_events.iter()
                    .map(|e| e.timestamp)
                    .min()
                    .unwrap_or_else(SystemTime::now);

                let last_detected = type_events.iter()
                    .map(|e| e.timestamp)
                    .max()
                    .unwrap_or_else(SystemTime::now);

                let affected_addresses: Vec<Address> = type_events.iter()
                    .filter_map(|e| e.address.clone())
                    .collect();

                let max_severity = type_events.iter()
                    .map(|e| &e.severity)
                    .max()
                    .unwrap_or(&AuditSeverity::Info);

                incidents.push(SecurityIncidentSummary {
                    incident_id: format!("incident-{}-{}", event_type, first_detected
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs()),
                    incident_type: event_type,
                    severity: max_severity.clone(),
                    affected_addresses,
                    event_count: type_events.len() as u64,
                    first_detected,
                    last_detected,
                    resolved: false,
                });
            }
        }

        Ok(incidents)
    }

    async fn assess_compliance(&self, events: &[AuditEvent]) -> Result<ComplianceStatus> {
        // Simple compliance assessment - in production, implement proper compliance checks
        let total_requirements = 10;
        let requirements_met = 8; // Example

        Ok(ComplianceStatus {
            framework: "SOC2".to_string(),
            overall_compliance: requirements_met >= total_requirements,
            compliance_score: requirements_met as f64 / total_requirements as f64,
            requirements_met,
            requirements_total: total_requirements,
            violations: Vec::new(),
            last_assessment: SystemTime::now(),
        })
    }

    async fn generate_recommendations(
        &self,
        events: &[AuditEvent],
        incidents: &[SecurityIncidentSummary],
    ) -> Result<Vec<SecurityRecommendation>> {
        let mut recommendations = Vec::new();

        // Analyze patterns and generate recommendations
        if incidents.len() > 5 {
            recommendations.push(SecurityRecommendation {
                category: "Monitoring".to_string(),
                priority: RecommendationPriority::High,
                title: "Increase Security Monitoring".to_string(),
                description: "High number of security incidents detected".to_string(),
                remediation_steps: vec![
                    "Review incident response procedures".to_string(),
                    "Enhance real-time monitoring".to_string(),
                    "Consider additional security controls".to_string(),
                ],
                estimated_effort: "2-4 weeks".to_string(),
                risk_reduction: 0.3,
            });
        }

        let critical_events = events.iter()
            .filter(|e| e.severity == AuditSeverity::Critical)
            .count();

        if critical_events > 10 {
            recommendations.push(SecurityRecommendation {
                category: "Security".to_string(),
                priority: RecommendationPriority::Critical,
                title: "Address Critical Security Events".to_string(),
                description: format!("Found {} critical security events", critical_events),
                remediation_steps: vec![
                    "Investigate all critical events".to_string(),
                    "Implement additional security controls".to_string(),
                    "Review access controls".to_string(),
                ],
                estimated_effort: "1-2 weeks".to_string(),
                risk_reduction: 0.7,
            });
        }

        Ok(recommendations)
    }

    async fn count_events_today(&self, store: &AuditEventStore) -> usize {
        let today = self.format_date(SystemTime::now());
        store.daily_counts.get(&today).unwrap_or(&0).clone() as usize
    }

    async fn archive_event(&self, event: &AuditEvent) -> Result<()> {
        // TODO: Implement actual archival to external storage
        debug!("Archiving event: {}", event.event_id);
        Ok(())
    }

    async fn rebuild_indices(&self, store: &mut AuditEventStore) {
        // Rebuild all indices after cleanup
        store.event_index.clear();
        store.address_index.clear();
        store.type_index.clear();

        for (index, event) in store.events.iter().enumerate() {
            store.event_index.insert(event.event_id.clone(), index);

            if let Some(address) = &event.address {
                store.address_index
                    .entry(address.clone())
                    .or_insert_with(Vec::new)
                    .push(index);
            }

            store.type_index
                .entry(event.event_type.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }
    }

    fn initialize_compliance_frameworks() -> HashMap<String, ComplianceFramework> {
        let mut frameworks = HashMap::new();

        frameworks.insert("SOC2".to_string(), ComplianceFramework {
            name: "SOC 2 Type II".to_string(),
            description: "Security, Availability, Processing Integrity, Confidentiality, Privacy".to_string(),
            requirements: vec![
                "Access controls".to_string(),
                "Change management".to_string(),
                "Risk assessment".to_string(),
            ],
            audit_frequency: Duration::from_secs(365 * 24 * 60 * 60), // Annual
            retention_period: Duration::from_secs(7 * 365 * 24 * 60 * 60), // 7 years
            reporting_requirements: vec![],
        });

        frameworks
    }

    fn initialize_audit_requirements() -> HashMap<String, AuditRequirement> {
        let mut requirements = HashMap::new();

        requirements.insert("authentication".to_string(), AuditRequirement {
            name: "Authentication Events".to_string(),
            description: "Log all authentication attempts".to_string(),
            event_types: vec!["login".to_string(), "logout".to_string(), "auth_failure".to_string()],
            retention_period: Duration::from_secs(365 * 24 * 60 * 60), // 1 year
            real_time_alerting: true,
            reporting_frequency: Duration::from_secs(30 * 24 * 60 * 60), // Monthly
        });

        requirements
    }

    fn initialize_retention_policies() -> HashMap<AuditCategory, Duration> {
        let mut policies = HashMap::new();

        policies.insert(AuditCategory::Authentication, Duration::from_secs(365 * 24 * 60 * 60)); // 1 year
        policies.insert(AuditCategory::SecurityEvent, Duration::from_secs(7 * 365 * 24 * 60 * 60)); // 7 years
        policies.insert(AuditCategory::ComplianceEvent, Duration::from_secs(7 * 365 * 24 * 60 * 60)); // 7 years
        policies.insert(AuditCategory::DataAccess, Duration::from_secs(2 * 365 * 24 * 60 * 60)); // 2 years

        policies
    }
}

/// Audit statistics
#[derive(Debug, Clone)]
pub struct AuditStatistics {
    pub total_events: usize,
    pub events_today: usize,
    pub critical_events: usize,
    pub storage_size_mb: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let config = GuardianConfig::default();
        let logger = AuditLogger::new(config).await.unwrap();
        assert!(logger.is_healthy().await);
    }

    #[tokio::test]
    async fn test_event_logging() {
        let config = GuardianConfig::default();
        let logger = AuditLogger::new(config).await.unwrap();

        let event = AuditEvent {
            event_id: "test-event".to_string(),
            event_type: "test".to_string(),
            category: AuditCategory::SecurityEvent,
            severity: AuditSeverity::Info,
            transaction_id: None,
            address: None,
            user_id: None,
            result: true,
            details: "Test event".to_string(),
            metadata: HashMap::new(),
            timestamp: SystemTime::now(),
            source_system: "test".to_string(),
            correlation_id: None,
        };

        logger.log_event(event).await.unwrap();

        let stats = logger.get_audit_statistics().await.unwrap();
        assert_eq!(stats.total_events, 1);
    }
}