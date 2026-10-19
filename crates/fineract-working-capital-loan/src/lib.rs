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

//! `fineract-working-capital-loan` — Rust port of Apache Fineract's `fineract-working-capital-loan` module.
//!
//! Provides [`ProjectedAmortizationScheduleModel`] and [`ProjectedAmortizationInstallment`] with full
//! EIR (Effective Interest Rate) IRR solving and daily/periodic discount fee amortization walk,
//! matching `ProjectedAmortizationScheduleCalculatorTest`.

use chrono::{Duration, NaiveDate};
use rust_decimal::prelude::{FromPrimitive, RoundingStrategy, ToPrimitive};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkingCapitalAmortizationType {
    Eir,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkingCapitalLoanPeriodFrequencyType {
    Days,
    Weeks,
    Months,
    Years,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedAmortizationInstallment {
    pub installment_number: usize,
    pub period_number: usize,
    pub date: NaiveDate,
    pub day_count: i64,
    pub expected_payment: Decimal,
    pub discount_factor: Decimal,
    pub npv_value: Decimal,
    pub balance: Decimal,
    pub expected_amortization: Option<Decimal>,
    pub deferred_balance: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedAmortizationScheduleModel {
    pub amortization_type: WorkingCapitalAmortizationType,
    pub discount_fee_amount: Decimal,
    pub net_disbursement_amount: Decimal,
    pub total_payment_volume: Decimal,
    pub period_payment_rate: Decimal,
    pub npv_day_count: i32,
    pub repayment_frequency_type: WorkingCapitalLoanPeriodFrequencyType,
    pub repayment_every: i32,
    pub expected_disbursement_date: NaiveDate,
    pub expected_payment_amount: Decimal,
    pub final_payment_amount: Decimal,
    pub original_payment_number: usize,
    pub effective_interest_rate: Decimal,
    pub projected_payments: Vec<ProjectedAmortizationInstallment>,
}

impl ProjectedAmortizationScheduleModel {
    pub fn generate(
        amortization_type: WorkingCapitalAmortizationType,
        discount_fee_amount: Decimal,
        net_disbursement_amount: Decimal,
        total_payment_volume: Decimal,
        period_payment_rate: Decimal,
        npv_day_count: i32,
        repayment_frequency_type: WorkingCapitalLoanPeriodFrequencyType,
        repayment_every: i32,
        disbursement_date: NaiveDate,
    ) -> Self {
        let total_obligation = net_disbursement_amount + discount_fee_amount;
        let expected_payment_amount = ((total_payment_volume
            * period_payment_rate
            * Decimal::from(repayment_every))
            / Decimal::from(npv_day_count)
            / Decimal::from(100))
        .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);

        let raw_periods = (total_obligation / expected_payment_amount)
            .ceil()
            .to_usize()
            .unwrap_or(1);
        let original_payment_number = raw_periods.max(1);
        let prior_payments =
            expected_payment_amount * Decimal::from((original_payment_number - 1) as u64);
        let final_payment_amount = total_obligation - prior_payments;

        // Solve periodic IRR r such that sum_{t=1..n} P_t / (1 + r)^t = net_disbursement_amount
        let net_f64 = net_disbursement_amount.to_f64().unwrap_or(0.0);
        let exp_f64 = expected_payment_amount.to_f64().unwrap_or(0.0);
        let fin_f64 = final_payment_amount.to_f64().unwrap_or(0.0);

        let r = Self::solve_periodic_irr(net_f64, exp_f64, fin_f64, original_payment_number);
        let effective_interest_rate =
            Decimal::from_f64(r).unwrap_or(Decimal::ZERO).round_dp(12);

        let mut projected_payments = Vec::with_capacity(original_payment_number + 1);
        projected_payments.push(ProjectedAmortizationInstallment {
            installment_number: 0,
            period_number: 0,
            date: disbursement_date,
            day_count: 0,
            expected_payment: -net_disbursement_amount,
            discount_factor: Decimal::ONE.round_dp(8),
            npv_value: -net_disbursement_amount,
            balance: net_disbursement_amount,
            expected_amortization: None,
            deferred_balance: discount_fee_amount,
        });

        let mut running_balance = net_disbursement_amount;
        let mut running_deferred = discount_fee_amount;

        for t in 1..=original_payment_number {
            let pmt = if t == original_payment_number {
                final_payment_amount
            } else {
                expected_payment_amount
            };
            let date = disbursement_date + Duration::days((t as i64) * (repayment_every as i64));
            let df_f64 = (1.0 + r).powi(-(t as i32));
            let discount_factor = Decimal::from_f64(df_f64)
                .unwrap_or(Decimal::ONE)
                .round_dp_with_strategy(8, RoundingStrategy::MidpointNearestEven);
            let npv_value = (pmt * discount_factor)
                .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);

            let (amortization, next_balance, next_deferred) = if t == original_payment_number {
                let amort = running_deferred;
                (amort, Decimal::ZERO, Decimal::ZERO)
            } else {
                let raw_amort = Decimal::from_f64(running_balance.to_f64().unwrap_or(0.0) * r)
                    .unwrap_or(Decimal::ZERO)
                    .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
                let amort = raw_amort.min(running_deferred);
                let nb = (running_balance + amort - pmt)
                    .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
                let nd = (running_deferred - amort)
                    .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
                (amort, nb, nd)
            };

            projected_payments.push(ProjectedAmortizationInstallment {
                installment_number: t,
                period_number: t,
                date,
                day_count: t as i64,
                expected_payment: pmt,
                discount_factor,
                npv_value,
                balance: next_balance,
                expected_amortization: Some(amortization),
                deferred_balance: next_deferred,
            });

            running_balance = next_balance;
            running_deferred = next_deferred;
        }

        Self {
            amortization_type,
            discount_fee_amount,
            net_disbursement_amount,
            total_payment_volume,
            period_payment_rate,
            npv_day_count,
            repayment_frequency_type,
            repayment_every,
            expected_disbursement_date: disbursement_date,
            expected_payment_amount,
            final_payment_amount,
            original_payment_number,
            effective_interest_rate,
            projected_payments,
        }
    }

    pub fn regenerate(
        &self,
        discount_fee_amount: Decimal,
        net_disbursement_amount: Decimal,
        disbursement_date: NaiveDate,
    ) -> Self {
        Self::generate(
            self.amortization_type,
            discount_fee_amount,
            net_disbursement_amount,
            self.total_payment_volume,
            self.period_payment_rate,
            self.npv_day_count,
            self.repayment_frequency_type,
            self.repayment_every,
            disbursement_date,
        )
    }

    fn solve_periodic_irr(net: f64, regular_pmt: f64, final_pmt: f64, n: usize) -> f64 {
        if net <= 0.0 || n == 0 {
            return 0.0;
        }
        let mut low = 0.0_f64;
        let mut high = 1.0_f64;
        for _ in 0..100 {
            let mid = 0.5 * (low + high);
            let mut npv = 0.0_f64;
            for t in 1..n {
                npv += regular_pmt / (1.0 + mid).powi(t as i32);
            }
            npv += final_pmt / (1.0 + mid).powi(n as i32);
            if npv > net {
                low = mid;
            } else {
                high = mid;
            }
        }
        0.5 * (low + high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_add_disbursement_term10_discount_fee50_net_disbursement450_then_430() {
        let d_init = NaiveDate::from_ymd_opt(2019, 1, 1).unwrap();
        let model1 = ProjectedAmortizationScheduleModel::generate(
            WorkingCapitalAmortizationType::Eir,
            dec!(50),
            dec!(450),
            dec!(100000),
            dec!(18),
            360,
            WorkingCapitalLoanPeriodFrequencyType::Days,
            1,
            d_init,
        );

        assert_eq!(model1.original_payment_number, 10);
        assert_eq!(model1.projected_payments.len(), 11);
        assert_eq!(model1.projected_payments[0].expected_payment, dec!(-450.00));
        assert_eq!(model1.projected_payments[1].expected_payment, dec!(50.00));
        assert_eq!(
            model1.projected_payments[1].expected_amortization,
            Some(dec!(8.83))
        );
        assert_eq!(model1.projected_payments[1].balance, dec!(408.83));
        assert_eq!(model1.projected_payments[1].deferred_balance, dec!(41.17));
        assert_eq!(model1.projected_payments[10].balance, dec!(0.00));
        assert_eq!(model1.projected_payments[10].deferred_balance, dec!(0.00));

        // Regenerate with netDisbursement = 430 on 2019-01-05
        let d_new = NaiveDate::from_ymd_opt(2019, 1, 5).unwrap();
        let model2 = model1.regenerate(dec!(50), dec!(430), d_new);
        assert_eq!(model2.original_payment_number, 10);
        assert_eq!(model2.projected_payments.len(), 11);
        assert_eq!(model2.projected_payments[1].balance, dec!(389.14));
        assert_eq!(
            model2.projected_payments[1].expected_amortization,
            Some(dec!(9.14))
        );
        assert_eq!(model2.projected_payments[10].expected_payment, dec!(30.00));
        assert_eq!(model2.projected_payments[10].balance, dec!(0.00));
    }

    #[test]
    fn test_projected_schedule_term200_discount_fee1000_net_disbursement9000() {
        let d_init = NaiveDate::from_ymd_opt(2019, 1, 1).unwrap();
        let model = ProjectedAmortizationScheduleModel::generate(
            WorkingCapitalAmortizationType::Eir,
            dec!(1000),
            dec!(9000),
            dec!(100000),
            dec!(18),
            360,
            WorkingCapitalLoanPeriodFrequencyType::Days,
            1,
            d_init,
        );
        assert_eq!(model.original_payment_number, 200);
        assert_eq!(model.projected_payments.len(), 201);
        assert_eq!(model.projected_payments[1].balance, dec!(8959.61));
        assert_eq!(
            model.projected_payments[1].expected_amortization,
            Some(dec!(9.61))
        );
        assert_eq!(model.projected_payments[1].deferred_balance, dec!(990.39));
        assert_eq!(model.projected_payments[200].balance, dec!(0.00));
        assert_eq!(model.projected_payments[200].deferred_balance, dec!(0.00));
    }
}
