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
// module `fineract-branch` to Rust Edition 2024.

use chrono::NaiveDate;
use fineract_schedule_generator::BigDec;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

pub const OFFICE_ID: &str = "officeId";
pub const NAME: &str = "name";
pub const DESCRIPTION: &str = "description";
pub const START_DATE: &str = "startDate";
pub const END_DATE: &str = "endDate";
pub const STATUS: &str = "status";
pub const DATE_FORMAT: &str = "dateFormat";
pub const LOCALE: &str = "locale";
pub const IS_FULL_DAY: &str = "isFullDay";
pub const STAFF_ID: &str = "staffId";
pub const ENTITY_TYPE: &str = "entityType";
pub const ENTITY_ID: &str = "entityId";
pub const CURRENCY_CODE: &str = "currencyCode";
pub const HOUR_START_TIME: &str = "hourStartTime";
pub const MIN_START_TIME: &str = "minStartTime";
pub const HOUR_END_TIME: &str = "hourEndTime";
pub const MIN_END_TIME: &str = "minEndTime";
pub const TXN_AMOUNT: &str = "txnAmount";
pub const TXN_DATE: &str = "txnDate";
pub const TXN_NOTE: &str = "txnNote";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiParameterError {
    pub parameter_name: String,
    pub default_user_message: String,
    pub user_message_globalisation_code: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BranchError {
    #[error("Invalid JSON")]
    InvalidJsonException,
    #[error("Unsupported parameter: {0}")]
    UnsupportedParameterException(String),
    #[error("Start date {start_date} cannot be after end date {end_date}")]
    InvalidDateInputException {
        start_date: String,
        end_date: String,
    },
    #[error("Teller with id {0} not found")]
    TellerNotFoundException(i64),
    #[error("Cashier with id {0} not found")]
    CashierNotFoundException(i64),
    #[error("Cashier has insufficient amount for settlement")]
    CashierInsufficientAmountException,
    #[error("Cashier date range is outside teller date range")]
    CashierDateRangeOutOfTellerDateRangeException,
    #[error("Cashier already allocated for this time range")]
    CashierAlreadyAllocated,
    #[error("Validation errors exist")]
    PlatformApiDataValidationException { errors: Vec<ApiParameterError> },
}

/// Port of `org.apache.fineract.organisation.teller.domain.TellerStatus`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TellerStatus {
    Invalid = 0,
    Pending = 100,
    Active = 300,
    Inactive = 400,
    Closed = 600,
}

impl TellerStatus {
    pub fn from_int(v: Option<i32>) -> Self {
        match v {
            Some(100) => Self::Pending,
            Some(300) => Self::Active,
            Some(400) => Self::Inactive,
            Some(600) => Self::Closed,
            _ => Self::Invalid,
        }
    }

    pub fn value(&self) -> i32 {
        *self as i32
    }
}

/// Port of `org.apache.fineract.organisation.teller.domain.CashierTxnType`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashierTxnType {
    pub id: i32,
    pub value: &'static str,
}

impl CashierTxnType {
    pub const ALLOCATE: Self = Self {
        id: 101,
        value: "Allocate Cash",
    };
    pub const SETTLE: Self = Self {
        id: 102,
        value: "Settle Cash",
    };
    pub const INWARD_CASH_TXN: Self = Self {
        id: 103,
        value: "Cash In",
    };
    pub const OUTWARD_CASH_TXN: Self = Self {
        id: 104,
        value: "Cash Out",
    };

    pub fn get_cashier_txn_type(id: i32) -> Option<Self> {
        match id {
            101 => Some(Self::ALLOCATE),
            102 => Some(Self::SETTLE),
            103 => Some(Self::INWARD_CASH_TXN),
            104 => Some(Self::OUTWARD_CASH_TXN),
            _ => None,
        }
    }
}

/// Port of `org.apache.fineract.organisation.teller.util.DateRange`
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DateRange {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

impl DateRange {
    pub fn from_string(date_to_parse: Option<&str>, business_date: NaiveDate) -> Self {
        let Some(raw) = date_to_parse else {
            return Self {
                start_date: Some(business_date),
                end_date: None,
            };
        };

        let mut parts = raw.split("..").filter(|s| !s.is_empty());
        let start_date = parts
            .next()
            .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
        let end_date = parts
            .next()
            .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());

        Self {
            start_date,
            end_date,
        }
    }
}

/// Port of `org.apache.fineract.organisation.teller.domain.Teller`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Teller {
    pub id: Option<i64>,
    pub office_id: i64,
    pub debit_account_id: Option<i64>,
    pub credit_account_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: i32,
}

/// Port of `org.apache.fineract.organisation.teller.domain.Cashier`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cashier {
    pub id: Option<i64>,
    pub office_id: i64,
    pub staff_id: i64,
    pub teller_id: i64,
    pub description: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_full_day: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

impl Cashier {
    pub fn validate_within_teller_dates(&self, teller: &Teller) -> Result<(), BranchError> {
        if self.start_date < teller.start_date {
            return Err(BranchError::CashierDateRangeOutOfTellerDateRangeException);
        }
        if let Some(teller_end) = teller.end_date {
            if self.end_date > teller_end {
                return Err(BranchError::CashierDateRangeOutOfTellerDateRangeException);
            }
        }
        Ok(())
    }
}

/// Port of `org.apache.fineract.organisation.teller.domain.CashierTransaction`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashierTransaction {
    pub id: Option<i64>,
    pub cashier_id: i64,
    pub txn_type: i32,
    pub txn_amount: BigDec,
    pub txn_date: NaiveDate,
    pub txn_note: Option<String>,
    pub currency_code: Option<String>,
}

/// Port of `CashierTransactionsWithSummaryData`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashierTransactionsWithSummaryData {
    pub sum_cash_allocation: BigDec,
    pub sum_inward_cash: BigDec,
    pub sum_outward_cash: BigDec,
    pub sum_cash_settlement: BigDec,
    pub net_cash: BigDec,
    pub cashier_transactions: Vec<CashierTransaction>,
}

impl CashierTransactionsWithSummaryData {
    pub fn from_transactions(txns: Vec<CashierTransaction>) -> Self {
        let mut sum_alloc = BigDec::ZERO;
        let mut sum_in = BigDec::ZERO;
        let mut sum_out = BigDec::ZERO;
        let mut sum_settle = BigDec::ZERO;

        for t in &txns {
            match t.txn_type {
                101 => sum_alloc = sum_alloc + t.txn_amount,
                102 => sum_settle = sum_settle + t.txn_amount,
                103 => sum_in = sum_in + t.txn_amount,
                104 => sum_out = sum_out + t.txn_amount,
                _ => {}
            }
        }

        let net_cash = (sum_alloc + sum_in) - (sum_out + sum_settle);
        Self {
            sum_cash_allocation: sum_alloc,
            sum_inward_cash: sum_in,
            sum_outward_cash: sum_out,
            sum_cash_settlement: sum_settle,
            net_cash,
            cashier_transactions: txns,
        }
    }

    pub fn validate_settlement(&self, settle_amount: BigDec) -> Result<(), BranchError> {
        if settle_amount > self.net_cash {
            return Err(BranchError::CashierInsufficientAmountException);
        }
        Ok(())
    }
}

/// Port of `org.apache.fineract.organisation.teller.serialization.TellerCommandFromApiJsonDeserializer`
#[derive(Debug, Default, Clone)]
pub struct TellerCommandFromApiJsonDeserializer;

impl TellerCommandFromApiJsonDeserializer {
    pub fn supported_parameters() -> HashSet<&'static str> {
        HashSet::from([
            OFFICE_ID,
            NAME,
            DESCRIPTION,
            START_DATE,
            END_DATE,
            STATUS,
            DATE_FORMAT,
            LOCALE,
            IS_FULL_DAY,
            STAFF_ID,
            HOUR_START_TIME,
            MIN_START_TIME,
            HOUR_END_TIME,
            MIN_END_TIME,
            TXN_AMOUNT,
            TXN_DATE,
            TXN_NOTE,
            ENTITY_TYPE,
            ENTITY_ID,
            CURRENCY_CODE,
        ])
    }

    fn parse_and_check_supported(
        &self,
        json: Option<&str>,
    ) -> Result<serde_json::Map<String, Value>, BranchError> {
        let Some(raw) = json else {
            return Err(BranchError::InvalidJsonException);
        };
        if raw.trim().is_empty() {
            return Err(BranchError::InvalidJsonException);
        }
        let val: Value =
            serde_json::from_str(raw).map_err(|_| BranchError::InvalidJsonException)?;
        let Some(obj) = val.as_object() else {
            return Err(BranchError::InvalidJsonException);
        };

        let supported = Self::supported_parameters();
        for key in obj.keys() {
            if !supported.contains(key.as_str()) {
                return Err(BranchError::UnsupportedParameterException(key.clone()));
            }
        }

        Ok(obj.clone())
    }

    pub fn validate_for_cash_txn_for_cashier(&self, json: Option<&str>) -> Result<(), BranchError> {
        let obj = self.parse_and_check_supported(json)?;
        let mut errors = Vec::new();

        match obj.get(TXN_AMOUNT) {
            None | Some(Value::Null) => {
                errors.push(ApiParameterError {
                    parameter_name: TXN_AMOUNT.to_string(),
                    default_user_message: "The parameter `txnAmount` is mandatory.".to_string(),
                    user_message_globalisation_code:
                        "validation.msg.teller.txnAmount.cannot.be.blank".to_string(),
                });
            }
            _ => {}
        }

        match obj.get(TXN_DATE) {
            None | Some(Value::Null) => {
                errors.push(ApiParameterError {
                    parameter_name: TXN_DATE.to_string(),
                    default_user_message: "The parameter `txnDate` is mandatory.".to_string(),
                    user_message_globalisation_code:
                        "validation.msg.teller.txnDate.cannot.be.blank".to_string(),
                });
            }
            Some(Value::String(s)) if s.trim().is_empty() => {
                errors.push(ApiParameterError {
                    parameter_name: TXN_DATE.to_string(),
                    default_user_message: "The parameter `txnDate` is mandatory.".to_string(),
                    user_message_globalisation_code:
                        "validation.msg.teller.txnDate.cannot.be.blank".to_string(),
                });
            }
            _ => {}
        }

        if let Some(Value::String(note)) = obj.get(TXN_NOTE) {
            if note.chars().count() > 200 {
                errors.push(ApiParameterError {
                    parameter_name: TXN_NOTE.to_string(),
                    default_user_message:
                        "The parameter `txnNote` exceeds max length of 200.".to_string(),
                    user_message_globalisation_code:
                        "validation.msg.teller.txnNote.exceeds.max.length".to_string(),
                });
            }
        }

        if let Some(Value::String(cc)) = obj.get(CURRENCY_CODE) {
            if cc.chars().count() > 3 {
                errors.push(ApiParameterError {
                    parameter_name: CURRENCY_CODE.to_string(),
                    default_user_message:
                        "The parameter `currencyCode` exceeds max length of 3.".to_string(),
                    user_message_globalisation_code:
                        "validation.msg.teller.currencyCode.exceeds.max.length".to_string(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(BranchError::PlatformApiDataValidationException { errors });
        }

        Ok(())
    }

    pub fn validate_for_create_and_update_teller(
        &self,
        json: Option<&str>,
    ) -> Result<(), BranchError> {
        let obj = self.parse_and_check_supported(json)?;
        let mut errors = Vec::new();

        match obj.get(OFFICE_ID).and_then(|v| v.as_i64()) {
            Some(id) if id > 0 => {}
            _ => errors.push(ApiParameterError {
                parameter_name: OFFICE_ID.to_string(),
                default_user_message: "The parameter `officeId` is mandatory and must be > 0."
                    .to_string(),
                user_message_globalisation_code: "validation.msg.teller.officeId.invalid"
                    .to_string(),
            }),
        }

        match obj.get(NAME).and_then(|v| v.as_str()) {
            Some(name) if !name.trim().is_empty() && name.chars().count() <= 50 => {}
            _ => errors.push(ApiParameterError {
                parameter_name: NAME.to_string(),
                default_user_message: "The parameter `name` is invalid.".to_string(),
                user_message_globalisation_code: "validation.msg.teller.name.invalid".to_string(),
            }),
        }

        let start_date = obj
            .get(START_DATE)
            .and_then(|v| v.as_str())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let end_date = obj
            .get(END_DATE)
            .and_then(|v| v.as_str())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        if let (Some(sd), Some(ed)) = (start_date, end_date) {
            if ed < sd {
                return Err(BranchError::InvalidDateInputException {
                    start_date: sd.to_string(),
                    end_date: ed.to_string(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(BranchError::PlatformApiDataValidationException { errors });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cash_txn_json(
        txn_amount: Option<i64>,
        txn_date: Option<&str>,
        txn_note: Option<&str>,
        currency_code: Option<&str>,
    ) -> String {
        let mut map = std::collections::HashMap::new();
        map.insert("dateFormat", Value::String("dd MMMM yyyy".to_string()));
        map.insert("locale", Value::String("en".to_string()));
        if let Some(a) = txn_amount {
            map.insert("txnAmount", Value::Number(a.into()));
        }
        if let Some(d) = txn_date {
            map.insert("txnDate", Value::String(d.to_string()));
        }
        if let Some(n) = txn_note {
            map.insert("txnNote", Value::String(n.to_string()));
        }
        if let Some(c) = currency_code {
            map.insert("currencyCode", Value::String(c.to_string()));
        }
        serde_json::to_string_pretty(&map).unwrap()
    }

    fn assert_platform_validation_exception(
        expected_message: &str,
        expected_code: &str,
        res: Result<(), BranchError>,
    ) {
        match res {
            Err(BranchError::PlatformApiDataValidationException { errors }) => {
                assert_eq!(errors[0].default_user_message, expected_message);
                assert_eq!(errors[0].user_message_globalisation_code, expected_code);
            }
            other => panic!("Expected PlatformApiDataValidationException, got {other:?}"),
        }
    }

    #[test]
    fn test_cash_txn_valid_json_passes_validation() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        let json = cash_txn_json(
            Some(1000),
            Some("10 September 2022"),
            Some("Test note"),
            Some("USD"),
        );
        assert!(under_test.validate_for_cash_txn_for_cashier(Some(&json)).is_ok());
    }

    #[test]
    fn test_cash_txn_blank_or_null_json_throws_invalid_json_exception() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        assert_eq!(
            under_test.validate_for_cash_txn_for_cashier(None),
            Err(BranchError::InvalidJsonException)
        );
        assert_eq!(
            under_test.validate_for_cash_txn_for_cashier(Some("")),
            Err(BranchError::InvalidJsonException)
        );
        assert_eq!(
            under_test.validate_for_cash_txn_for_cashier(Some("   ")),
            Err(BranchError::InvalidJsonException)
        );
    }

    #[test]
    fn test_cash_txn_missing_txn_amount_throws_validation_exception() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        let json = cash_txn_json(None, Some("10 September 2022"), Some("Test note"), Some("USD"));
        assert_platform_validation_exception(
            "The parameter `txnAmount` is mandatory.",
            "validation.msg.teller.txnAmount.cannot.be.blank",
            under_test.validate_for_cash_txn_for_cashier(Some(&json)),
        );
    }

    #[test]
    fn test_cash_txn_missing_txn_date_throws_validation_exception() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        let json = cash_txn_json(Some(1000), None, Some("Test note"), Some("USD"));
        assert_platform_validation_exception(
            "The parameter `txnDate` is mandatory.",
            "validation.msg.teller.txnDate.cannot.be.blank",
            under_test.validate_for_cash_txn_for_cashier(Some(&json)),
        );
    }

    #[test]
    fn test_cash_txn_txn_note_exceeding_max_length_throws_validation_exception() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        let long_note = "A".repeat(201);
        let json = cash_txn_json(
            Some(1000),
            Some("10 September 2022"),
            Some(&long_note),
            Some("USD"),
        );
        assert_platform_validation_exception(
            "The parameter `txnNote` exceeds max length of 200.",
            "validation.msg.teller.txnNote.exceeds.max.length",
            under_test.validate_for_cash_txn_for_cashier(Some(&json)),
        );
    }

    #[test]
    fn test_cash_txn_currency_code_exceeding_max_length_throws_validation_exception() {
        let under_test = TellerCommandFromApiJsonDeserializer;
        let json = cash_txn_json(
            Some(1000),
            Some("10 September 2022"),
            Some("Test note"),
            Some("ABCD"),
        );
        assert_platform_validation_exception(
            "The parameter `currencyCode` exceeds max length of 3.",
            "validation.msg.teller.currencyCode.exceeds.max.length",
            under_test.validate_for_cash_txn_for_cashier(Some(&json)),
        );
    }

    #[test]
    fn test_date_range_and_cashier_settlement_summary() {
        let bdate = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let range = DateRange::from_string(Some("2026-01-01..2026-12-31"), bdate);
        assert_eq!(range.start_date, NaiveDate::from_ymd_opt(2026, 1, 1));
        assert_eq!(range.end_date, NaiveDate::from_ymd_opt(2026, 12, 31));

        let summary = CashierTransactionsWithSummaryData::from_transactions(vec![
            CashierTransaction {
                id: Some(1),
                cashier_id: 10,
                txn_type: CashierTxnType::ALLOCATE.id,
                txn_amount: BigDec::parse_lossy("1000.00"),
                txn_date: bdate,
                txn_note: None,
                currency_code: Some("USD".to_string()),
            },
            CashierTransaction {
                id: Some(2),
                cashier_id: 10,
                txn_type: CashierTxnType::INWARD_CASH_TXN.id,
                txn_amount: BigDec::parse_lossy("250.00"),
                txn_date: bdate,
                txn_note: None,
                currency_code: Some("USD".to_string()),
            },
            CashierTransaction {
                id: Some(3),
                cashier_id: 10,
                txn_type: CashierTxnType::OUTWARD_CASH_TXN.id,
                txn_amount: BigDec::parse_lossy("400.00"),
                txn_date: bdate,
                txn_note: None,
                currency_code: Some("USD".to_string()),
            },
        ]);

        // Net cash = 1000 + 250 - 400 = 850
        assert_eq!(summary.net_cash, BigDec::parse_lossy("850.00"));
        assert!(summary.validate_settlement(BigDec::parse_lossy("850.00")).is_ok());
        assert_eq!(
            summary.validate_settlement(BigDec::parse_lossy("850.01")),
            Err(BranchError::CashierInsufficientAmountException)
        );
    }
}
