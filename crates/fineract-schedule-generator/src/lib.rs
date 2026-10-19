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

//! Rust migration of `fineract-progressive-loan-embeddable-schedule-generator`
//! and the core progressive loan EMI & schedule calculation engine (`ProgressiveEMICalculator`,
//! `ProgressiveLoanScheduleGenerator`, `DefaultScheduledDateGenerator`, and `LoanSchedulePlan`).

use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

// ============================================================================
// 1. Java-Compatible Arbitrary-Scale Decimal & MathContext (`BigDec`, `Money`)
// ============================================================================

/// Rounding modes matching `java.math.RoundingMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundingMode {
    HalfUp,
    HalfEven,
    HalfDown,
    Down,
    Up,
    Floor,
    Ceiling,
}

/// Precision and rounding settings matching `java.math.MathContext`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MathContext {
    pub precision: u32,
    pub rounding_mode: RoundingMode,
}

impl MathContext {
    pub const fn new(precision: u32, rounding_mode: RoundingMode) -> Self {
        Self {
            precision,
            rounding_mode,
        }
    }
}

impl Default for MathContext {
    fn default() -> Self {
        Self {
            precision: 12,
            rounding_mode: RoundingMode::HalfUp,
        }
    }
}

/// Exact base-10 decimal (`unscaled * 10^-scale`) with `java.math.BigDecimal` semantics.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BigDec {
    unscaled: i128,
    scale: i32,
}

impl BigDec {
    pub const ZERO: Self = Self {
        unscaled: 0,
        scale: 0,
    };
    pub const ONE: Self = Self {
        unscaled: 1,
        scale: 0,
    };
    pub const HUNDRED: Self = Self {
        unscaled: 100,
        scale: 0,
    };

    pub const fn from_unscaled(unscaled: i128, scale: i32) -> Self {
        Self { unscaled, scale }
    }

    pub const fn from_i64(val: i64) -> Self {
        Self {
            unscaled: val as i128,
            scale: 0,
        }
    }

    pub fn from_f64(val: f64) -> Self {
        let s = format!("{}", val);
        Self::parse(&s).unwrap_or(Self::ZERO)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let (sign, rest) = if let Some(r) = s.strip_prefix('-') {
            (-1i128, r)
        } else if let Some(r) = s.strip_prefix('+') {
            (1i128, r)
        } else {
            (1i128, s)
        };
        if let Some((int_part, frac_part)) = rest.split_once('.') {
            let scale = frac_part.len() as i32;
            let combined = format!("{}{}", int_part, frac_part);
            let mag: i128 = combined.parse().ok()?;
            Some(Self {
                unscaled: sign * mag,
                scale,
            })
        } else {
            let mag: i128 = rest.parse().ok()?;
            Some(Self {
                unscaled: sign * mag,
                scale: 0,
            })
        }
    }

    pub fn is_zero(&self) -> bool {
        self.unscaled == 0
    }

    pub fn is_positive(&self) -> bool {
        self.unscaled > 0
    }

    pub fn is_negative(&self) -> bool {
        self.unscaled < 0
    }

    pub fn abs(self) -> Self {
        Self {
            unscaled: self.unscaled.abs(),
            scale: self.scale,
        }
    }

    pub fn negated(self) -> Self {
        Self {
            unscaled: -self.unscaled,
            scale: self.scale,
        }
    }

    pub fn negative_to_zero(self) -> Self {
        if self.unscaled < 0 {
            Self::from_unscaled(0, self.scale.max(0))
        } else {
            self
        }
    }

    pub fn strip_trailing_zeros(self) -> Self {
        if self.unscaled == 0 {
            return Self::ZERO;
        }
        let mut u = self.unscaled;
        let mut s = self.scale;
        while s > 0 && u % 10 == 0 {
            u /= 10;
            s -= 1;
        }
        Self {
            unscaled: u,
            scale: s,
        }
    }

    pub fn to_f64(self) -> f64 {
        if self.scale >= 0 {
            (self.unscaled as f64) / 10f64.powi(self.scale)
        } else {
            (self.unscaled as f64) * 10f64.powi(-self.scale)
        }
    }

    pub fn add(self, rhs: Self, mc: Option<MathContext>) -> Self {
        let target_scale = self.scale.max(rhs.scale);
        let lhs_u = scale_up(self.unscaled, target_scale - self.scale);
        let rhs_u = scale_up(rhs.unscaled, target_scale - rhs.scale);
        let res = Self {
            unscaled: lhs_u + rhs_u,
            scale: target_scale,
        };
        match mc {
            Some(m) => res.round_mc(m),
            None => res,
        }
    }

    pub fn sub(self, rhs: Self, mc: Option<MathContext>) -> Self {
        self.add(rhs.negated(), mc)
    }

    pub fn mul(self, rhs: Self, mc: Option<MathContext>) -> Self {
        let a = self.strip_trailing_zeros_keep_nonneg();
        let b = rhs.strip_trailing_zeros_keep_nonneg();
        let res = Self {
            unscaled: a.unscaled * b.unscaled,
            scale: a.scale + b.scale,
        };
        match mc {
            Some(m) => res.round_mc(m),
            None => res,
        }
    }

    pub fn div(self, rhs: Self, mc: MathContext) -> Self {
        assert!(rhs.unscaled != 0, "Division by zero");
        if self.unscaled == 0 {
            return Self::ZERO;
        }
        let a = self.strip_trailing_zeros();
        let b = rhs.strip_trailing_zeros();
        let a_digits = num_digits(a.unscaled);
        let b_digits = num_digits(b.unscaled);
        // Ensure quotient has at least mc.precision + 4 digits before rounding
        let extra_scale = (mc.precision as i32 + 4 + b_digits as i32 - a_digits as i32).max(0);
        let scaled_dividend = scale_up(a.unscaled, extra_scale);
        let quot = scaled_dividend / b.unscaled;
        let rem = scaled_dividend % b.unscaled;
        let raw_scale = a.scale + extra_scale - b.scale;

        // First strip exact trailing zeros if remainder is 0
        if rem == 0 {
            let exact = Self {
                unscaled: quot,
                scale: raw_scale,
            }
            .strip_trailing_zeros_to_min_scale((a.scale - b.scale).max(0));
            return exact.round_mc(mc);
        }

        // Round to mc.precision significant digits using exact remainder
        round_to_precision_with_rem(quot, rem, b.unscaled, raw_scale, mc)
    }

    pub fn set_scale(self, new_scale: i32, mode: RoundingMode) -> Self {
        if new_scale == self.scale {
            return self;
        }
        if new_scale > self.scale {
            return Self {
                unscaled: scale_up(self.unscaled, new_scale - self.scale),
                scale: new_scale,
            };
        }
        let diff = (self.scale - new_scale) as u32;
        let divisor = 10i128.pow(diff);
        let quot = self.unscaled / divisor;
        let rem = self.unscaled % divisor;
        let rounded = apply_rounding(quot, rem, divisor, mode);
        Self {
            unscaled: rounded,
            scale: new_scale,
        }
    }

    pub fn round_mc(self, mc: MathContext) -> Self {
        if mc.precision == 0 || self.unscaled == 0 {
            return self;
        }
        let digits = num_digits(self.unscaled);
        if digits <= mc.precision {
            return self;
        }
        let drop = digits - mc.precision;
        let divisor = 10i128.pow(drop);
        let quot = self.unscaled / divisor;
        let rem = self.unscaled % divisor;
        let mut rounded = apply_rounding(quot, rem, divisor, mc.rounding_mode);
        let mut new_scale = self.scale - (drop as i32);
        if num_digits(rounded) > mc.precision {
            rounded /= 10;
            new_scale -= 1;
        }
        Self {
            unscaled: rounded,
            scale: new_scale,
        }
    }

    fn strip_trailing_zeros_keep_nonneg(self) -> Self {
        let mut u = self.unscaled;
        let mut s = self.scale;
        while s > 12 && u != 0 && u % 10 == 0 {
            u /= 10;
            s -= 1;
        }
        Self {
            unscaled: u,
            scale: s,
        }
    }

    fn strip_trailing_zeros_to_min_scale(self, min_scale: i32) -> Self {
        let mut u = self.unscaled;
        let mut s = self.scale;
        while s > min_scale && u != 0 && u % 10 == 0 {
            u /= 10;
            s -= 1;
        }
        Self {
            unscaled: u,
            scale: s,
        }
    }
}

fn scale_up(val: i128, exp: i32) -> i128 {
    if exp <= 0 || val == 0 {
        val
    } else {
        val * 10i128.pow(exp as u32)
    }
}

fn num_digits(val: i128) -> u32 {
    let mut v = val.unsigned_abs();
    if v == 0 {
        return 1;
    }
    let mut count = 0;
    while v > 0 {
        v /= 10;
        count += 1;
    }
    count
}

fn round_to_precision_with_rem(
    quot: i128,
    rem: i128,
    divisor_orig: i128,
    scale: i32,
    mc: MathContext,
) -> BigDec {
    let digits = num_digits(quot);
    if digits <= mc.precision {
        let rounded = apply_rounding(quot, rem, divisor_orig, mc.rounding_mode);
        return BigDec::from_unscaled(rounded, scale);
    }
    let drop = digits - mc.precision;
    let factor = 10i128.pow(drop);
    let final_quot = quot / factor;
    let dropped_part = quot % factor;
    // Combine dropped_part and rem into a single remainder check against factor * |divisor_orig|
    let sign = if quot < 0 || (quot == 0 && (rem < 0) != (divisor_orig < 0)) {
        -1i128
    } else {
        1i128
    };
    let abs_dropped = dropped_part.abs();
    let half_factor = factor / 2;
    let cmp_half = match abs_dropped.cmp(&half_factor) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => {
            if rem != 0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
    };
    let has_any_rem = abs_dropped != 0 || rem != 0;
    let inc = should_increment(final_quot, sign, has_any_rem, cmp_half, mc.rounding_mode);
    let mut result = if inc {
        final_quot + sign
    } else {
        final_quot
    };
    let mut new_scale = scale - (drop as i32);
    if num_digits(result) > mc.precision {
        result /= 10;
        new_scale -= 1;
    }
    BigDec::from_unscaled(result, new_scale)
}

fn apply_rounding(quot: i128, rem: i128, divisor: i128, mode: RoundingMode) -> i128 {
    if rem == 0 {
        return quot;
    }
    let sign = if rem < 0 && divisor > 0 || rem > 0 && divisor < 0 {
        -1i128
    } else {
        1i128
    };
    let abs_rem_2 = rem.abs() * 2;
    let abs_div = divisor.abs();
    let cmp_half = abs_rem_2.cmp(&abs_div);
    if should_increment(quot, sign, true, cmp_half, mode) {
        quot + sign
    } else {
        quot
    }
}

fn should_increment(
    quot: i128,
    sign: i128,
    has_rem: bool,
    cmp_half: Ordering,
    mode: RoundingMode,
) -> bool {
    if !has_rem {
        return false;
    }
    match mode {
        RoundingMode::Down => false,
        RoundingMode::Up => true,
        RoundingMode::Ceiling => sign > 0,
        RoundingMode::Floor => sign < 0,
        RoundingMode::HalfUp => cmp_half != Ordering::Less,
        RoundingMode::HalfDown => cmp_half == Ordering::Greater,
        RoundingMode::HalfEven => match cmp_half {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => quot.abs() % 2 != 0,
        },
    }
}

impl PartialEq for BigDec {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for BigDec {}

impl PartialOrd for BigDec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigDec {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.strip_trailing_zeros();
        let b = other.strip_trailing_zeros();
        let target_scale = a.scale.max(b.scale);
        let lhs = scale_up(a.unscaled, target_scale - a.scale);
        let rhs = scale_up(b.unscaled, target_scale - b.scale);
        lhs.cmp(&rhs)
    }
}

impl std::hash::Hash for BigDec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let norm = self.strip_trailing_zeros();
        norm.unscaled.hash(state);
        norm.scale.hash(state);
    }
}

impl From<i64> for BigDec {
    fn from(val: i64) -> Self {
        Self::from_i64(val)
    }
}

impl From<i32> for BigDec {
    fn from(val: i32) -> Self {
        Self::from_i64(val as i64)
    }
}

impl std::ops::Add for BigDec {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::add(self, rhs, None)
    }
}

impl std::ops::Sub for BigDec {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::sub(self, rhs, None)
    }
}

impl std::ops::Mul for BigDec {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::mul(self, rhs, None)
    }
}

impl BigDec {
    pub fn divide_scale(self, rhs: Self, scale: u32, mode: RoundingMode) -> Self {
        let mc = MathContext {
            precision: 18,
            rounding_mode: mode,
        };
        self.div(rhs, mc).set_scale(scale as i32, mode)
    }

    pub fn to_string_with_scale(self, scale: u32) -> String {
        self.set_scale(scale as i32, RoundingMode::HalfEven)
            .to_string()
    }

    pub fn parse_lossy(s: &str) -> Self {
        Self::parse(s).unwrap_or(Self::ZERO)
    }
}

impl fmt::Display for BigDec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale <= 0 {
            let v = scale_up(self.unscaled, -self.scale);
            return write!(f, "{}", v);
        }
        let sign = if self.unscaled < 0 { "-" } else { "" };
        let abs = self.unscaled.unsigned_abs();
        let scale = self.scale as usize;
        let digits = format!("{:0width$}", abs, width = scale + 1);
        let split_idx = digits.len() - scale;
        write!(f, "{}{}.{}", sign, &digits[..split_idx], &digits[split_idx..])
    }
}

/// Currency metadata matching `org.apache.fineract.organisation.monetary.data.CurrencyData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencyData {
    pub code: String,
    pub name: String,
    pub decimal_places: u32,
    pub in_multiples_of: Option<i32>,
    pub name_code: String,
    pub display_symbol: String,
}

impl CurrencyData {
    pub fn new(
        code: impl Into<String>,
        name: impl Into<String>,
        decimal_places: u32,
        in_multiples_of: Option<i32>,
        name_code: impl Into<String>,
        display_symbol: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            name: name.into(),
            decimal_places,
            in_multiples_of,
            name_code: name_code.into(),
            display_symbol: display_symbol.into(),
        }
    }

    pub fn usd() -> Self {
        Self::new("usd", "US Dollar", 2, None, "usd", "$")
    }
}

/// Monetary value rounded to currency scale or `in_multiples_of`, matching `org.apache.fineract.organisation.monetary.domain.Money`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub currency: CurrencyData,
    pub amount: BigDec,
    pub mc: MathContext,
}

impl Money {
    pub fn zero(currency: CurrencyData, mc: MathContext) -> Self {
        let amount = BigDec::from_unscaled(0, currency.decimal_places as i32);
        Self {
            currency,
            amount,
            mc,
        }
    }

    pub fn of(currency: CurrencyData, raw_amount: BigDec, mc: MathContext) -> Self {
        let scale = currency.decimal_places as i32;
        let mut rounded = raw_amount.set_scale(scale, mc.rounding_mode);
        if let Some(mult) = currency.in_multiples_of {
            if mult > 1 {
                let m = BigDec::from_i64(mult as i64);
                let div = rounded
                    .div(m, MathContext::new(12, mc.rounding_mode))
                    .set_scale(0, mc.rounding_mode);
                rounded = div.mul(m, None).set_scale(scale, mc.rounding_mode);
            }
        }
        Self {
            currency,
            amount: rounded,
            mc,
        }
    }

    pub fn round_to_multiples_of(money: &Self, multiples_of: i32) -> Self {
        if multiples_of <= 0 {
            return money.clone();
        }
        let scale = money.currency.decimal_places as i32;
        let m = BigDec::from_i64(multiples_of as i64);
        let div = money
            .amount
            .div(m, MathContext::new(12, money.mc.rounding_mode))
            .set_scale(0, money.mc.rounding_mode);
        let rounded = div.mul(m, None).set_scale(scale, money.mc.rounding_mode);
        Self {
            currency: money.currency.clone(),
            amount: rounded,
            mc: money.mc,
        }
    }

    pub fn plus(&self, other: &Self) -> Self {
        Self::of(
            self.currency.clone(),
            self.amount.add(other.amount, Some(self.mc)),
            self.mc,
        )
    }

    pub fn minus(&self, other: &Self) -> Self {
        Self::of(
            self.currency.clone(),
            self.amount.sub(other.amount, Some(self.mc)),
            self.mc,
        )
    }

    pub fn negated(&self) -> Self {
        Self::of(self.currency.clone(), self.amount.negated(), self.mc)
    }

    pub fn negative_to_zero(&self) -> Self {
        if self.amount.is_negative() {
            Self::zero(self.currency.clone(), self.mc)
        } else {
            self.clone()
        }
    }

    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    pub fn is_greater_than_zero(&self) -> bool {
        self.amount.is_positive()
    }

    pub fn is_greater_than(&self, other: &Self) -> bool {
        self.amount > other.amount
    }

    pub fn is_less_than(&self, other: &Self) -> bool {
        self.amount < other.amount
    }
}

// ============================================================================
// 2. Domain Enums (`DaysInMonthType`, `DaysInYearType`, `InterestMethod`, etc.)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaysInMonthType {
    Actual,
    Days30,
}

impl DaysInMonthType {
    pub fn number_of_days(self, date: NaiveDate) -> u32 {
        match self {
            Self::Actual => days_in_month(date.year(), date.month()),
            Self::Days30 => 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaysInYearType {
    Actual,
    Days360,
    Days364,
    Days365,
}

impl DaysInYearType {
    pub fn number_of_days(self, date: NaiveDate) -> u32 {
        match self {
            Self::Actual => {
                if is_leap_year(date.year()) {
                    366
                } else {
                    365
                }
            }
            Self::Days360 => 360,
            Self::Days364 => 364,
            Self::Days365 => 365,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaysInYearCustomStrategyType {
    FullLeapYear,
    Feb29PeriodOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterestMethod {
    DecliningBalance,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodFrequencyType {
    Days,
    Weeks,
    Months,
    Years,
}

impl PeriodFrequencyType {
    pub fn from_str_name(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "DAYS" => Self::Days,
            "WEEKS" => Self::Weeks,
            "MONTHS" => Self::Months,
            "YEARS" => Self::Years,
            other => panic!("Unsupported repaymentFrequencyType: {}", other),
        }
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

pub fn days_between(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

// ============================================================================
// 3. Input Model & Output Schedule Plan Types
// ============================================================================

/// Rust equivalent of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.LoanRepaymentScheduleModelData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanRepaymentScheduleModelData {
    pub schedule_generation_start_date: NaiveDate,
    pub currency: CurrencyData,
    pub disbursement_amount: BigDec,
    pub disbursement_date: NaiveDate,
    pub number_of_repayments: usize,
    pub repayment_frequency: u32,
    pub repayment_frequency_type: String,
    pub annual_nominal_interest_rate: BigDec,
    pub down_payment_enabled: bool,
    pub days_in_month: DaysInMonthType,
    pub days_in_year: DaysInYearType,
    pub down_payment_percentage: BigDec,
    pub installment_amount_in_multiples_of: Option<i32>,
    pub fixed_length: Option<u32>,
    pub interest_recognition_on_disbursement_date: bool,
    pub days_in_year_custom_strategy: Option<DaysInYearCustomStrategyType>,
    pub interest_method: InterestMethod,
    pub allow_partial_period_interest_calculation: bool,
    pub allow_full_term_for_tranche: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanSchedulePlanDisbursementPeriod {
    pub period_from_date: NaiveDate,
    pub period_due_date: NaiveDate,
    pub principal_amount: BigDec,
    pub outstanding_loan_balance: BigDec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanSchedulePlanDownPaymentPeriod {
    pub period_number: usize,
    pub period_from_date: NaiveDate,
    pub period_due_date: NaiveDate,
    pub principal_amount: BigDec,
    pub total_due_amount: BigDec,
    pub outstanding_loan_balance: BigDec,
    pub total_outstanding_loan_balance: BigDec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanSchedulePlanRepaymentPeriod {
    pub period_number: usize,
    pub period_from_date: NaiveDate,
    pub period_due_date: NaiveDate,
    pub principal_amount: BigDec,
    pub interest_amount: BigDec,
    pub fee_amount: BigDec,
    pub penalty_amount: BigDec,
    pub total_due_amount: BigDec,
    pub outstanding_loan_balance: BigDec,
    pub total_outstanding_loan_balance: BigDec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoanSchedulePlanPeriod {
    Disbursement(LoanSchedulePlanDisbursementPeriod),
    DownPayment(LoanSchedulePlanDownPaymentPeriod),
    Repayment(LoanSchedulePlanRepaymentPeriod),
}

impl LoanSchedulePlanPeriod {
    pub fn period_number(&self) -> Option<usize> {
        match self {
            Self::Disbursement(_) => None,
            Self::DownPayment(p) => Some(p.period_number),
            Self::Repayment(p) => Some(p.period_number),
        }
    }

    pub fn period_from_date(&self) -> NaiveDate {
        match self {
            Self::Disbursement(p) => p.period_from_date,
            Self::DownPayment(p) => p.period_from_date,
            Self::Repayment(p) => p.period_from_date,
        }
    }

    pub fn period_due_date(&self) -> NaiveDate {
        match self {
            Self::Disbursement(p) => p.period_due_date,
            Self::DownPayment(p) => p.period_due_date,
            Self::Repayment(p) => p.period_due_date,
        }
    }
}

/// Rust equivalent of `org.apache.fineract.portfolio.loanaccount.loanschedule.data.LoanSchedulePlan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanSchedulePlan {
    pub periods: Vec<LoanSchedulePlanPeriod>,
    pub currency: CurrencyData,
    pub loan_term_in_days: i64,
    pub total_disbursed_amount: BigDec,
    pub total_principal_amount: BigDec,
    pub total_interest_amount: BigDec,
    pub total_fee_amount: BigDec,
    pub total_penalty_amount: BigDec,
    pub total_repayment_amount: BigDec,
    pub total_outstanding_amount: BigDec,
}

// ============================================================================
// 4. Scheduled Date Generator (`DefaultScheduledDateGenerator`)
// ============================================================================

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultScheduledDateGenerator;

impl DefaultScheduledDateGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_repayment_period_dates(
        &self,
        schedule_start_date: NaiveDate,
        number_of_repayments: usize,
        repayment_every: u32,
        frequency_type: PeriodFrequencyType,
        fixed_length: Option<u32>,
    ) -> Vec<(NaiveDate, NaiveDate)> {
        let mut periods = Vec::with_capacity(number_of_repayments);
        let mut last_date = schedule_start_date;
        for period_num in 1..=number_of_repayments {
            let mut next_date =
                self.generate_next_repayment_date(last_date, schedule_start_date, repayment_every, frequency_type);
            if period_num == number_of_repayments {
                if let Some(fixed) = fixed_length {
                    next_date = add_frequency(schedule_start_date, fixed, frequency_type);
                }
            }
            periods.push((last_date, next_date));
            last_date = next_date;
        }
        periods
    }

    pub fn generate_next_repayment_date(
        &self,
        last_repayment_date: NaiveDate,
        seed_date: NaiveDate,
        repayment_every: u32,
        frequency_type: PeriodFrequencyType,
    ) -> NaiveDate {
        let due_date = add_frequency(last_repayment_date, repayment_every, frequency_type);
        adjust_date(due_date, seed_date, frequency_type)
    }
}

fn add_frequency(date: NaiveDate, count: u32, frequency_type: PeriodFrequencyType) -> NaiveDate {
    match frequency_type {
        PeriodFrequencyType::Days => date + Duration::days(count as i64),
        PeriodFrequencyType::Weeks => date + Duration::weeks(count as i64),
        PeriodFrequencyType::Months => date
            .checked_add_months(Months::new(count))
            .expect("valid month addition"),
        PeriodFrequencyType::Years => date
            .checked_add_months(Months::new(count * 12))
            .expect("valid year addition"),
    }
}

fn adjust_date(date: NaiveDate, seed_date: NaiveDate, frequency_type: PeriodFrequencyType) -> NaiveDate {
    if frequency_type == PeriodFrequencyType::Months && seed_date.day() > 28 && date.day() >= 28 {
        let max_days = days_in_month(date.year(), date.month());
        let adjusted_day = max_days.min(seed_date.day());
        NaiveDate::from_ymd_opt(date.year(), date.month(), adjusted_day).unwrap_or(date)
    } else {
        date
    }
}

// ============================================================================
// 5. Progressive EMI Calculator & Interest Schedule Model
// ============================================================================

#[derive(Debug, Clone)]
pub struct LoanProductRelatedDetail {
    pub currency: CurrencyData,
    pub annual_nominal_interest_rate: BigDec,
    pub days_in_month: DaysInMonthType,
    pub days_in_year: DaysInYearType,
    pub days_in_year_custom_strategy: Option<DaysInYearCustomStrategyType>,
    pub repayment_frequency_type: PeriodFrequencyType,
    pub repay_every: u32,
    pub number_of_repayments: usize,
    pub interest_method: InterestMethod,
    pub interest_recognition_on_disbursement_date: bool,
    pub allow_partial_period_interest_calculation: bool,
    pub allow_full_term_for_tranche: bool,
}

#[derive(Debug, Clone)]
pub struct InterestPeriod {
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub rate_factor: BigDec,
    pub rate_factor_till_period_due_date: BigDec,
    pub credited_principal: Money,
    pub credited_interest: Money,
    pub disbursement_amount: Money,
    pub balance_correction_amount: Money,
    pub outstanding_loan_balance: Money,
    pub capitalized_income_principal: Money,
    pub is_paused: bool,
}

impl InterestPeriod {
    pub fn with_empty_amounts(
        from_date: NaiveDate,
        due_date: NaiveDate,
        zero: &Money,
        is_paused: bool,
    ) -> Self {
        Self {
            from_date,
            due_date,
            rate_factor: BigDec::ZERO,
            rate_factor_till_period_due_date: BigDec::ZERO,
            credited_principal: zero.clone(),
            credited_interest: zero.clone(),
            disbursement_amount: zero.clone(),
            balance_correction_amount: zero.clone(),
            outstanding_loan_balance: zero.clone(),
            capitalized_income_principal: zero.clone(),
            is_paused,
        }
    }

    pub fn length(&self) -> i64 {
        days_between(self.from_date, self.due_date)
    }

    pub fn length_till_period_due_date(&self, rp_due_date: NaiveDate) -> i64 {
        days_between(self.from_date, rp_due_date)
    }

    pub fn calculated_due_interest(
        &self,
        rp_due_date: NaiveDate,
        method: InterestMethod,
        total_disbursed_till_ip: &Money,
        mc: MathContext,
    ) -> BigDec {
        if self.is_paused {
            return self.credited_interest.amount;
        }
        let len_till_due = self.length_till_period_due_date(rp_due_date);
        if len_till_due == 0 {
            return self.credited_interest.amount.negative_to_zero();
        }
        let base_amount = match method {
            InterestMethod::Flat => total_disbursed_till_ip.amount,
            InterestMethod::DecliningBalance => self.outstanding_loan_balance.amount,
        };
        let interest_due = base_amount
            .mul(self.rate_factor_till_period_due_date, Some(mc))
            .div(BigDec::from_i64(len_till_due), mc)
            .mul(BigDec::from_i64(self.length()), Some(mc));
        self.credited_interest
            .amount
            .add(interest_due, Some(mc))
            .negative_to_zero()
    }

    pub fn credited_amounts(&self) -> Money {
        self.disbursement_amount
            .plus(&self.credited_principal)
            .plus(&self.capitalized_income_principal)
    }
}

#[derive(Debug, Clone)]
pub struct RepaymentPeriod {
    pub from_date: NaiveDate,
    pub due_date: NaiveDate,
    pub interest_periods: Vec<InterestPeriod>,
    pub emi: Money,
    pub original_emi: Money,
    pub paid_principal: Money,
    pub paid_interest: Money,
    pub future_unrecognized_interest: Money,
    pub total_disbursed_amount: Money,
    pub fixed_interest: Money,
    pub is_interest_moved_upward: bool,
    pub is_interest_moved_downward: bool,
    pub interest_payment_grace: bool,
    pub principal_payment_grace: bool,
    pub mc: MathContext,
    pub interest_method: InterestMethod,
}

impl RepaymentPeriod {
    pub fn create(
        from_date: NaiveDate,
        due_date: NaiveDate,
        zero: Money,
        mc: MathContext,
        interest_method: InterestMethod,
    ) -> Self {
        let ip = InterestPeriod::with_empty_amounts(from_date, due_date, &zero, false);
        Self {
            from_date,
            due_date,
            interest_periods: vec![ip],
            emi: zero.clone(),
            original_emi: zero.clone(),
            paid_principal: zero.clone(),
            paid_interest: zero.clone(),
            future_unrecognized_interest: zero.clone(),
            total_disbursed_amount: zero.clone(),
            fixed_interest: zero,
            is_interest_moved_upward: false,
            is_interest_moved_downward: false,
            interest_payment_grace: false,
            principal_payment_grace: false,
            mc,
            interest_method,
        }
    }

    pub fn zero(&self) -> Money {
        Money::zero(self.emi.currency.clone(), self.mc)
    }

    pub fn rate_factor_plus_1(&self) -> BigDec {
        self.interest_periods
            .iter()
            .fold(BigDec::ONE, |acc, ip| acc.add(ip.rate_factor, None))
    }

    pub fn total_credited_amount(&self) -> Money {
        let mut total = self.zero();
        for ip in &self.interest_periods {
            total = total.plus(&ip.credited_principal).plus(&ip.credited_interest);
        }
        total
    }

    pub fn credited_principal(&self) -> Money {
        let mut total = self.zero();
        for ip in &self.interest_periods {
            total = total.plus(&ip.credited_principal);
        }
        total.negative_to_zero()
    }

    pub fn credited_amounts(&self) -> Money {
        let mut total = self.zero();
        for ip in &self.interest_periods {
            total = total.plus(&ip.credited_amounts());
        }
        total
    }

    pub fn emi_plus_credited_amounts_plus_future_unrecognized_interest(&self) -> Money {
        self.emi
            .plus(&self.total_credited_amount())
            .plus(&self.future_unrecognized_interest)
    }

    pub fn total_paid_amount(&self) -> Money {
        self.paid_principal.plus(&self.paid_interest)
    }

    pub fn is_fully_paid(&self) -> bool {
        self.emi_plus_credited_amounts_plus_future_unrecognized_interest() == self.total_paid_amount()
    }

    pub fn calculate_calculated_due_interest(&self, prev_unrecognized: Option<Money>) -> Money {
        let mut sum = BigDec::ZERO;
        if !self.is_interest_moved_upward && !self.is_interest_moved_downward {
            for ip in &self.interest_periods {
                sum = sum.add(
                    ip.calculated_due_interest(
                        self.due_date,
                        self.interest_method,
                        &self.total_disbursed_amount,
                        self.mc,
                    ),
                    None,
                );
            }
        }
        let mut calc = Money::of(self.emi.currency.clone(), sum, self.mc);
        calc = calc
            .plus(&self.fixed_interest)
            .plus(&self.future_unrecognized_interest);
        if let Some(unrec) = prev_unrecognized {
            calc = calc.plus(&unrec);
        }
        calc.negative_to_zero()
    }

    pub fn calculated_due_interest(&self) -> Money {
        self.calculate_calculated_due_interest(None)
    }

    pub fn calculated_due_principal(&self) -> Money {
        self.emi_plus_credited_amounts_plus_future_unrecognized_interest()
            .minus(&self.calculated_due_interest())
            .negative_to_zero()
    }

    pub fn due_interest(&self) -> Money {
        if self.interest_payment_grace {
            return self.paid_interest.clone();
        }
        let candidate = if self.paid_principal.is_greater_than(&self.calculated_due_principal()) {
            self.paid_interest.clone()
        } else {
            let calc_int = self.calculated_due_interest();
            let cap = self.emi_plus_credited_amounts_plus_future_unrecognized_interest();
            if calc_int.is_less_than(&cap) {
                calc_int
            } else {
                cap
            }
        };
        if candidate.is_greater_than(&self.paid_interest) {
            candidate
        } else {
            self.paid_interest.clone()
        }
    }

    pub fn due_principal(&self) -> Money {
        let diff = self
            .emi_plus_credited_amounts_plus_future_unrecognized_interest()
            .minus(&self.due_interest())
            .negative_to_zero();
        if diff.is_greater_than(&self.paid_principal) {
            diff
        } else {
            self.paid_principal.clone()
        }
    }

    pub fn unrecognized_interest(&self) -> Money {
        self.calculated_due_interest()
            .minus(&self.due_interest())
            .negative_to_zero()
    }

    pub fn outstanding_loan_balance(&self) -> Money {
        let last_ip = self.interest_periods.last().expect("at least one interest period");
        last_ip
            .outstanding_loan_balance
            .plus(&last_ip.balance_correction_amount)
            .plus(&last_ip.capitalized_income_principal)
            .plus(&last_ip.disbursement_amount)
            .plus(&self.paid_principal)
            .minus(&self.due_principal())
            .negative_to_zero()
    }
}

#[derive(Debug, Clone)]
pub struct InterestRateChange {
    pub effective_from: NaiveDate,
    pub interest_rate: BigDec,
}

#[derive(Debug, Clone)]
pub struct ProgressiveLoanInterestScheduleModel {
    pub repayment_periods: Vec<RepaymentPeriod>,
    pub interest_rates: Vec<InterestRateChange>,
    pub loan_product_related_detail: LoanProductRelatedDetail,
    pub installment_amount_in_multiples_of: Option<i32>,
    pub mc: MathContext,
    pub is_copy: bool,
}

impl ProgressiveLoanInterestScheduleModel {
    pub fn new(
        repayment_periods: Vec<RepaymentPeriod>,
        loan_product_related_detail: LoanProductRelatedDetail,
        installment_amount_in_multiples_of: Option<i32>,
        mc: MathContext,
    ) -> Self {
        Self {
            repayment_periods,
            interest_rates: Vec::new(),
            loan_product_related_detail,
            installment_amount_in_multiples_of,
            mc,
            is_copy: false,
        }
    }

    pub fn zero(&self) -> Money {
        Money::zero(self.loan_product_related_detail.currency.clone(), self.mc)
    }

    pub fn is_empty(&self) -> bool {
        self.repayment_periods.iter().all(|rp| rp.emi.is_zero())
    }

    pub fn copy_without_paid_amounts(&self) -> Self {
        let mut cloned = self.clone();
        cloned.is_copy = true;
        let zero = cloned.zero();
        for rp in &mut cloned.repayment_periods {
            rp.paid_principal = zero.clone();
            rp.paid_interest = zero.clone();
            rp.future_unrecognized_interest = zero.clone();
            for ip in &mut rp.interest_periods {
                ip.balance_correction_amount = zero.clone();
            }
        }
        cloned
    }

    pub fn loan_term_in_days(&self) -> i64 {
        let Some(first) = self.repayment_periods.first() else {
            return 0;
        };
        let last = self.repayment_periods.last().unwrap_or(first);
        days_between(first.from_date, last.due_date)
    }

    pub fn start_date(&self) -> Option<NaiveDate> {
        self.repayment_periods.first().map(|rp| rp.from_date)
    }

    pub fn maturity_date(&self) -> Option<NaiveDate> {
        self.repayment_periods.last().map(|rp| rp.due_date)
    }

    pub fn add_interest_rate(&mut self, effective_from: NaiveDate, interest_rate: BigDec) {
        self.interest_rates.push(InterestRateChange {
            effective_from,
            interest_rate,
        });
        // Sort descending by effective_from
        self.interest_rates
            .sort_by(|a, b| b.effective_from.cmp(&a.effective_from));
    }

    pub fn get_interest_rate(&self, effective_date: NaiveDate) -> BigDec {
        for ir in &self.interest_rates {
            if ir.effective_from <= effective_date {
                return ir.interest_rate;
            }
        }
        self.loan_product_related_detail.annual_nominal_interest_rate
    }

    pub fn total_due_principal(&self) -> Money {
        let mut sum = self.zero();
        for rp in &self.repayment_periods {
            sum = sum.plus(&rp.credited_amounts());
        }
        sum.negative_to_zero()
    }

    pub fn total_paid_principal(&self) -> Money {
        let mut sum = self.zero();
        for rp in &self.repayment_periods {
            sum = sum.plus(&rp.paid_principal);
        }
        sum.negative_to_zero()
    }

    pub fn total_credited_principal(&self) -> Money {
        let mut sum = self.zero();
        for rp in &self.repayment_periods {
            sum = sum.plus(&rp.credited_principal());
        }
        sum.negative_to_zero()
    }

    pub fn find_repayment_period_by_from_and_due_date(
        &self,
        from_date: NaiveDate,
        due_date: NaiveDate,
    ) -> Option<&RepaymentPeriod> {
        self.repayment_periods
            .iter()
            .find(|rp| rp.from_date == from_date && rp.due_date == due_date)
            .or_else(|| {
                self.repayment_periods
                    .iter()
                    .find(|rp| rp.from_date <= from_date && rp.due_date >= due_date)
            })
    }

    pub fn change_outstanding_balance_and_update_interest_periods(
        &mut self,
        balance_change_date: NaiveDate,
        disbursed_amount: Money,
        correction_amount: Money,
        capitalized_income_principal: Money,
    ) -> Option<usize> {
        let rp_idx = self.repayment_periods.iter().position(|rp| {
            let is_first = rp.from_date == self.repayment_periods[0].from_date;
            if is_first {
                balance_change_date >= rp.from_date && balance_change_date <= rp.due_date
            } else {
                balance_change_date > rp.from_date && balance_change_date <= rp.due_date
            }
        })?;

        let zero = self.zero();
        let is_last = rp_idx + 1 == self.repayment_periods.len();
        let rp = &mut self.repayment_periods[rp_idx];
        let is_change_on_maturity = is_last && balance_change_date == rp.due_date;

        let existing_ip_idx = if is_change_on_maturity {
            let last_idx = rp.interest_periods.len() - 1;
            if rp.interest_periods[last_idx].length() == 0 {
                Some(last_idx)
            } else {
                None
            }
        } else {
            rp.interest_periods
                .iter()
                .position(|ip| ip.due_date == balance_change_date)
        };

        if let Some(ip_idx) = existing_ip_idx {
            let ip = &mut rp.interest_periods[ip_idx];
            ip.disbursement_amount = ip.disbursement_amount.plus(&disbursed_amount);
            ip.capitalized_income_principal =
                ip.capitalized_income_principal.plus(&capitalized_income_principal);
            ip.balance_correction_amount = ip.balance_correction_amount.plus(&correction_amount);
        } else {
            // Find previous interest period to split
            let prev_idx = rp
                .interest_periods
                .iter()
                .rposition(|ip| balance_change_date > ip.from_date && balance_change_date <= ip.due_date)
                .unwrap_or(0);
            let orig_due = rp.interest_periods[prev_idx].due_date;
            let new_due = balance_change_date
                .max(rp.interest_periods[prev_idx].from_date)
                .min(orig_due);
            let is_paused = rp.interest_periods[prev_idx].is_paused;

            rp.interest_periods[prev_idx].due_date = new_due;
            rp.interest_periods[prev_idx].disbursement_amount = rp.interest_periods[prev_idx]
                .disbursement_amount
                .plus(&disbursed_amount);
            rp.interest_periods[prev_idx].capitalized_income_principal = rp.interest_periods
                [prev_idx]
                .capitalized_income_principal
                .plus(&capitalized_income_principal);
            rp.interest_periods[prev_idx].balance_correction_amount = rp.interest_periods[prev_idx]
                .balance_correction_amount
                .plus(&correction_amount);

            let new_ip = InterestPeriod::with_empty_amounts(new_due, orig_due, &zero, is_paused);
            rp.interest_periods.insert(prev_idx + 1, new_ip);
        }

        Some(rp_idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmiChangeAction {
    Disbursement,
    InterestRateChange,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ProgressiveEMICalculator {
    pub scheduled_date_generator: DefaultScheduledDateGenerator,
}

impl ProgressiveEMICalculator {
    pub fn new(scheduled_date_generator: DefaultScheduledDateGenerator) -> Self {
        Self {
            scheduled_date_generator,
        }
    }

    pub fn generate_period_interest_schedule_model(
        &self,
        period_dates: &[(NaiveDate, NaiveDate)],
        detail: LoanProductRelatedDetail,
        installment_amount_in_multiples_of: Option<i32>,
        mc: MathContext,
    ) -> ProgressiveLoanInterestScheduleModel {
        let zero = Money::zero(detail.currency.clone(), mc);
        let repayment_periods = period_dates
            .iter()
            .map(|&(from, to)| {
                RepaymentPeriod::create(from, to, zero.clone(), mc, detail.interest_method)
            })
            .collect();
        ProgressiveLoanInterestScheduleModel::new(
            repayment_periods,
            detail,
            installment_amount_in_multiples_of,
            mc,
        )
    }

    pub fn add_disbursement(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        disbursement_due_date: NaiveDate,
        disbursed_amount: Money,
    ) {
        self.apply_disbursement_internal(
            model,
            disbursement_due_date,
            disbursed_amount,
            EmiChangeAction::Disbursement,
        );
    }

    fn apply_disbursement_internal(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        submitted_on_date: NaiveDate,
        amount: Money,
        action: EmiChangeAction,
    ) {
        for rp in &mut model.repayment_periods {
            if submitted_on_date <= rp.from_date {
                rp.total_disbursed_amount = rp.total_disbursed_amount.plus(&amount);
            }
        }
        let was_empty = model.is_empty();
        if let Some(rp_idx) = model.change_outstanding_balance_and_update_interest_periods(
            submitted_on_date,
            amount,
            model.zero(),
            model.zero(),
        ) {
            let eff_due_date = self.effective_repayment_due_date(model, rp_idx, submitted_on_date);
            self.calculate_emi_value_and_rate_factors(
                eff_due_date,
                model,
                action,
                submitted_on_date,
                was_empty,
            );
        }
    }

    pub fn change_interest_rate(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        new_interest_submitted_on_date: NaiveDate,
        new_interest_rate: BigDec,
    ) {
        let effective_date = new_interest_submitted_on_date - Duration::days(1);
        model.add_interest_rate(effective_date, new_interest_rate);
        let was_empty = model.is_empty();
        if let Some(rp_idx) = model.change_outstanding_balance_and_update_interest_periods(
            effective_date,
            model.zero(),
            model.zero(),
            model.zero(),
        ) {
            let eff_due_date = self.effective_repayment_due_date(model, rp_idx, effective_date);
            self.calculate_emi_value_and_rate_factors(
                eff_due_date,
                model,
                EmiChangeAction::InterestRateChange,
                effective_date,
                was_empty,
            );
        }
    }

    fn effective_repayment_due_date(
        &self,
        model: &ProgressiveLoanInterestScheduleModel,
        rp_idx: usize,
        operation_date: NaiveDate,
    ) -> NaiveDate {
        let changed_due = model.repayment_periods[rp_idx].due_date;
        if changed_due == operation_date && rp_idx + 1 < model.repayment_periods.len() {
            return model.repayment_periods[rp_idx + 1].due_date;
        }
        changed_due
    }

    fn calculate_emi_value_and_rate_factors(
        &self,
        calculate_from_due_date: NaiveDate,
        model: &mut ProgressiveLoanInterestScheduleModel,
        action: EmiChangeAction,
        operation_date: NaiveDate,
        was_empty: bool,
    ) {
        let start_idx = model
            .repayment_periods
            .iter()
            .position(|rp| rp.due_date >= calculate_from_due_date)
            .unwrap_or(model.repayment_periods.len());

        self.calculate_rate_factors_from_idx(model, start_idx);
        self.calculate_outstanding_balance(model);

        let only_on_actual = was_empty
            || action == EmiChangeAction::InterestRateChange
            || model.is_copy;

        if only_on_actual {
            self.calculate_emi_on_actual_model(model, start_idx);
        } else {
            // Copy without paid amounts and merge
            let mut model_copy = model.copy_without_paid_amounts();
            let zero = model_copy.zero();
            self.apply_disbursement_internal(
                &mut model_copy,
                operation_date,
                zero,
                EmiChangeAction::Disbursement,
            );
            for i in start_idx..model.repayment_periods.len() {
                model.repayment_periods[i].emi = model_copy.repayment_periods[i].emi.clone();
                model.repayment_periods[i].original_emi =
                    model_copy.repayment_periods[i].original_emi.clone();
            }
        }

        self.calculate_outstanding_balance(model);
        self.calculate_last_unpaid_repayment_period_emi(model);

        if only_on_actual && start_idx < model.repayment_periods.len() {
            self.check_and_adjust_emi_if_needed(model, start_idx);
        }
    }

    fn calculate_rate_factors_from_idx(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        start_idx: usize,
    ) {
        for i in start_idx..model.repayment_periods.len() {
            let rp_from = model.repayment_periods[i].from_date;
            let rp_due = model.repayment_periods[i].due_date;
            let ip_len = model.repayment_periods[i].interest_periods.len();
            for j in 0..ip_len {
                let ip_from = model.repayment_periods[i].interest_periods[j].from_date;
                let ip_due = model.repayment_periods[i].interest_periods[j].due_date;
                let rf = self.calculate_rate_factor_per_period(model, rp_from, rp_due, ip_from, ip_due);
                let rf_till = self.calculate_rate_factor_per_period_for_interest(
                    model, rp_from, rp_due, ip_from, ip_due,
                );
                model.repayment_periods[i].interest_periods[j].rate_factor = rf;
                model.repayment_periods[i].interest_periods[j].rate_factor_till_period_due_date =
                    rf_till;
            }
        }
    }

    pub fn calculate_outstanding_balance(&self, model: &mut ProgressiveLoanInterestScheduleModel) {
        let n = model.repayment_periods.len();
        for i in 0..n {
            let prev_end_balance = if i > 0 {
                let prev_rp = &model.repayment_periods[i - 1];
                let prev_last_ip = prev_rp
                    .interest_periods
                    .last()
                    .expect("at least one interest period");
                Some(
                    prev_last_ip
                        .outstanding_loan_balance
                        .plus(&prev_last_ip.disbursement_amount)
                        .plus(&prev_last_ip.capitalized_income_principal)
                        .plus(&prev_last_ip.balance_correction_amount)
                        .minus(&prev_rp.due_principal())
                        .plus(&prev_rp.paid_principal)
                        .negative_to_zero(),
                )
            } else {
                None
            };

            let rp = &mut model.repayment_periods[i];
            for j in 0..rp.interest_periods.len() {
                if j == 0 {
                    if let Some(ref bal) = prev_end_balance {
                        rp.interest_periods[0].outstanding_loan_balance = bal.clone();
                    }
                } else {
                    let prev_ip = &rp.interest_periods[j - 1];
                    let next_bal = prev_ip
                        .outstanding_loan_balance
                        .plus(&prev_ip.balance_correction_amount)
                        .plus(&prev_ip.capitalized_income_principal)
                        .plus(&prev_ip.disbursement_amount)
                        .negative_to_zero();
                    rp.interest_periods[j].outstanding_loan_balance = next_bal;
                }
            }
        }
    }

    fn initial_balance_for_emi_recalculation(
        &self,
        model: &ProgressiveLoanInterestScheduleModel,
        start_idx: usize,
    ) -> Money {
        let initial = if start_idx > 0 {
            model.repayment_periods[start_idx - 1].outstanding_loan_balance()
        } else {
            model.zero()
        };
        let rp = &model.repayment_periods[start_idx];
        let mut disbursed = model.zero();
        let mut cap_inc = model.zero();
        for ip in &rp.interest_periods {
            disbursed = disbursed.plus(&ip.disbursement_amount);
            cap_inc = cap_inc.plus(&ip.capitalized_income_principal);
        }
        initial.plus(&disbursed).plus(&cap_inc)
    }

    fn calculate_emi_on_actual_model(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        start_idx: usize,
    ) {
        if start_idx >= model.repayment_periods.len() {
            return;
        }
        let mc = model.mc;
        let rate_factor_n = model.repayment_periods[start_idx..]
            .iter()
            .fold(BigDec::ONE, |acc, rp| {
                acc.mul(rp.rate_factor_plus_1(), Some(mc))
            })
            .strip_trailing_zeros();

        let fn_result = model.repayment_periods[start_idx..]
            .iter()
            .skip(1)
            .fold(BigDec::ONE, |prev_fn, rp| {
                self.fn_value(prev_fn, rp.rate_factor_plus_1(), mc)
            })
            .strip_trailing_zeros();

        let outstanding_balance = self.initial_balance_for_emi_recalculation(model, start_idx);
        let raw_emi = rate_factor_n
            .mul(outstanding_balance.amount, Some(mc))
            .div(fn_result, mc);
        let emi_money = Money::of(
            model.loan_product_related_detail.currency.clone(),
            raw_emi,
            mc,
        );
        let final_emi = match model.installment_amount_in_multiples_of {
            Some(mult) if mult > 0 => {
                let rounded = Money::round_to_multiples_of(&emi_money, mult);
                if rounded.is_zero() && emi_money.is_greater_than_zero() {
                    emi_money
                } else {
                    rounded
                }
            }
            _ => emi_money,
        };

        for rp in &mut model.repayment_periods[start_idx..] {
            if !final_emi.is_less_than(&rp.total_paid_amount()) {
                rp.emi = final_emi.clone();
                rp.original_emi = final_emi.clone();
            }
        }
    }

    fn calculate_last_unpaid_repayment_period_emi(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
    ) {
        let total_due_paid_diff = model
            .total_due_principal()
            .minus(&model.total_paid_principal());
        for rp in &mut model.repayment_periods {
            let out_prin = rp
                .due_principal()
                .minus(&rp.paid_principal)
                .negative_to_zero();
            if out_prin.is_greater_than(&total_due_paid_diff) {
                let delta = out_prin.minus(&total_due_paid_diff);
                rp.emi = rp.emi.minus(&delta);
                let min_emi = rp
                    .paid_interest
                    .plus(&rp.paid_principal)
                    .minus(&rp.total_credited_amount())
                    .negative_to_zero();
                if rp.emi.is_less_than(&min_emi) {
                    rp.emi = min_emi;
                }
            }
        }

        let last_unpaid_idx = model
            .repayment_periods
            .iter()
            .rposition(|rp| !rp.is_fully_paid())
            .or_else(|| {
                if model.total_paid_principal().is_zero() && !model.repayment_periods.is_empty() {
                    let last = model.repayment_periods.len() - 1;
                    if model.repayment_periods[last]
                        .outstanding_loan_balance()
                        .is_greater_than_zero()
                    {
                        Some(last)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        if let Some(idx) = last_unpaid_idx {
            let zero = model.zero();
            model.repayment_periods[idx].future_unrecognized_interest = zero.clone();
            for rp in &mut model.repayment_periods {
                rp.is_interest_moved_upward = false;
            }

            let mut total_due_interest = zero.clone();
            let mut total_emi = zero.clone();
            let mut total_disbursed = zero.clone();
            let mut total_cap_inc = zero;

            for rp in &model.repayment_periods {
                total_due_interest = total_due_interest.plus(&rp.due_interest());
                total_emi = total_emi
                    .plus(&rp.emi_plus_credited_amounts_plus_future_unrecognized_interest());
                for ip in &rp.interest_periods {
                    total_disbursed = total_disbursed.plus(&ip.disbursement_amount);
                    total_cap_inc = total_cap_inc.plus(&ip.capitalized_income_principal);
                }
            }

            let diff = total_disbursed
                .plus(&total_cap_inc)
                .plus(&model.total_credited_principal())
                .plus(&total_due_interest)
                .minus(&total_emi);

            let adjusted_emi = model.repayment_periods[idx].emi.plus(&diff);
            model.repayment_periods[idx].emi = adjusted_emi;
        }
    }

    fn check_and_adjust_emi_if_needed(
        &self,
        model: &mut ProgressiveLoanInterestScheduleModel,
        start_idx: usize,
    ) {
        let mut adjust_counter = 1;
        let mut new_model: Option<ProgressiveLoanInterestScheduleModel> = None;

        while adjust_counter <= 3 {
            let (orig_emi, emi_diff, should_adjust, adjusted_emi) =
                self.compute_emi_adjustment(&model.repayment_periods[start_idx..], model.mc);
            if !should_adjust {
                break;
            }
            let adjusted_val = match model.installment_amount_in_multiples_of {
                Some(mult) if mult > 0 => Money::round_to_multiples_of(&adjusted_emi, mult),
                _ => adjusted_emi,
            };
            if adjusted_val == orig_emi {
                break;
            }
            let candidate = new_model.get_or_insert_with(|| model.clone());
            for rp in &mut candidate.repayment_periods[start_idx..] {
                if !adjusted_val.is_less_than(&rp.total_paid_amount()) {
                    rp.emi = adjusted_val.clone();
                    rp.original_emi = adjusted_val.clone();
                }
            }
            self.calculate_outstanding_balance(candidate);
            self.calculate_last_unpaid_repayment_period_emi(candidate);

            let (_, new_diff, _, _) =
                self.compute_emi_adjustment(&candidate.repayment_periods[start_idx..], model.mc);
            if new_diff.amount.abs() >= emi_diff.amount.abs() {
                break;
            }

            for i in start_idx..model.repayment_periods.len() {
                let e = candidate.repayment_periods[i].emi.clone();
                model.repayment_periods[i].emi = e.clone();
                model.repayment_periods[i].original_emi = e;
            }
            self.calculate_outstanding_balance(model);
            adjust_counter += 1;
        }
    }

    fn compute_emi_adjustment(
        &self,
        related_periods: &[RepaymentPeriod],
        mc: MathContext,
    ) -> (Money, Money, bool, Money) {
        let n = related_periods.len();
        for idx in (1..n).rev() {
            let last = &related_periods[idx];
            let penult = &related_periods[idx - 1];
            if !last.is_fully_paid() && !penult.is_fully_paid() {
                let orig_emi = penult.emi.clone();
                let diff = last.emi.minus(&penult.emi);
                let uncountable = related_periods
                    .iter()
                    .filter(|rp| orig_emi.is_less_than(&rp.total_paid_amount()))
                    .count();
                let lower_half = ((n as f64) / 2.0).floor();
                let lower_half_money =
                    Money::of(orig_emi.currency.clone(), BigDec::from_f64(lower_half), mc);
                let diff_x100 = Money::of(
                    orig_emi.currency.clone(),
                    diff.amount.abs().mul(BigDec::HUNDRED, Some(mc)),
                    mc,
                );
                let should_adjust = lower_half > 0.0
                    && !diff.is_zero()
                    && diff_x100.is_greater_than(&lower_half_money);
                let denom = (n.saturating_sub(uncountable)).max(1) as i64;
                let adj = Money::of(
                    orig_emi.currency.clone(),
                    diff.amount.div(BigDec::from_i64(denom), mc),
                    mc,
                );
                let adjusted_emi = orig_emi.plus(&adj);
                return (orig_emi, diff, should_adjust, adjusted_emi);
            }
        }
        let first_emi = related_periods[0].emi.clone();
        let zero = related_periods[0].zero();
        (first_emi.clone(), zero, false, first_emi)
    }

    pub fn calculate_rate_factor_per_period(
        &self,
        model: &ProgressiveLoanInterestScheduleModel,
        rp_from: NaiveDate,
        rp_due: NaiveDate,
        ip_from: NaiveDate,
        ip_due: NaiveDate,
    ) -> BigDec {
        let mc = model.mc;
        let detail = &model.loan_product_related_detail;
        let interest_rate = model
            .get_interest_rate(ip_from)
            .div(BigDec::HUNDRED, mc);
        let days_in_year = BigDec::from_i64(detail.days_in_year.number_of_days(ip_from) as i64);
        let actual_days = BigDec::from_i64(days_between(ip_from, ip_due));
        let calc_days = BigDec::from_i64(days_between(rp_from, rp_due));
        let repay_every = BigDec::from_i64(detail.repay_every as i64);
        let days_in_month = match detail.days_in_month {
            DaysInMonthType::Days30 => BigDec::from_i64(30),
            DaysInMonthType::Actual => calc_days,
        };

        match detail.days_in_month {
            DaysInMonthType::Actual => self.rate_factor_by_repayment_period(
                interest_rate,
                actual_days,
                BigDec::ONE,
                days_in_year,
                BigDec::ONE,
                BigDec::ONE,
                mc,
            ),
            DaysInMonthType::Days30 => self.rate_factor_by_repayment_every_month(
                interest_rate,
                repay_every,
                days_in_month,
                days_in_year,
                actual_days,
                calc_days,
                mc,
            ),
        }
    }

    pub fn calculate_rate_factor_per_period_for_interest(
        &self,
        model: &ProgressiveLoanInterestScheduleModel,
        rp_from: NaiveDate,
        rp_due: NaiveDate,
        ip_from: NaiveDate,
        ip_due: NaiveDate,
    ) -> BigDec {
        // For standard monthly/weekly/daily periods where periodRatio == repaymentEvery
        self.calculate_rate_factor_per_period(model, rp_from, rp_due, ip_from, ip_due)
    }

    pub fn rate_factor_by_repayment_every_month(
        &self,
        interest_rate: BigDec,
        repayment_every: BigDec,
        days_in_month: BigDec,
        days_in_year: BigDec,
        actual_days_in_period: BigDec,
        calculated_days_in_period: BigDec,
        mc: MathContext,
    ) -> BigDec {
        self.rate_factor_by_repayment_period(
            interest_rate,
            days_in_month,
            repayment_every,
            days_in_year,
            actual_days_in_period,
            calculated_days_in_period,
            mc,
        )
    }

    pub fn rate_factor_by_repayment_period(
        &self,
        interest_rate: BigDec,
        repayment_period_multiplier_in_days: BigDec,
        repayment_every: BigDec,
        days_in_year: BigDec,
        actual_days_in_period: BigDec,
        calculated_days_in_period: BigDec,
        mc: MathContext,
    ) -> BigDec {
        if calculated_days_in_period.is_zero() {
            return BigDec::ZERO;
        }
        let interest_fraction_per_period = repayment_period_multiplier_in_days
            .mul(repayment_every, Some(mc))
            .div(days_in_year, mc);
        interest_rate
            .mul(interest_fraction_per_period, Some(mc))
            .mul(actual_days_in_period, Some(mc))
            .div(calculated_days_in_period, mc)
            .set_scale(mc.precision as i32, mc.rounding_mode)
    }

    /// `fn = 1 + previousFnValue * currentRateFactor`
    pub fn fn_value(
        &self,
        previous_fn_value: BigDec,
        current_rate_factor: BigDec,
        mc: MathContext,
    ) -> BigDec {
        BigDec::ONE.add(previous_fn_value.mul(current_rate_factor, Some(mc)), Some(mc))
    }
}

// ============================================================================
// 6. Embeddable Progressive Loan Schedule Generator
// ============================================================================

/// Rust migration of `org.apache.fineract.portfolio.loanaccount.loanschedule.domain.EmbeddableProgressiveLoanScheduleGenerator`.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddableProgressiveLoanScheduleGenerator {
    pub date_generator: DefaultScheduledDateGenerator,
    pub emi_calculator: ProgressiveEMICalculator,
}

impl EmbeddableProgressiveLoanScheduleGenerator {
    pub fn new() -> Self {
        let date_generator = DefaultScheduledDateGenerator::new();
        let emi_calculator = ProgressiveEMICalculator::new(date_generator);
        Self {
            date_generator,
            emi_calculator,
        }
    }

    pub fn generate(
        &self,
        mc: MathContext,
        model_data: &LoanRepaymentScheduleModelData,
    ) -> LoanSchedulePlan {
        let freq_type = PeriodFrequencyType::from_str_name(&model_data.repayment_frequency_type);
        let expected_period_dates = self.date_generator.generate_repayment_period_dates(
            model_data.schedule_generation_start_date,
            model_data.number_of_repayments,
            model_data.repayment_frequency,
            freq_type,
            model_data.fixed_length,
        );

        let detail = LoanProductRelatedDetail {
            currency: model_data.currency.clone(),
            annual_nominal_interest_rate: model_data.annual_nominal_interest_rate,
            days_in_month: model_data.days_in_month,
            days_in_year: model_data.days_in_year,
            days_in_year_custom_strategy: model_data.days_in_year_custom_strategy,
            repayment_frequency_type: freq_type,
            repay_every: model_data.repayment_frequency,
            number_of_repayments: model_data.number_of_repayments,
            interest_method: model_data.interest_method,
            interest_recognition_on_disbursement_date: model_data
                .interest_recognition_on_disbursement_date,
            allow_partial_period_interest_calculation: model_data
                .allow_partial_period_interest_calculation,
            allow_full_term_for_tranche: model_data.allow_full_term_for_tranche,
        };

        let mut schedule_model = self.emi_calculator.generate_period_interest_schedule_model(
            &expected_period_dates,
            detail,
            model_data.installment_amount_in_multiples_of,
            mc,
        );

        let disbursed_money = Money::of(
            model_data.currency.clone(),
            model_data.disbursement_amount,
            mc,
        );

        let mut down_payment_money = Money::zero(model_data.currency.clone(), mc);
        if model_data.down_payment_enabled {
            let raw_dp = disbursed_money
                .amount
                .mul(model_data.down_payment_percentage, Some(mc))
                .div(BigDec::HUNDRED, mc);
            down_payment_money = Money::of(model_data.currency.clone(), raw_dp, mc);
            if let Some(mult) = model_data.installment_amount_in_multiples_of {
                down_payment_money = Money::round_to_multiples_of(&down_payment_money, mult);
            }
        }

        let remaining_disbursement = disbursed_money.minus(&down_payment_money);
        self.emi_calculator.add_disbursement(
            &mut schedule_model,
            model_data.disbursement_date,
            remaining_disbursement,
        );

        // Compute totals
        let mut total_cumulative_principal = down_payment_money.clone();
        let mut total_cumulative_interest = Money::zero(model_data.currency.clone(), mc);
        for rp in &schedule_model.repayment_periods {
            total_cumulative_principal = total_cumulative_principal.plus(&rp.due_principal());
            total_cumulative_interest = total_cumulative_interest.plus(&rp.due_interest());
        }
        let total_repayment_expected =
            total_cumulative_principal.plus(&total_cumulative_interest);
        let mut remaining_total_outstanding = total_repayment_expected.amount;

        let mut periods = Vec::with_capacity(expected_period_dates.len() + 2);
        let mut instalment_num = 1usize;

        for (from_date, due_date) in &expected_period_dates {
            if model_data.disbursement_date >= *from_date
                && model_data.disbursement_date < *due_date
            {
                periods.push(LoanSchedulePlanPeriod::Disbursement(
                    LoanSchedulePlanDisbursementPeriod {
                        period_from_date: model_data.disbursement_date,
                        period_due_date: model_data.disbursement_date,
                        principal_amount: disbursed_money.amount,
                        outstanding_loan_balance: disbursed_money.amount,
                    },
                ));

                if model_data.down_payment_enabled {
                    remaining_total_outstanding =
                        remaining_total_outstanding.sub(down_payment_money.amount, None);
                    let bal_after_dp = disbursed_money.minus(&down_payment_money).amount;
                    periods.push(LoanSchedulePlanPeriod::DownPayment(
                        LoanSchedulePlanDownPaymentPeriod {
                            period_number: instalment_num,
                            period_from_date: model_data.disbursement_date,
                            period_due_date: model_data.disbursement_date,
                            principal_amount: down_payment_money.amount,
                            total_due_amount: down_payment_money.amount,
                            outstanding_loan_balance: bal_after_dp,
                            total_outstanding_loan_balance: remaining_total_outstanding,
                        },
                    ));
                    instalment_num += 1;
                }
            }

            if let Some(rp) = schedule_model
                .find_repayment_period_by_from_and_due_date(*from_date, *due_date)
            {
                let principal_due = rp.due_principal();
                let interest_due = rp.due_interest();
                let total_due = principal_due.plus(&interest_due);
                let zero_amt = Money::zero(model_data.currency.clone(), mc).amount;
                remaining_total_outstanding =
                    remaining_total_outstanding.sub(total_due.amount, None);

                periods.push(LoanSchedulePlanPeriod::Repayment(
                    LoanSchedulePlanRepaymentPeriod {
                        period_number: instalment_num,
                        period_from_date: *from_date,
                        period_due_date: *due_date,
                        principal_amount: principal_due.amount,
                        interest_amount: interest_due.amount,
                        fee_amount: zero_amt,
                        penalty_amount: zero_amt,
                        total_due_amount: total_due.amount,
                        outstanding_loan_balance: rp.outstanding_loan_balance().amount,
                        total_outstanding_loan_balance: remaining_total_outstanding,
                    },
                ));
                instalment_num += 1;
            }
        }

        let zero_amt = Money::zero(model_data.currency.clone(), mc).amount;
        LoanSchedulePlan {
            periods,
            currency: model_data.currency.clone(),
            loan_term_in_days: schedule_model.loan_term_in_days(),
            total_disbursed_amount: disbursed_money.amount,
            total_principal_amount: total_cumulative_principal.amount,
            total_interest_amount: total_cumulative_interest.amount,
            total_fee_amount: zero_amt,
            total_penalty_amount: zero_amt,
            total_repayment_amount: total_repayment_expected.amount,
            total_outstanding_amount: BigDec::ZERO,
        }
    }
}

// ============================================================================
// 7. Unit Tests (Migrated from EmbeddableProgressiveLoanScheduleGeneratorTest
//    and ProgressiveEMICalculatorTest)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_embeddable_progressive_loan_schedule_generator() {
        let mc = MathContext::new(12, RoundingMode::HalfUp);
        let calculator = EmbeddableProgressiveLoanScheduleGenerator::new();

        let currency = CurrencyData::new("usd", "US Dollar", 2, None, "usd", "$");
        let start_date = ymd(2024, 1, 1);
        let disbursement_date = ymd(2024, 1, 1);
        let disbursed_amount = BigDec::from_i64(100);

        let config = LoanRepaymentScheduleModelData {
            schedule_generation_start_date: start_date,
            currency,
            disbursement_amount: disbursed_amount,
            disbursement_date,
            number_of_repayments: 6,
            repayment_frequency: 1,
            repayment_frequency_type: "MONTHS".to_string(),
            annual_nominal_interest_rate: BigDec::parse("7.0").unwrap(),
            down_payment_enabled: false,
            days_in_month: DaysInMonthType::Days30,
            days_in_year: DaysInYearType::Days360,
            down_payment_percentage: BigDec::ZERO,
            installment_amount_in_multiples_of: None,
            fixed_length: None,
            interest_recognition_on_disbursement_date: false,
            days_in_year_custom_strategy: None,
            interest_method: InterestMethod::DecliningBalance,
            allow_partial_period_interest_calculation: true,
            allow_full_term_for_tranche: false,
        };

        let plan = calculator.generate(mc, &config);

        assert_eq!(182, plan.loan_term_in_days);
        assert_eq!("100.00", plan.total_disbursed_amount.to_string());
        assert_eq!("2.05", plan.total_interest_amount.to_string());
        assert_eq!("102.05", plan.total_repayment_amount.to_string());

        assert_eq!(7, plan.periods.len());
        check_disbursement_period(&plan.periods[0], ymd(2024, 1, 1), ymd(2024, 1, 1), 100.0, 100.0);
        check_repayment_period(
            &plan.periods[1],
            1,
            ymd(2024, 1, 1),
            ymd(2024, 2, 1),
            16.43,
            0.58,
            0.0,
            0.0,
            17.01,
            83.57,
            85.04,
        );
        check_repayment_period(
            &plan.periods[2],
            2,
            ymd(2024, 2, 1),
            ymd(2024, 3, 1),
            16.52,
            0.49,
            0.0,
            0.0,
            17.01,
            67.05,
            68.03,
        );
        check_repayment_period(
            &plan.periods[3],
            3,
            ymd(2024, 3, 1),
            ymd(2024, 4, 1),
            16.62,
            0.39,
            0.0,
            0.0,
            17.01,
            50.43,
            51.02,
        );
        check_repayment_period(
            &plan.periods[4],
            4,
            ymd(2024, 4, 1),
            ymd(2024, 5, 1),
            16.72,
            0.29,
            0.0,
            0.0,
            17.01,
            33.71,
            34.01,
        );
        check_repayment_period(
            &plan.periods[5],
            5,
            ymd(2024, 5, 1),
            ymd(2024, 6, 1),
            16.81,
            0.20,
            0.0,
            0.0,
            17.01,
            16.90,
            17.00,
        );
        check_repayment_period(
            &plan.periods[6],
            6,
            ymd(2024, 6, 1),
            ymd(2024, 7, 1),
            16.90,
            0.10,
            0.0,
            0.0,
            17.00,
            0.0,
            0.0,
        );
    }

    fn check_disbursement_period(
        period: &LoanSchedulePlanPeriod,
        from_date: NaiveDate,
        due_date: NaiveDate,
        principal: f64,
        outstanding_balance: f64,
    ) {
        assert_eq!(None, period.period_number());
        assert_eq!(from_date, period.period_from_date());
        assert_eq!(due_date, period.period_due_date());
        match period {
            LoanSchedulePlanPeriod::Disbursement(dp) => {
                assert!((dp.principal_amount.to_f64() - principal).abs() < 1e-6);
                assert!((dp.outstanding_loan_balance.to_f64() - outstanding_balance).abs() < 1e-6);
            }
            other => panic!("Expected DisbursementPeriod, got {:?}", other),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_repayment_period(
        period: &LoanSchedulePlanPeriod,
        period_number: usize,
        from_date: NaiveDate,
        due_date: NaiveDate,
        principal: f64,
        interest: f64,
        fee: f64,
        penalty: f64,
        total_due: f64,
        outstanding_balance: f64,
        total_outstanding_balance: f64,
    ) {
        assert_eq!(Some(period_number), period.period_number());
        assert_eq!(from_date, period.period_from_date());
        assert_eq!(due_date, period.period_due_date());
        match period {
            LoanSchedulePlanPeriod::Repayment(rp) => {
                assert!((rp.principal_amount.to_f64() - principal).abs() < 1e-6);
                assert!((rp.interest_amount.to_f64() - interest).abs() < 1e-6);
                assert!((rp.fee_amount.to_f64() - fee).abs() < 1e-6);
                assert!((rp.penalty_amount.to_f64() - penalty).abs() < 1e-6);
                assert!((rp.total_due_amount.to_f64() - total_due).abs() < 1e-6);
                assert!((rp.outstanding_loan_balance.to_f64() - outstanding_balance).abs() < 1e-6);
                assert!(
                    (rp.total_outstanding_loan_balance.to_f64() - total_outstanding_balance).abs()
                        < 1e-6
                );
            }
            other => panic!("Expected RepaymentPeriod, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // ProgressiveEMICalculatorTest ports
    // ------------------------------------------------------------------------

    fn six_month_periods() -> Vec<(NaiveDate, NaiveDate)> {
        vec![
            (ymd(2024, 1, 1), ymd(2024, 2, 1)),
            (ymd(2024, 2, 1), ymd(2024, 3, 1)),
            (ymd(2024, 3, 1), ymd(2024, 4, 1)),
            (ymd(2024, 4, 1), ymd(2024, 5, 1)),
            (ymd(2024, 5, 1), ymd(2024, 6, 1)),
            (ymd(2024, 6, 1), ymd(2024, 7, 1)),
        ]
    }

    fn get_rate_factor_by_month(
        emi_calc: &ProgressiveEMICalculator,
        days_in_year_type: DaysInYearType,
        days_in_month_type: DaysInMonthType,
        interest_rate: BigDec,
        from: NaiveDate,
        due: NaiveDate,
        mc: MathContext,
    ) -> BigDec {
        let days_in_period = BigDec::from_i64(days_between(from, due));
        let days_in_year = BigDec::from_i64(days_in_year_type.number_of_days(from) as i64);
        let days_in_month = BigDec::from_i64(days_in_month_type.number_of_days(from) as i64);
        emi_calc.rate_factor_by_repayment_every_month(
            interest_rate,
            BigDec::ONE,
            days_in_month,
            days_in_year,
            days_in_period,
            days_in_period,
            mc,
        )
    }

    #[test]
    fn test_rate_factor_by_repayment_every_month_day_in_year_365_days_in_month_actual() {
        let mc = MathContext::new(12, RoundingMode::HalfEven);
        let emi_calc = ProgressiveEMICalculator::default();
        let interest_rate = BigDec::parse("0.094822").unwrap();
        let expected = [
            "0.008053375342",
            "0.007533802740",
            "0.008053375342",
            "0.007793589041",
            "0.008053375342",
            "0.007793589041",
        ];

        for (idx, (from, due)) in six_month_periods().into_iter().enumerate() {
            let rf = get_rate_factor_by_month(
                &emi_calc,
                DaysInYearType::Days365,
                DaysInMonthType::Actual,
                interest_rate,
                from,
                due,
                mc,
            );
            assert_eq!(expected[idx], rf.to_string());
        }
    }

    #[test]
    fn test_rate_factor_by_repayment_every_month_day_in_year_actual_days_in_month_actual() {
        let mc = MathContext::new(12, RoundingMode::HalfEven);
        let emi_calc = ProgressiveEMICalculator::default();
        let interest_rate = BigDec::parse("0.094822").unwrap();
        let expected = [
            "0.008031371585",
            "0.007513218579",
            "0.008031371585",
            "0.007772295082",
            "0.008031371585",
            "0.007772295082",
        ];

        for (idx, (from, due)) in six_month_periods().into_iter().enumerate() {
            let rf = get_rate_factor_by_month(
                &emi_calc,
                DaysInYearType::Actual,
                DaysInMonthType::Actual,
                interest_rate,
                from,
                due,
                mc,
            );
            assert_eq!(expected[idx], rf.to_string());
        }
    }

    #[test]
    fn test_fn_value_function_repay_every_1_month_day_in_year_365_days_in_month_actual() {
        let mc = MathContext::new(12, RoundingMode::HalfEven);
        let emi_calc = ProgressiveEMICalculator::default();
        let interest_rate = BigDec::parse("0.094822").unwrap();
        let expected = [
            "1.00000000000",
            "2.00753380274",
            "3.02370122596",
            "4.04726671069",
            "5.07986086861",
            "6.11945121660",
        ];

        let mut prev_fn = BigDec::ZERO;
        for (idx, (from, due)) in six_month_periods().into_iter().enumerate() {
            let rf_plus_1 = get_rate_factor_by_month(
                &emi_calc,
                DaysInYearType::Days365,
                DaysInMonthType::Actual,
                interest_rate,
                from,
                due,
                mc,
            )
            .add(BigDec::ONE, Some(mc));
            let current_fn = emi_calc.fn_value(prev_fn, rf_plus_1, mc);
            assert_eq!(expected[idx], current_fn.to_string());
            prev_fn = current_fn;
        }
    }

    #[test]
    fn test_disbursed_amt_100_day_in_years_360_days_in_month_30_repay_every_1_month() {
        let mc = MathContext::new(12, RoundingMode::HalfEven);
        let emi_calc = ProgressiveEMICalculator::default();
        let currency = CurrencyData::new("USD", "USD", 2, Some(1), "USD", "$");
        let detail = LoanProductRelatedDetail {
            currency: currency.clone(),
            annual_nominal_interest_rate: BigDec::parse("9.4822").unwrap(),
            days_in_month: DaysInMonthType::Days30,
            days_in_year: DaysInYearType::Days360,
            days_in_year_custom_strategy: None,
            repayment_frequency_type: PeriodFrequencyType::Months,
            repay_every: 1,
            number_of_repayments: 6,
            interest_method: InterestMethod::DecliningBalance,
            interest_recognition_on_disbursement_date: false,
            allow_partial_period_interest_calculation: true,
            allow_full_term_for_tranche: false,
        };

        let mut schedule = emi_calc.generate_period_interest_schedule_model(
            &six_month_periods(),
            detail,
            None,
            mc,
        );
        emi_calc.add_disbursement(
            &mut schedule,
            ymd(2024, 1, 1),
            Money::of(currency.clone(), BigDec::from_i64(100), mc),
        );

        assert_model_period(&schedule, 0, 17.13, 0.79, 16.34, 83.66);
        assert_model_period(&schedule, 1, 17.13, 0.66, 16.47, 67.19);
        assert_model_period(&schedule, 2, 17.13, 0.53, 16.60, 50.59);
        assert_model_period(&schedule, 3, 17.13, 0.40, 16.73, 33.86);
        assert_model_period(&schedule, 4, 17.13, 0.27, 16.86, 17.00);
        assert_model_period(&schedule, 5, 17.13, 0.13, 17.00, 0.00);

        // Second disbursement of 100 on 2024-03-01 (test_multi_disbursedAmt200_2ndOnDueDate)
        emi_calc.add_disbursement(
            &mut schedule,
            ymd(2024, 3, 1),
            Money::of(currency, BigDec::from_i64(100), mc),
        );
        assert_model_period(&schedule, 0, 17.13, 0.79, 16.34, 83.66);
        assert_model_period(&schedule, 1, 17.13, 0.66, 16.47, 167.19);
        assert_model_period(&schedule, 2, 42.63, 1.32, 41.31, 125.88);
        assert_model_period(&schedule, 3, 42.63, 0.99, 41.64, 84.24);
        assert_model_period(&schedule, 4, 42.63, 0.67, 41.96, 42.28);
        assert_model_period(&schedule, 5, 42.61, 0.33, 42.28, 0.00);
    }

    #[test]
    fn test_reschedule_interest_on_0201_4per_disbursed_amt_100() {
        let mc = MathContext::new(12, RoundingMode::HalfEven);
        let emi_calc = ProgressiveEMICalculator::default();
        let currency = CurrencyData::new("USD", "USD", 2, Some(1), "USD", "$");
        let detail = LoanProductRelatedDetail {
            currency: currency.clone(),
            annual_nominal_interest_rate: BigDec::parse("7.0").unwrap(),
            days_in_month: DaysInMonthType::Days30,
            days_in_year: DaysInYearType::Days360,
            days_in_year_custom_strategy: None,
            repayment_frequency_type: PeriodFrequencyType::Months,
            repay_every: 1,
            number_of_repayments: 6,
            interest_method: InterestMethod::DecliningBalance,
            interest_recognition_on_disbursement_date: false,
            allow_partial_period_interest_calculation: true,
            allow_full_term_for_tranche: false,
        };

        let mut schedule = emi_calc.generate_period_interest_schedule_model(
            &six_month_periods(),
            detail,
            None,
            mc,
        );
        emi_calc.add_disbursement(
            &mut schedule,
            ymd(2024, 1, 1),
            Money::of(currency, BigDec::from_i64(100), mc),
        );
        emi_calc.change_interest_rate(&mut schedule, ymd(2024, 2, 2), BigDec::parse("4.0").unwrap());

        assert_model_period(&schedule, 0, 17.01, 0.58, 16.43, 83.57);
        assert_model_period(&schedule, 1, 16.88, 0.28, 16.60, 66.97);
        assert_model_period(&schedule, 2, 16.88, 0.22, 16.66, 50.31);
        assert_model_period(&schedule, 3, 16.88, 0.17, 16.71, 33.60);
        assert_model_period(&schedule, 4, 16.88, 0.11, 16.77, 16.83);
        assert_model_period(&schedule, 5, 16.89, 0.06, 16.83, 0.00);
    }

    fn assert_model_period(
        schedule: &ProgressiveLoanInterestScheduleModel,
        rp_idx: usize,
        emi: f64,
        due_interest: f64,
        due_principal: f64,
        outstanding_balance: f64,
    ) {
        let rp = &schedule.repayment_periods[rp_idx];
        assert!(
            (rp.emi.amount.to_f64() - emi).abs() < 1e-6,
            "Period {} EMI mismatch: expected {}, got {}",
            rp_idx,
            emi,
            rp.emi.amount
        );
        assert!(
            (rp.due_interest().amount.to_f64() - due_interest).abs() < 1e-6,
            "Period {} due_interest mismatch: expected {}, got {}",
            rp_idx,
            due_interest,
            rp.due_interest().amount
        );
        assert!(
            (rp.due_principal().amount.to_f64() - due_principal).abs() < 1e-6,
            "Period {} due_principal mismatch: expected {}, got {}",
            rp_idx,
            due_principal,
            rp.due_principal().amount
        );
        assert!(
            (rp.outstanding_loan_balance().amount.to_f64() - outstanding_balance).abs() < 1e-6,
            "Period {} outstanding_balance mismatch: expected {}, got {}",
            rp_idx,
            outstanding_balance,
            rp.outstanding_loan_balance().amount
        );
    }
}
