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

//! Rust migration of `fineract-progressive-loan-embeddable-schedule-generator/misc/Main.java`.

use chrono::NaiveDate;
use fineract_schedule_generator::{
    BigDec, CurrencyData, DaysInMonthType, DaysInYearType,
    EmbeddableProgressiveLoanScheduleGenerator, InterestMethod, LoanRepaymentScheduleModelData,
    LoanSchedulePlan, LoanSchedulePlanPeriod, MathContext, RoundingMode,
};
use std::env;

fn main() {
    let mc = MathContext::new(12, RoundingMode::HalfUp);
    let calculator = EmbeddableProgressiveLoanScheduleGenerator::new();

    let currency = CurrencyData::new("usd", "US Dollar", 2, None, "usd", "$");
    let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let disbursement_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let disbursed_amount = BigDec::from_i64(100);

    let args: Vec<String> = env::args().collect();
    let down_payment_percentage = if args.len() > 1 {
        BigDec::parse(&args[1]).unwrap_or(BigDec::ZERO)
    } else {
        BigDec::ZERO
    };
    let is_down_payment_enabled = !down_payment_percentage.is_zero();

    let config = LoanRepaymentScheduleModelData {
        schedule_generation_start_date: start_date,
        currency,
        disbursement_amount: disbursed_amount,
        disbursement_date,
        number_of_repayments: 6,
        repayment_frequency: 1,
        repayment_frequency_type: "MONTHS".to_string(),
        annual_nominal_interest_rate: BigDec::parse("7.0").unwrap(),
        down_payment_enabled: is_down_payment_enabled,
        days_in_month: DaysInMonthType::Days30,
        days_in_year: DaysInYearType::Days360,
        down_payment_percentage,
        installment_amount_in_multiples_of: None,
        fixed_length: None,
        interest_recognition_on_disbursement_date: false,
        days_in_year_custom_strategy: None,
        interest_method: InterestMethod::DecliningBalance,
        allow_partial_period_interest_calculation: true,
        allow_full_term_for_tranche: false,
    };

    let plan = calculator.generate(mc, &config);
    print_plan(&plan);
}

fn print_plan(plan: &LoanSchedulePlan) {
    let non_disbursement_count = plan
        .periods
        .iter()
        .filter(|p| !matches!(p, LoanSchedulePlanPeriod::Disbursement(_)))
        .count();

    println!("#------ Loan Schedule -----------------#");
    println!("  Number of Periods: {}", non_disbursement_count);
    println!("  Loan Term in Days: {}", plan.loan_term_in_days);
    println!("  Total Disbursed Amount: {}", plan.total_disbursed_amount);
    println!("  Total Interest Amount: {}", plan.total_interest_amount);
    println!("  Total Repayment Amount: {}", plan.total_repayment_amount);
    println!("#------ Repayment Schedule ------------#");

    for period in &plan.periods {
        match period {
            LoanSchedulePlanPeriod::Disbursement(dp) => {
                println!(
                    "  Disbursement - Date: {}, Amount: {}",
                    dp.period_due_date, dp.principal_amount
                );
            }
            LoanSchedulePlanPeriod::DownPayment(rp) => {
                println!(
                    "  Down payment Period: #{}, Due Date: {}, Balance: {}, Principal: {}, Total: {}, Total Outstanding Balance: {}",
                    rp.period_number,
                    rp.period_due_date,
                    rp.outstanding_loan_balance,
                    rp.principal_amount,
                    rp.total_due_amount,
                    rp.total_outstanding_loan_balance
                );
            }
            LoanSchedulePlanPeriod::Repayment(rp) => {
                println!(
                    "  Repayment Period: #{}, Due Date: {}, Balance: {}, Principal: {}, Interest: {}, Total: {}, Total Outstanding Balance: {}",
                    rp.period_number,
                    rp.period_due_date,
                    rp.outstanding_loan_balance,
                    rp.principal_amount,
                    rp.interest_amount,
                    rp.total_due_amount,
                    rp.total_outstanding_loan_balance
                );
            }
        }
    }
}
