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

//! `fineract-loan` — Unified Rust port of Apache Fineract's `fineract-loan` and `fineract-progressive-loan` modules.
//!
//! Provides:
//! - [`AprCalculator`], [`PeriodFrequencyType`], [`DaysInYearType`] (including the `FINERACT-2492` fix for `ACTUAL` days in year)
//! - [`LoanOverAppliedCalculationType`] & [`EnumOptionData`] (`LoanOverAppliedCalculationTypeTest` parity)
//! - [`LoanRepaymentScheduleInstallment`] (`LoanRepaymentScheduleInstallmentTest` parity)
//! - [`PaymentAllocationType`], [`LoanProductPaymentAllocationRule`], and [`AdvancedPaymentAllocationsValidator`]
//! - [`AllocationType`], [`LoanProductCreditAllocationRule`], and [`CreditAllocationsValidator`]
//! - [`LoanStatus`], [`LoanAccount`], and progressive/advanced repayment processing

use chrono::NaiveDate;
use rust_decimal::prelude::RoundingStrategy;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

pub const ADVANCED_PAYMENT_ALLOCATION_STRATEGY: &str = "advanced-payment-allocation-strategy";

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LoanError {
    #[error("Platform API data validation error [{code}]: {message}")]
    ValidationError { code: String, message: String },
    #[error("Invalid loan state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: LoanStatus, to: LoanStatus },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PeriodFrequencyType {
    Days,
    Weeks,
    Months,
    Years,
    WholeTerm,
}

impl PeriodFrequencyType {
    pub fn payment_periods_in_one_year(&self) -> u32 {
        match self {
            Self::Days => 365,
            Self::Weeks => 52,
            Self::Months => 12,
            Self::Years => 1,
            Self::WholeTerm => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaysInYearType {
    Actual,
    Days360,
    Days364,
    Days365,
}

/// Port of Fineract's `AprCalculator` (including FINERACT-2492 fix).
#[derive(Debug, Default, Clone)]
pub struct AprCalculator;

impl AprCalculator {
    pub fn calculate_from(
        &self,
        interest_frequency_type: PeriodFrequencyType,
        interest_rate_per_period: Decimal,
        number_of_repayments: u32,
        repayment_every: u32,
        repayment_frequency_type: PeriodFrequencyType,
        days_in_year_type: DaysInYearType,
    ) -> Decimal {
        let days_in_year = match days_in_year_type {
            DaysInYearType::Actual => PeriodFrequencyType::Days.payment_periods_in_one_year(),
            DaysInYearType::Days360 => 360,
            DaysInYearType::Days364 => 364,
            DaysInYearType::Days365 => 365,
        };

        match interest_frequency_type {
            PeriodFrequencyType::Days => interest_rate_per_period * Decimal::from(days_in_year),
            PeriodFrequencyType::Weeks => interest_rate_per_period * Decimal::from(52),
            PeriodFrequencyType::Months => interest_rate_per_period * Decimal::from(12),
            PeriodFrequencyType::Years => interest_rate_per_period,
            PeriodFrequencyType::WholeTerm => {
                let total_periods = Decimal::from(number_of_repayments * repayment_every);
                let rate_per_period = (interest_rate_per_period / total_periods)
                    .round_dp_with_strategy(8, RoundingStrategy::MidpointNearestEven);
                match repayment_frequency_type {
                    PeriodFrequencyType::Days => rate_per_period * Decimal::from(days_in_year),
                    PeriodFrequencyType::Weeks => rate_per_period * Decimal::from(52),
                    PeriodFrequencyType::Months => rate_per_period * Decimal::from(12),
                    PeriodFrequencyType::Years | PeriodFrequencyType::WholeTerm => rate_per_period,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumOptionData {
    pub id: i64,
    pub code: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoanOverAppliedCalculationType {
    Flat = 1,
    Percentage = 2,
}

impl LoanOverAppliedCalculationType {
    pub fn values() -> &'static [Self] {
        &[Self::Flat, Self::Percentage]
    }

    pub fn as_enum_option_data(&self) -> EnumOptionData {
        match self {
            Self::Flat => EnumOptionData {
                id: 1,
                code: "loanOverAppliedCalculationType.flat".to_string(),
                value: "Flat".to_string(),
            },
            Self::Percentage => EnumOptionData {
                id: 2,
                code: "loanOverAppliedCalculationType.percentage".to_string(),
                value: "Percentage".to_string(),
            },
        }
    }

    pub fn get_values_as_enum_option_data_list() -> Vec<EnumOptionData> {
        Self::values()
            .iter()
            .map(|v| v.as_enum_option_data())
            .collect()
    }
}

/// Port of Fineract's `LoanRepaymentScheduleInstallment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanRepaymentScheduleInstallment {
    pub installment_number: i32,
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub principal: Option<Decimal>,
    pub principal_completed: Option<Decimal>,
    pub principal_written_off: Option<Decimal>,
    pub interest_charged: Option<Decimal>,
    pub interest_paid: Option<Decimal>,
    pub interest_waived: Option<Decimal>,
    pub interest_written_off: Option<Decimal>,
    pub interest_accrued: Option<Decimal>,
    pub reschedule_interest_portion: Option<Decimal>,
    pub fee_charges_charged: Option<Decimal>,
    pub fee_charges_paid: Option<Decimal>,
    pub fee_charges_written_off: Option<Decimal>,
    pub fee_charges_waived: Option<Decimal>,
    pub fee_accrued: Option<Decimal>,
    pub penalty_charges: Option<Decimal>,
    pub penalty_charges_paid: Option<Decimal>,
    pub penalty_charges_written_off: Option<Decimal>,
    pub penalty_charges_waived: Option<Decimal>,
    pub penalty_accrued: Option<Decimal>,
}

impl LoanRepaymentScheduleInstallment {
    pub fn new(
        installment_number: i32,
        from_date: NaiveDate,
        due_date: NaiveDate,
        principal: Decimal,
        interest_charged: Decimal,
        fee_charges: Decimal,
        penalty_charges: Decimal,
    ) -> Self {
        Self {
            installment_number,
            from_date,
            due_date,
            principal: Some(principal),
            principal_completed: Some(Decimal::ZERO),
            principal_written_off: Some(Decimal::ZERO),
            interest_charged: Some(interest_charged),
            interest_paid: Some(Decimal::ZERO),
            interest_waived: Some(Decimal::ZERO),
            interest_written_off: Some(Decimal::ZERO),
            interest_accrued: Some(Decimal::ZERO),
            reschedule_interest_portion: Some(Decimal::ZERO),
            fee_charges_charged: Some(fee_charges),
            fee_charges_paid: Some(Decimal::ZERO),
            fee_charges_written_off: Some(Decimal::ZERO),
            fee_charges_waived: Some(Decimal::ZERO),
            fee_accrued: Some(Decimal::ZERO),
            penalty_charges: Some(penalty_charges),
            penalty_charges_paid: Some(Decimal::ZERO),
            penalty_charges_written_off: Some(Decimal::ZERO),
            penalty_charges_waived: Some(Decimal::ZERO),
            penalty_accrued: Some(Decimal::ZERO),
        }
    }

    pub fn normalize_scale(val: Option<Decimal>) -> Option<Decimal> {
        val.map(|d| d.round_dp_with_strategy(6, RoundingStrategy::MidpointNearestEven))
    }

    pub fn set_principal(&mut self, val: Option<Decimal>) {
        self.principal = Self::normalize_scale(val);
    }

    pub fn set_interest_charged(&mut self, val: Option<Decimal>) {
        self.interest_charged = Self::normalize_scale(val);
    }

    pub fn set_fee_charges_charged(&mut self, val: Option<Decimal>) {
        self.fee_charges_charged = Self::normalize_scale(val);
    }

    pub fn set_penalty_charges(&mut self, val: Option<Decimal>) {
        self.penalty_charges = Self::normalize_scale(val);
    }

    pub fn total_outstanding(&self) -> Decimal {
        let p = self.principal.unwrap_or(Decimal::ZERO)
            - self.principal_completed.unwrap_or(Decimal::ZERO)
            - self.principal_written_off.unwrap_or(Decimal::ZERO);
        let i = self.interest_charged.unwrap_or(Decimal::ZERO)
            - self.interest_paid.unwrap_or(Decimal::ZERO)
            - self.interest_waived.unwrap_or(Decimal::ZERO)
            - self.interest_written_off.unwrap_or(Decimal::ZERO);
        let f = self.fee_charges_charged.unwrap_or(Decimal::ZERO)
            - self.fee_charges_paid.unwrap_or(Decimal::ZERO)
            - self.fee_charges_waived.unwrap_or(Decimal::ZERO)
            - self.fee_charges_written_off.unwrap_or(Decimal::ZERO);
        let pen = self.penalty_charges.unwrap_or(Decimal::ZERO)
            - self.penalty_charges_paid.unwrap_or(Decimal::ZERO)
            - self.penalty_charges_waived.unwrap_or(Decimal::ZERO)
            - self.penalty_charges_written_off.unwrap_or(Decimal::ZERO);
        p + i + f + pen
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DueType {
    PastDue,
    Due,
    InAdvance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaymentAllocationType {
    PastDuePenalty,
    PastDueFee,
    PastDuePrincipal,
    PastDueInterest,
    DuePenalty,
    DueFee,
    DuePrincipal,
    DueInterest,
    InAdvancePenalty,
    InAdvanceFee,
    InAdvancePrincipal,
    InAdvanceInterest,
}

impl PaymentAllocationType {
    pub fn all_12() -> Vec<Self> {
        vec![
            Self::PastDuePenalty,
            Self::PastDueFee,
            Self::PastDuePrincipal,
            Self::PastDueInterest,
            Self::DuePenalty,
            Self::DueFee,
            Self::DuePrincipal,
            Self::DueInterest,
            Self::InAdvancePenalty,
            Self::InAdvanceFee,
            Self::InAdvancePrincipal,
            Self::InAdvanceInterest,
        ]
    }

    pub fn due_type(&self) -> DueType {
        match self {
            Self::PastDuePenalty
            | Self::PastDueFee
            | Self::PastDuePrincipal
            | Self::PastDueInterest => DueType::PastDue,
            Self::DuePenalty | Self::DueFee | Self::DuePrincipal | Self::DueInterest => {
                DueType::Due
            }
            Self::InAdvancePenalty
            | Self::InAdvanceFee
            | Self::InAdvancePrincipal
            | Self::InAdvanceInterest => DueType::InAdvance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FutureInstallmentAllocationRule {
    NextInstallment,
    LastInstallment,
    Reamortization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaymentAllocationTransactionType {
    Default,
    Repayment,
    DownPayment,
    MerchantIssuedRefund,
    PayoutRefund,
    GoodwillCredit,
    ChargeRefund,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanProductPaymentAllocationRule {
    pub transaction_type: Option<PaymentAllocationTransactionType>,
    pub allocation_types: Vec<PaymentAllocationType>,
    pub future_installment_allocation_rule: Option<FutureInstallmentAllocationRule>,
}

/// Port of `AdvancedPaymentAllocationsValidator` from `fineract-progressive-loan`.
#[derive(Debug, Default, Clone)]
pub struct AdvancedPaymentAllocationsValidator;

impl AdvancedPaymentAllocationsValidator {
    pub fn validate_pair_of_order_and_payment_allocation_type(
        &self,
        pairs: &[(i32, PaymentAllocationType)],
    ) -> Result<(), LoanError> {
        if pairs.len() != 12 {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy.each_payment_allocation_order.must.contain.12.entries"
                    .to_string(),
                message: format!(
                    "Each provided payment allocation must contain exactly 12 allocation rules, but {} were provided",
                    pairs.len()
                ),
            });
        }
        for (order, _) in pairs {
            if !(1..=12).contains(order) {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy.invalid.order".to_string(),
                    message: "The provided orders must be between 1 and 12".to_string(),
                });
            }
        }
        let unique_types: HashSet<PaymentAllocationType> =
            pairs.iter().map(|(_, t)| *t).collect();
        if unique_types.len() != 12 {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy.must.not.have.duplicate.payment.allocation.rule"
                    .to_string(),
                message: "The list of provided payment allocation rules must not contain any duplicates"
                    .to_string(),
            });
        }
        Ok(())
    }

    pub fn validate(
        &self,
        rules: Option<&[LoanProductPaymentAllocationRule]>,
        strategy: &str,
    ) -> Result<(), LoanError> {
        if strategy != ADVANCED_PAYMENT_ALLOCATION_STRATEGY {
            if let Some(r) = rules {
                if !r.is_empty() {
                    return Err(LoanError::ValidationError {
                        code: "payment_allocation.must.not.be.provided.when.allocation.strategy.is.not.advanced-payment-strategy"
                            .to_string(),
                        message: format!(
                            "In case '{strategy}' payment strategy, payment_allocation must not be provided"
                        ),
                    });
                }
            }
            return Ok(());
        }

        let Some(list) = rules else {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy-without-default-payment-allocation".to_string(),
                message: "Advanced-payment-allocation-strategy was selected but no DEFAULT payment allocation was provided"
                    .to_string(),
            });
        };
        if !list
            .iter()
            .any(|r| r.transaction_type == Some(PaymentAllocationTransactionType::Default))
        {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy-without-default-payment-allocation".to_string(),
                message: "Advanced-payment-allocation-strategy was selected but no DEFAULT payment allocation was provided"
                    .to_string(),
            });
        }

        let mut seen_tx_types = HashSet::new();
        for rule in list {
            let Some(tx_type) = rule.transaction_type else {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy.with.not.valid.transaction.type".to_string(),
                    message: "Payment allocation was provided with a not valid transaction type"
                        .to_string(),
                });
            };
            if !seen_tx_types.insert(tx_type) {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy-with-duplicate-payment-allocation".to_string(),
                    message: "The same transaction type must be provided only once".to_string(),
                });
            }
            if rule.future_installment_allocation_rule.is_none() {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy.with.not.valid.future.installment.allocation.rule"
                        .to_string(),
                    message: "Payment allocation was provided without a valid future installment allocation rule"
                        .to_string(),
                });
            }
            let pairs: Vec<(i32, PaymentAllocationType)> = rule
                .allocation_types
                .iter()
                .enumerate()
                .map(|(idx, t)| ((idx + 1) as i32, *t))
                .collect();
            self.validate_pair_of_order_and_payment_allocation_type(&pairs)?;
        }
        Ok(())
    }

    /// Checks that allocation rules group all 4 rules of each `DueType` contiguously.
    pub fn check_grouping_of_allocation_rules(
        &self,
        rules: &[LoanProductPaymentAllocationRule],
    ) -> Result<(), LoanError> {
        for rule in rules {
            let mut seen_groups: Vec<DueType> = Vec::new();
            for alloc in &rule.allocation_types {
                let dt = alloc.due_type();
                if seen_groups.last().copied() != Some(dt) {
                    if seen_groups.contains(&dt) {
                        return Err(LoanError::ValidationError {
                            code: "mixed.due.type.allocation.rules.are.not.supported.with.horizontal.installment.processing"
                                .to_string(),
                            message: "Horizontal repayment schedule processing is not supporting mixed due type allocation rules!"
                                .to_string(),
                        });
                    }
                    seen_groups.push(dt);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllocationType {
    Penalty,
    Fee,
    Interest,
    Principal,
}

impl AllocationType {
    pub fn all_4() -> Vec<Self> {
        vec![Self::Penalty, Self::Fee, Self::Interest, Self::Principal]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreditAllocationTransactionType {
    Chargeback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanProductCreditAllocationRule {
    pub transaction_type: Option<CreditAllocationTransactionType>,
    pub allocation_types: Vec<AllocationType>,
}

/// Port of `CreditAllocationsValidator` from `fineract-progressive-loan`.
#[derive(Debug, Default, Clone)]
pub struct CreditAllocationsValidator;

impl CreditAllocationsValidator {
    pub fn validate_pair_of_order_and_credit_allocation_type(
        &self,
        pairs: &[(i32, AllocationType)],
    ) -> Result<(), LoanError> {
        if pairs.len() != 4 {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy.each_credit_allocation_order.must.contain.4.entries"
                    .to_string(),
                message: format!(
                    "Each provided credit allocation must contain exactly 4 allocation rules, but {} were provided",
                    pairs.len()
                ),
            });
        }
        for (order, _) in pairs {
            if !(1..=4).contains(order) {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy.invalid.order".to_string(),
                    message: "The provided orders must be between 1 and 4".to_string(),
                });
            }
        }
        let unique: HashSet<AllocationType> = pairs.iter().map(|(_, t)| *t).collect();
        if unique.len() != 4 {
            return Err(LoanError::ValidationError {
                code: "advanced-payment-strategy.must.not.have.duplicate.credit.allocation.rule"
                    .to_string(),
                message: "The list of provided credit allocation rules must not contain any duplicates"
                    .to_string(),
            });
        }
        Ok(())
    }

    pub fn validate(
        &self,
        rules: &[LoanProductCreditAllocationRule],
        strategy: &str,
    ) -> Result<(), LoanError> {
        if strategy != ADVANCED_PAYMENT_ALLOCATION_STRATEGY {
            if !rules.is_empty() {
                return Err(LoanError::ValidationError {
                    code: "credit_allocation.must.not.be.provided.when.allocation.strategy.is.not.advanced-payment-strategy"
                        .to_string(),
                    message: format!(
                        "In case '{strategy}' payment strategy, creditAllocation must not be provided"
                    ),
                });
            }
            return Ok(());
        }
        let mut seen = HashSet::new();
        for rule in rules {
            let Some(tx) = rule.transaction_type else {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy.with.not.valid.transaction.type".to_string(),
                    message: "Credit allocation was provided with a not valid transaction type"
                        .to_string(),
                });
            };
            if !seen.insert(tx) {
                return Err(LoanError::ValidationError {
                    code: "advanced-payment-strategy-with-duplicate-credit-allocation".to_string(),
                    message: "The same transaction type must be provided only once".to_string(),
                });
            }
            let pairs: Vec<(i32, AllocationType)> = rule
                .allocation_types
                .iter()
                .enumerate()
                .map(|(i, t)| ((i + 1) as i32, *t))
                .collect();
            self.validate_pair_of_order_and_credit_allocation_type(&pairs)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoanStatus {
    SubmittedAndPendingApproval,
    Approved,
    Active,
    ClosedObligationsMet,
    ClosedWrittenOff,
    Overpaid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanAccount {
    pub id: i64,
    pub external_id: Option<String>,
    pub client_id: i64,
    pub product_id: i64,
    pub principal: Decimal,
    pub annual_nominal_interest_rate: Decimal,
    pub number_of_repayments: u32,
    pub status: LoanStatus,
    pub installments: Vec<LoanRepaymentScheduleInstallment>,
    pub total_overpaid: Decimal,
}

impl LoanAccount {
    pub fn new_submitted(
        id: i64,
        client_id: i64,
        product_id: i64,
        principal: Decimal,
        annual_nominal_interest_rate: Decimal,
        number_of_repayments: u32,
    ) -> Self {
        Self {
            id,
            external_id: None,
            client_id,
            product_id,
            principal,
            annual_nominal_interest_rate,
            number_of_repayments,
            status: LoanStatus::SubmittedAndPendingApproval,
            installments: Vec::new(),
            total_overpaid: Decimal::ZERO,
        }
    }

    pub fn approve(&mut self) -> Result<(), LoanError> {
        if self.status != LoanStatus::SubmittedAndPendingApproval {
            return Err(LoanError::InvalidStateTransition {
                from: self.status,
                to: LoanStatus::Approved,
            });
        }
        self.status = LoanStatus::Approved;
        Ok(())
    }

    pub fn disburse(
        &mut self,
        installments: Vec<LoanRepaymentScheduleInstallment>,
    ) -> Result<(), LoanError> {
        if self.status != LoanStatus::Approved {
            return Err(LoanError::InvalidStateTransition {
                from: self.status,
                to: LoanStatus::Active,
            });
        }
        self.installments = installments;
        self.status = LoanStatus::Active;
        Ok(())
    }

    pub fn make_repayment(&mut self, mut amount: Decimal) -> Result<Decimal, LoanError> {
        if self.status != LoanStatus::Active {
            return Err(LoanError::InvalidStateTransition {
                from: self.status,
                to: LoanStatus::Active,
            });
        }
        for inst in &mut self.installments {
            if amount <= Decimal::ZERO {
                break;
            }
            // Allocate Interest -> Principal
            let int_rem = inst.interest_charged.unwrap_or(Decimal::ZERO)
                - inst.interest_paid.unwrap_or(Decimal::ZERO);
            if int_rem > Decimal::ZERO {
                let pay_int = amount.min(int_rem);
                inst.interest_paid = Some(inst.interest_paid.unwrap_or(Decimal::ZERO) + pay_int);
                amount -= pay_int;
            }
            let prin_rem = inst.principal.unwrap_or(Decimal::ZERO)
                - inst.principal_completed.unwrap_or(Decimal::ZERO);
            if amount > Decimal::ZERO && prin_rem > Decimal::ZERO {
                let pay_prin = amount.min(prin_rem);
                inst.principal_completed =
                    Some(inst.principal_completed.unwrap_or(Decimal::ZERO) + pay_prin);
                amount -= pay_prin;
            }
        }
        if amount > Decimal::ZERO {
            self.total_overpaid += amount;
            self.status = LoanStatus::Overpaid;
        } else if self.total_outstanding() == Decimal::ZERO {
            self.status = LoanStatus::ClosedObligationsMet;
        }
        Ok(self.total_outstanding())
    }

    pub fn total_outstanding(&self) -> Decimal {
        self.installments
            .iter()
            .map(|i| i.total_outstanding())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn apr_calculator_matches_fineract_2492_actual_and_360_days_tests() {
        let calc = AprCalculator;
        // 1. DAYS frequency with ACTUAL -> 10% * 365 = 3650%
        let apr_actual = calc.calculate_from(
            PeriodFrequencyType::Days,
            dec!(10.0),
            1,
            1,
            PeriodFrequencyType::Days,
            DaysInYearType::Actual,
        );
        assert_eq!(apr_actual, dec!(3650.0));

        // 2. WHOLE_TERM frequency with 3 daily repayments and ACTUAL -> (10 / 3 rounded 8dp) * 365 = 1216.66666545
        let apr_whole_term = calc.calculate_from(
            PeriodFrequencyType::WholeTerm,
            dec!(10.0),
            3,
            1,
            PeriodFrequencyType::Days,
            DaysInYearType::Actual,
        );
        let expected = (dec!(10.0) / dec!(3))
            .round_dp_with_strategy(8, RoundingStrategy::MidpointNearestEven)
            * dec!(365);
        assert_eq!(apr_whole_term, expected);

        // 3. DAYS frequency with DAYS_360 -> 10% * 360 = 3600%
        let apr_360 = calc.calculate_from(
            PeriodFrequencyType::Days,
            dec!(10.0),
            1,
            1,
            PeriodFrequencyType::Days,
            DaysInYearType::Days360,
        );
        assert_eq!(apr_360, dec!(3600.0));
    }

    #[test]
    fn loan_over_applied_calculation_type_options_match_java_tests() {
        let list = LoanOverAppliedCalculationType::get_values_as_enum_option_data_list();
        for v in LoanOverAppliedCalculationType::values() {
            assert!(list.contains(&v.as_enum_option_data()));
        }
        assert_eq!(
            LoanOverAppliedCalculationType::Flat.as_enum_option_data().id,
            1
        );
        assert_eq!(
            LoanOverAppliedCalculationType::Percentage
                .as_enum_option_data()
                .id,
            2
        );
        assert!(list.iter().all(|o| o.id != 0));
    }

    #[test]
    fn advanced_payment_and_credit_allocations_validators_match_progressive_loan_tests() {
        let validator = AdvancedPaymentAllocationsValidator;
        let all_12 = PaymentAllocationType::all_12();
        let pairs: Vec<(i32, PaymentAllocationType)> = all_12
            .iter()
            .enumerate()
            .map(|(i, t)| ((i + 1) as i32, *t))
            .collect();
        assert!(validator
            .validate_pair_of_order_and_payment_allocation_type(&pairs)
            .is_ok());

        // Fewer than 12 fails
        let err_11 = validator
            .validate_pair_of_order_and_payment_allocation_type(&pairs[..11])
            .unwrap_err();
        assert!(matches!(
            err_11,
            LoanError::ValidationError { ref code, .. }
            if code == "advanced-payment-strategy.each_payment_allocation_order.must.contain.12.entries"
        ));

        // Valid DEFAULT + REPAYMENT rules pass
        let rule_default = LoanProductPaymentAllocationRule {
            transaction_type: Some(PaymentAllocationTransactionType::Default),
            allocation_types: all_12.clone(),
            future_installment_allocation_rule: Some(FutureInstallmentAllocationRule::LastInstallment),
        };
        let rule_repayment = LoanProductPaymentAllocationRule {
            transaction_type: Some(PaymentAllocationTransactionType::Repayment),
            allocation_types: all_12,
            future_installment_allocation_rule: Some(FutureInstallmentAllocationRule::LastInstallment),
        };
        assert!(validator
            .validate(
                Some(&[rule_default.clone(), rule_repayment]),
                ADVANCED_PAYMENT_ALLOCATION_STRATEGY
            )
            .is_ok());
        assert!(validator
            .check_grouping_of_allocation_rules(&[rule_default])
            .is_ok());

        // Mixed due-type grouping fails horizontal check
        let mut mixed_types = PaymentAllocationType::all_12();
        mixed_types.swap(0, 4); // swap PastDuePenalty with DuePenalty
        let mixed_rule = LoanProductPaymentAllocationRule {
            transaction_type: Some(PaymentAllocationTransactionType::Default),
            allocation_types: mixed_types,
            future_installment_allocation_rule: Some(FutureInstallmentAllocationRule::LastInstallment),
        };
        assert!(validator
            .check_grouping_of_allocation_rules(&[mixed_rule])
            .is_err());

        // Credit allocations validator
        let credit_validator = CreditAllocationsValidator;
        let credit_rule = LoanProductCreditAllocationRule {
            transaction_type: Some(CreditAllocationTransactionType::Chargeback),
            allocation_types: AllocationType::all_4(),
        };
        assert!(credit_validator
            .validate(&[credit_rule], ADVANCED_PAYMENT_ALLOCATION_STRATEGY)
            .is_ok());
    }

    #[test]
    fn loan_account_state_machine_and_repayment_allocation() {
        let mut loan = LoanAccount::new_submitted(1, 10, 1, dec!(1000.00), dec!(12.0), 2);
        loan.approve().unwrap();
        let d0 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let d1 = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        loan.disburse(vec![
            LoanRepaymentScheduleInstallment::new(
                1,
                d0,
                d1,
                dec!(500.00),
                dec!(10.00),
                dec!(0),
                dec!(0),
            ),
            LoanRepaymentScheduleInstallment::new(
                2,
                d1,
                d2,
                dec!(500.00),
                dec!(5.00),
                dec!(0),
                dec!(0),
            ),
        ])
        .unwrap();

        assert_eq!(loan.total_outstanding(), dec!(1015.00));
        let rem = loan.make_repayment(dec!(510.00)).unwrap();
        assert_eq!(rem, dec!(505.00));
        assert_eq!(loan.status, LoanStatus::Active);

        let rem2 = loan.make_repayment(dec!(505.00)).unwrap();
        assert_eq!(rem2, Decimal::ZERO);
        assert_eq!(loan.status, LoanStatus::ClosedObligationsMet);
    }
}
