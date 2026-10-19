// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership. The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License. You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the
// specific language governing permissions and limitations
// under the License.

//! `fineract-loan-origination` — Rust port of Apache Fineract's `fineract-loan-origination` module.
//!
//! Provides:
//! - [`LoanOriginatorStatus`], [`LoanOriginator`], [`LoanOriginatorMapping`], and [`WorkingCapitalLoanOriginatorMapping`]
//! - [`LoanOriginatorAvroMapper`] and [`LoanOriginatorDetailsResolver`] for Avro event enrichment
//! - [`LoanOriginatorHelper`] for resolving or creating originators during loan application vs disbursement
//! - [`LoanOriginatorLinkingService`] for reconciling originator sets on loan applications and disbursements
//! - [`LoanOriginatorWritePlatformService`] and [`WorkingCapitalLoanOriginatorWritePlatformService`]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

pub const ENABLE_ORIGINATOR_CREATION_DURING_LOAN_APPLICATION: &str =
    "enable-originator-creation-during-loan-application";

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LoanOriginatorError {
    #[error("Loan originator with identifier {0} does not exist")]
    NotFound(String),
    #[error("Loan originator {id} is not active (current status: {status})")]
    NotActive { id: i64, status: String },
    #[error("Creating a new loan originator during loan application is not allowed for externalId: {0}")]
    CreationNotAllowed(String),
    #[error("Loan originator with externalId {0} already exists")]
    DuplicateExternalId(String),
    #[error("Invalid loan originator status: {0}")]
    InvalidStatus(String),
    #[error("Loan originator {0} cannot be deleted because it is mapped to one or more loans")]
    CannotBeDeleted(i64),
    #[error("Originator {originator_id} is already attached to loan {loan_id}")]
    MappingAlreadyExists { loan_id: i64, originator_id: i64 },
    #[error("Originator {originator_id} is not attached to loan {loan_id}")]
    MappingNotFound { loan_id: i64, originator_id: i64 },
    #[error("Loan {loan_id} is not in submitted and pending approval status (current: {status})")]
    LoanNotInSubmittedStatus { loan_id: i64, status: String },
    #[error("Working capital loan {loan_id} is not in submitted status for origination (current: {status})")]
    WorkingCapitalLoanNotInSubmittedStatus { loan_id: i64, status: String },
    #[error("Platform API data validation error on '{parameter}': {message}")]
    ValidationError { parameter: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LoanOriginatorStatus {
    Pending,
    Active,
    Inactive,
}

impl LoanOriginatorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
        }
    }

    pub fn from_str_case_insensitive(value: &str) -> Result<Self, LoanOriginatorError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "ACTIVE" => Ok(Self::Active),
            "INACTIVE" => Ok(Self::Inactive),
            other => Err(LoanOriginatorError::InvalidStatus(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeValue {
    pub id: i64,
    pub label: String,
    pub description: Option<String>,
    pub position: i32,
    pub active: bool,
    pub mandatory: bool,
}

impl CodeValue {
    pub fn new(id: i64, label: impl Into<String>, description: Option<String>) -> Self {
        Self {
            id,
            label: label.into(),
            description,
            position: 1,
            active: true,
            mandatory: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanOriginator {
    pub id: Option<i64>,
    pub external_id: Option<String>,
    pub name: Option<String>,
    pub status: LoanOriginatorStatus,
    pub originator_type: Option<CodeValue>,
    pub channel_type: Option<CodeValue>,
}

impl LoanOriginator {
    pub fn create(
        external_id: Option<String>,
        name: Option<String>,
        status: LoanOriginatorStatus,
        originator_type: Option<CodeValue>,
        channel_type: Option<CodeValue>,
    ) -> Self {
        Self {
            id: None,
            external_id,
            name,
            status,
            originator_type,
            channel_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanOriginatorMapping {
    pub id: Option<i64>,
    pub loan_id: i64,
    pub originator: Option<LoanOriginator>,
}

impl LoanOriginatorMapping {
    pub fn create(loan_id: i64, originator: LoanOriginator) -> Self {
        Self {
            id: None,
            loan_id,
            originator: Some(originator),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingCapitalLoanOriginatorMapping {
    pub id: Option<i64>,
    pub loan_id: i64,
    pub originator: Option<LoanOriginator>,
}

impl WorkingCapitalLoanOriginatorMapping {
    pub fn create(loan_id: i64, originator: LoanOriginator) -> Self {
        Self {
            id: None,
            loan_id,
            originator: Some(originator),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanApplicationOriginatorData {
    pub id: Option<i64>,
    pub external_id: Option<String>,
    pub name: Option<String>,
    pub originator_type_id: Option<i64>,
    pub channel_type_id: Option<i64>,
}

impl LoanApplicationOriginatorData {
    pub fn new(
        id: Option<i64>,
        external_id: Option<String>,
        name: Option<String>,
        originator_type_id: Option<i64>,
        channel_type_id: Option<i64>,
    ) -> Self {
        Self {
            id,
            external_id,
            name,
            originator_type_id,
            channel_type_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeValueV1 {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginatorDetailsV1 {
    pub id: Option<i64>,
    pub external_id: Option<String>,
    pub name: Option<String>,
    pub status: String,
    pub originator_type: Option<CodeValueV1>,
    pub channel_type: Option<CodeValueV1>,
}

/// Port of Fineract's `LoanOriginatorAvroMapper`.
#[derive(Debug, Default, Clone)]
pub struct LoanOriginatorAvroMapper;

impl LoanOriginatorAvroMapper {
    pub fn to_avro(&self, originator: Option<&LoanOriginator>) -> Option<OriginatorDetailsV1> {
        let orig = originator?;
        Some(OriginatorDetailsV1 {
            id: orig.id,
            external_id: orig.external_id.clone(),
            name: orig.name.clone(),
            status: orig.status.as_str().to_string(),
            originator_type: orig.originator_type.as_ref().map(|cv| CodeValueV1 {
                id: cv.id,
                name: cv.label.clone(),
                description: cv.description.clone(),
            }),
            channel_type: orig.channel_type.as_ref().map(|cv| CodeValueV1 {
                id: cv.id,
                name: cv.label.clone(),
                description: cv.description.clone(),
            }),
        })
    }
}

/// Port of Fineract's `LoanOriginatorDetailsResolver`.
pub struct LoanOriginatorDetailsResolver {
    mapper: LoanOriginatorAvroMapper,
}

impl Default for LoanOriginatorDetailsResolver {
    fn default() -> Self {
        Self::new(LoanOriginatorAvroMapper)
    }
}

impl LoanOriginatorDetailsResolver {
    pub fn new(mapper: LoanOriginatorAvroMapper) -> Self {
        Self { mapper }
    }

    pub fn resolve_originator_details(
        &self,
        mappings: Option<&[LoanOriginatorMapping]>,
    ) -> Vec<OriginatorDetailsV1> {
        let Some(list) = mappings else {
            return Vec::new();
        };
        if list.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for mapping in list {
            if let Some(ref originator) = mapping.originator {
                if let Some(details) = self.mapper.to_avro(Some(originator)) {
                    out.push(details);
                }
            }
        }
        out
    }
}

/// Port of Fineract's `LoanApplicationOriginatorDataValidator`.
#[derive(Debug, Default, Clone)]
pub struct LoanApplicationOriginatorDataValidator;

impl LoanApplicationOriginatorDataValidator {
    pub fn validate_and_extract(
        &self,
        obj: &Value,
    ) -> Result<LoanApplicationOriginatorData, LoanOriginatorError> {
        let Some(map) = obj.as_object() else {
            return Err(LoanOriginatorError::ValidationError {
                parameter: "originators".to_string(),
                message: "Each originator entry must be a JSON object".to_string(),
            });
        };
        let id = map.get("id").and_then(|v| v.as_i64());
        let external_id = map
            .get("externalId")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let name = map
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let originator_type_id = map.get("originatorTypeId").and_then(|v| v.as_i64());
        let channel_type_id = map.get("channelTypeId").and_then(|v| v.as_i64());

        if id.is_none() && external_id.is_none() {
            return Err(LoanOriginatorError::ValidationError {
                parameter: "originators".to_string(),
                message: "Either 'id' or 'externalId' must be provided".to_string(),
            });
        }
        Ok(LoanApplicationOriginatorData {
            id,
            external_id,
            name,
            originator_type_id,
            channel_type_id,
        })
    }
}

/// In-memory repository for `LoanOriginator` entities and mappings.
#[derive(Debug, Default, Clone)]
pub struct InMemoryLoanOriginatorStore {
    next_originator_id: i64,
    next_mapping_id: i64,
    pub originators: HashMap<i64, LoanOriginator>,
    pub loan_mappings: Vec<LoanOriginatorMapping>,
    pub wc_loan_mappings: Vec<WorkingCapitalLoanOriginatorMapping>,
    pub enable_creation_during_loan_application: bool,
    pub code_values: HashMap<i64, CodeValue>,
}

impl InMemoryLoanOriginatorStore {
    pub fn new() -> Self {
        Self {
            next_originator_id: 1,
            next_mapping_id: 1,
            originators: HashMap::new(),
            loan_mappings: Vec::new(),
            wc_loan_mappings: Vec::new(),
            enable_creation_during_loan_application: false,
            code_values: HashMap::new(),
        }
    }

    pub fn save_originator(&mut self, mut originator: LoanOriginator) -> LoanOriginator {
        let id = originator.id.unwrap_or_else(|| {
            let assigned = self.next_originator_id;
            self.next_originator_id += 1;
            assigned
        });
        if id >= self.next_originator_id {
            self.next_originator_id = id + 1;
        }
        originator.id = Some(id);
        self.originators.insert(id, originator.clone());
        originator
    }

    pub fn find_by_external_id(&self, external_id: &str) -> Option<LoanOriginator> {
        self.originators
            .values()
            .find(|o| o.external_id.as_deref() == Some(external_id))
            .cloned()
    }

    pub fn find_by_id(&self, id: i64) -> Option<LoanOriginator> {
        self.originators.get(&id).cloned()
    }

    pub fn find_mappings_by_loan_id(&self, loan_id: i64) -> Vec<LoanOriginatorMapping> {
        self.loan_mappings
            .iter()
            .filter(|m| m.loan_id == loan_id)
            .cloned()
            .collect()
    }
}

/// Port of Fineract's `LoanOriginatorHelper`.
#[derive(Debug, Default, Clone)]
pub struct LoanOriginatorHelper;

impl LoanOriginatorHelper {
    pub fn find_or_create_originator_id(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        data: &LoanApplicationOriginatorData,
    ) -> Result<i64, LoanOriginatorError> {
        self.resolve_or_create(store, data, false)
    }

    pub fn find_or_create_originator_id_for_loan_disbursement(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        data: &LoanApplicationOriginatorData,
    ) -> Result<i64, LoanOriginatorError> {
        self.resolve_or_create(store, data, true)
    }

    fn resolve_or_create(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        data: &LoanApplicationOriginatorData,
        allow_unconditional_creation: bool,
    ) -> Result<i64, LoanOriginatorError> {
        let ext_id = data.external_id.as_deref().ok_or_else(|| {
            LoanOriginatorError::ValidationError {
                parameter: "externalId".to_string(),
                message: "externalId is required".to_string(),
            }
        })?;

        if let Some(existing) = store.find_by_external_id(ext_id) {
            let id = existing.id.unwrap_or(0);
            if existing.status != LoanOriginatorStatus::Active {
                return Err(LoanOriginatorError::NotActive {
                    id,
                    status: existing.status.as_str().to_string(),
                });
            }
            return Ok(id);
        }

        if !allow_unconditional_creation && !store.enable_creation_during_loan_application {
            return Err(LoanOriginatorError::CreationNotAllowed(ext_id.to_string()));
        }

        let orig_type = data
            .originator_type_id
            .and_then(|id| store.code_values.get(&id).cloned());
        let chan_type = data
            .channel_type_id
            .and_then(|id| store.code_values.get(&id).cloned());
        let created = LoanOriginator::create(
            Some(ext_id.to_string()),
            data.name.clone().or_else(|| Some(ext_id.to_string())),
            LoanOriginatorStatus::Active,
            orig_type,
            chan_type,
        );
        let saved = store.save_originator(created);
        Ok(saved.id.unwrap())
    }
}

/// Port of Fineract's `LoanOriginatorLinkingServiceImpl`.
#[derive(Debug, Default, Clone)]
pub struct LoanOriginatorLinkingService {
    validator: LoanApplicationOriginatorDataValidator,
    helper: LoanOriginatorHelper,
}

impl LoanOriginatorLinkingService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconciles originators during loan disbursement:
    /// - `None` array -> no-op
    /// - `Some(&[])` -> detaches all existing mappings for `loan_id`
    /// - `Some(items)` -> validates all entries, resolves/creates by `id` or `externalId`, detaches removed mappings, and saves newly added mappings.
    pub fn process_originators_for_loan_disbursement(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        loan_id: i64,
        originators_array: Option<&[Value]>,
    ) -> Result<(), LoanOriginatorError> {
        let Some(entries) = originators_array else {
            return Ok(());
        };

        if entries.is_empty() {
            store.loan_mappings.retain(|m| m.loan_id != loan_id);
            return Ok(());
        }

        let mut target_originator_ids = Vec::new();
        let mut seen = HashSet::new();
        for entry in entries {
            let data = self.validator.validate_and_extract(entry)?;
            let resolved_id = if let Some(id) = data.id {
                let existing = store
                    .find_by_id(id)
                    .ok_or_else(|| LoanOriginatorError::NotFound(id.to_string()))?;
                if existing.status != LoanOriginatorStatus::Active {
                    return Err(LoanOriginatorError::NotActive {
                        id,
                        status: existing.status.as_str().to_string(),
                    });
                }
                id
            } else {
                self.helper
                    .find_or_create_originator_id_for_loan_disbursement(store, &data)?
            };
            if seen.insert(resolved_id) {
                target_originator_ids.push(resolved_id);
            }
        }

        let target_set: HashSet<i64> = target_originator_ids.iter().copied().collect();
        let existing_mappings = store.find_mappings_by_loan_id(loan_id);
        let existing_ids: HashSet<i64> = existing_mappings
            .iter()
            .filter_map(|m| m.originator.as_ref().and_then(|o| o.id))
            .collect();

        // Remove stale mappings not in target_set
        store.loan_mappings.retain(|m| {
            if m.loan_id != loan_id {
                return true;
            }
            let Some(orig_id) = m.originator.as_ref().and_then(|o| o.id) else {
                return false;
            };
            target_set.contains(&orig_id)
        });

        // Add new mappings
        for orig_id in target_originator_ids {
            if !existing_ids.contains(&orig_id) {
                if let Some(originator) = store.find_by_id(orig_id) {
                    let mut mapping = LoanOriginatorMapping::create(loan_id, originator);
                    mapping.id = Some(store.next_mapping_id);
                    store.next_mapping_id += 1;
                    store.loan_mappings.push(mapping);
                }
            }
        }

        Ok(())
    }

    pub fn attach_originator_to_loan(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        loan_id: i64,
        loan_is_submitted_and_pending_approval: bool,
        loan_status_code: &str,
        originator_id: i64,
    ) -> Result<LoanOriginatorMapping, LoanOriginatorError> {
        if !loan_is_submitted_and_pending_approval {
            return Err(LoanOriginatorError::LoanNotInSubmittedStatus {
                loan_id,
                status: loan_status_code.to_string(),
            });
        }
        let originator = store
            .find_by_id(originator_id)
            .ok_or_else(|| LoanOriginatorError::NotFound(originator_id.to_string()))?;
        if originator.status != LoanOriginatorStatus::Active {
            return Err(LoanOriginatorError::NotActive {
                id: originator_id,
                status: originator.status.as_str().to_string(),
            });
        }
        if store.loan_mappings.iter().any(|m| {
            m.loan_id == loan_id && m.originator.as_ref().and_then(|o| o.id) == Some(originator_id)
        }) {
            return Err(LoanOriginatorError::MappingAlreadyExists {
                loan_id,
                originator_id,
            });
        }
        let mut mapping = LoanOriginatorMapping::create(loan_id, originator);
        mapping.id = Some(store.next_mapping_id);
        store.next_mapping_id += 1;
        store.loan_mappings.push(mapping.clone());
        Ok(mapping)
    }

    pub fn detach_originator_from_loan(
        &self,
        store: &mut InMemoryLoanOriginatorStore,
        loan_id: i64,
        loan_is_submitted_and_pending_approval: bool,
        loan_status_code: &str,
        originator_id: i64,
    ) -> Result<(), LoanOriginatorError> {
        if !loan_is_submitted_and_pending_approval {
            return Err(LoanOriginatorError::LoanNotInSubmittedStatus {
                loan_id,
                status: loan_status_code.to_string(),
            });
        }
        let _originator = store
            .find_by_id(originator_id)
            .ok_or_else(|| LoanOriginatorError::NotFound(originator_id.to_string()))?;
        let idx = store
            .loan_mappings
            .iter()
            .position(|m| {
                m.loan_id == loan_id
                    && m.originator.as_ref().and_then(|o| o.id) == Some(originator_id)
            })
            .ok_or(LoanOriginatorError::MappingNotFound {
                loan_id,
                originator_id,
            })?;
        store.loan_mappings.remove(idx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn find_or_create_originator_id_for_loan_disbursement_returns_id_when_active() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let mut orig = LoanOriginator::create(
            Some("EXT-A".to_string()),
            Some("EXT-A".to_string()),
            LoanOriginatorStatus::Active,
            None,
            None,
        );
        orig.id = Some(7);
        store.save_originator(orig);

        let helper = LoanOriginatorHelper;
        let data = LoanApplicationOriginatorData::new(
            None,
            Some("EXT-A".to_string()),
            Some("n".to_string()),
            None,
            None,
        );
        let id = helper
            .find_or_create_originator_id_for_loan_disbursement(&mut store, &data)
            .unwrap();
        assert_eq!(id, 7);
    }

    #[test]
    fn find_or_create_originator_id_for_loan_disbursement_throws_not_active_when_inactive() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let mut orig = LoanOriginator::create(
            Some("EXT-B".to_string()),
            Some("EXT-B".to_string()),
            LoanOriginatorStatus::Inactive,
            None,
            None,
        );
        orig.id = Some(8);
        store.save_originator(orig);

        let helper = LoanOriginatorHelper;
        let data = LoanApplicationOriginatorData::new(
            None,
            Some("EXT-B".to_string()),
            Some("n".to_string()),
            None,
            None,
        );
        let err = helper
            .find_or_create_originator_id_for_loan_disbursement(&mut store, &data)
            .unwrap_err();
        assert_eq!(
            err,
            LoanOriginatorError::NotActive {
                id: 8,
                status: "INACTIVE".to_string()
            }
        );
    }

    #[test]
    fn find_or_create_originator_id_for_loan_disbursement_creates_new_when_missing() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let helper = LoanOriginatorHelper;
        let data = LoanApplicationOriginatorData::new(
            None,
            Some("EXT-NEW".to_string()),
            Some("New Co".to_string()),
            None,
            None,
        );
        let id = helper
            .find_or_create_originator_id_for_loan_disbursement(&mut store, &data)
            .unwrap();
        let created = store.find_by_id(id).unwrap();
        assert_eq!(created.status, LoanOriginatorStatus::Active);
        assert_eq!(created.external_id.as_deref(), Some("EXT-NEW"));
    }

    #[test]
    fn find_or_create_originator_id_throws_creation_not_allowed_when_flag_disabled() {
        let mut store = InMemoryLoanOriginatorStore::new();
        store.enable_creation_during_loan_application = false;
        let helper = LoanOriginatorHelper;
        let data = LoanApplicationOriginatorData::new(
            None,
            Some("EXT-APP".to_string()),
            Some("n".to_string()),
            None,
            None,
        );
        let err = helper
            .find_or_create_originator_id(&mut store, &data)
            .unwrap_err();
        assert_eq!(
            err,
            LoanOriginatorError::CreationNotAllowed("EXT-APP".to_string())
        );
    }

    #[test]
    fn process_originators_for_loan_disbursement_reconciles_and_detaches() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let service = LoanOriginatorLinkingService::new();

        for (id, ext) in [(1, "originator-a"), (2, "originator-b"), (3, "originator-c")] {
            let mut o = LoanOriginator::create(
                Some(ext.to_string()),
                Some(ext.to_string()),
                LoanOriginatorStatus::Active,
                None,
                None,
            );
            o.id = Some(id);
            store.save_originator(o);
        }

        // Initial mappings: A (1) and B (2)
        store.loan_mappings.push(LoanOriginatorMapping::create(
            100,
            store.find_by_id(1).unwrap(),
        ));
        store.loan_mappings.push(LoanOriginatorMapping::create(
            100,
            store.find_by_id(2).unwrap(),
        ));

        // Reconcile with [2, 3]: should remove 1, keep 2, add 3
        let payload = vec![json!({"id": 2}), json!({"id": 3})];
        service
            .process_originators_for_loan_disbursement(&mut store, 100, Some(&payload))
            .unwrap();

        let mappings = store.find_mappings_by_loan_id(100);
        let ids: Vec<i64> = mappings
            .iter()
            .filter_map(|m| m.originator.as_ref().and_then(|o| o.id))
            .collect();
        assert_eq!(ids, vec![2, 3]);

        // Empty array detaches all existing mappings
        service
            .process_originators_for_loan_disbursement(&mut store, 100, Some(&[]))
            .unwrap();
        assert!(store.find_mappings_by_loan_id(100).is_empty());
    }

    #[test]
    fn process_originators_for_loan_disbursement_fails_fast_for_malformed_or_inactive() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let mut inactive = LoanOriginator::create(
            Some("inactive".to_string()),
            Some("inactive".to_string()),
            LoanOriginatorStatus::Inactive,
            None,
            None,
        );
        inactive.id = Some(2);
        store.save_originator(inactive);

        let service = LoanOriginatorLinkingService::new();
        let malformed = vec![json!("not-an-object")];
        assert!(matches!(
            service.process_originators_for_loan_disbursement(&mut store, 100, Some(&malformed)),
            Err(LoanOriginatorError::ValidationError { .. })
        ));

        let inactive_payload = vec![json!({"id": 2})];
        assert_eq!(
            service
                .process_originators_for_loan_disbursement(&mut store, 100, Some(&inactive_payload))
                .unwrap_err(),
            LoanOriginatorError::NotActive {
                id: 2,
                status: "INACTIVE".to_string()
            }
        );
    }

    #[test]
    fn avro_mapper_and_details_resolver_work_as_expected() {
        let orig_type = CodeValue::new(1, "MERCHANT", Some("Merchant".to_string()));
        let chan_type = CodeValue::new(2, "ONLINE", Some("Online".to_string()));
        let mut orig = LoanOriginator::create(
            Some("test-external-id".to_string()),
            Some("Test Originator".to_string()),
            LoanOriginatorStatus::Active,
            Some(orig_type),
            Some(chan_type),
        );
        orig.id = Some(100);

        let mapper = LoanOriginatorAvroMapper;
        let avro = mapper.to_avro(Some(&orig)).unwrap();
        assert_eq!(avro.id, Some(100));
        assert_eq!(avro.external_id.as_deref(), Some("test-external-id"));
        assert_eq!(avro.status, "ACTIVE");
        assert_eq!(avro.originator_type.as_ref().unwrap().name, "MERCHANT");
        assert_eq!(avro.channel_type.as_ref().unwrap().name, "ONLINE");
        assert!(mapper.to_avro(None).is_none());

        let resolver = LoanOriginatorDetailsResolver::default();
        let mappings = vec![LoanOriginatorMapping::create(1, orig)];
        let resolved = resolver.resolve_originator_details(Some(&mappings));
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].external_id.as_deref(), Some("test-external-id"));
        assert!(resolver.resolve_originator_details(None).is_empty());
        assert!(resolver.resolve_originator_details(Some(&[])).is_empty());
    }

    #[test]
    fn attach_and_detach_originator_enforces_submitted_loan_status() {
        let mut store = InMemoryLoanOriginatorStore::new();
        let mut orig = LoanOriginator::create(
            Some("EXT-1".to_string()),
            Some("Orig 1".to_string()),
            LoanOriginatorStatus::Active,
            None,
            None,
        );
        orig.id = Some(10);
        store.save_originator(orig);

        let service = LoanOriginatorLinkingService::new();
        // Rejects non-submitted loan
        assert_eq!(
            service
                .attach_originator_to_loan(&mut store, 50, false, "loanStatusType.active", 10)
                .unwrap_err(),
            LoanOriginatorError::LoanNotInSubmittedStatus {
                loan_id: 50,
                status: "loanStatusType.active".to_string()
            }
        );

        // Attaches when submitted
        service
            .attach_originator_to_loan(
                &mut store,
                50,
                true,
                "loanStatusType.submitted.and.pending.approval",
                10,
            )
            .unwrap();
        // Duplicate attach fails
        assert_eq!(
            service
                .attach_originator_to_loan(
                    &mut store,
                    50,
                    true,
                    "loanStatusType.submitted.and.pending.approval",
                    10,
                )
                .unwrap_err(),
            LoanOriginatorError::MappingAlreadyExists {
                loan_id: 50,
                originator_id: 10
            }
        );

        // Detach succeeds
        service
            .detach_originator_from_loan(
                &mut store,
                50,
                true,
                "loanStatusType.submitted.and.pending.approval",
                10,
            )
            .unwrap();
        assert!(store.find_mappings_by_loan_id(50).is_empty());
    }
}
