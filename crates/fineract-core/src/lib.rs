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

//! `fineract-core` — Rust port of Apache Fineract's `fineract-core` module.
//!
//! Provides:
//! - [`MonetaryCurrency`] and [`CoreMoney`] (`MoneyTest` parity)
//! - [`BusinessDateType`], [`FineractPlatformTenant`], and [`BackdatedTransactionValidationService`]
//! - [`BatchRequest`], [`BatchResponse`], [`BatchRequestNode`], and [`ResolutionHelper`] (`ResolutionHelperTest` parity)
//! - [`LoopGuard`] (`LoopGuardTest` parity)

use chrono::{Duration, NaiveDate};
use rust_decimal::prelude::{FromPrimitive, RoundingStrategy};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CoreError {
    #[error("Batch reference {reference} on request {request_id} is invalid")]
    BatchReferenceInvalid { request_id: i64, reference: i64 },
    #[error("General platform domain rule violation [{code}]: {message}")]
    DomainRuleViolation { code: String, message: String },
    #[error("Money total requires at least one Money element")]
    EmptyMoneyTotal,
    #[error("Currency mismatch: {0} vs {1}")]
    CurrencyMismatch(String, String),
    #[error("Loop exceeded {0} iterations")]
    LoopExceeded(i32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonetaryCurrency {
    pub code: String,
    pub digits_after_decimal: u32,
    pub in_multiples_of: Option<i32>,
}

impl MonetaryCurrency {
    pub fn new(
        code: impl Into<String>,
        digits_after_decimal: u32,
        in_multiples_of: Option<i32>,
    ) -> Self {
        Self {
            code: code.into(),
            digits_after_decimal,
            in_multiples_of,
        }
    }
}

/// Port of Fineract's `org.apache.fineract.organisation.monetary.domain.Money`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreMoney {
    currency: MonetaryCurrency,
    amount: Decimal,
}

impl CoreMoney {
    pub fn of(currency: MonetaryCurrency, amount: Decimal) -> Self {
        let rounded = amount.round_dp_with_strategy(
            currency.digits_after_decimal,
            RoundingStrategy::MidpointNearestEven,
        );
        Self {
            currency,
            amount: rounded,
        }
    }

    pub fn zero(currency: MonetaryCurrency) -> Self {
        Self::of(currency, Decimal::ZERO)
    }

    pub fn amount(&self) -> Decimal {
        self.amount
    }

    pub fn currency(&self) -> &MonetaryCurrency {
        &self.currency
    }

    pub fn plus(&self, other: Option<&CoreMoney>) -> Self {
        match other {
            Some(m) => Self::of(self.currency.clone(), self.amount + m.amount),
            None => self.clone(),
        }
    }

    pub fn add(&self, other: Option<&CoreMoney>) -> Self {
        self.plus(other)
    }

    pub fn minus(&self, other: Option<&CoreMoney>) -> Self {
        match other {
            Some(m) => Self::of(self.currency.clone(), self.amount - m.amount),
            None => self.clone(),
        }
    }

    pub fn plus_f64(&self, amount_to_add: f64) -> Self {
        // Parse via formatted decimal representation to avoid IEEE-754 binary noise (e.g. 0.10000000000000001 -> 0.1)
        let formatted = format!("{amount_to_add:.12}");
        let delta = formatted.parse::<Decimal>().unwrap_or_else(|_| {
            Decimal::from_f64(amount_to_add).unwrap_or(Decimal::ZERO)
        });
        Self::of(self.currency.clone(), self.amount + delta)
    }

    pub fn plus_iterable<'a, I>(&self, monies: I) -> Self
    where
        I: IntoIterator<Item = Option<&'a CoreMoney>>,
    {
        let mut sum = self.amount;
        for item in monies.into_iter().flatten() {
            sum += item.amount;
        }
        Self::of(self.currency.clone(), sum)
    }

    pub fn total(monies: &[CoreMoney]) -> Result<Self, CoreError> {
        let Some(first) = monies.first() else {
            return Err(CoreError::EmptyMoneyTotal);
        };
        let mut sum = Decimal::ZERO;
        for m in monies {
            if m.currency.code != first.currency.code {
                return Err(CoreError::CurrencyMismatch(
                    first.currency.code.clone(),
                    m.currency.code.clone(),
                ));
            }
            sum += m.amount;
        }
        Ok(Self::of(first.currency.clone(), sum))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BusinessDateType {
    BusinessDate,
    CobDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FineractPlatformTenant {
    pub id: i64,
    pub tenant_identifier: String,
    pub name: String,
    pub timezone_id: String,
}

/// Port of Fineract's `BackdatedTransactionValidationService`.
#[derive(Debug, Clone)]
pub struct BackdatedTransactionValidationService {
    pub is_backdated_transactions_disallowed: bool,
    pub tolerance_days: i64,
    pub business_date: NaiveDate,
}

impl BackdatedTransactionValidationService {
    pub fn new(
        is_backdated_transactions_disallowed: bool,
        tolerance_days: i64,
        business_date: NaiveDate,
    ) -> Self {
        Self {
            is_backdated_transactions_disallowed,
            tolerance_days,
            business_date,
        }
    }

    pub fn validate_transaction_date(
        &self,
        transaction_date: Option<NaiveDate>,
    ) -> Result<(), CoreError> {
        let Some(tx_date) = transaction_date else {
            return Ok(());
        };
        if !self.is_backdated_transactions_disallowed {
            return Ok(());
        }
        let earliest_allowed = self.business_date - Duration::days(self.tolerance_days.max(0));
        if tx_date < earliest_allowed {
            return Err(CoreError::DomainRuleViolation {
                code: "error.msg.transaction.date.cannot.be.earlier.than.allowed.backdated.window"
                    .to_string(),
                message: format!(
                    "Transaction date {tx_date} cannot be earlier than {earliest_allowed}"
                ),
            });
        }
        Ok(())
    }
}

/// Port of Fineract's `BatchRequest`, `BatchResponse`, and `ResolutionHelper`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub request_id: i64,
    pub relative_url: String,
    pub method: String,
    pub reference: Option<i64>,
    pub body: Option<String>,
}

impl BatchRequest {
    pub fn new(
        request_id: i64,
        relative_url: impl Into<String>,
        reference: Option<i64>,
        body: Option<String>,
    ) -> Self {
        Self {
            request_id,
            relative_url: relative_url.into(),
            method: "POST".to_string(),
            reference,
            body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResponse {
    pub request_id: i64,
    pub status_code: u16,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRequestNode {
    pub request: BatchRequest,
    pub child_nodes: Vec<BatchRequestNode>,
}

#[derive(Debug, Default, Clone)]
pub struct ResolutionHelper;

impl ResolutionHelper {
    pub fn build_nodes_tree(
        &self,
        requests: &[BatchRequest],
    ) -> Result<Vec<BatchRequestNode>, CoreError> {
        let mut by_id: HashMap<i64, BatchRequest> = HashMap::new();
        for req in requests {
            by_id.insert(req.request_id, req.clone());
        }
        for req in requests {
            if let Some(parent_id) = req.reference {
                if !by_id.contains_key(&parent_id) {
                    return Err(CoreError::BatchReferenceInvalid {
                        request_id: req.request_id,
                        reference: parent_id,
                    });
                }
            }
        }

        fn attach_children(parent_id: i64, all: &[BatchRequest]) -> Vec<BatchRequestNode> {
            all.iter()
                .filter(|r| r.reference == Some(parent_id))
                .map(|r| BatchRequestNode {
                    request: r.clone(),
                    child_nodes: attach_children(r.request_id, all),
                })
                .collect()
        }

        let roots = requests
            .iter()
            .filter(|r| r.reference.is_none())
            .map(|r| BatchRequestNode {
                request: r.clone(),
                child_nodes: attach_children(r.request_id, requests),
            })
            .collect();
        Ok(roots)
    }

    pub fn resolve_request(
        &self,
        request: &BatchRequest,
        parent_response: &BatchResponse,
    ) -> Result<BatchRequest, CoreError> {
        let parent_json: Value = parent_response
            .body
            .as_deref()
            .and_then(|b| serde_json::from_str(b).ok())
            .unwrap_or(Value::Null);

        let mut resolved_url = request.relative_url.clone();
        // Substitute $.key tokens in relative_url
        if let Some(idx) = resolved_url.find("$.") {
            let after = &resolved_url[idx + 2..];
            let end_offset = after
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            let key = &after[..end_offset];
            if let Some(val) = parent_json.get(key) {
                let replacement = match val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                resolved_url.replace_range(idx..idx + 2 + end_offset, &replacement);
            }
        }

        let resolved_body = if let Some(ref body_str) = request.body {
            if let Ok(mut body_val) = serde_json::from_str::<Value>(body_str) {
                Self::resolve_json_value(&mut body_val, &parent_json);
                Some(serde_json::to_string(&body_val).unwrap_or_else(|_| body_str.clone()))
            } else {
                Some(body_str.clone())
            }
        } else {
            None
        };

        let mut out = request.clone();
        out.relative_url = resolved_url;
        out.body = resolved_body;
        Ok(out)
    }

    fn resolve_json_value(val: &mut Value, parent: &Value) {
        match val {
            Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    Self::resolve_json_value(v, parent);
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::resolve_json_value(item, parent);
                }
            }
            Value::String(s) => {
                if let Some(path) = s.strip_prefix("$[ARRAYDATE].") {
                    if let Some(arr_val) = Self::eval_simple_path(parent, path) {
                        if let Some(arr) = arr_val.as_array() {
                            if arr.len() == 3 {
                                let y = arr[0].as_i64().unwrap_or(1970) as i32;
                                let m = arr[1].as_u64().unwrap_or(1) as u32;
                                let d = arr[2].as_u64().unwrap_or(1) as u32;
                                if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                                    *val = Value::String(date.format("%d %B %Y").to_string());
                                }
                            }
                        }
                    }
                } else if let Some(path) = s.strip_prefix("$.") {
                    if let Some(resolved) = Self::eval_simple_path(parent, path) {
                        *val = resolved.clone();
                    }
                }
            }
            _ => {}
        }
    }

    fn eval_simple_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = root;
        for segment in path.split('.') {
            if let Some((field, idx_str)) = segment.split_once('[') {
                let idx: usize = idx_str.trim_end_matches(']').parse().ok()?;
                current = current.get(field)?.get(idx)?;
            } else {
                current = current.get(segment)?;
            }
        }
        Some(current)
    }
}

/// Port of Fineract's `LoopGuard`.
pub struct LoopGuard;

impl LoopGuard {
    pub fn run_safe_do_while_loop<C, P, B>(
        max_iterations: i32,
        context: &mut C,
        condition: P,
        mut body: B,
    ) -> Result<(), CoreError>
    where
        P: Fn(&C) -> bool,
        B: FnMut(&mut C),
    {
        if max_iterations <= 0 {
            return Err(CoreError::LoopExceeded(max_iterations));
        }
        let mut iterations = 0;
        loop {
            if iterations >= max_iterations {
                return Err(CoreError::LoopExceeded(max_iterations));
            }
            body(context);
            iterations += 1;
            if !condition(context) {
                break;
            }
        }
        Ok(())
    }

    pub fn run_safe_while_loop<C, P, B>(
        max_iterations: i32,
        context: &mut C,
        condition: P,
        mut body: B,
    ) -> Result<(), CoreError>
    where
        P: Fn(&C) -> bool,
        B: FnMut(&mut C),
    {
        if max_iterations <= 0 {
            return Err(CoreError::LoopExceeded(max_iterations));
        }
        let mut iterations = 0;
        while condition(context) {
            if iterations >= max_iterations {
                return Err(CoreError::LoopExceeded(max_iterations));
            }
            body(context);
            iterations += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn money_operations_match_fineract_money_test() {
        let usd = MonetaryCurrency::new("USD", 2, None);
        let ten = CoreMoney::of(usd.clone(), dec!(10.00));
        let one = CoreMoney::of(usd.clone(), dec!(1.00));

        // plus with None in iterable
        let list = vec![Some(&one), None, Some(&one)];
        assert_eq!(ten.plus_iterable(list).amount(), dec!(12.00));

        // plus with empty iterable
        let empty: Vec<Option<&CoreMoney>> = vec![];
        assert_eq!(ten.plus_iterable(empty).amount(), dec!(10.00));

        // plus/minus/add with None
        assert_eq!(ten.plus(None).amount(), dec!(10.00));
        assert_eq!(ten.minus(None).amount(), dec!(10.00));
        assert_eq!(ten.add(None).amount(), dec!(10.00));

        // plus/minus with Some
        assert_eq!(ten.plus(Some(&one)).amount(), dec!(11.00));
        assert_eq!(ten.minus(Some(&one)).amount(), dec!(9.00));

        // plus_f64 including IEEE-754 0.10000000000000001
        assert_eq!(ten.plus_f64(0.10000000000000001).amount(), dec!(10.10));
        assert_eq!(ten.plus_f64(999999.99).amount(), dec!(1000009.99));
        assert_eq!(ten.plus_f64(-10.01).amount(), dec!(-0.01));

        // total varargs
        let total = CoreMoney::total(&[one.clone(), one.clone(), one]).unwrap();
        assert_eq!(total.amount(), dec!(3.00));
        assert_eq!(CoreMoney::total(&[]).unwrap_err(), CoreError::EmptyMoneyTotal);
    }

    #[test]
    fn backdated_transaction_validation_service_matches_java_tests() {
        let biz_date = NaiveDate::from_ymd_opt(2026, 7, 11).unwrap();
        // Disabled -> any date allowed
        let disabled = BackdatedTransactionValidationService::new(false, 0, biz_date);
        assert!(disabled
            .validate_transaction_date(NaiveDate::from_ymd_opt(2020, 1, 1))
            .is_ok());

        // Enabled with 0 tolerance -> current business date allowed, previous day rejected
        let strict = BackdatedTransactionValidationService::new(true, 0, biz_date);
        assert!(strict.validate_transaction_date(None).is_ok());
        assert!(strict.validate_transaction_date(Some(biz_date)).is_ok());
        assert!(strict
            .validate_transaction_date(NaiveDate::from_ymd_opt(2026, 7, 10))
            .is_err());

        // Enabled with 7-day tolerance -> 2026-07-04 allowed, 2026-07-03 rejected
        let window_7 = BackdatedTransactionValidationService::new(true, 7, biz_date);
        assert!(window_7
            .validate_transaction_date(NaiveDate::from_ymd_opt(2026, 7, 4))
            .is_ok());
        assert!(window_7
            .validate_transaction_date(NaiveDate::from_ymd_opt(2026, 7, 3))
            .is_err());
    }

    #[test]
    fn batch_resolution_helper_builds_tree_and_resolves_jsonpath_and_arraydate() {
        let helper = ResolutionHelper;
        let r1 = BatchRequest::new(
            1,
            "/resource/id",
            None,
            Some(r#"{"key1": "value1", "key2": 12}"#.to_string()),
        );
        let r2 = BatchRequest::new(
            2,
            "/resource/$.id?key=value",
            Some(1),
            Some(r#"{"nested":{"key1":"$.value1"},"startDate":"$[ARRAYDATE].dates[0]"}"#.to_string()),
        );
        let nodes = helper.build_nodes_tree(&[r1, r2.clone()]).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].child_nodes.len(), 1);
        assert_eq!(nodes[0].child_nodes[0].request.request_id, 2);

        // Invalid reference fails
        let bad = BatchRequest::new(2, "/resource/id", Some(99), None);
        assert_eq!(
            helper.build_nodes_tree(&[bad]).unwrap_err(),
            CoreError::BatchReferenceInvalid {
                request_id: 2,
                reference: 99
            }
        );

        // Resolve request parameters from parent response
        let parent_resp = BatchResponse {
            request_id: 1,
            status_code: 200,
            body: Some(r#"{"value1":"resolvedValue","id":"123","dates":[[2023,5,15]]}"#.to_string()),
        };
        let resolved = helper.resolve_request(&r2, &parent_resp).unwrap();
        assert_eq!(resolved.relative_url, "/resource/123?key=value");
        let body_str = resolved.body.unwrap();
        assert!(body_str.contains(r#""key1":"resolvedValue""#));
        assert!(body_str.contains(r#""startDate":"15 May 2023""#));
    }

    #[test]
    fn loop_guard_executes_and_prevents_infinite_loops() {
        let mut iter = 0;
        LoopGuard::run_safe_do_while_loop(5, &mut iter, |c| *c < 3, |c| *c += 1).unwrap();
        assert_eq!(iter, 3);

        // Do-while runs at least once when condition is initially false
        let mut once = 0;
        LoopGuard::run_safe_do_while_loop(3, &mut once, |_| false, |c| *c += 1).unwrap();
        assert_eq!(once, 1);

        // While skips if condition is initially false
        let mut zero = 0;
        LoopGuard::run_safe_while_loop(3, &mut zero, |_| false, |c| *c += 1).unwrap();
        assert_eq!(zero, 0);

        // Exceeding max iterations errors
        let mut inf = 0;
        assert_eq!(
            LoopGuard::run_safe_while_loop(5, &mut inf, |_| true, |c| *c += 1).unwrap_err(),
            CoreError::LoopExceeded(5)
        );
    }
}
