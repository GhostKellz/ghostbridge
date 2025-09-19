/*!
Privacy policy engine and compliance system

Implements GDPR-compliant privacy policies, consent management, and data
minimization for zero-trust security.
*/

use crate::error::{BridgeError, Result, SecurityError};
use crate::types::{Address, Transaction};
use crate::security::{GuardianConfig, PolicyResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use serde::{Deserialize, Serialize};

/// Privacy policy engine
pub struct PolicyEngine {
    config: GuardianConfig,
    policies: Arc<RwLock<PolicyStore>>,
    consent_manager: ConsentManager,
    compliance_checker: ComplianceChecker,
}

/// Policy storage and management
#[derive(Debug, Clone)]
struct PolicyStore {
    privacy_policies: HashMap<String, PrivacyPolicy>,
    policy_rules: HashMap<String, PolicyRule>,
    active_policies: Vec<String>,
    policy_versions: HashMap<String, Vec<PolicyVersion>>,
}

/// Privacy policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub rules: Vec<PolicyRule>,
    pub data_categories: Vec<DataCategory>,
    pub retention_policy: RetentionPolicy,
    pub consent_requirements: ConsentRequirements,
    pub geographical_scope: Vec<String>, // ISO country codes
    pub created_at: SystemTime,
    pub effective_date: SystemTime,
    pub expiry_date: Option<SystemTime>,
    pub status: PolicyStatus,
}

/// Policy rule for specific checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub rule_type: PolicyRuleType,
    pub conditions: Vec<PolicyCondition>,
    pub actions: Vec<PolicyAction>,
    pub severity: PolicySeverity,
    pub enabled: bool,
}

/// Types of policy rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyRuleType {
    DataMinimization,
    ConsentValidation,
    RetentionLimit,
    TransferRestriction,
    AccessControl,
    AuditRequirement,
}

/// Policy condition for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
    pub data_type: DataType,
}

/// Condition operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    NotContains,
    In,
    NotIn,
    Exists,
    NotExists,
}

/// Policy action to take
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    pub action_type: ActionType,
    pub parameters: HashMap<String, serde_json::Value>,
    pub required: bool,
}

/// Action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Block,
    RequireConsent,
    MinimizeData,
    AuditLog,
    Notify,
    Escalate,
}

/// Policy severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Data categories for classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCategory {
    pub name: String,
    pub description: String,
    pub sensitivity_level: SensitivityLevel,
    pub legal_basis: Vec<LegalBasis>,
    pub retention_period: Duration,
}

/// Data sensitivity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SensitivityLevel {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// Legal basis for data processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegalBasis {
    Consent,
    Contract,
    LegalObligation,
    VitalInterests,
    PublicTask,
    LegitimateInterests,
}

/// Data retention policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub default_retention: Duration,
    pub category_specific: HashMap<String, Duration>,
    pub deletion_schedule: DeletionSchedule,
    pub archival_policy: ArchivalPolicy,
}

/// Deletion schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionSchedule {
    pub automatic_deletion: bool,
    pub deletion_intervals: Vec<Duration>,
    pub deletion_criteria: Vec<DeletionCriterion>,
}

/// Deletion criterion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionCriterion {
    pub name: String,
    pub condition: PolicyCondition,
    pub retention_override: Option<Duration>,
}

/// Archival policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivalPolicy {
    pub archive_after: Duration,
    pub archive_location: String,
    pub archive_encryption: bool,
    pub archive_retention: Duration,
}

/// Consent requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRequirements {
    pub required_purposes: Vec<String>,
    pub granular_consent: bool,
    pub withdrawal_mechanism: bool,
    pub consent_expiry: Option<Duration>,
    pub reconfirmation_period: Option<Duration>,
}

/// Policy status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyStatus {
    Draft,
    Active,
    Deprecated,
    Revoked,
}

/// Policy version tracking
#[derive(Debug, Clone)]
struct PolicyVersion {
    version: String,
    policy: PrivacyPolicy,
    created_at: SystemTime,
    deprecated_at: Option<SystemTime>,
}

/// Data types for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Address,
    Amount,
    Timestamp,
}

/// Consent manager for tracking user consent
struct ConsentManager {
    consent_records: Arc<RwLock<HashMap<Address, UserConsent>>>,
}

/// User consent record
#[derive(Debug, Clone)]
struct UserConsent {
    address: Address,
    consents: HashMap<String, ConsentRecord>,
    last_updated: SystemTime,
}

/// Individual consent record
#[derive(Debug, Clone)]
struct ConsentRecord {
    purpose: String,
    granted: bool,
    granted_at: SystemTime,
    expires_at: Option<SystemTime>,
    withdrawn_at: Option<SystemTime>,
    legal_basis: LegalBasis,
}

/// Compliance checker
struct ComplianceChecker {
    jurisdiction_rules: HashMap<String, JurisdictionRules>,
}

/// Jurisdiction-specific rules
#[derive(Debug, Clone)]
struct JurisdictionRules {
    country_code: String,
    gdpr_applicable: bool,
    ccpa_applicable: bool,
    data_localization_required: bool,
    consent_age_minimum: u8,
    additional_requirements: Vec<String>,
}

impl PolicyEngine {
    /// Initialize policy engine
    #[instrument(skip(config))]
    pub async fn new(config: GuardianConfig) -> Result<Self> {
        info!("Initializing privacy policy engine");

        let policies = Arc::new(RwLock::new(PolicyStore {
            privacy_policies: HashMap::new(),
            policy_rules: HashMap::new(),
            active_policies: Vec::new(),
            policy_versions: HashMap::new(),
        }));

        let consent_manager = ConsentManager {
            consent_records: Arc::new(RwLock::new(HashMap::new())),
        };

        let compliance_checker = ComplianceChecker {
            jurisdiction_rules: Self::initialize_jurisdiction_rules(),
        };

        let mut engine = Self {
            config,
            policies,
            consent_manager,
            compliance_checker,
        };

        // Load default policies
        engine.load_default_policies().await?;

        Ok(engine)
    }

    /// Evaluate transaction against privacy policies
    #[instrument(skip(self, transaction))]
    pub async fn evaluate_transaction(&self, transaction: &Transaction) -> Result<PolicyResult> {
        debug!("Evaluating transaction {} against privacy policies", transaction.id);

        let mut violations = Vec::new();
        let mut required_actions = Vec::new();
        let mut applicable_policies = Vec::new();

        let policies = self.policies.read().await;

        // Check each active policy
        for policy_id in &policies.active_policies {
            if let Some(policy) = policies.privacy_policies.get(policy_id) {
                applicable_policies.push(policy.name.clone());

                // Evaluate each rule in the policy
                for rule in &policy.rules {
                    if rule.enabled {
                        let rule_result = self.evaluate_rule(rule, transaction).await?;

                        if !rule_result.compliant {
                            violations.extend(rule_result.violations);
                            required_actions.extend(rule_result.actions);
                        }
                    }
                }
            }
        }

        let compliant = violations.is_empty();

        debug!("Policy evaluation completed: compliant = {}", compliant);

        Ok(PolicyResult {
            compliant,
            violations,
            required_actions,
            applicable_policies,
        })
    }

    /// Check consent for specific purpose
    #[instrument(skip(self))]
    pub async fn check_consent(&self, address: &Address, purpose: &str) -> Result<bool> {
        debug!("Checking consent for address {} and purpose {}", address, purpose);

        let consent_records = self.consent_manager.consent_records.read().await;

        if let Some(user_consent) = consent_records.get(address) {
            if let Some(consent_record) = user_consent.consents.get(purpose) {
                // Check if consent is still valid
                let now = SystemTime::now();
                let is_valid = consent_record.granted &&
                              consent_record.withdrawn_at.is_none() &&
                              consent_record.expires_at.map_or(true, |exp| exp > now);

                return Ok(is_valid);
            }
        }

        // No consent found
        Ok(false)
    }

    /// Grant consent for specific purpose
    #[instrument(skip(self))]
    pub async fn grant_consent(
        &self,
        address: &Address,
        purpose: &str,
        legal_basis: LegalBasis,
        expires_in: Option<Duration>,
    ) -> Result<()> {
        debug!("Granting consent for address {} and purpose {}", address, purpose);

        let mut consent_records = self.consent_manager.consent_records.write().await;

        let user_consent = consent_records.entry(address.clone()).or_insert_with(|| UserConsent {
            address: address.clone(),
            consents: HashMap::new(),
            last_updated: SystemTime::now(),
        });

        let expires_at = expires_in.map(|duration| SystemTime::now() + duration);

        let consent_record = ConsentRecord {
            purpose: purpose.to_string(),
            granted: true,
            granted_at: SystemTime::now(),
            expires_at,
            withdrawn_at: None,
            legal_basis,
        };

        user_consent.consents.insert(purpose.to_string(), consent_record);
        user_consent.last_updated = SystemTime::now();

        info!("Consent granted for address {} and purpose {}", address, purpose);
        Ok(())
    }

    /// Withdraw consent
    #[instrument(skip(self))]
    pub async fn withdraw_consent(&self, address: &Address, purpose: &str) -> Result<()> {
        debug!("Withdrawing consent for address {} and purpose {}", address, purpose);

        let mut consent_records = self.consent_manager.consent_records.write().await;

        if let Some(user_consent) = consent_records.get_mut(address) {
            if let Some(consent_record) = user_consent.consents.get_mut(purpose) {
                consent_record.withdrawn_at = Some(SystemTime::now());
                user_consent.last_updated = SystemTime::now();

                info!("Consent withdrawn for address {} and purpose {}", address, purpose);
            }
        }

        Ok(())
    }

    /// Add new privacy policy
    #[instrument(skip(self, policy))]
    pub async fn add_policy(&self, policy: PrivacyPolicy) -> Result<()> {
        debug!("Adding new privacy policy: {}", policy.name);

        let mut policies = self.policies.write().await;

        // Add to policy store
        policies.privacy_policies.insert(policy.id.clone(), policy.clone());

        // Add rules
        for rule in &policy.rules {
            policies.policy_rules.insert(rule.id.clone(), rule.clone());
        }

        // Activate if status is active
        if policy.status == PolicyStatus::Active {
            if !policies.active_policies.contains(&policy.id) {
                policies.active_policies.push(policy.id.clone());
            }
        }

        // Version tracking
        let version = PolicyVersion {
            version: policy.version.clone(),
            policy: policy.clone(),
            created_at: SystemTime::now(),
            deprecated_at: None,
        };

        policies.policy_versions
            .entry(policy.id.clone())
            .or_insert_with(Vec::new)
            .push(version);

        info!("Added privacy policy: {}", policy.name);
        Ok(())
    }

    /// Health check
    pub async fn is_healthy(&self) -> bool {
        let policies = self.policies.read().await;
        !policies.active_policies.is_empty()
    }

    async fn evaluate_rule(&self, rule: &PolicyRule, transaction: &Transaction) -> Result<RuleEvaluationResult> {
        let mut violations = Vec::new();
        let mut actions = Vec::new();

        // Check all conditions
        let mut all_conditions_met = true;

        for condition in &rule.conditions {
            let condition_met = self.evaluate_condition(condition, transaction).await?;
            if !condition_met {
                all_conditions_met = false;
                break;
            }
        }

        // If conditions are met, the rule applies
        if all_conditions_met {
            violations.push(format!("Policy rule violation: {}", rule.name));

            // Execute actions
            for action in &rule.actions {
                match action.action_type {
                    ActionType::Block => {
                        actions.push("Transaction blocked due to policy violation".to_string());
                    }
                    ActionType::RequireConsent => {
                        actions.push("User consent required".to_string());
                    }
                    ActionType::MinimizeData => {
                        actions.push("Data minimization required".to_string());
                    }
                    ActionType::AuditLog => {
                        actions.push("Audit logging required".to_string());
                    }
                    ActionType::Notify => {
                        actions.push("Notification required".to_string());
                    }
                    ActionType::Escalate => {
                        actions.push("Escalation required".to_string());
                    }
                }
            }
        }

        Ok(RuleEvaluationResult {
            compliant: violations.is_empty(),
            violations,
            actions,
        })
    }

    async fn evaluate_condition(&self, condition: &PolicyCondition, transaction: &Transaction) -> Result<bool> {
        // Extract field value from transaction
        let field_value = match condition.field.as_str() {
            "from_address" => serde_json::Value::String(transaction.from_address.to_string()),
            "to_address" => serde_json::Value::String(transaction.to_address.to_string()),
            "amount" => serde_json::Value::Number(serde_json::Number::from(transaction.amount.amount.to_u64())),
            "chain_id" => serde_json::Value::Number(serde_json::Number::from(transaction.chain_id)),
            _ => return Ok(false), // Unknown field
        };

        // Evaluate condition
        match condition.operator {
            ConditionOperator::Equals => Ok(field_value == condition.value),
            ConditionOperator::NotEquals => Ok(field_value != condition.value),
            ConditionOperator::GreaterThan => {
                if let (Some(field_num), Some(condition_num)) = (field_value.as_u64(), condition.value.as_u64()) {
                    Ok(field_num > condition_num)
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::LessThan => {
                if let (Some(field_num), Some(condition_num)) = (field_value.as_u64(), condition.value.as_u64()) {
                    Ok(field_num < condition_num)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false), // TODO: Implement other operators
        }
    }

    async fn load_default_policies(&mut self) -> Result<()> {
        // Create default GDPR compliance policy
        let gdpr_policy = PrivacyPolicy {
            id: "gdpr-compliance".to_string(),
            name: "GDPR Compliance Policy".to_string(),
            version: "1.0".to_string(),
            description: "General Data Protection Regulation compliance".to_string(),
            rules: vec![
                PolicyRule {
                    id: "consent-required".to_string(),
                    name: "Consent Required for Data Processing".to_string(),
                    rule_type: PolicyRuleType::ConsentValidation,
                    conditions: vec![],
                    actions: vec![
                        PolicyAction {
                            action_type: ActionType::RequireConsent,
                            parameters: HashMap::new(),
                            required: true,
                        }
                    ],
                    severity: PolicySeverity::High,
                    enabled: true,
                },
                PolicyRule {
                    id: "data-minimization".to_string(),
                    name: "Data Minimization Principle".to_string(),
                    rule_type: PolicyRuleType::DataMinimization,
                    conditions: vec![],
                    actions: vec![
                        PolicyAction {
                            action_type: ActionType::MinimizeData,
                            parameters: HashMap::new(),
                            required: true,
                        }
                    ],
                    severity: PolicySeverity::Medium,
                    enabled: true,
                },
            ],
            data_categories: vec![],
            retention_policy: RetentionPolicy {
                default_retention: Duration::from_secs(365 * 24 * 60 * 60), // 1 year
                category_specific: HashMap::new(),
                deletion_schedule: DeletionSchedule {
                    automatic_deletion: true,
                    deletion_intervals: vec![Duration::from_secs(30 * 24 * 60 * 60)], // 30 days
                    deletion_criteria: vec![],
                },
                archival_policy: ArchivalPolicy {
                    archive_after: Duration::from_secs(365 * 24 * 60 * 60), // 1 year
                    archive_location: "secure_archive".to_string(),
                    archive_encryption: true,
                    archive_retention: Duration::from_secs(7 * 365 * 24 * 60 * 60), // 7 years
                },
            },
            consent_requirements: ConsentRequirements {
                required_purposes: vec!["data_processing".to_string()],
                granular_consent: true,
                withdrawal_mechanism: true,
                consent_expiry: Some(Duration::from_secs(365 * 24 * 60 * 60)), // 1 year
                reconfirmation_period: Some(Duration::from_secs(180 * 24 * 60 * 60)), // 6 months
            },
            geographical_scope: vec!["EU".to_string()],
            created_at: SystemTime::now(),
            effective_date: SystemTime::now(),
            expiry_date: None,
            status: PolicyStatus::Active,
        };

        self.add_policy(gdpr_policy).await?;

        Ok(())
    }

    fn initialize_jurisdiction_rules() -> HashMap<String, JurisdictionRules> {
        let mut rules = HashMap::new();

        // EU GDPR
        rules.insert("EU".to_string(), JurisdictionRules {
            country_code: "EU".to_string(),
            gdpr_applicable: true,
            ccpa_applicable: false,
            data_localization_required: false,
            consent_age_minimum: 16,
            additional_requirements: vec!["right_to_be_forgotten".to_string()],
        });

        // US CCPA
        rules.insert("US".to_string(), JurisdictionRules {
            country_code: "US".to_string(),
            gdpr_applicable: false,
            ccpa_applicable: true,
            data_localization_required: false,
            consent_age_minimum: 13,
            additional_requirements: vec!["opt_out_rights".to_string()],
        });

        rules
    }
}

/// Rule evaluation result
#[derive(Debug)]
struct RuleEvaluationResult {
    compliant: bool,
    violations: Vec<String>,
    actions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TokenAmount, TokenType, U256};

    #[tokio::test]
    async fn test_policy_engine_creation() {
        let config = GuardianConfig::default();
        let engine = PolicyEngine::new(config).await.unwrap();
        assert!(engine.is_healthy().await);
    }

    #[tokio::test]
    async fn test_consent_management() {
        let config = GuardianConfig::default();
        let engine = PolicyEngine::new(config).await.unwrap();

        let address = Address::from("0x1234567890123456789012345678901234567890");

        // Grant consent
        engine.grant_consent(&address, "data_processing", LegalBasis::Consent, None).await.unwrap();

        // Check consent
        let has_consent = engine.check_consent(&address, "data_processing").await.unwrap();
        assert!(has_consent);

        // Withdraw consent
        engine.withdraw_consent(&address, "data_processing").await.unwrap();

        // Check consent again
        let has_consent = engine.check_consent(&address, "data_processing").await.unwrap();
        assert!(!has_consent);
    }
}