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
// module `fineract-rates` to Rust Edition 2024.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::NaiveDate;
use fineract_schedule_generator::BigDec;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use thiserror::Error;

pub const NAME_NOT_BLANK: &str = "{org.apache.fineract.portfolio.floatingrate.name.not-blank}";
pub const RATE_PERIODS_NOT_EMPTY: &str =
    "{org.apache.fineract.portfolio.floatingrate.rate-periods.not-empty}";
pub const FROM_DATE_INVALID: &str =
    "{org.apache.fineract.portfolio.floatingrate.from-date.invalid}";
pub const FROM_DATE_FUTURE: &str =
    "{org.apache.fineract.portfolio.floatingrate.from-date.future}";
pub const FROM_DATE_DUPLICATE: &str =
    "{org.apache.fineract.portfolio.floatingrate.from-date.duplicate}";
pub const INTEREST_RATE_POSITIVE_OR_ZERO: &str =
    "{org.apache.fineract.portfolio.floatingrate.interest-rate.positive-or-zero}";
pub const BLR_DUPLICATE: &str =
    "{org.apache.fineract.portfolio.floatingrate.base-lending-rate.duplicate}";
pub const BLR_LINKED: &str =
    "{org.apache.fineract.portfolio.floatingrate.base-lending-rate.linked}";
pub const DIFFERENTIAL_NOT_AVAILABLE: &str =
    "{org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.not-available}";
pub const DIFFERENTIAL_INVALID_FOR_BLR: &str =
    "{org.apache.fineract.portfolio.floatingrate.differential-to-base-lending-rate.invalid-for-base-lending-rate}";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FloatingRateError {
    #[error("Floating rate with id {0} does not exist")]
    NotFound(i64),
    #[error("Validation failed: {0:?}")]
    ValidationErrors(Vec<String>),
}

/// Port of `org.apache.fineract.portfolio.floatingrates.data.FloatingRateDTO`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatingRateDTO {
    pub is_floating_interest_rate: bool,
    pub start_date: NaiveDate,
    pub interest_rate_diff: BigDec,
    pub base_lending_rate_periods: Vec<FloatingRatePeriodData>,
}

impl FloatingRateDTO {
    pub fn fetch_base_rate(&self, date: NaiveDate) -> BigDec {
        let mut rate = BigDec::ZERO;
        for period_data in &self.base_lending_rate_periods {
            if period_data.from_date <= date {
                rate = period_data.interest_rate;
            }
        }
        rate
    }
}

/// Port of `org.apache.fineract.portfolio.floatingrates.data.FloatingRatePeriodData`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRatePeriodData {
    pub id: Option<i64>,
    pub from_date: NaiveDate,
    pub interest_rate: BigDec,
    pub is_differential_to_base_lending_rate: bool,
    pub is_active: bool,
}

/// Port of `org.apache.fineract.portfolio.floatingrates.data.FloatingRateData`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRateData {
    pub id: Option<i64>,
    pub name: String,
    pub is_base_lending_rate: bool,
    pub is_active: bool,
    pub rate_periods: Vec<FloatingRatePeriodData>,
}

/// Port of `org.apache.fineract.portfolio.floatingrates.domain.FloatingRatePeriod`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingRatePeriod {
    pub id: Option<i64>,
    pub from_date: NaiveDate,
    pub interest_rate: BigDec,
    pub is_differential_to_base_lending_rate: bool,
    pub is_active: bool,
}

impl FloatingRatePeriod {
    pub fn new(
        from_date: NaiveDate,
        interest_rate: BigDec,
        is_differential_to_base_lending_rate: bool,
        is_active: bool,
    ) -> Self {
        Self {
            id: None,
            from_date,
            interest_rate,
            is_differential_to_base_lending_rate,
            is_active,
        }
    }

    pub fn to_data(&self, dto: &FloatingRateDTO) -> FloatingRatePeriodData {
        let mut interest = self.interest_rate + dto.interest_rate_diff;
        if self.is_differential_to_base_lending_rate {
            interest = interest + dto.fetch_base_rate(self.from_date);
        }
        FloatingRatePeriodData {
            id: self.id,
            from_date: self.from_date,
            interest_rate: interest,
            is_differential_to_base_lending_rate: self.is_differential_to_base_lending_rate,
            is_active: self.is_active,
        }
    }
}

/// Port of `org.apache.fineract.portfolio.floatingrates.domain.FloatingRate`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingRate {
    pub id: Option<i64>,
    pub name: String,
    pub is_base_lending_rate: bool,
    pub is_active: bool,
    pub floating_rate_periods: Vec<FloatingRatePeriod>,
}

impl FloatingRate {
    pub fn new(
        id: Option<i64>,
        name: impl Into<String>,
        is_base_lending_rate: bool,
        is_active: bool,
        floating_rate_periods: Vec<FloatingRatePeriod>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            is_base_lending_rate,
            is_active,
            floating_rate_periods,
        }
    }

    pub fn replace_rate_periods(
        &mut self,
        new_rate_periods: Vec<FloatingRatePeriod>,
        business_today: NaiveDate,
    ) {
        for period in &mut self.floating_rate_periods {
            if period.from_date > business_today {
                period.is_active = false;
            }
        }
        self.floating_rate_periods.extend(new_rate_periods);
    }

    pub fn fetch_interest_rates(&self, dto: &FloatingRateDTO) -> Vec<FloatingRatePeriodData> {
        let mut applicable_rates = Vec::new();
        let mut previous_period: Option<&FloatingRatePeriod> = None;
        let mut add_period_data = false;

        for period in &self.floating_rate_periods {
            if period.is_active {
                if applicable_rates.is_empty() && dto.start_date < period.from_date {
                    if dto.is_floating_interest_rate {
                        add_period_data = true;
                    }
                    if let Some(prev) = previous_period {
                        applicable_rates.push(prev.to_data(dto));
                    } else if !add_period_data {
                        applicable_rates.push(period.to_data(dto));
                    }
                }
                if add_period_data {
                    applicable_rates.push(period.to_data(dto));
                }
                previous_period = Some(period);
            }
        }

        if applicable_rates.is_empty() {
            if let Some(prev) = previous_period {
                applicable_rates.push(prev.to_data(dto));
            }
        }

        applicable_rates
    }

    pub fn to_data(&self) -> FloatingRateData {
        FloatingRateData {
            id: self.id,
            name: self.name.clone(),
            is_base_lending_rate: self.is_base_lending_rate,
            is_active: self.is_active,
            rate_periods: self
                .floating_rate_periods
                .iter()
                .map(|p| FloatingRatePeriodData {
                    id: p.id,
                    from_date: p.from_date,
                    interest_rate: p.interest_rate,
                    is_differential_to_base_lending_rate: p.is_differential_to_base_lending_rate,
                    is_active: p.is_active,
                })
                .collect(),
        }
    }
}

/// Port of `FloatingRatePeriodRequest`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRatePeriodRequest {
    pub from_date: Option<String>,
    pub interest_rate: Option<BigDec>,
    pub is_differential_to_base_lending_rate: Option<bool>,
    pub locale: Option<String>,
    pub date_format: Option<String>,
}

impl FloatingRatePeriodRequest {
    pub fn from_date_as_local_date(&self) -> Result<NaiveDate, chrono::ParseError> {
        let raw = self.from_date.as_deref().unwrap_or("");
        NaiveDate::parse_from_str(raw, "%Y-%m-%d")
    }
}

/// Port of `FloatingRateCreateRequest`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRateCreateRequest {
    pub name: Option<String>,
    pub is_base_lending_rate: Option<bool>,
    pub is_active: Option<bool>,
    pub rate_periods: Option<Vec<FloatingRatePeriodRequest>>,
}

/// Port of `FloatingRateUpdateRequest`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingRateUpdateRequest {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub is_base_lending_rate: Option<bool>,
    pub is_active: Option<bool>,
    pub rate_periods: Option<Vec<FloatingRatePeriodRequest>>,
}

/// Port of `FloatingRateRepository`
pub trait FloatingRateRepository: Send + Sync {
    fn find_by_id(&self, id: i64) -> Option<FloatingRate>;
    fn find_all(&self) -> Vec<FloatingRate>;
    fn retrieve_base_lending_rate(&self) -> Option<FloatingRate>;
    fn retrieve_floating_rates_linked_to_blr(&self) -> Vec<FloatingRate>;
    fn save(&self, rate: FloatingRate) -> FloatingRate;
}

#[derive(Default)]
pub struct InMemoryFloatingRateRepository {
    rates: RwLock<Vec<FloatingRate>>,
}

impl InMemoryFloatingRateRepository {
    pub fn new() -> Self {
        Self {
            rates: RwLock::new(Vec::new()),
        }
    }
}

impl FloatingRateRepository for InMemoryFloatingRateRepository {
    fn find_by_id(&self, id: i64) -> Option<FloatingRate> {
        self.rates
            .read()
            .unwrap()
            .iter()
            .find(|r| r.id == Some(id))
            .cloned()
    }

    fn find_all(&self) -> Vec<FloatingRate> {
        self.rates.read().unwrap().clone()
    }

    fn retrieve_base_lending_rate(&self) -> Option<FloatingRate> {
        self.rates
            .read()
            .unwrap()
            .iter()
            .find(|r| r.is_base_lending_rate && r.is_active)
            .cloned()
    }

    fn retrieve_floating_rates_linked_to_blr(&self) -> Vec<FloatingRate> {
        self.rates
            .read()
            .unwrap()
            .iter()
            .filter(|r| {
                r.is_active
                    && r.floating_rate_periods
                        .iter()
                        .any(|p| p.is_active && p.is_differential_to_base_lending_rate)
            })
            .cloned()
            .collect()
    }

    fn save(&self, mut rate: FloatingRate) -> FloatingRate {
        let mut store = self.rates.write().unwrap();
        if let Some(id) = rate.id {
            if let Some(existing) = store.iter_mut().find(|r| r.id == Some(id)) {
                *existing = rate.clone();
                return rate;
            }
        } else {
            let next_id = store.iter().filter_map(|r| r.id).max().unwrap_or(0) + 1;
            rate.id = Some(next_id);
        }
        store.push(rate.clone());
        rate
    }
}

// ============================================================================
// Validators (Port of FloatingRateCreateValidator, FloatingRateUpdateValidator, FloatingRateRatePeriodsChecker)
// ============================================================================

pub fn check_rate_periods(
    rate_periods: Option<&[FloatingRatePeriodRequest]>,
    is_base_lending_rate: bool,
    base_lending_rate_available: bool,
    business_date: NaiveDate,
    violations: &mut HashSet<String>,
) {
    let Some(periods) = rate_periods else {
        return;
    };
    if periods.is_empty() {
        return;
    }

    let mut from_dates = Vec::new();
    for period in periods {
        // Bean validation annotations on FloatingRatePeriodRequest
        match period.from_date.as_deref() {
            Some(s) if !s.trim().is_empty() => match period.from_date_as_local_date() {
                Ok(from_date) => {
                    if from_date <= business_date {
                        violations.insert(FROM_DATE_FUTURE.to_string());
                    }
                    from_dates.push(from_date);
                }
                Err(_) => {
                    violations.insert(FROM_DATE_INVALID.to_string());
                }
            },
            _ => {
                violations.insert(FROM_DATE_INVALID.to_string());
            }
        }

        if let Some(rate) = period.interest_rate {
            if rate.is_negative() {
                violations.insert(INTEREST_RATE_POSITIVE_OR_ZERO.to_string());
            }
        } else {
            violations.insert(INTEREST_RATE_POSITIVE_OR_ZERO.to_string());
        }

        if period.is_differential_to_base_lending_rate == Some(true) {
            if !base_lending_rate_available {
                violations.insert(DIFFERENTIAL_NOT_AVAILABLE.to_string());
            }
            if is_base_lending_rate {
                violations.insert(DIFFERENTIAL_INVALID_FOR_BLR.to_string());
            }
        }
    }

    let unique_dates: HashSet<NaiveDate> = from_dates.iter().copied().collect();
    if from_dates.len() != unique_dates.len() {
        violations.insert(FROM_DATE_DUPLICATE.to_string());
    }
}

pub fn validate_create_request(
    request: &FloatingRateCreateRequest,
    repository: &dyn FloatingRateRepository,
    business_date: NaiveDate,
) -> HashSet<String> {
    let mut violations = HashSet::new();

    if request
        .name
        .as_deref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        violations.insert(NAME_NOT_BLANK.to_string());
    }

    if let Some(periods) = &request.rate_periods {
        if periods.is_empty() {
            violations.insert(RATE_PERIODS_NOT_EMPTY.to_string());
        }
    }

    let is_blr = request.is_base_lending_rate == Some(true);
    let existing_blr = repository.retrieve_base_lending_rate();
    if is_blr && existing_blr.is_some() {
        violations.insert(BLR_DUPLICATE.to_string());
    }

    check_rate_periods(
        request.rate_periods.as_deref(),
        is_blr,
        existing_blr.is_some(),
        business_date,
        &mut violations,
    );

    violations
}

pub fn validate_update_request(
    request: &FloatingRateUpdateRequest,
    repository: &dyn FloatingRateRepository,
    business_date: NaiveDate,
) -> HashSet<String> {
    let mut violations = HashSet::new();

    if let Some(name) = &request.name {
        if name.trim().is_empty() {
            violations.insert(NAME_NOT_BLANK.to_string());
        }
    }

    let Some(id) = request.id else {
        return violations;
    };
    let Some(floating_rate_for_update) = repository.find_by_id(id) else {
        return violations;
    };

    let base_lending_rate = repository.retrieve_base_lending_rate();
    if request.is_base_lending_rate == Some(true) {
        if let Some(ref blr) = base_lending_rate {
            if blr.id != floating_rate_for_update.id {
                violations.insert(BLR_DUPLICATE.to_string());
            }
        }
    }

    let is_blr = request
        .is_base_lending_rate
        .unwrap_or(floating_rate_for_update.is_base_lending_rate);
    let is_active = request.is_active.unwrap_or(floating_rate_for_update.is_active);
    let is_blr_modified_as_non_blr = base_lending_rate
        .as_ref()
        .map(|blr| blr.id == floating_rate_for_update.id && (!is_blr || !is_active))
        .unwrap_or(false);

    if is_blr_modified_as_non_blr {
        let linked = repository.retrieve_floating_rates_linked_to_blr();
        if !linked.is_empty() {
            violations.insert(BLR_LINKED.to_string());
        }
    }

    check_rate_periods(
        request.rate_periods.as_deref(),
        is_blr,
        base_lending_rate.is_some() && !is_blr_modified_as_non_blr,
        business_date,
        &mut violations,
    );

    violations
}

// ============================================================================
// Axum 0.8 REST API Resource (`FloatingRatesApiResource`)
// ============================================================================

#[derive(Clone)]
pub struct FloatingRatesAppState {
    pub repo: Arc<dyn FloatingRateRepository>,
    pub business_date: NaiveDate,
}

pub fn floating_rates_router(state: FloatingRatesAppState) -> Router {
    Router::new()
        .route(
            "/floatingrates",
            get(retrieve_all_floating_rates).post(create_floating_rate),
        )
        .route(
            "/floatingrates/{id}",
            get(retrieve_one_floating_rate).put(update_floating_rate),
        )
        .with_state(state)
}

async fn retrieve_all_floating_rates(
    State(state): State<FloatingRatesAppState>,
) -> impl IntoResponse {
    let data: Vec<FloatingRateData> = state.repo.find_all().iter().map(|r| r.to_data()).collect();
    (StatusCode::OK, Json(data))
}

async fn retrieve_one_floating_rate(
    State(state): State<FloatingRatesAppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.repo.find_by_id(id) {
        Some(r) => (StatusCode::OK, Json(serde_json::to_value(r.to_data()).unwrap())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("FloatingRate {id} not found") })),
        ),
    }
}

async fn create_floating_rate(
    State(state): State<FloatingRatesAppState>,
    Json(req): Json<FloatingRateCreateRequest>,
) -> impl IntoResponse {
    let violations = validate_create_request(&req, state.repo.as_ref(), state.business_date);
    if !violations.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "violations": violations })),
        );
    }

    let periods = req
        .rate_periods
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            FloatingRatePeriod::new(
                p.from_date_as_local_date().unwrap(),
                p.interest_rate.unwrap_or(BigDec::ZERO),
                p.is_differential_to_base_lending_rate.unwrap_or(false),
                true,
            )
        })
        .collect();

    let saved = state.repo.save(FloatingRate::new(
        None,
        req.name.unwrap_or_default(),
        req.is_base_lending_rate.unwrap_or(false),
        req.is_active.unwrap_or(true),
        periods,
    ));

    (
        StatusCode::OK,
        Json(serde_json::json!({ "resourceId": saved.id })),
    )
}

async fn update_floating_rate(
    State(state): State<FloatingRatesAppState>,
    Path(id): Path<i64>,
    Json(mut req): Json<FloatingRateUpdateRequest>,
) -> impl IntoResponse {
    req.id = Some(id);
    let violations = validate_update_request(&req, state.repo.as_ref(), state.business_date);
    if !violations.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "violations": violations })),
        );
    }

    let Some(mut existing) = state.repo.find_by_id(id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Not found" })),
        );
    };

    if let Some(name) = req.name {
        existing.name = name;
    }
    if let Some(is_blr) = req.is_base_lending_rate {
        existing.is_base_lending_rate = is_blr;
    }
    if let Some(is_active) = req.is_active {
        existing.is_active = is_active;
    }
    if let Some(periods) = req.rate_periods {
        let new_periods = periods
            .into_iter()
            .map(|p| {
                FloatingRatePeriod::new(
                    p.from_date_as_local_date().unwrap(),
                    p.interest_rate.unwrap_or(BigDec::ZERO),
                    p.is_differential_to_base_lending_rate.unwrap_or(false),
                    true,
                )
            })
            .collect();
        existing.replace_rate_periods(new_periods, state.business_date);
    }

    let saved = state.repo.save(existing);
    (
        StatusCode::OK,
        Json(serde_json::json!({ "resourceId": saved.id })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Days;

    fn business_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    }

    fn period(
        from_date: NaiveDate,
        interest_rate: &str,
        is_diff: Option<bool>,
    ) -> FloatingRatePeriodRequest {
        FloatingRatePeriodRequest {
            from_date: Some(from_date.to_string()),
            interest_rate: Some(BigDec::parse_lossy(interest_rate)),
            is_differential_to_base_lending_rate: is_diff,
            locale: None,
            date_format: None,
        }
    }

    fn create_request(
        name: &str,
        is_blr: Option<bool>,
        rate_periods: Option<Vec<FloatingRatePeriodRequest>>,
    ) -> FloatingRateCreateRequest {
        FloatingRateCreateRequest {
            name: Some(name.to_string()),
            is_base_lending_rate: is_blr,
            is_active: None,
            rate_periods,
        }
    }

    fn update_request(
        id: i64,
        name: Option<&str>,
        is_blr: Option<bool>,
        is_active: Option<bool>,
        rate_periods: Option<Vec<FloatingRatePeriodRequest>>,
    ) -> FloatingRateUpdateRequest {
        FloatingRateUpdateRequest {
            id: Some(id),
            name: name.map(|s| s.to_string()),
            is_base_lending_rate: is_blr,
            is_active,
            rate_periods,
        }
    }

    fn base_lending_rate(id: i64) -> FloatingRate {
        FloatingRate::new(Some(id), format!("Rate {id}"), true, true, vec![])
    }

    fn floating_rate(id: i64, is_blr: bool, is_active: bool) -> FloatingRate {
        FloatingRate::new(Some(id), format!("Rate {id}"), is_blr, is_active, vec![])
    }

    #[test]
    fn create_valid_request_has_no_violations() {
        let repo = InMemoryFloatingRateRepository::new();
        let bdate = business_date();
        let req = create_request(
            "Floating Rate",
            None,
            Some(vec![period(bdate.checked_add_days(Days::new(1)).unwrap(), "10", None)]),
        );
        assert!(validate_create_request(&req, &repo, bdate).is_empty());
    }

    #[test]
    fn create_blank_name_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let req = create_request(" ", None, None);
        assert_eq!(
            validate_create_request(&req, &repo, business_date()),
            HashSet::from([NAME_NOT_BLANK.to_string()])
        );
    }

    #[test]
    fn create_empty_rate_periods_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let req = create_request("Floating Rate", None, Some(vec![]));
        assert_eq!(
            validate_create_request(&req, &repo, business_date()),
            HashSet::from([RATE_PERIODS_NOT_EMPTY.to_string()])
        );
    }

    #[test]
    fn create_non_future_from_date_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let bdate = business_date();
        let req = create_request("Floating Rate", None, Some(vec![period(bdate, "10", None)]));
        assert_eq!(
            validate_create_request(&req, &repo, bdate),
            HashSet::from([FROM_DATE_FUTURE.to_string()])
        );
    }

    #[test]
    fn create_unparseable_from_date_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let p = FloatingRatePeriodRequest {
            from_date: Some("not-a-date".to_string()),
            interest_rate: Some(BigDec::from(10)),
            is_differential_to_base_lending_rate: None,
            locale: None,
            date_format: None,
        };
        let req = create_request("Floating Rate", None, Some(vec![p]));
        assert_eq!(
            validate_create_request(&req, &repo, business_date()),
            HashSet::from([FROM_DATE_INVALID.to_string()])
        );
    }

    #[test]
    fn create_duplicate_from_dates_fail() {
        let repo = InMemoryFloatingRateRepository::new();
        let bdate = business_date();
        let from_date = bdate.checked_add_days(Days::new(5)).unwrap();
        let req = create_request(
            "Floating Rate",
            None,
            Some(vec![
                period(from_date, "10", None),
                period(from_date, "11", None),
            ]),
        );
        assert_eq!(
            validate_create_request(&req, &repo, bdate),
            HashSet::from([FROM_DATE_DUPLICATE.to_string()])
        );
    }

    #[test]
    fn create_negative_interest_rate_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let bdate = business_date();
        let req = create_request(
            "Floating Rate",
            None,
            Some(vec![period(
                bdate.checked_add_days(Days::new(1)).unwrap(),
                "-1",
                None,
            )]),
        );
        assert_eq!(
            validate_create_request(&req, &repo, bdate),
            HashSet::from([INTEREST_RATE_POSITIVE_OR_ZERO.to_string()])
        );
    }

    #[test]
    fn create_second_base_lending_rate_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(base_lending_rate(1));
        let req = create_request("Floating Rate", Some(true), None);
        assert_eq!(
            validate_create_request(&req, &repo, business_date()),
            HashSet::from([BLR_DUPLICATE.to_string()])
        );
    }

    #[test]
    fn create_differential_period_without_base_lending_rate_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        let bdate = business_date();
        let req = create_request(
            "Floating Rate",
            None,
            Some(vec![period(
                bdate.checked_add_days(Days::new(1)).unwrap(),
                "10",
                Some(true),
            )]),
        );
        assert_eq!(
            validate_create_request(&req, &repo, bdate),
            HashSet::from([DIFFERENTIAL_NOT_AVAILABLE.to_string()])
        );
    }

    #[test]
    fn create_differential_period_on_base_lending_rate_itself_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(base_lending_rate(1));
        let bdate = business_date();
        let req = create_request(
            "Floating Rate",
            Some(true),
            Some(vec![period(
                bdate.checked_add_days(Days::new(1)).unwrap(),
                "10",
                Some(true),
            )]),
        );
        assert!(
            validate_create_request(&req, &repo, bdate).contains(DIFFERENTIAL_INVALID_FOR_BLR)
        );
    }

    #[test]
    fn update_base_lending_rate_itself_is_not_a_duplicate() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(base_lending_rate(1));
        let req = update_request(1, Some("New name"), Some(true), Some(true), None);
        assert!(validate_update_request(&req, &repo, business_date()).is_empty());
    }

    #[test]
    fn update_other_rate_to_base_lending_rate_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(base_lending_rate(1));
        repo.save(floating_rate(2, false, true));
        let req = update_request(2, None, Some(true), None, None);
        assert_eq!(
            validate_update_request(&req, &repo, business_date()),
            HashSet::from([BLR_DUPLICATE.to_string()])
        );
    }

    #[test]
    fn update_deactivating_linked_base_lending_rate_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(base_lending_rate(1));
        let mut linked = floating_rate(2, false, true);
        linked.floating_rate_periods.push(FloatingRatePeriod::new(
            business_date(),
            BigDec::from(2),
            true,
            true,
        ));
        repo.save(linked);

        let req = update_request(1, None, None, Some(false), None);
        assert_eq!(
            validate_update_request(&req, &repo, business_date()),
            HashSet::from([BLR_LINKED.to_string()])
        );
    }

    #[test]
    fn update_blank_name_fails() {
        let repo = InMemoryFloatingRateRepository::new();
        repo.save(floating_rate(2, false, true));
        let req = update_request(2, Some(" "), None, None, None);
        assert_eq!(
            validate_update_request(&req, &repo, business_date()),
            HashSet::from([NAME_NOT_BLANK.to_string()])
        );
    }

    #[test]
    fn fetch_interest_rates_with_differential_to_base_lending_rate() {
        let d1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let rate = FloatingRate::new(
            Some(10),
            "Product Rate",
            false,
            true,
            vec![
                FloatingRatePeriod::new(d1, BigDec::parse_lossy("2.5"), true, true),
                FloatingRatePeriod::new(d2, BigDec::parse_lossy("3.0"), true, true),
            ],
        );

        let dto = FloatingRateDTO {
            is_floating_interest_rate: true,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            interest_rate_diff: BigDec::parse_lossy("1.0"),
            base_lending_rate_periods: vec![FloatingRatePeriodData {
                id: Some(1),
                from_date: d1,
                interest_rate: BigDec::parse_lossy("8.0"),
                is_differential_to_base_lending_rate: false,
                is_active: true,
            }],
        };

        let applicable = rate.fetch_interest_rates(&dto);
        assert_eq!(applicable.len(), 2);
        // Period 1: 2.5 + 1.0 (diff) + 8.0 (BLR) = 11.5
        assert_eq!(applicable[0].interest_rate, BigDec::parse_lossy("11.5"));
        // Period 2: 3.0 + 1.0 (diff) + 8.0 (BLR) = 12.0
        assert_eq!(applicable[1].interest_rate, BigDec::parse_lossy("12.0"));
    }
}
