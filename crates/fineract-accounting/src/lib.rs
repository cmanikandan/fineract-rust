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

//! `fineract-accounting` — Rust port of Apache Fineract's `fineract-accounting` module.
//!
//! Provides:
//! - [`GLAccountType`], [`GLAccountUsage`], [`GLAccount`], and [`GLAccountWritePlatformService`]
//! - [`GLClosure`] and [`GLClosureWritePlatformService`] (branch accounting closure guards)
//! - [`FinancialActivity`] and [`FinancialActivityAccount`]
//! - [`AccountingRule`], [`AccountingTagRule`], [`AccountRuleRequest`], and [`AccountingRuleValidator`]
//! - [`JournalEntryType`], [`JournalEntry`], [`JournalEntryCommand`], [`JournalEntryWritePlatformService`],
//!   [`JournalEntryRunningBalanceUpdateService`], and [`TrialBalance`]
//! - [`ProvisioningEntriesDefinitionValidator`]

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum AccountingError {
    #[error("GL Account {0} not found")]
    GLAccountNotFound(i64),
    #[error("Duplicate GL code '{0}'")]
    GLAccountDuplicateCode(String),
    #[error("Cannot delete GL Account {id}: {reason}")]
    GLAccountInvalidDelete { id: i64, reason: String },
    #[error("Cannot post journal entry to header or disabled GL Account {0}")]
    GLAccountInvalidUsage(i64),
    #[error("Parent GL Account {parent_id} must be a HEADER account of matching classification")]
    GLAccountInvalidParent { parent_id: i64 },
    #[error("Accounting closure for office {office_id} on {closing_date} already exists")]
    GLClosureDuplicate {
        office_id: i64,
        closing_date: NaiveDate,
    },
    #[error("Accounting closure cannot be created for a future date {0}")]
    GLClosureFutureDate(NaiveDate),
    #[error("Accounting is closed for office {office_id} on or before {closing_date}; cannot post on {transaction_date}")]
    BranchAccountingClosed {
        office_id: i64,
        closing_date: NaiveDate,
        transaction_date: NaiveDate,
    },
    #[error("Cannot delete closure on {closing_date} because a later closure exists on {latest_date}")]
    GLClosureInvalidDelete {
        closing_date: NaiveDate,
        latest_date: NaiveDate,
    },
    #[error("Financial activity {activity:?} requires a GL account of type {expected:?}, got {actual:?}")]
    FinancialActivityInvalidAccountType {
        activity: FinancialActivity,
        expected: GLAccountType,
        actual: GLAccountType,
    },
    #[error("Duplicate financial activity account mapping for activity {0:?}")]
    DuplicateFinancialActivityAccount(FinancialActivity),
    #[error("Journal entry validation failed: {0}")]
    JournalEntryInvalid(String),
    #[error("Platform API data validation error on '{parameter}': {message}")]
    ValidationError { parameter: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GLAccountType {
    Asset = 1,
    Liability = 2,
    Equity = 3,
    Income = 4,
    Expense = 5,
}

impl GLAccountType {
    /// Asset and Expense accounts increase with Debits; Liability, Equity, and Income increase with Credits.
    pub fn is_debit_normal(&self) -> bool {
        matches!(self, Self::Asset | Self::Expense)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GLAccountUsage {
    Detail = 1,
    Header = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GLAccount {
    pub id: i64,
    pub name: String,
    pub gl_code: String,
    pub disabled: bool,
    pub manual_entries_allowed: bool,
    pub account_type: GLAccountType,
    pub usage: GLAccountUsage,
    pub parent_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub description: Option<String>,
}

impl GLAccount {
    pub fn new_detail(
        id: i64,
        name: impl Into<String>,
        gl_code: impl Into<String>,
        account_type: GLAccountType,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            gl_code: gl_code.into(),
            disabled: false,
            manual_entries_allowed: true,
            account_type,
            usage: GLAccountUsage::Detail,
            parent_id: None,
            tag_id: None,
            description: None,
        }
    }

    pub fn new_header(
        id: i64,
        name: impl Into<String>,
        gl_code: impl Into<String>,
        account_type: GLAccountType,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            gl_code: gl_code.into(),
            disabled: false,
            manual_entries_allowed: false,
            account_type,
            usage: GLAccountUsage::Header,
            parent_id: None,
            tag_id: None,
            description: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinancialActivity {
    AssetFundSource = 100,
    CashAtMainvault = 101,
    CashAtTeller = 102,
    AssetTransfer = 103,
    LiabilityTransfer = 200,
    PayableDividends = 201,
    OpeningBalancesTransferContra = 300,
}

impl FinancialActivity {
    pub fn mapped_gl_account_type(&self) -> GLAccountType {
        match self {
            Self::AssetFundSource
            | Self::CashAtMainvault
            | Self::CashAtTeller
            | Self::AssetTransfer => GLAccountType::Asset,
            Self::LiabilityTransfer | Self::PayableDividends => GLAccountType::Liability,
            Self::OpeningBalancesTransferContra => GLAccountType::Equity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinancialActivityAccount {
    pub id: i64,
    pub gl_account_id: i64,
    pub financial_activity: FinancialActivity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GLClosure {
    pub id: i64,
    pub office_id: i64,
    pub closing_date: NaiveDate,
    pub deleted: bool,
    pub comments: Option<String>,
}

impl GLClosure {
    pub fn new(id: i64, office_id: i64, closing_date: NaiveDate, comments: Option<String>) -> Self {
        Self {
            id,
            office_id,
            closing_date,
            deleted: false,
            comments: comments.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()),
        }
    }
}

/// Port of Fineract's `AccountRuleRequest` and `AccountingRuleCommandFromApiJsonDeserializer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRuleRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub office_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_to_debit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_to_credit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_tags: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_tags: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_multiple_credit_entries: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_multiple_debit_entries: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct AccountingRuleCommandValidator;

impl AccountingRuleCommandValidator {
    pub fn validate_for_create(&self, json_str: &str) -> Result<AccountRuleRequest, AccountingError> {
        let req: AccountRuleRequest =
            serde_json::from_str(json_str).map_err(|e| AccountingError::ValidationError {
                parameter: "json".to_string(),
                message: e.to_string(),
            })?;
        if req.name.as_deref().unwrap_or("").trim().is_empty() {
            return Err(AccountingError::ValidationError {
                parameter: "name".to_string(),
                message: "name is mandatory".to_string(),
            });
        }
        if req.office_id.is_none() {
            return Err(AccountingError::ValidationError {
                parameter: "officeId".to_string(),
                message: "officeId is mandatory".to_string(),
            });
        }
        let has_debit = req.account_to_debit.is_some()
            || req
                .debit_tags
                .as_ref()
                .map(|t| !t.is_empty())
                .unwrap_or(false);
        let has_credit = req.account_to_credit.is_some()
            || req
                .credit_tags
                .as_ref()
                .map(|t| !t.is_empty())
                .unwrap_or(false);
        if !has_debit || !has_credit {
            return Err(AccountingError::ValidationError {
                parameter: "accountToDebitOrCredit".to_string(),
                message: "Both debit (account or tags) and credit (account or tags) must be specified".to_string(),
            });
        }
        Ok(req)
    }

    pub fn validate_for_update(&self, json_str: &str) -> Result<AccountRuleRequest, AccountingError> {
        self.validate_for_create(json_str)
    }
}

/// Port of Fineract's `ProvisioningEntriesDefinitionJsonDeserializer`.
#[derive(Debug, Default, Clone)]
pub struct ProvisioningEntriesDefinitionJsonDeserializer;

impl ProvisioningEntriesDefinitionJsonDeserializer {
    pub fn validate_for_create(&self, json_str: &str) -> Result<(), AccountingError> {
        let val: Value =
            serde_json::from_str(json_str).map_err(|e| AccountingError::ValidationError {
                parameter: "json".to_string(),
                message: e.to_string(),
            })?;
        let Some(obj) = val.as_object() else {
            return Err(AccountingError::ValidationError {
                parameter: "json".to_string(),
                message: "Expected JSON object".to_string(),
            });
        };
        if !obj.contains_key("dateFormat")
            || obj
                .get("dateFormat")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(AccountingError::ValidationError {
                parameter: "dateFormat".to_string(),
                message: "The parameter dateFormat is mandatory.".to_string(),
            });
        }
        if !obj.contains_key("locale")
            || obj
                .get("locale")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(AccountingError::ValidationError {
                parameter: "locale".to_string(),
                message: "The parameter locale is mandatory.".to_string(),
            });
        }
        if !obj.contains_key("date")
            || obj
                .get("date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(AccountingError::ValidationError {
                parameter: "date".to_string(),
                message: "The parameter date is mandatory.".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JournalEntryType {
    Credit = 1,
    Debit = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SingleDebitOrCreditEntryCommand {
    pub gl_account_id: i64,
    pub amount: Decimal,
    pub comments: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntryCommand {
    pub office_id: i64,
    pub currency_code: String,
    pub transaction_date: NaiveDate,
    pub credits: Vec<SingleDebitOrCreditEntryCommand>,
    pub debits: Vec<SingleDebitOrCreditEntryCommand>,
    pub reference_number: Option<String>,
    pub comments: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: i64,
    pub office_id: i64,
    pub gl_account_id: i64,
    pub currency_code: String,
    pub transaction_id: String,
    pub transaction_date: NaiveDate,
    pub entry_type: JournalEntryType,
    pub amount: Decimal,
    pub description: Option<String>,
    pub reversed: bool,
    pub manual_entry: bool,
    pub office_running_balance: Decimal,
    pub organization_running_balance: Decimal,
}

impl JournalEntry {
    pub fn is_debit_entry(&self) -> bool {
        self.entry_type == JournalEntryType::Debit
    }

    pub fn is_credit_entry(&self) -> bool {
        self.entry_type == JournalEntryType::Credit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialBalance {
    pub office_id: i64,
    pub gl_account_id: i64,
    pub amount: Decimal,
    pub entry_date: NaiveDate,
    pub closing_balance: Decimal,
}

/// Unified General Ledger & Journal Entry Engine porting:
/// - `GLAccountWritePlatformServiceJpaRepositoryImpl`
/// - `GLClosureWritePlatformServiceJpaRepositoryImpl`
/// - `FinancialActivityAccountWritePlatformServiceImpl`
/// - `JournalEntryWritePlatformServiceJpaRepositoryImpl`
/// - `JournalEntryRunningBalanceUpdateServiceImpl`
#[derive(Debug, Default, Clone)]
pub struct AccountingLedgerEngine {
    next_entry_id: i64,
    pub gl_accounts: HashMap<i64, GLAccount>,
    pub closures: Vec<GLClosure>,
    pub financial_activity_accounts: Vec<FinancialActivityAccount>,
    pub product_mappings: Vec<(i64, i64)>, // (product_id, gl_account_id)
    pub journal_entries: Vec<JournalEntry>,
}

impl AccountingLedgerEngine {
    pub fn new() -> Self {
        Self {
            next_entry_id: 1,
            gl_accounts: HashMap::new(),
            closures: Vec::new(),
            financial_activity_accounts: Vec::new(),
            product_mappings: Vec::new(),
            journal_entries: Vec::new(),
        }
    }

    pub fn create_gl_account(&mut self, account: GLAccount) -> Result<i64, AccountingError> {
        if self
            .gl_accounts
            .values()
            .any(|a| a.gl_code.eq_ignore_ascii_case(&account.gl_code))
        {
            return Err(AccountingError::GLAccountDuplicateCode(account.gl_code));
        }
        if let Some(parent_id) = account.parent_id {
            let parent = self
                .gl_accounts
                .get(&parent_id)
                .ok_or(AccountingError::GLAccountNotFound(parent_id))?;
            if parent.usage != GLAccountUsage::Header || parent.account_type != account.account_type
            {
                return Err(AccountingError::GLAccountInvalidParent { parent_id });
            }
        }
        let id = account.id;
        self.gl_accounts.insert(id, account);
        Ok(id)
    }

    pub fn delete_gl_account(&mut self, gl_account_id: i64) -> Result<i64, AccountingError> {
        if !self.gl_accounts.contains_key(&gl_account_id) {
            return Err(AccountingError::GLAccountNotFound(gl_account_id));
        }
        if self
            .gl_accounts
            .values()
            .any(|a| a.parent_id == Some(gl_account_id))
        {
            return Err(AccountingError::GLAccountInvalidDelete {
                id: gl_account_id,
                reason: "Account has child GL accounts".to_string(),
            });
        }
        if self
            .journal_entries
            .iter()
            .any(|j| j.gl_account_id == gl_account_id)
        {
            return Err(AccountingError::GLAccountInvalidDelete {
                id: gl_account_id,
                reason: "Account has journal entries".to_string(),
            });
        }
        if self
            .product_mappings
            .iter()
            .any(|(_, acc_id)| *acc_id == gl_account_id)
        {
            return Err(AccountingError::GLAccountInvalidDelete {
                id: gl_account_id,
                reason: "Account is mapped to a financial product".to_string(),
            });
        }
        self.gl_accounts.remove(&gl_account_id);
        Ok(gl_account_id)
    }

    pub fn create_gl_closure(
        &mut self,
        closure: GLClosure,
        business_date: NaiveDate,
    ) -> Result<i64, AccountingError> {
        if closure.closing_date > business_date {
            return Err(AccountingError::GLClosureFutureDate(closure.closing_date));
        }
        if self.closures.iter().any(|c| {
            !c.deleted && c.office_id == closure.office_id && c.closing_date == closure.closing_date
        }) {
            return Err(AccountingError::GLClosureDuplicate {
                office_id: closure.office_id,
                closing_date: closure.closing_date,
            });
        }
        let id = closure.id;
        self.closures.push(closure);
        Ok(id)
    }

    pub fn delete_gl_closure(&mut self, closure_id: i64) -> Result<(), AccountingError> {
        let target = self
            .closures
            .iter()
            .find(|c| c.id == closure_id && !c.deleted)
            .cloned()
            .ok_or_else(|| {
                AccountingError::JournalEntryInvalid(format!("Closure {closure_id} not found"))
            })?;

        if let Some(latest) = self
            .closures
            .iter()
            .filter(|c| !c.deleted && c.office_id == target.office_id)
            .map(|c| c.closing_date)
            .max()
        {
            if latest > target.closing_date {
                return Err(AccountingError::GLClosureInvalidDelete {
                    closing_date: target.closing_date,
                    latest_date: latest,
                });
            }
        }
        if let Some(c) = self.closures.iter_mut().find(|c| c.id == closure_id) {
            c.deleted = true;
        }
        Ok(())
    }

    pub fn create_financial_activity_account(
        &mut self,
        id: i64,
        financial_activity: FinancialActivity,
        gl_account_id: i64,
    ) -> Result<FinancialActivityAccount, AccountingError> {
        if self
            .financial_activity_accounts
            .iter()
            .any(|fa| fa.financial_activity == financial_activity)
        {
            return Err(AccountingError::DuplicateFinancialActivityAccount(
                financial_activity,
            ));
        }
        let gl_account = self
            .gl_accounts
            .get(&gl_account_id)
            .ok_or(AccountingError::GLAccountNotFound(gl_account_id))?;
        let expected = financial_activity.mapped_gl_account_type();
        if gl_account.account_type != expected {
            return Err(AccountingError::FinancialActivityInvalidAccountType {
                activity: financial_activity,
                expected,
                actual: gl_account.account_type,
            });
        }
        let mapping = FinancialActivityAccount {
            id,
            gl_account_id,
            financial_activity,
        };
        self.financial_activity_accounts.push(mapping.clone());
        Ok(mapping)
    }

    /// Creates a balanced double-entry journal batch, enforcing:
    /// - Branch closure date check
    /// - Non-empty debits and credits with positive amounts
    /// - Total debits == Total credits
    /// - GLAccounts exist, are not disabled, are `DETAIL` usage, and allow manual entries when `manual_entry == true`
    pub fn create_journal_entries(
        &mut self,
        cmd: &JournalEntryCommand,
        transaction_id: impl Into<String>,
        manual_entry: bool,
    ) -> Result<String, AccountingError> {
        // 1. Check branch closures
        if let Some(latest_closure) = self
            .closures
            .iter()
            .filter(|c| !c.deleted && c.office_id == cmd.office_id)
            .map(|c| c.closing_date)
            .max()
        {
            if cmd.transaction_date <= latest_closure {
                return Err(AccountingError::BranchAccountingClosed {
                    office_id: cmd.office_id,
                    closing_date: latest_closure,
                    transaction_date: cmd.transaction_date,
                });
            }
        }

        if cmd.debits.is_empty() || cmd.credits.is_empty() {
            return Err(AccountingError::JournalEntryInvalid(
                "Both debits and credits must be non-empty".to_string(),
            ));
        }

        let mut total_debit = Decimal::ZERO;
        for d in &cmd.debits {
            if d.amount <= Decimal::ZERO {
                return Err(AccountingError::JournalEntryInvalid(
                    "Debit amount must be greater than zero".to_string(),
                ));
            }
            let acc = self
                .gl_accounts
                .get(&d.gl_account_id)
                .ok_or(AccountingError::GLAccountNotFound(d.gl_account_id))?;
            if acc.disabled
                || acc.usage != GLAccountUsage::Detail
                || (manual_entry && !acc.manual_entries_allowed)
            {
                return Err(AccountingError::GLAccountInvalidUsage(d.gl_account_id));
            }
            total_debit += d.amount;
        }

        let mut total_credit = Decimal::ZERO;
        for c in &cmd.credits {
            if c.amount <= Decimal::ZERO {
                return Err(AccountingError::JournalEntryInvalid(
                    "Credit amount must be greater than zero".to_string(),
                ));
            }
            let acc = self
                .gl_accounts
                .get(&c.gl_account_id)
                .ok_or(AccountingError::GLAccountNotFound(c.gl_account_id))?;
            if acc.disabled
                || acc.usage != GLAccountUsage::Detail
                || (manual_entry && !acc.manual_entries_allowed)
            {
                return Err(AccountingError::GLAccountInvalidUsage(c.gl_account_id));
            }
            total_credit += c.amount;
        }

        if total_debit != total_credit {
            return Err(AccountingError::JournalEntryInvalid(format!(
                "Unbalanced journal entry: total debits ({total_debit}) != total credits ({total_credit})"
            )));
        }

        let tx_id = transaction_id.into();
        for d in &cmd.debits {
            let id = self.next_entry_id;
            self.next_entry_id += 1;
            self.journal_entries.push(JournalEntry {
                id,
                office_id: cmd.office_id,
                gl_account_id: d.gl_account_id,
                currency_code: cmd.currency_code.clone(),
                transaction_id: tx_id.clone(),
                transaction_date: cmd.transaction_date,
                entry_type: JournalEntryType::Debit,
                amount: d.amount,
                description: d.comments.clone().or_else(|| cmd.comments.clone()),
                reversed: false,
                manual_entry,
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
            });
        }
        for c in &cmd.credits {
            let id = self.next_entry_id;
            self.next_entry_id += 1;
            self.journal_entries.push(JournalEntry {
                id,
                office_id: cmd.office_id,
                gl_account_id: c.gl_account_id,
                currency_code: cmd.currency_code.clone(),
                transaction_id: tx_id.clone(),
                transaction_date: cmd.transaction_date,
                entry_type: JournalEntryType::Credit,
                amount: c.amount,
                description: c.comments.clone().or_else(|| cmd.comments.clone()),
                reversed: false,
                manual_entry,
                office_running_balance: Decimal::ZERO,
                organization_running_balance: Decimal::ZERO,
            });
        }

        self.update_running_balances();
        Ok(tx_id)
    }

    /// Recomputes `office_running_balance` and `organization_running_balance` across all non-reversed journal entries.
    pub fn update_running_balances(&mut self) {
        let mut office_balances: HashMap<(i64, i64), Decimal> = HashMap::new();
        let mut org_balances: HashMap<i64, Decimal> = HashMap::new();

        for entry in &mut self.journal_entries {
            if entry.reversed {
                continue;
            }
            let Some(acc) = self.gl_accounts.get(&entry.gl_account_id) else {
                continue;
            };
            let delta = match (acc.account_type.is_debit_normal(), entry.entry_type) {
                (true, JournalEntryType::Debit) | (false, JournalEntryType::Credit) => entry.amount,
                (true, JournalEntryType::Credit) | (false, JournalEntryType::Debit) => {
                    -entry.amount
                }
            };
            let off_bal = office_balances
                .entry((entry.office_id, entry.gl_account_id))
                .or_insert(Decimal::ZERO);
            *off_bal += delta;
            entry.office_running_balance = *off_bal;

            let org_bal = org_balances
                .entry(entry.gl_account_id)
                .or_insert(Decimal::ZERO);
            *org_bal += delta;
            entry.organization_running_balance = *org_bal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_delete_gl_account_success_with_no_dependencies_and_blocks_with_dependencies() {
        let mut engine = AccountingLedgerEngine::new();
        engine
            .create_gl_account(GLAccount::new_detail(
                123,
                "Test Account",
                "TEST001",
                GLAccountType::Asset,
            ))
            .unwrap();

        let deleted_id = engine.delete_gl_account(123).unwrap();
        assert_eq!(deleted_id, 123);
        assert!(!engine.gl_accounts.contains_key(&123));

        // Recreate and map to a product -> deletion should fail
        engine
            .create_gl_account(GLAccount::new_detail(
                123,
                "Test Account",
                "TEST001",
                GLAccountType::Asset,
            ))
            .unwrap();
        engine.product_mappings.push((1, 123));
        assert!(matches!(
            engine.delete_gl_account(123),
            Err(AccountingError::GLAccountInvalidDelete { id: 123, .. })
        ));
    }

    #[test]
    fn account_rule_request_serialization_and_validation() {
        let validator = AccountingRuleCommandValidator;

        // 1. Tag-based request keeps tags and multiple entry flags through reserialization
        let tag_request = AccountRuleRequest {
            name: Some("Compound Cash-Bank Rule".to_string()),
            office_id: Some(1),
            account_to_debit: None,
            account_to_credit: None,
            description: Some("test compound rule".to_string()),
            credit_tags: Some(vec![22, 23]),
            debit_tags: Some(vec![24, 205]),
            allow_multiple_credit_entries: Some(true),
            allow_multiple_debit_entries: Some(true),
        };
        let json = serde_json::to_string(&tag_request).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["creditTags"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["debitTags"].as_array().unwrap().len(), 2);
        assert!(parsed["allowMultipleCreditEntries"].as_bool().unwrap());
        assert!(parsed["allowMultipleDebitEntries"].as_bool().unwrap());
        assert!(parsed.get("accountToDebit").is_none());
        assert!(parsed.get("accountToCredit").is_none());
        assert!(validator.validate_for_create(&json).is_ok());
        assert!(validator.validate_for_update(&json).is_ok());

        // 2. Account-based request still validates
        let acc_request = AccountRuleRequest {
            name: Some("Account Rule".to_string()),
            office_id: Some(1),
            account_to_debit: Some(10),
            account_to_credit: Some(11),
            description: None,
            credit_tags: None,
            debit_tags: None,
            allow_multiple_credit_entries: None,
            allow_multiple_debit_entries: None,
        };
        let acc_json = serde_json::to_string(&acc_request).unwrap();
        assert!(validator.validate_for_create(&acc_json).is_ok());

        // 3. Request without accounts or tags is rejected
        let empty_request = AccountRuleRequest {
            name: Some("Empty Rule".to_string()),
            office_id: Some(1),
            account_to_debit: None,
            account_to_credit: None,
            description: None,
            credit_tags: None,
            debit_tags: None,
            allow_multiple_credit_entries: None,
            allow_multiple_debit_entries: None,
        };
        let empty_json = serde_json::to_string(&empty_request).unwrap();
        assert!(matches!(
            validator.validate_for_create(&empty_json),
            Err(AccountingError::ValidationError { .. })
        ));
    }

    #[test]
    fn provisioning_entries_validator_without_locale_reports_locale_parameter() {
        let deserializer = ProvisioningEntriesDefinitionJsonDeserializer;
        let json = r#"{ "dateFormat": "dd MMMM yyyy" }"#;
        let err = deserializer.validate_for_create(json).unwrap_err();
        assert_eq!(
            err,
            AccountingError::ValidationError {
                parameter: "locale".to_string(),
                message: "The parameter locale is mandatory.".to_string(),
            }
        );
    }

    #[test]
    fn journal_entries_enforce_double_entry_balance_closures_and_running_balances() {
        let mut engine = AccountingLedgerEngine::new();
        // Cash Asset (10) and Loan Portfolio Asset (11) and Interest Income (40)
        engine
            .create_gl_account(GLAccount::new_detail(
                10,
                "Cash",
                "1001",
                GLAccountType::Asset,
            ))
            .unwrap();
        engine
            .create_gl_account(GLAccount::new_detail(
                11,
                "Loan Portfolio",
                "1002",
                GLAccountType::Asset,
            ))
            .unwrap();
        engine
            .create_gl_account(GLAccount::new_detail(
                40,
                "Interest Income",
                "4001",
                GLAccountType::Income,
            ))
            .unwrap();

        let tx_date = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();
        // Unbalanced entry fails
        let unbalanced = JournalEntryCommand {
            office_id: 1,
            currency_code: "USD".to_string(),
            transaction_date: tx_date,
            debits: vec![SingleDebitOrCreditEntryCommand {
                gl_account_id: 10,
                amount: dec!(100.00),
                comments: None,
            }],
            credits: vec![SingleDebitOrCreditEntryCommand {
                gl_account_id: 11,
                amount: dec!(90.00),
                comments: None,
            }],
            reference_number: None,
            comments: None,
        };
        assert!(matches!(
            engine.create_journal_entries(&unbalanced, "TX-BAD", true),
            Err(AccountingError::JournalEntryInvalid(_))
        ));

        // Balanced repayment: Debit Cash 110, Credit Loan Portfolio 100 + Credit Interest Income 10
        let balanced = JournalEntryCommand {
            office_id: 1,
            currency_code: "USD".to_string(),
            transaction_date: tx_date,
            debits: vec![SingleDebitOrCreditEntryCommand {
                gl_account_id: 10,
                amount: dec!(110.00),
                comments: None,
            }],
            credits: vec![
                SingleDebitOrCreditEntryCommand {
                    gl_account_id: 11,
                    amount: dec!(100.00),
                    comments: None,
                },
                SingleDebitOrCreditEntryCommand {
                    gl_account_id: 40,
                    amount: dec!(10.00),
                    comments: None,
                },
            ],
            reference_number: Some("REF-1".to_string()),
            comments: Some("Loan repayment".to_string()),
        };
        engine
            .create_journal_entries(&balanced, "L100", true)
            .unwrap();

        assert_eq!(engine.journal_entries.len(), 3);
        // Asset (10) debited 110 -> +110 running balance
        assert_eq!(engine.journal_entries[0].office_running_balance, dec!(110.00));
        // Asset (11) credited 100 -> -100 running balance
        assert_eq!(
            engine.journal_entries[1].office_running_balance,
            dec!(-100.00)
        );
        // Income (40) credited 10 -> +10 running balance (credit-normal)
        assert_eq!(engine.journal_entries[2].office_running_balance, dec!(10.00));

        // Close accounting on 2026-03-20 -> posting on 2026-03-20 is now rejected
        engine
            .create_gl_closure(
                GLClosure::new(1, 1, tx_date, Some("Month end".to_string())),
                tx_date,
            )
            .unwrap();
        assert!(matches!(
            engine.create_journal_entries(&balanced, "L101", true),
            Err(AccountingError::BranchAccountingClosed { .. })
        ));
    }

    #[test]
    fn financial_activity_account_validates_gl_account_classification() {
        let mut engine = AccountingLedgerEngine::new();
        engine
            .create_gl_account(GLAccount::new_detail(
                10,
                "Asset Transfer Clearing",
                "1099",
                GLAccountType::Asset,
            ))
            .unwrap();
        engine
            .create_gl_account(GLAccount::new_detail(
                20,
                "Savings Control",
                "2001",
                GLAccountType::Liability,
            ))
            .unwrap();

        // Mapping AssetTransfer (103) to Liability account (20) fails
        assert_eq!(
            engine
                .create_financial_activity_account(1, FinancialActivity::AssetTransfer, 20)
                .unwrap_err(),
            AccountingError::FinancialActivityInvalidAccountType {
                activity: FinancialActivity::AssetTransfer,
                expected: GLAccountType::Asset,
                actual: GLAccountType::Liability,
            }
        );

        // Mapping AssetTransfer (103) to Asset account (10) succeeds
        let fa = engine
            .create_financial_activity_account(1, FinancialActivity::AssetTransfer, 10)
            .unwrap();
        assert_eq!(fa.gl_account_id, 10);
    }
}
