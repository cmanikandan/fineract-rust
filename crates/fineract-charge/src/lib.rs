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
//
// MODIFICATION NOTICE: Translated and adapted from Apache Fineract (Java)
// module `fineract-charge` to Rust Edition 2024.

use fineract_schedule_generator::BigDec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

pub const CHARGE_TIME_PARAM_NAME: &str = "chargeTimeType";
pub const CHARGE_CALCULATION_TYPE_PARAM_NAME: &str = "chargeCalculationType";
pub const FEE_ON_MONTH_DAY_PARAM_NAME: &str = "feeOnMonthDay";
pub const FEE_INTERVAL_PARAM_NAME: &str = "feeInterval";
pub const LOCALE_PARAM_NAME: &str = "locale";
pub const FEE_FREQUENCY_PARAM_NAME: &str = "feeFrequency";
pub const PAYMENT_MODE_PARAM_NAME: &str = "chargePaymentMode";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiParameterError {
    pub parameter_name: String,
    pub user_message_globalisation_code: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChargeError {
    #[error("Charge due at disbursement cannot be penalty: {0}")]
    ChargeDueAtDisbursementCannotBePenaltyException(String),
    #[error("Overdue charge must be penalty: {0}")]
    ChargeMustBePenaltyException(String),
    #[error("Charge parameter update not supported: {0}")]
    ChargeParameterUpdateNotSupportedException(String),
    #[error("Charge with id {0} not found")]
    ChargeNotFoundException(i64),
    #[error("Platform data validation exception")]
    PlatformApiDataValidationException { errors: Vec<ApiParameterError> },
    #[error("Invalid JSON")]
    InvalidJson,
}

/// Port of `org.apache.fineract.portfolio.charge.domain.ChargeAppliesTo`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChargeAppliesTo {
    Invalid = 0,
    Loan = 1,
    Savings = 2,
    Client = 3,
    Shares = 4,
    WorkingCapitalLoan = 5,
}

impl ChargeAppliesTo {
    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "chargeAppliesTo.invalid",
            Self::Loan => "chargeAppliesTo.loan",
            Self::Savings => "chargeAppliesTo.savings",
            Self::Client => "chargeAppliesTo.client",
            Self::Shares => "chargeAppliesTo.shares",
            Self::WorkingCapitalLoan => "chargeAppliesTo.workingCapitalLoan",
        }
    }

    pub fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(1) => Self::Loan,
            Some(2) => Self::Savings,
            Some(3) => Self::Client,
            Some(4) => Self::Shares,
            Some(5) => Self::WorkingCapitalLoan,
            _ => Self::Invalid,
        }
    }

    pub fn is_loan_charge(&self) -> bool {
        *self == Self::Loan
    }

    pub fn is_savings_charge(&self) -> bool {
        *self == Self::Savings
    }

    pub fn is_client_charge(&self) -> bool {
        *self == Self::Client
    }

    pub fn is_working_capital_loan_charge(&self) -> bool {
        *self == Self::WorkingCapitalLoan
    }

    pub fn is_shares_charge(&self) -> bool {
        *self == Self::Shares
    }
}

/// Port of `org.apache.fineract.portfolio.charge.domain.ChargeTimeType`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChargeTimeType {
    Invalid = 0,
    Disbursement = 1,
    SpecifiedDueDate = 2,
    SavingsActivation = 3,
    SavingsClosure = 4,
    WithdrawalFee = 5,
    AnnualFee = 6,
    MonthlyFee = 7,
    InstalmentFee = 8,
    OverdueInstallment = 9,
    OverdraftFee = 10,
    WeeklyFee = 11,
    TrancheDisbursement = 12,
    ShareAccountActivation = 13,
    SharePurchase = 14,
    ShareRedeem = 15,
    SavingsNoActivityFee = 16,
}

impl ChargeTimeType {
    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(1) => Self::Disbursement,
            Some(2) => Self::SpecifiedDueDate,
            Some(3) => Self::SavingsActivation,
            Some(4) => Self::SavingsClosure,
            Some(5) => Self::WithdrawalFee,
            Some(6) => Self::AnnualFee,
            Some(7) => Self::MonthlyFee,
            Some(8) => Self::InstalmentFee,
            Some(9) => Self::OverdueInstallment,
            Some(10) => Self::OverdraftFee,
            Some(11) => Self::WeeklyFee,
            Some(12) => Self::TrancheDisbursement,
            Some(13) => Self::ShareAccountActivation,
            Some(14) => Self::SharePurchase,
            Some(15) => Self::ShareRedeem,
            Some(16) => Self::SavingsNoActivityFee,
            _ => Self::Invalid,
        }
    }

    pub fn valid_working_capital_loan_product() -> Vec<Self> {
        vec![Self::Disbursement, Self::SpecifiedDueDate]
    }

    pub fn valid_working_capital_loan_product_values() -> Vec<i32> {
        Self::valid_working_capital_loan_product()
            .into_iter()
            .map(|e| e.value())
            .collect()
    }

    pub fn is_time_of_disbursement(&self) -> bool {
        *self == Self::Disbursement
    }

    pub fn is_tranche_disbursement(&self) -> bool {
        *self == Self::TrancheDisbursement
    }

    pub fn is_on_specified_due_date(&self) -> bool {
        *self == Self::SpecifiedDueDate
    }

    pub fn is_overdue_installment(&self) -> bool {
        *self == Self::OverdueInstallment
    }

    pub fn is_withdrawal_fee(&self) -> bool {
        *self == Self::WithdrawalFee
    }

    pub fn is_savings_no_activity_fee(&self) -> bool {
        *self == Self::SavingsNoActivityFee
    }

    pub fn is_monthly_fee(&self) -> bool {
        *self == Self::MonthlyFee
    }

    pub fn is_annual_fee(&self) -> bool {
        *self == Self::AnnualFee
    }

    pub fn is_allowed_loan_charge_time(&self) -> bool {
        matches!(
            self,
            Self::Disbursement
                | Self::SpecifiedDueDate
                | Self::InstalmentFee
                | Self::OverdueInstallment
                | Self::TrancheDisbursement
        )
    }

    pub fn is_allowed_savings_charge_time(&self) -> bool {
        matches!(
            self,
            Self::SpecifiedDueDate
                | Self::SavingsActivation
                | Self::SavingsClosure
                | Self::WithdrawalFee
                | Self::AnnualFee
                | Self::MonthlyFee
                | Self::WeeklyFee
                | Self::OverdraftFee
                | Self::SavingsNoActivityFee
        )
    }

    pub fn is_allowed_client_charge_time(&self) -> bool {
        self.is_on_specified_due_date()
    }
}

/// Port of `org.apache.fineract.portfolio.charge.domain.ChargeCalculationType`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChargeCalculationType {
    Invalid = 0,
    Flat = 1,
    PercentOfAmount = 2,
    PercentOfAmountAndInterest = 3,
    PercentOfInterest = 4,
    PercentOfDisbursementAmount = 5,
}

impl ChargeCalculationType {
    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(1) => Self::Flat,
            Some(2) => Self::PercentOfAmount,
            Some(3) => Self::PercentOfAmountAndInterest,
            Some(4) => Self::PercentOfInterest,
            Some(5) => Self::PercentOfDisbursementAmount,
            _ => Self::Invalid,
        }
    }

    pub fn valid_enums_for_working_capital_loan_by_time(
        charge_time_type: Option<ChargeTimeType>,
    ) -> Vec<Self> {
        match charge_time_type {
            Some(ChargeTimeType::Disbursement) => vec![Self::Flat, Self::PercentOfAmount],
            Some(ChargeTimeType::SpecifiedDueDate) => vec![Self::Flat],
            _ => vec![],
        }
    }

    pub fn valid_enums_for_working_capital_loan() -> Vec<Self> {
        let mut out = Vec::new();
        for tt in ChargeTimeType::valid_working_capital_loan_product() {
            for ct in Self::valid_enums_for_working_capital_loan_by_time(Some(tt)) {
                if !out.contains(&ct) {
                    out.push(ct);
                }
            }
        }
        out
    }

    pub fn valid_values_for_working_capital_loan(charge_time_type: i32) -> Vec<i32> {
        Self::valid_enums_for_working_capital_loan_by_time(Some(ChargeTimeType::from_int(Some(
            charge_time_type,
        ))))
        .into_iter()
        .map(|e| e.value())
        .collect()
    }

    pub fn is_flat(&self) -> bool {
        *self == Self::Flat
    }

    pub fn is_percentage_of_amount(&self) -> bool {
        *self == Self::PercentOfAmount
    }

    pub fn is_percentage_of_disbursement_amount(&self) -> bool {
        *self == Self::PercentOfDisbursementAmount
    }

    pub fn is_allowed_savings_charge_calculation_type(&self) -> bool {
        self.is_flat() || self.is_percentage_of_amount()
    }

    pub fn is_allowed_client_charge_calculation_type(&self) -> bool {
        self.is_flat()
    }
}

/// Port of `org.apache.fineract.portfolio.charge.domain.ChargePaymentMode`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChargePaymentMode {
    Regular = 0,
    AccountTransfer = 1,
}

impl ChargePaymentMode {
    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn from_int(v: i32) -> Self {
        match v {
            1 => Self::AccountTransfer,
            _ => Self::Regular,
        }
    }

    pub fn valid_enums_for_working_capital_loan() -> Vec<Self> {
        vec![Self::Regular]
    }

    pub fn valid_values_for_working_capital_loan() -> Vec<i32> {
        vec![Self::Regular.value()]
    }
}

/// Port of `org.apache.fineract.portfolio.charge.domain.Charge`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Charge {
    pub id: Option<i64>,
    pub name: String,
    pub amount: BigDec,
    pub currency_code: String,
    pub charge_applies_to: i32,
    pub charge_time_type: i32,
    pub charge_calculation: i32,
    pub charge_payment_mode: Option<i32>,
    pub penalty: bool,
    pub active: bool,
    pub deleted: bool,
    pub min_cap: Option<BigDec>,
    pub max_cap: Option<BigDec>,
}

impl Charge {
    pub fn from_json(json_str: &str) -> Result<Self, ChargeError> {
        let v: Value = serde_json::from_str(json_str).map_err(|_| ChargeError::InvalidJson)?;
        let Some(obj) = v.as_object() else {
            return Err(ChargeError::InvalidJson);
        };

        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let amount = obj
            .get("amount")
            .and_then(|v| {
                v.as_f64()
                    .map(|f| BigDec::parse_lossy(&f.to_string()))
                    .or_else(|| v.as_str().map(BigDec::parse_lossy))
            })
            .unwrap_or(BigDec::ZERO);
        let currency_code = obj
            .get("currencyCode")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_string();

        let charge_applies_to = ChargeAppliesTo::from_int(
            obj.get("chargeAppliesTo")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
        );
        let charge_time = ChargeTimeType::from_int(
            obj.get(CHARGE_TIME_PARAM_NAME)
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
        );
        let charge_calc = ChargeCalculationType::from_int(
            obj.get(CHARGE_CALCULATION_TYPE_PARAM_NAME)
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
        );
        let raw_payment_mode = obj
            .get(PAYMENT_MODE_PARAM_NAME)
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);

        let payment_mode = match raw_payment_mode {
            None => {
                if charge_applies_to.is_working_capital_loan_charge() {
                    Some(ChargePaymentMode::Regular)
                } else {
                    None
                }
            }
            Some(m) => Some(ChargePaymentMode::from_int(m)),
        };

        let penalty = obj.get("penalty").and_then(|v| v.as_bool()).unwrap_or(false);
        let active = obj.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
        let min_cap = obj
            .get("minCap")
            .and_then(|v| v.as_f64().map(|f| BigDec::parse_lossy(&f.to_string())));
        let max_cap = obj
            .get("maxCap")
            .and_then(|v| v.as_f64().map(|f| BigDec::parse_lossy(&f.to_string())));

        let mut errors = Vec::new();

        if charge_applies_to.is_savings_charge() {
            if !charge_time.is_allowed_savings_charge_time() {
                errors.push(ApiParameterError {
                    parameter_name: CHARGE_TIME_PARAM_NAME.to_string(),
                    user_message_globalisation_code:
                        "validation.msg.charges.not.allowed.charge.time.for.savings".to_string(),
                });
            }
            if !charge_calc.is_allowed_savings_charge_calculation_type() {
                errors.push(ApiParameterError {
                    parameter_name: CHARGE_CALCULATION_TYPE_PARAM_NAME.to_string(),
                    user_message_globalisation_code:
                        "validation.msg.charges.not.allowed.charge.calculation.type.for.savings"
                            .to_string(),
                });
            }
        } else if charge_applies_to.is_loan_charge() {
            if penalty
                && (charge_time.is_time_of_disbursement()
                    || charge_time.is_tranche_disbursement())
            {
                return Err(ChargeError::ChargeDueAtDisbursementCannotBePenaltyException(
                    name,
                ));
            }
            if !penalty && charge_time.is_overdue_installment() {
                return Err(ChargeError::ChargeMustBePenaltyException(name));
            }
            if !charge_time.is_allowed_loan_charge_time() {
                errors.push(ApiParameterError {
                    parameter_name: CHARGE_TIME_PARAM_NAME.to_string(),
                    user_message_globalisation_code:
                        "validation.msg.charges.not.allowed.charge.time.for.loan".to_string(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(ChargeError::PlatformApiDataValidationException { errors });
        }

        Ok(Self {
            id: Some(1),
            name,
            amount,
            currency_code,
            charge_applies_to: charge_applies_to.value(),
            charge_time_type: charge_time.value(),
            charge_calculation: charge_calc.value(),
            charge_payment_mode: payment_mode.map(|m| m.value()),
            penalty,
            active,
            deleted: false,
            min_cap,
            max_cap,
        })
    }

    pub fn is_loan_charge(&self) -> bool {
        ChargeAppliesTo::from_int(Some(self.charge_applies_to)).is_loan_charge()
    }

    pub fn is_working_capital_loan_charge(&self) -> bool {
        ChargeAppliesTo::from_int(Some(self.charge_applies_to)).is_working_capital_loan_charge()
    }

    pub fn is_savings_charge(&self) -> bool {
        ChargeAppliesTo::from_int(Some(self.charge_applies_to)).is_savings_charge()
    }

    pub fn is_client_charge(&self) -> bool {
        ChargeAppliesTo::from_int(Some(self.charge_applies_to)).is_client_charge()
    }

    pub fn update(&mut self, json_str: &str) -> Result<HashMap<String, Value>, ChargeError> {
        let v: Value = serde_json::from_str(json_str).map_err(|_| ChargeError::InvalidJson)?;
        let Some(obj) = v.as_object() else {
            return Err(ChargeError::InvalidJson);
        };

        let mut actual_changes: HashMap<String, Value> = HashMap::new();
        let mut errors: Vec<ApiParameterError> = Vec::new();
        let locale = obj
            .get(LOCALE_PARAM_NAME)
            .and_then(|v| v.as_str())
            .unwrap_or("en")
            .to_string();

        if let Some(Value::String(new_name)) = obj.get("name") {
            if new_name != &self.name {
                actual_changes.insert("name".to_string(), Value::String(new_name.clone()));
                self.name = new_name.clone();
            }
        }

        if let Some(amt_val) = obj.get("amount") {
            let new_amt = amt_val
                .as_f64()
                .map(|f| BigDec::parse_lossy(&f.to_string()))
                .or_else(|| amt_val.as_str().map(BigDec::parse_lossy));
            if let Some(na) = new_amt {
                if na != self.amount {
                    actual_changes.insert("amount".to_string(), Value::String(na.to_string()));
                    actual_changes
                        .insert(LOCALE_PARAM_NAME.to_string(), Value::String(locale.clone()));
                    self.amount = na;
                }
            }
        }

        if let Some(ct_val) = obj.get(CHARGE_TIME_PARAM_NAME).and_then(|v| v.as_i64()) {
            let new_val = ct_val as i32;
            if new_val != self.charge_time_type {
                actual_changes.insert(
                    CHARGE_TIME_PARAM_NAME.to_string(),
                    Value::Number(new_val.into()),
                );
                actual_changes
                    .insert(LOCALE_PARAM_NAME.to_string(), Value::String(locale.clone()));
                self.charge_time_type = ChargeTimeType::from_int(Some(new_val)).value();
            }
        }

        if let Some(ca_val) = obj.get("chargeAppliesTo").and_then(|v| v.as_i64()) {
            if (ca_val as i32) != self.charge_applies_to {
                return Err(ChargeError::ChargeParameterUpdateNotSupportedException(
                    "charge.applies.to".to_string(),
                ));
            }
        }

        if let Some(cc_val) = obj
            .get(CHARGE_CALCULATION_TYPE_PARAM_NAME)
            .and_then(|v| v.as_i64())
        {
            let new_val = cc_val as i32;
            if new_val != self.charge_calculation {
                actual_changes.insert(
                    CHARGE_CALCULATION_TYPE_PARAM_NAME.to_string(),
                    Value::Number(new_val.into()),
                );
                actual_changes
                    .insert(LOCALE_PARAM_NAME.to_string(), Value::String(locale.clone()));
                self.charge_calculation = ChargeCalculationType::from_int(Some(new_val)).value();
            }
        }

        if self.is_working_capital_loan_charge()
            && (actual_changes.contains_key(CHARGE_TIME_PARAM_NAME)
                || actual_changes.contains_key(CHARGE_CALCULATION_TYPE_PARAM_NAME))
        {
            let valid_vals =
                ChargeCalculationType::valid_values_for_working_capital_loan(self.charge_time_type);
            if !valid_vals.contains(&self.charge_calculation) {
                errors.push(ApiParameterError {
                    parameter_name: CHARGE_CALCULATION_TYPE_PARAM_NAME.to_string(),
                    user_message_globalisation_code:
                        "validation.msg.charges.chargeCalculationType.is.not.one.of.expected.enumerations"
                            .to_string(),
                });
            }
        }

        if (self.is_loan_charge() || self.is_working_capital_loan_charge())
            && obj.contains_key(PAYMENT_MODE_PARAM_NAME)
        {
            let raw_mode = obj
                .get(PAYMENT_MODE_PARAM_NAME)
                .and_then(|v| v.as_i64())
                .map(|n| n as i32);
            let errors_before = errors.len();

            let resolved_mode = if self.is_working_capital_loan_charge() {
                let m = raw_mode.unwrap_or_else(|| ChargePaymentMode::Regular.value());
                let allowed = ChargePaymentMode::valid_values_for_working_capital_loan();
                if !allowed.contains(&m) {
                    errors.push(ApiParameterError {
                        parameter_name: PAYMENT_MODE_PARAM_NAME.to_string(),
                        user_message_globalisation_code:
                            "validation.msg.charges.chargePaymentMode.is.not.one.of.expected.enumerations"
                                .to_string(),
                    });
                }
                Some(m)
            } else {
                if raw_mode.is_none() {
                    errors.push(ApiParameterError {
                        parameter_name: PAYMENT_MODE_PARAM_NAME.to_string(),
                        user_message_globalisation_code:
                            "validation.msg.charges.chargePaymentMode.cannot.be.blank".to_string(),
                    });
                }
                raw_mode
            };

            if errors.len() == errors_before {
                if let Some(new_val) = resolved_mode {
                    if Some(new_val) != self.charge_payment_mode {
                        actual_changes.insert(
                            PAYMENT_MODE_PARAM_NAME.to_string(),
                            Value::Number(new_val.into()),
                        );
                        actual_changes
                            .insert(LOCALE_PARAM_NAME.to_string(), Value::String(locale.clone()));
                        self.charge_payment_mode = Some(new_val);
                    }
                }
            }
        }

        if !self.is_working_capital_loan_charge()
            && self.penalty
            && ChargeTimeType::from_int(Some(self.charge_time_type)).is_time_of_disbursement()
        {
            return Err(ChargeError::ChargeDueAtDisbursementCannotBePenaltyException(
                self.name.clone(),
            ));
        }

        if !self.is_working_capital_loan_charge()
            && !self.penalty
            && ChargeTimeType::from_int(Some(self.charge_time_type)).is_overdue_installment()
        {
            return Err(ChargeError::ChargeMustBePenaltyException(self.name.clone()));
        }

        if !errors.is_empty() {
            return Err(ChargeError::PlatformApiDataValidationException { errors });
        }

        Ok(actual_changes)
    }

    pub fn delete(&mut self) {
        self.deleted = true;
        let id = self.id.unwrap_or(0);
        self.name = format!("{id}_{}", self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Port of ChargeCalculationTypeTest
    // ========================================================================

    #[test]
    fn every_working_capital_product_time_type_has_calculation_types() {
        for charge_time_type in ChargeTimeType::valid_working_capital_loan_product() {
            assert!(
                !ChargeCalculationType::valid_enums_for_working_capital_loan_by_time(Some(
                    charge_time_type
                ))
                .is_empty(),
                "{charge_time_type:?} is a valid WC product time type but maps to no calculation type"
            );
        }
    }

    #[test]
    fn working_capital_calculation_types_per_time_type() {
        assert_eq!(
            vec![
                ChargeCalculationType::Flat,
                ChargeCalculationType::PercentOfAmount
            ],
            ChargeCalculationType::valid_enums_for_working_capital_loan_by_time(Some(
                ChargeTimeType::Disbursement
            ))
        );
        assert_eq!(
            vec![ChargeCalculationType::Flat],
            ChargeCalculationType::valid_enums_for_working_capital_loan_by_time(Some(
                ChargeTimeType::SpecifiedDueDate
            ))
        );
        assert!(
            ChargeCalculationType::valid_enums_for_working_capital_loan_by_time(Some(
                ChargeTimeType::InstalmentFee
            ))
            .is_empty()
        );
        assert!(ChargeCalculationType::valid_enums_for_working_capital_loan_by_time(None).is_empty());
    }

    #[test]
    fn working_capital_union_is_derived_from_product_time_types() {
        assert_eq!(
            vec![
                ChargeCalculationType::Flat,
                ChargeCalculationType::PercentOfAmount
            ],
            ChargeCalculationType::valid_enums_for_working_capital_loan()
        );
    }

    // ========================================================================
    // Port of ChargeTest
    // ========================================================================

    fn working_capital_disbursement_charge() -> Charge {
        Charge::from_json(
            r#"{"name": "wc-disbursement", "amount": 20, "currencyCode": "USD", "chargeAppliesTo": 5, "chargeTimeType": 1, "chargeCalculationType": 1, "penalty": false, "active": true, "locale": "en"}"#,
        )
        .unwrap()
    }

    fn savings_specified_due_date_charge() -> Charge {
        Charge::from_json(
            r#"{"name": "savings-fee", "amount": 20, "currencyCode": "USD", "chargeAppliesTo": 2, "chargeTimeType": 2, "chargeCalculationType": 1, "penalty": false, "active": true, "locale": "en"}"#,
        )
        .unwrap()
    }

    fn loan_disbursement_charge() -> Charge {
        Charge::from_json(
            r#"{"name": "loan-disbursement", "amount": 20, "currencyCode": "USD", "chargeAppliesTo": 1, "chargeTimeType": 1, "chargeCalculationType": 1, "chargePaymentMode": 0, "penalty": false, "active": true, "locale": "en"}"#,
        )
        .unwrap()
    }

    fn codes_of(err: ChargeError) -> Vec<String> {
        match err {
            ChargeError::PlatformApiDataValidationException { errors } => errors
                .into_iter()
                .map(|e| e.user_message_globalisation_code)
                .collect(),
            other => panic!("Expected PlatformApiDataValidationException, got {other:?}"),
        }
    }

    #[test]
    fn update_on_working_capital_charge_explicit_null_payment_mode_resolves_to_regular() {
        let mut charge = working_capital_disbursement_charge();
        let changes = charge
            .update(r#"{"chargePaymentMode": null, "locale": "en"}"#)
            .unwrap();
        assert!(!changes.contains_key(PAYMENT_MODE_PARAM_NAME));
        assert_eq!(
            Some(ChargePaymentMode::Regular.value()),
            charge.charge_payment_mode
        );
    }

    #[test]
    fn update_on_working_capital_charge_explicit_regular_payment_mode_is_no_change() {
        let mut charge = working_capital_disbursement_charge();
        let changes = charge
            .update(r#"{"chargePaymentMode": 0, "locale": "en"}"#)
            .unwrap();
        assert!(!changes.contains_key(PAYMENT_MODE_PARAM_NAME));
        assert_eq!(
            Some(ChargePaymentMode::Regular.value()),
            charge.charge_payment_mode
        );
    }

    #[test]
    fn update_on_working_capital_charge_account_transfer_payment_mode_is_rejected() {
        let mut charge = working_capital_disbursement_charge();
        let rejection = charge
            .update(r#"{"chargePaymentMode": 1, "locale": "en"}"#)
            .unwrap_err();
        assert_eq!(
            vec!["validation.msg.charges.chargePaymentMode.is.not.one.of.expected.enumerations"],
            codes_of(rejection)
        );
        assert_eq!(
            Some(ChargePaymentMode::Regular.value()),
            charge.charge_payment_mode
        );
    }

    #[test]
    fn update_on_savings_charge_explicit_null_payment_mode_is_ignored() {
        let mut charge = savings_specified_due_date_charge();
        let changes = charge
            .update(r#"{"chargePaymentMode": null, "locale": "en"}"#)
            .unwrap();
        assert!(!changes.contains_key(PAYMENT_MODE_PARAM_NAME));
    }

    #[test]
    fn update_on_working_capital_disbursement_penalty_amount_only_succeeds() {
        let mut charge = Charge::from_json(
            r#"{"name": "wc-disbursement-penalty", "amount": 20, "currencyCode": "USD", "chargeAppliesTo": 5, "chargeTimeType": 1, "chargeCalculationType": 1, "penalty": true, "active": true, "locale": "en"}"#,
        )
        .unwrap();
        let changes = charge.update(r#"{"amount": 30, "locale": "en"}"#).unwrap();
        assert_eq!(changes.get("amount"), Some(&Value::String("30".to_string())));
        assert_eq!(charge.amount, BigDec::from(30));
    }

    #[test]
    fn update_on_client_penalty_charge_to_disbursement_time_type_is_still_rejected() {
        let mut charge = Charge::from_json(
            r#"{"name": "client-penalty", "amount": 20, "currencyCode": "USD", "chargeAppliesTo": 3, "chargeTimeType": 2, "chargeCalculationType": 1, "penalty": true, "active": true, "locale": "en"}"#,
        )
        .unwrap();
        let err = charge
            .update(r#"{"chargeTimeType": 1, "locale": "en"}"#)
            .unwrap_err();
        assert!(matches!(
            err,
            ChargeError::ChargeDueAtDisbursementCannotBePenaltyException(_)
        ));
    }

    #[test]
    fn update_on_loan_charge_explicit_null_payment_mode_is_rejected() {
        let mut charge = loan_disbursement_charge();
        let rejection = charge
            .update(r#"{"chargePaymentMode": null, "locale": "en"}"#)
            .unwrap_err();
        assert_eq!(
            vec!["validation.msg.charges.chargePaymentMode.cannot.be.blank"],
            codes_of(rejection)
        );
        assert_eq!(
            Some(ChargePaymentMode::Regular.value()),
            charge.charge_payment_mode
        );
    }

    #[test]
    fn update_on_loan_charge_payment_mode_change_is_persisted_and_reported() {
        let mut charge = loan_disbursement_charge();
        let changes = charge
            .update(r#"{"chargePaymentMode": 1, "locale": "en"}"#)
            .unwrap();
        assert_eq!(
            changes.get(PAYMENT_MODE_PARAM_NAME),
            Some(&Value::Number(
                ChargePaymentMode::AccountTransfer.value().into()
            ))
        );
        assert_eq!(
            Some(ChargePaymentMode::AccountTransfer.value()),
            charge.charge_payment_mode
        );
    }
}
