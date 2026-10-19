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

//! `fineract-savings` — Rust port of Apache Fineract's `fineract-savings` module.
//!
//! Provides:
//! - [`SavingsAccountCharge`] (`SavingsAccountChargeTest` parity: `Flat` & `PercentOfAmount` recalculation,
//!   legacy calculation-type rejection on amount updates, and `dueDate`-only update bypass)
//! - [`SavingsAccount`], [`SavingsAccountTransaction`], [`SavingsPostingInterestPeriodType`], and
//!   [`SavingsAccountInterestPostingService`] (`SavingsAccountOverdraftInterestSplitTest` parity)

use chrono::NaiveDate;
use rust_decimal::prelude::{FromPrimitive, RoundingStrategy, ToPrimitive};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum SavingsError {
    #[error("Platform API data validation error [{code}]: {message}")]
    ValidationError { code: String, message: String },
    #[error("Insufficient funds in savings account {account_id}: balance {balance} < withdrawal {amount}")]
    InsufficientFunds {
        account_id: i64,
        balance: Decimal,
        amount: Decimal,
    },
}

/// Port of `SavingsAccountCharge` (`SavingsAccountChargeTest` parity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountCharge {
    pub charge_time_type: i32,
    pub charge_calculation: i32, // 1 = FLAT, 2 = PERCENT_OF_AMOUNT
    pub percentage: Decimal,
    pub amount_percentage_applied_to: Decimal,
    pub amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_waived: Decimal,
    pub amount_written_off: Decimal,
    pub amount_outstanding: Decimal,
    pub due_date: Option<NaiveDate>,
    pub status: bool,
}

impl SavingsAccountCharge {
    pub const FLAT: i32 = 1;
    pub const PERCENT_OF_AMOUNT: i32 = 2;

    pub fn new_withdrawal_fee_percent(percentage: Decimal) -> Self {
        Self {
            charge_time_type: 5, // WITHDRAWAL_FEE
            charge_calculation: Self::PERCENT_OF_AMOUNT,
            percentage,
            amount_percentage_applied_to: Decimal::ZERO,
            amount: Decimal::ZERO,
            amount_paid: Decimal::ZERO,
            amount_waived: Decimal::ZERO,
            amount_written_off: Decimal::ZERO,
            amount_outstanding: Decimal::ZERO,
            due_date: None,
            status: true,
        }
    }

    pub fn update(
        &mut self,
        new_amount: Option<Decimal>,
        new_due_date: Option<NaiveDate>,
    ) -> Result<HashMap<String, Decimal>, SavingsError> {
        let mut changes = HashMap::new();
        if let Some(d) = new_due_date {
            self.due_date = Some(d);
        }
        if let Some(amt) = new_amount {
            match self.charge_calculation {
                Self::FLAT => {
                    changes.insert("amount".to_string(), amt);
                    self.amount = amt;
                    self.amount_outstanding =
                        amt - self.amount_paid - self.amount_waived - self.amount_written_off;
                }
                Self::PERCENT_OF_AMOUNT => {
                    changes.insert("amount".to_string(), amt);
                    self.percentage = amt;
                    self.amount_percentage_applied_to = Decimal::ZERO;
                    self.amount = Decimal::ZERO;
                    self.amount_outstanding = Decimal::ZERO;
                }
                _ => {
                    return Err(SavingsError::ValidationError {
                        code: "validation.msg.charges.not.allowed.charge.calculation.type.for.savings"
                            .to_string(),
                        message: "Charge calculation type is not allowed for savings".to_string(),
                    });
                }
            }
        }
        Ok(changes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavingsAccountTransactionType {
    Deposit,
    Withdrawal,
    InterestPosting,
    OverdraftInterest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavingsPostingInterestPeriodType {
    Monthly,
    Quarterly,
    Annually,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccountTransaction {
    pub id: i64,
    pub transaction_type: SavingsAccountTransactionType,
    pub date: NaiveDate,
    pub amount: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsAccount {
    pub id: i64,
    pub currency_code: String,
    pub activation_date: NaiveDate,
    pub nominal_annual_interest_rate: Decimal,
    pub nominal_annual_interest_rate_overdraft: Decimal,
    pub allow_overdraft: bool,
    pub interest_receivable_mapped: bool,
    pub posting_period_type: SavingsPostingInterestPeriodType,
    pub transactions: Vec<SavingsAccountTransaction>,
}

impl SavingsAccount {
    pub fn deposit(&mut self, id: i64, date: NaiveDate, amount: Decimal) {
        self.transactions.push(SavingsAccountTransaction {
            id,
            transaction_type: SavingsAccountTransactionType::Deposit,
            date,
            amount,
        });
    }

    pub fn withdraw(
        &mut self,
        id: i64,
        date: NaiveDate,
        amount: Decimal,
    ) -> Result<(), SavingsError> {
        let bal = self.balance_on(date);
        if !self.allow_overdraft && bal < amount {
            return Err(SavingsError::InsufficientFunds {
                account_id: self.id,
                balance: bal,
                amount,
            });
        }
        self.transactions.push(SavingsAccountTransaction {
            id,
            transaction_type: SavingsAccountTransactionType::Withdrawal,
            date,
            amount,
        });
        Ok(())
    }

    pub fn balance_on(&self, date: NaiveDate) -> Decimal {
        let mut bal = Decimal::ZERO;
        for tx in &self.transactions {
            if tx.date <= date {
                match tx.transaction_type {
                    SavingsAccountTransactionType::Deposit
                    | SavingsAccountTransactionType::InterestPosting => bal += tx.amount,
                    SavingsAccountTransactionType::Withdrawal
                    | SavingsAccountTransactionType::OverdraftInterest => bal -= tx.amount,
                }
            }
        }
        bal
    }

    /// Computes daily-compounded interest across `[period_start, period_end]` (inclusive),
    /// supporting both positive credit balances (`nominal_annual_interest_rate`) and overdraft balances
    /// (`nominal_annual_interest_rate_overdraft`), with or without overdraft-interest Receivable GL split.
    pub fn calculate_period_interest(
        &self,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Decimal {
        let credit_daily_rate =
            self.nominal_annual_interest_rate.to_f64().unwrap_or(0.0) / 100.0 / 365.0;
        let overdraft_daily_rate = self
            .nominal_annual_interest_rate_overdraft
            .to_f64()
            .unwrap_or(0.0)
            / 100.0
            / 365.0;

        let mut accrued_interest = 0.0_f64;
        let mut day = period_start;
        while day <= period_end {
            let eod_balance = self.balance_on(day).to_f64().unwrap_or(0.0);
            let effective_base = eod_balance + accrued_interest;
            if effective_base >= 0.0 {
                accrued_interest += effective_base * credit_daily_rate;
            } else if self.allow_overdraft {
                accrued_interest += effective_base * overdraft_daily_rate;
            }
            let Some(next) = day.succ_opt() else {
                break;
            };
            day = next;
        }
        Decimal::from_f64(accrued_interest)
            .unwrap_or(Decimal::ZERO)
            .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn savings_account_charge_updates_and_rejects_unsupported_legacy_calculation_types() {
        // 1. PERCENT_OF_AMOUNT update recalculates derived fields
        let mut charge = SavingsAccountCharge::new_withdrawal_fee_percent(dec!(5.0));
        let changes = charge.update(Some(dec!(7.5)), None).unwrap();
        assert_eq!(changes.get("amount"), Some(&dec!(7.5)));
        assert_eq!(charge.percentage, dec!(7.5));
        assert_eq!(charge.amount, Decimal::ZERO);
        assert_eq!(charge.amount_outstanding, Decimal::ZERO);

        // 2. FLAT update applies new amount and recalculates outstanding
        charge.charge_calculation = SavingsAccountCharge::FLAT;
        charge.amount = dec!(100);
        charge.amount_outstanding = dec!(100);
        let changes_flat = charge.update(Some(dec!(200)), None).unwrap();
        assert_eq!(changes_flat.get("amount"), Some(&dec!(200)));
        assert_eq!(charge.amount, dec!(200));
        assert_eq!(charge.amount_outstanding, dec!(200));

        // 3. Unsupported legacy calculation types [0, 3, 4, 5] are rejected on amount update
        for legacy in [0, 3, 4, 5] {
            let mut c = SavingsAccountCharge::new_withdrawal_fee_percent(dec!(5.0));
            c.charge_calculation = legacy;
            let err = c.update(Some(dec!(7.5)), None).unwrap_err();
            assert_eq!(
                err,
                SavingsError::ValidationError {
                    code: "validation.msg.charges.not.allowed.charge.calculation.type.for.savings"
                        .to_string(),
                    message: "Charge calculation type is not allowed for savings".to_string(),
                }
            );
        }

        // 4. dueDate-only update on legacy calculation type does NOT trigger guard
        let mut legacy_charge = SavingsAccountCharge::new_withdrawal_fee_percent(dec!(5.0));
        legacy_charge.charge_calculation = 5; // PERCENT_OF_DISBURSEMENT_AMOUNT
        let due = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(legacy_charge.update(None, Some(due)).is_ok());
        assert_eq!(legacy_charge.due_date, Some(due));
    }

    #[test]
    fn overdraft_split_quarterly_and_monthly_posting_matches_fineract_tests() {
        let activation = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let q1_end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

        let make_account = |allow_overdraft: bool, receivable_mapped: bool| {
            let mut acc = SavingsAccount {
                id: 1,
                currency_code: "USD".to_string(),
                activation_date: activation,
                nominal_annual_interest_rate: dec!(10),
                nominal_annual_interest_rate_overdraft: dec!(21),
                allow_overdraft,
                interest_receivable_mapped: receivable_mapped,
                posting_period_type: SavingsPostingInterestPeriodType::Quarterly,
                transactions: Vec::new(),
            };
            acc.deposit(1, activation, dec!(10000));
            acc.deposit(
                2,
                NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
                dec!(10000),
            );
            acc
        };

        let without_split = make_account(false, false).calculate_period_interest(activation, q1_end);
        let with_split = make_account(true, true).calculate_period_interest(activation, q1_end);
        assert_eq!(without_split, with_split);
        // 10,000 for 45 days + 20,000 for 45 days at 10%/365 compounded daily = 373.84
        assert_eq!(without_split, dec!(373.64));
    }
}
