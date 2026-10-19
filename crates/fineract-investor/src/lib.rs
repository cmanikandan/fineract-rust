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

//! `fineract-investor` — Rust port of Apache Fineract's `fineract-investor` module.
//!
//! Provides:
//! - [`ExternalTransferStatus`], [`ExternalTransferSubStatus`], [`AttributeKey`], [`SettlementModel`]
//! - [`ExternalAssetOwner`], [`ExternalAssetOwnerTransfer`], [`ExternalAssetOwnerTransferDetails`]
//! - [`DelayedSettlementAttributeService`] & [`LoanTransferabilityService`]
//! - [`ExternalAssetOwnerTransferOutstandingInterestCalculation`]
//! - [`ExternalAssetOwnersWriteService`], [`LoanAccountOwnerTransferService`], and [`InvestorAccountingService`]

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub const FUTURE_DATE_9999_12_31: NaiveDate = match NaiveDate::from_ymd_opt(9999, 12, 31) {
    Some(d) => d,
    None => unreachable!(),
};

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum InvestorError {
    #[error("{0}")]
    InitiateTransferException(String),
    #[error("External asset owner with externalId {0} not found")]
    OwnerNotFound(String),
    #[error("External asset owner transfer {0} not found")]
    TransferNotFound(i64),
    #[error("External asset owner loan product attribute already exists for product {product_id} and key {key}")]
    AttributeAlreadyExists { product_id: i64, key: String },
    #[error("Invalid attribute value '{value}' for key '{key}'")]
    InvalidAttributeValue { key: String, value: String },
    #[error("Unknown outstanding interest calculation: {0}")]
    UnsupportedInterestStrategy(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalTransferStatus {
    Pending,
    Active,
    Declined,
    Buyback,
    Cancelled,
    PendingIntermediate,
    ActiveIntermediate,
    BuybackIntermediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalTransferSubStatus {
    BalanceZero,
    BalanceNegative,
    SamDayTransfers,
    UserRequested,
    Unsold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttributeKey {
    SettlementModel,
    BuyDownFeeAmortizationStrategy,
    CapitalizedIncomeAmortizationStrategy,
    ExcludedTransactionTypes,
}

impl AttributeKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SettlementModel => "SETTLEMENT_MODEL",
            Self::BuyDownFeeAmortizationStrategy => "BUY_DOWN_FEE_AMORTIZATION_STRATEGY",
            Self::CapitalizedIncomeAmortizationStrategy => {
                "CAPITALIZED_INCOME_AMORTIZATION_STRATEGY"
            }
            Self::ExcludedTransactionTypes => "EXCLUDED_TRANSACTION_TYPES",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettlementModel {
    DefaultSettlement,
    DelayedSettlement,
}

impl SettlementModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DefaultSettlement => "DEFAULT_SETTLEMENT",
            Self::DelayedSettlement => "DELAYED_SETTLEMENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAssetOwner {
    pub id: Option<i64>,
    pub external_id: String,
}

impl ExternalAssetOwner {
    pub fn new(id: Option<i64>, external_id: impl Into<String>) -> Self {
        Self {
            id,
            external_id: external_id.into(),
        }
    }
}

/// Port of Fineract's `ExternalAssetOwnerTransferDetails`.
/// Automatically recomputes `total_outstanding` whenever principal, interest, fee, or penalty components are updated.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExternalAssetOwnerTransferDetails {
    pub id: Option<i64>,
    total_outstanding: Decimal,
    total_principal_outstanding: Decimal,
    total_interest_outstanding: Decimal,
    total_fee_charges_outstanding: Decimal,
    total_penalty_charges_outstanding: Decimal,
    total_overpaid: Decimal,
}

impl ExternalAssetOwnerTransferDetails {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_outstanding(&self) -> Decimal {
        self.total_outstanding
    }

    pub fn total_principal_outstanding(&self) -> Decimal {
        self.total_principal_outstanding
    }

    pub fn total_interest_outstanding(&self) -> Decimal {
        self.total_interest_outstanding
    }

    pub fn total_fee_charges_outstanding(&self) -> Decimal {
        self.total_fee_charges_outstanding
    }

    pub fn total_penalty_charges_outstanding(&self) -> Decimal {
        self.total_penalty_charges_outstanding
    }

    pub fn total_overpaid(&self) -> Decimal {
        self.total_overpaid
    }

    pub fn set_total_principal_outstanding(&mut self, val: Option<Decimal>) {
        self.total_principal_outstanding = val.unwrap_or(Decimal::ZERO);
        self.update_total_outstanding();
    }

    pub fn set_total_interest_outstanding(&mut self, val: Option<Decimal>) {
        self.total_interest_outstanding = val.unwrap_or(Decimal::ZERO);
        self.update_total_outstanding();
    }

    pub fn set_total_fee_charges_outstanding(&mut self, val: Option<Decimal>) {
        self.total_fee_charges_outstanding = val.unwrap_or(Decimal::ZERO);
        self.update_total_outstanding();
    }

    pub fn set_total_penalty_charges_outstanding(&mut self, val: Option<Decimal>) {
        self.total_penalty_charges_outstanding = val.unwrap_or(Decimal::ZERO);
        self.update_total_outstanding();
    }

    pub fn set_total_overpaid(&mut self, val: Option<Decimal>) {
        self.total_overpaid = val.unwrap_or(Decimal::ZERO);
    }

    fn update_total_outstanding(&mut self) {
        self.total_outstanding = self.total_principal_outstanding
            + self.total_interest_outstanding
            + self.total_fee_charges_outstanding
            + self.total_penalty_charges_outstanding;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAssetOwnerTransfer {
    pub id: Option<i64>,
    pub owner: ExternalAssetOwner,
    pub previous_owner: Option<ExternalAssetOwner>,
    pub details: Option<ExternalAssetOwnerTransferDetails>,
    pub external_id: String,
    pub status: ExternalTransferStatus,
    pub sub_status: Option<ExternalTransferSubStatus>,
    pub purchase_price_ratio: String,
    pub settlement_date: NaiveDate,
    pub effective_date_from: NaiveDate,
    pub effective_date_to: NaiveDate,
    pub loan_id: i64,
    pub external_loan_id: Option<String>,
    pub external_group_id: Option<String>,
}

/// Snapshot of a Loan used by investor transferability, interest, and accounting services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanSnapshotForInvestor {
    pub loan_id: i64,
    pub loan_product_id: i64,
    pub loan_product_short_name: String,
    pub is_open: bool,
    pub is_overpaid: bool,
    pub total_principal_outstanding: Decimal,
    pub total_interest_outstanding: Decimal,
    pub total_fee_charges_outstanding: Decimal,
    pub total_penalty_charges_outstanding: Decimal,
    pub total_overpaid: Decimal,
    pub due_payable_interest: Decimal,
    pub not_due_payable_interest: Decimal,
}

impl LoanSnapshotForInvestor {
    pub fn total_outstanding(&self) -> Decimal {
        self.total_principal_outstanding
            + self.total_interest_outstanding
            + self.total_fee_charges_outstanding
            + self.total_penalty_charges_outstanding
    }
}

/// Port of Fineract's `DelayedSettlementAttributeServiceImpl`.
#[derive(Debug, Default, Clone)]
pub struct DelayedSettlementAttributeService {
    product_attributes: HashMap<(i64, String), String>,
}

impl DelayedSettlementAttributeService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_attribute(
        &mut self,
        loan_product_id: i64,
        attribute_key: impl Into<String>,
        attribute_value: impl Into<String>,
    ) {
        self.product_attributes.insert(
            (loan_product_id, attribute_key.into()),
            attribute_value.into(),
        );
    }

    pub fn is_enabled(&self, loan_product_id: i64) -> bool {
        self.product_attributes
            .get(&(
                loan_product_id,
                AttributeKey::SettlementModel.as_str().to_string(),
            ))
            .map(|v| {
                v.trim()
                    .eq_ignore_ascii_case(SettlementModel::DelayedSettlement.as_str())
            })
            .unwrap_or(false)
    }
}

/// Port of Fineract's `LoanTransferabilityServiceImpl`.
#[derive(Debug, Clone)]
pub struct LoanTransferabilityService {
    delayed_settlement_service: DelayedSettlementAttributeService,
}

impl LoanTransferabilityService {
    pub fn new(delayed_settlement_service: DelayedSettlementAttributeService) -> Self {
        Self {
            delayed_settlement_service,
        }
    }

    pub fn is_transferable(
        &self,
        loan: &LoanSnapshotForInvestor,
        transfer: &ExternalAssetOwnerTransfer,
    ) -> bool {
        if self.should_validate_transferable(loan, transfer) {
            return loan.total_outstanding() > Decimal::ZERO;
        }
        true
    }

    pub fn get_declined_sub_status(
        &self,
        loan: &LoanSnapshotForInvestor,
    ) -> ExternalTransferSubStatus {
        if loan.total_overpaid > Decimal::ZERO {
            ExternalTransferSubStatus::BalanceNegative
        } else {
            ExternalTransferSubStatus::BalanceZero
        }
    }

    fn should_validate_transferable(
        &self,
        loan: &LoanSnapshotForInvestor,
        transfer: &ExternalAssetOwnerTransfer,
    ) -> bool {
        if !self.delayed_settlement_service.is_enabled(loan.loan_product_id) {
            // When delayed settlement is disabled, asset is directly sold to investor. Need to validate.
            return true;
        }
        // When delayed settlement is enabled and asset is sold to intermediate. Need to validate.
        // When delayed settlement is enabled and asset is sold from intermediate to investor. No need to validate.
        transfer.status == ExternalTransferStatus::PendingIntermediate
    }
}

/// Port of Fineract's `ExternalAssetOwnerTransferOutstandingInterestCalculation`.
#[derive(Debug, Clone)]
pub struct ExternalAssetOwnerTransferOutstandingInterestCalculation {
    pub strategy: String,
}

impl Default for ExternalAssetOwnerTransferOutstandingInterestCalculation {
    fn default() -> Self {
        Self {
            strategy: "TOTAL_OUTSTANDING_INTEREST".to_string(),
        }
    }
}

impl ExternalAssetOwnerTransferOutstandingInterestCalculation {
    pub fn new(strategy: impl Into<String>) -> Self {
        Self {
            strategy: strategy.into(),
        }
    }

    pub fn calculate_outstanding_interest(
        &self,
        loan: &LoanSnapshotForInvestor,
    ) -> Result<Decimal, InvestorError> {
        if !loan.is_open {
            return Ok(Decimal::ZERO);
        }
        match self.strategy.as_str() {
            "TOTAL_OUTSTANDING_INTEREST" => Ok(loan.total_interest_outstanding),
            "PAYABLE_OUTSTANDING_INTEREST" => {
                Ok(loan.due_payable_interest + loan.not_due_payable_interest)
            }
            other => Err(InvestorError::UnsupportedInterestStrategy(
                other.to_string(),
            )),
        }
    }
}

/// Port of Fineract's `ExternalAssetOwnersWriteServiceImpl` and `LoanAccountOwnerTransferServiceImpl`.
#[derive(Debug, Default, Clone)]
pub struct ExternalAssetOwnerTransferEngine {
    next_transfer_id: i64,
    pub transfers: Vec<ExternalAssetOwnerTransfer>,
    pub delayed_settlement_service: DelayedSettlementAttributeService,
    pub emitted_events: Vec<(i64, ExternalTransferStatus, Option<ExternalTransferSubStatus>)>,
}

impl ExternalAssetOwnerTransferEngine {
    pub fn new(delayed_settlement_service: DelayedSettlementAttributeService) -> Self {
        Self {
            next_transfer_id: 1,
            transfers: Vec::new(),
            delayed_settlement_service,
            emitted_events: Vec::new(),
        }
    }

    pub fn intermediary_sale_loan_by_loan_id(
        &mut self,
        loan: &LoanSnapshotForInvestor,
        owner: ExternalAssetOwner,
        external_id: impl Into<String>,
        purchase_price_ratio: impl Into<String>,
        settlement_date: NaiveDate,
        business_date: NaiveDate,
    ) -> Result<ExternalAssetOwnerTransfer, InvestorError> {
        if !self.delayed_settlement_service.is_enabled(loan.loan_product_id) {
            return Err(InvestorError::InitiateTransferException(format!(
                "Delayed Settlement Configuration is not enabled for the loan product: {}",
                loan.loan_product_short_name
            )));
        }
        if !loan.is_open && !loan.is_overpaid {
            return Err(InvestorError::InitiateTransferException(
                "Loan is not in active status".to_string(),
            ));
        }
        if settlement_date < business_date {
            return Err(InvestorError::InitiateTransferException(
                "Settlement date cannot be in the past".to_string(),
            ));
        }

        // Check existing effective transfers
        let effective: Vec<&ExternalAssetOwnerTransfer> = self
            .transfers
            .iter()
            .filter(|t| t.loan_id == loan.loan_id && t.effective_date_to == FUTURE_DATE_9999_12_31)
            .collect();
        if let Some(last) = effective.last() {
            match last.status {
                ExternalTransferStatus::PendingIntermediate
                | ExternalTransferStatus::ActiveIntermediate => {
                    return Err(InvestorError::InitiateTransferException(
                        "This loan has already been transferred to an intermediary".to_string(),
                    ));
                }
                ExternalTransferStatus::Pending | ExternalTransferStatus::Active => {
                    return Err(InvestorError::InitiateTransferException(
                        "External asset owner transfer is already in PENDING or ACTIVE state"
                            .to_string(),
                    ));
                }
                _ => {}
            }
        }

        let id = self.next_transfer_id;
        self.next_transfer_id += 1;
        let transfer = ExternalAssetOwnerTransfer {
            id: Some(id),
            owner,
            previous_owner: None,
            details: None,
            external_id: external_id.into(),
            status: ExternalTransferStatus::PendingIntermediate,
            sub_status: None,
            purchase_price_ratio: purchase_price_ratio.into(),
            settlement_date,
            effective_date_from: business_date,
            effective_date_to: FUTURE_DATE_9999_12_31,
            loan_id: loan.loan_id,
            external_loan_id: None,
            external_group_id: None,
        };
        self.transfers.push(transfer.clone());
        Ok(transfer)
    }

    /// Port of `LoanAccountOwnerTransferServiceImpl::handleLoanClosedOrOverpaid`.
    /// Cancels or declines pending sale, buyback, and intermediate transfers when a loan is closed or overpaid,
    /// emitting a `LoanOwnershipTransferBusinessEvent` for each affected transfer.
    pub fn handle_loan_closed_or_overpaid(
        &mut self,
        loan: &LoanSnapshotForInvestor,
        business_date: NaiveDate,
    ) {
        let pending_statuses = [
            ExternalTransferStatus::Pending,
            ExternalTransferStatus::PendingIntermediate,
            ExternalTransferStatus::Buyback,
            ExternalTransferStatus::BuybackIntermediate,
        ];

        for idx in 0..self.transfers.len() {
            if self.transfers[idx].loan_id == loan.loan_id
                && self.transfers[idx].effective_date_to == FUTURE_DATE_9999_12_31
                && pending_statuses.contains(&self.transfers[idx].status)
            {
                self.transfers[idx].effective_date_to = business_date;
                let is_past_settlement = self.transfers[idx].settlement_date < business_date;
                let (new_status, sub_status) = if is_past_settlement
                    && matches!(
                        self.transfers[idx].status,
                        ExternalTransferStatus::Pending
                            | ExternalTransferStatus::PendingIntermediate
                    ) {
                    let sub = if loan.total_overpaid > Decimal::ZERO {
                        ExternalTransferSubStatus::BalanceNegative
                    } else {
                        ExternalTransferSubStatus::BalanceZero
                    };
                    (ExternalTransferStatus::Declined, Some(sub))
                } else {
                    (
                        ExternalTransferStatus::Cancelled,
                        Some(ExternalTransferSubStatus::BalanceZero),
                    )
                };
                self.transfers[idx].status = new_status;
                self.transfers[idx].sub_status = sub_status;
                self.emitted_events
                    .push((loan.loan_id, new_status, sub_status));
            }
        }
    }

    /// Port of `AccountingServiceImpl::determineOwnerForSale` and `determineOwnerForBuyback`.
    pub fn determine_owner_for_sale<'a>(
        is_overpaid: bool,
        is_credit_entry: bool,
        previous_owner: Option<&'a ExternalAssetOwner>,
        new_owner: &'a ExternalAssetOwner,
    ) -> Option<&'a ExternalAssetOwner> {
        if is_overpaid {
            if is_credit_entry {
                Some(new_owner)
            } else {
                previous_owner
            }
        } else if is_credit_entry {
            previous_owner
        } else {
            Some(new_owner)
        }
    }

    pub fn determine_owner_for_buyback<'a>(
        is_overpaid: bool,
        is_credit_entry: bool,
        previous_owner: &'a ExternalAssetOwner,
    ) -> Option<&'a ExternalAssetOwner> {
        if is_overpaid && !is_credit_entry {
            return Some(previous_owner);
        }
        if !is_overpaid && is_credit_entry {
            return Some(previous_owner);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_loan(outstanding_principal: Decimal, overpaid: Decimal) -> LoanSnapshotForInvestor {
        LoanSnapshotForInvestor {
            loan_id: 1,
            loan_product_id: 1,
            loan_product_short_name: "LP1".to_string(),
            is_open: true,
            is_overpaid: overpaid > Decimal::ZERO,
            total_principal_outstanding: outstanding_principal,
            total_interest_outstanding: Decimal::ZERO,
            total_fee_charges_outstanding: Decimal::ZERO,
            total_penalty_charges_outstanding: Decimal::ZERO,
            total_overpaid: overpaid,
            due_payable_interest: dec!(15.50),
            not_due_payable_interest: dec!(4.50),
        }
    }

    fn sample_transfer(status: ExternalTransferStatus) -> ExternalAssetOwnerTransfer {
        ExternalAssetOwnerTransfer {
            id: Some(10),
            owner: ExternalAssetOwner::new(Some(1), "OWNER-1"),
            previous_owner: None,
            details: None,
            external_id: "TX-1".to_string(),
            status,
            sub_status: None,
            purchase_price_ratio: "1.0".to_string(),
            settlement_date: NaiveDate::from_ymd_opt(2026, 3, 25).unwrap(),
            effective_date_from: NaiveDate::from_ymd_opt(2026, 3, 25).unwrap(),
            effective_date_to: FUTURE_DATE_9999_12_31,
            loan_id: 1,
            external_loan_id: None,
            external_group_id: None,
        }
    }

    #[test]
    fn delayed_settlement_attribute_service_is_case_insensitive() {
        let mut service = DelayedSettlementAttributeService::new();
        assert!(!service.is_enabled(1));

        service.set_attribute(1, "SETTLEMENT_MODEL", "DELAYED_SETTLEMENT");
        assert!(service.is_enabled(1));

        service.set_attribute(1, "SETTLEMENT_MODEL", "delayed_settlement");
        assert!(service.is_enabled(1));

        service.set_attribute(1, "SETTLEMENT_MODEL", "DEFAULT_SETTLEMENT");
        assert!(!service.is_enabled(1));
    }

    #[test]
    fn loan_transferability_when_delayed_settlement_disabled_or_enabled() {
        let mut delayed_service = DelayedSettlementAttributeService::new();
        let transferability = LoanTransferabilityService::new(delayed_service.clone());

        let pending = sample_transfer(ExternalTransferStatus::Pending);
        assert!(transferability.is_transferable(&sample_loan(dec!(1), dec!(0)), &pending));
        assert!(!transferability.is_transferable(&sample_loan(dec!(0), dec!(0)), &pending));
        assert!(!transferability.is_transferable(&sample_loan(dec!(-1), dec!(0)), &pending));

        // Enable delayed settlement
        delayed_service.set_attribute(1, "SETTLEMENT_MODEL", "DELAYED_SETTLEMENT");
        let transferability_delayed = LoanTransferabilityService::new(delayed_service);

        let pending_intermediate = sample_transfer(ExternalTransferStatus::PendingIntermediate);
        assert!(transferability_delayed
            .is_transferable(&sample_loan(dec!(1), dec!(0)), &pending_intermediate));
        assert!(!transferability_delayed
            .is_transferable(&sample_loan(dec!(0), dec!(0)), &pending_intermediate));

        // Selling from intermediate to investor (Status = PENDING) when delayed settlement enabled -> always true
        assert!(transferability_delayed.is_transferable(&sample_loan(dec!(0), dec!(0)), &pending));
    }

    #[test]
    fn declined_sub_status_reflects_overpaid_balance() {
        let service = LoanTransferabilityService::new(DelayedSettlementAttributeService::new());
        assert_eq!(
            service.get_declined_sub_status(&sample_loan(dec!(0), dec!(1))),
            ExternalTransferSubStatus::BalanceNegative
        );
        assert_eq!(
            service.get_declined_sub_status(&sample_loan(dec!(0), dec!(0))),
            ExternalTransferSubStatus::BalanceZero
        );
    }

    #[test]
    fn transfer_details_auto_computes_total_outstanding() {
        let mut details = ExternalAssetOwnerTransferDetails::new();
        details.set_total_principal_outstanding(Some(dec!(100.00)));
        details.set_total_interest_outstanding(Some(dec!(12.50)));
        details.set_total_fee_charges_outstanding(Some(dec!(5.00)));
        details.set_total_penalty_charges_outstanding(Some(dec!(2.50)));
        details.set_total_overpaid(Some(dec!(0.00)));

        assert_eq!(details.total_outstanding(), dec!(120.00));
    }

    #[test]
    fn outstanding_interest_calculation_supports_total_and_payable_strategies() {
        let mut loan = sample_loan(dec!(100), dec!(0));
        loan.total_interest_outstanding = dec!(25.00);
        loan.due_payable_interest = dec!(15.50);
        loan.not_due_payable_interest = dec!(4.50);

        let total_calc = ExternalAssetOwnerTransferOutstandingInterestCalculation::new(
            "TOTAL_OUTSTANDING_INTEREST",
        );
        assert_eq!(
            total_calc.calculate_outstanding_interest(&loan).unwrap(),
            dec!(25.00)
        );

        let payable_calc = ExternalAssetOwnerTransferOutstandingInterestCalculation::new(
            "PAYABLE_OUTSTANDING_INTEREST",
        );
        assert_eq!(
            payable_calc.calculate_outstanding_interest(&loan).unwrap(),
            dec!(20.00)
        );

        loan.is_open = false;
        assert_eq!(
            payable_calc.calculate_outstanding_interest(&loan).unwrap(),
            Decimal::ZERO
        );
    }

    #[test]
    fn intermediary_sale_validates_delayed_settlement_and_existing_transfers() {
        let loan = sample_loan(dec!(500), dec!(0));
        let owner = ExternalAssetOwner::new(Some(1), "OWNER-A");
        let biz_date = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();
        let settlement_date = NaiveDate::from_ymd_opt(2026, 3, 25).unwrap();

        // Delayed settlement disabled -> fails
        let mut engine =
            ExternalAssetOwnerTransferEngine::new(DelayedSettlementAttributeService::new());
        let err = engine
            .intermediary_sale_loan_by_loan_id(
                &loan,
                owner.clone(),
                "TX-1",
                "1.0",
                settlement_date,
                biz_date,
            )
            .unwrap_err();
        assert_eq!(
            err,
            InvestorError::InitiateTransferException(
                "Delayed Settlement Configuration is not enabled for the loan product: LP1"
                    .to_string()
            )
        );

        // Enable delayed settlement -> succeeds
        engine
            .delayed_settlement_service
            .set_attribute(1, "SETTLEMENT_MODEL", "DELAYED_SETTLEMENT");
        let created = engine
            .intermediary_sale_loan_by_loan_id(
                &loan,
                owner.clone(),
                "TX-1",
                "1.0",
                settlement_date,
                biz_date,
            )
            .unwrap();
        assert_eq!(created.status, ExternalTransferStatus::PendingIntermediate);

        // Second intermediary sale while PENDING_INTERMEDIATE -> rejected
        let dup_err = engine
            .intermediary_sale_loan_by_loan_id(
                &loan,
                owner,
                "TX-2",
                "1.0",
                settlement_date,
                biz_date,
            )
            .unwrap_err();
        assert_eq!(
            dup_err,
            InvestorError::InitiateTransferException(
                "This loan has already been transferred to an intermediary".to_string()
            )
        );
    }

    #[test]
    fn handle_loan_closed_or_overpaid_emits_events_and_cancels_transfers() {
        let mut delayed = DelayedSettlementAttributeService::new();
        delayed.set_attribute(1, "SETTLEMENT_MODEL", "DELAYED_SETTLEMENT");
        let mut engine = ExternalAssetOwnerTransferEngine::new(delayed);

        let biz_date = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();
        for status in [
            ExternalTransferStatus::Pending,
            ExternalTransferStatus::Buyback,
            ExternalTransferStatus::PendingIntermediate,
            ExternalTransferStatus::BuybackIntermediate,
        ] {
            let mut t = sample_transfer(status);
            t.settlement_date = biz_date;
            engine.transfers.push(t);
        }

        let loan = sample_loan(dec!(0), dec!(0));
        engine.handle_loan_closed_or_overpaid(&loan, biz_date);

        assert_eq!(engine.emitted_events.len(), 4);
        assert!(engine
            .transfers
            .iter()
            .all(|t| t.status == ExternalTransferStatus::Cancelled));
    }

    #[test]
    fn accounting_owner_determination_for_sale_and_buyback() {
        let prev = ExternalAssetOwner::new(Some(1), "PREV");
        let next = ExternalAssetOwner::new(Some(2), "NEXT");

        // Normal (non-overpaid) sale: credit -> prev, debit -> next
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_sale(
                false,
                true,
                Some(&prev),
                &next
            ),
            Some(&prev)
        );
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_sale(
                false,
                false,
                Some(&prev),
                &next
            ),
            Some(&next)
        );

        // Overpaid sale: credit -> next, debit -> prev
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_sale(
                true,
                true,
                Some(&prev),
                &next
            ),
            Some(&next)
        );
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_sale(
                true,
                false,
                Some(&prev),
                &next
            ),
            Some(&prev)
        );

        // Buyback
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_buyback(false, true, &prev),
            Some(&prev)
        );
        assert_eq!(
            ExternalAssetOwnerTransferEngine::determine_owner_for_buyback(false, false, &prev),
            None
        );
    }
}
