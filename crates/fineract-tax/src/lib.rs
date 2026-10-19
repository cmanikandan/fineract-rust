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
// module `fineract-tax` to Rust Edition 2024.

use chrono::{Days, NaiveDate};
use fineract_schedule_generator::{BigDec, RoundingMode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GLAccountType {
    Asset = 1,
    Liability = 2,
    Equity = 3,
    Income = 4,
    Expense = 5,
}

impl GLAccountType {
    pub fn from_int(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Asset),
            2 => Some(Self::Liability),
            3 => Some(Self::Equity),
            4 => Some(Self::Income),
            5 => Some(Self::Expense),
            _ => None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaxError {
    #[error("Invalid JSON payload")]
    InvalidJson,
    #[error("Tax component with id {0} does not exist")]
    TaxComponentNotFound(i64),
    #[error("Tax group with id {0} does not exist")]
    TaxGroupNotFound(i64),
    #[error("Tax mapping with id {0} does not exist")]
    TaxMappingNotFound(i64),
    #[error("Platform data validation error: {code} - {message}")]
    ValidationError {
        parameter: String,
        code: String,
        message: String,
    },
}

/// Port of `org.apache.fineract.portfolio.tax.domain.TaxComponentHistory`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaxComponentHistory {
    pub percentage: BigDec,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl TaxComponentHistory {
    pub fn occurs_on_day_from_and_up_to_and_including(&self, target: NaiveDate) -> bool {
        target > self.start_date && target <= self.end_date
    }
}

/// Port of `org.apache.fineract.portfolio.tax.domain.TaxComponent`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaxComponent {
    pub id: Option<i64>,
    pub name: String,
    pub percentage: BigDec,
    pub debit_account_type: Option<i32>,
    pub debit_account_id: Option<i64>,
    pub credit_account_type: Option<i32>,
    pub credit_account_id: Option<i64>,
    pub start_date: NaiveDate,
    pub tax_component_histories: Vec<TaxComponentHistory>,
}

impl TaxComponent {
    pub fn new(
        id: Option<i64>,
        name: impl Into<String>,
        percentage: BigDec,
        start_date: NaiveDate,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            percentage,
            debit_account_type: None,
            debit_account_id: None,
            credit_account_type: None,
            credit_account_id: None,
            start_date,
            tax_component_histories: Vec::new(),
        }
    }

    pub fn get_applicable_percentage(&self, date: NaiveDate) -> Option<BigDec> {
        if date > self.start_date {
            return Some(self.percentage);
        }
        for history in &self.tax_component_histories {
            if history.occurs_on_day_from_and_up_to_and_including(date) {
                return Some(history.percentage);
            }
        }
        None
    }

    pub fn all_start_dates(&self) -> Vec<NaiveDate> {
        let mut dates = vec![self.start_date];
        for history in &self.tax_component_histories {
            dates.push(history.start_date);
        }
        dates
    }
}

/// Port of `org.apache.fineract.portfolio.tax.domain.TaxGroupMappings`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaxGroupMappings {
    pub id: Option<i64>,
    pub tax_component: TaxComponent,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
}

impl TaxGroupMappings {
    pub fn new(
        id: Option<i64>,
        tax_component: TaxComponent,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            id,
            tax_component,
            start_date,
            end_date,
        }
    }

    /// Port of `TaxGroupMappings.occursOnDayFromAndUpToAndIncluding`
    pub fn occurs_on_day_from_and_up_to_and_including(&self, target: NaiveDate) -> bool {
        target > self.start_date && self.end_date.map(|ed| target <= ed).unwrap_or(true)
    }
}

/// Port of `org.apache.fineract.portfolio.tax.domain.TaxGroup`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxGroup {
    pub id: Option<i64>,
    pub name: String,
    pub tax_group_mappings: Vec<TaxGroupMappings>,
}

impl TaxGroup {
    pub fn new(id: Option<i64>, name: impl Into<String>, mappings: Vec<TaxGroupMappings>) -> Self {
        Self {
            id,
            name: name.into(),
            tax_group_mappings: mappings,
        }
    }
}

/// Port of `org.apache.fineract.portfolio.tax.service.TaxUtils`
pub struct TaxUtils;

impl TaxUtils {
    pub fn split_tax(
        amount: Option<BigDec>,
        date: NaiveDate,
        tax_group_mappings: &[TaxGroupMappings],
        scale: u32,
    ) -> HashMap<TaxComponent, BigDec> {
        let mut map = HashMap::new();
        let Some(amt) = amount else {
            return map;
        };
        let hundred = BigDec::from(100);
        for mapping in tax_group_mappings {
            if mapping.occurs_on_day_from_and_up_to_and_including(date) {
                let component = &mapping.tax_component;
                if let Some(percentage) = component.get_applicable_percentage(date) {
                    let raw_tax = (amt * percentage).divide_scale(
                        hundred,
                        scale,
                        RoundingMode::HalfEven,
                    );
                    map.insert(component.clone(), raw_tax);
                }
            }
        }
        map
    }

    pub fn total_tax_amount(map: &HashMap<TaxComponent, BigDec>) -> BigDec {
        map.values().copied().fold(BigDec::ZERO, |acc, v| acc + v)
    }

    pub fn income_amount(
        amount: BigDec,
        date: NaiveDate,
        tax_group_mappings: &[TaxGroupMappings],
        scale: u32,
    ) -> BigDec {
        let map = Self::split_tax(Some(amount), date, tax_group_mappings, scale);
        amount - Self::total_tax_amount(&map)
    }

    pub fn add_tax(
        amount: Option<BigDec>,
        date: NaiveDate,
        tax_group_mappings: &[TaxGroupMappings],
        scale: u32,
    ) -> Option<BigDec> {
        let amt = amount?;
        if amt <= BigDec::ZERO {
            return None;
        }
        let mut percentage_val = BigDec::ZERO;
        let hundred = BigDec::from(100);
        for mapping in tax_group_mappings {
            if mapping.occurs_on_day_from_and_up_to_and_including(date) {
                if let Some(p) = mapping.tax_component.get_applicable_percentage(date) {
                    percentage_val = percentage_val + p;
                }
            }
        }
        let denom = hundred - percentage_val;
        Some((amt * hundred).divide_scale(denom, scale, RoundingMode::HalfEven))
    }
}

/// Port of `org.apache.fineract.portfolio.tax.service.ChargeTaxApplicationService`
pub trait ChargeTaxApplicationService: Send + Sync {
    fn compute_tax(
        &self,
        tax_group: Option<&TaxGroup>,
        base_amount: Option<BigDec>,
        effective_date: NaiveDate,
        scale: u32,
    ) -> HashMap<TaxComponent, BigDec>;
}

#[derive(Debug, Default, Clone)]
pub struct ChargeTaxApplicationServiceImpl;

impl ChargeTaxApplicationService for ChargeTaxApplicationServiceImpl {
    fn compute_tax(
        &self,
        tax_group: Option<&TaxGroup>,
        base_amount: Option<BigDec>,
        effective_date: NaiveDate,
        scale: u32,
    ) -> HashMap<TaxComponent, BigDec> {
        let Some(group) = tax_group else {
            return HashMap::new();
        };
        let Some(amount) = base_amount else {
            return HashMap::new();
        };
        if amount.is_zero() {
            return HashMap::new();
        }
        TaxUtils::split_tax(
            Some(amount),
            effective_date,
            &group.tax_group_mappings,
            scale,
        )
    }
}

/// Port of `org.apache.fineract.portfolio.tax.serialization.TaxValidator`
pub struct TaxValidator;

impl TaxValidator {
    pub fn validate_tax_group(tax_group: &TaxGroup) -> Result<(), TaxError> {
        let mappings = &tax_group.tax_group_mappings;
        Self::validate_group_total(mappings, "total.percentage")?;
        Self::validate_overlapping_components(mappings)?;
        Ok(())
    }

    pub fn validate_group_total(
        mappings: &[TaxGroupMappings],
        parameter: &str,
    ) -> Result<(), TaxError> {
        let hundred = BigDec::from(100);
        for m1 in mappings {
            for date in m1.tax_component.all_start_dates() {
                let applicable_date = date.checked_add_days(Days::new(1)).unwrap_or(date);
                let mut total = BigDec::ZERO;
                for m2 in mappings {
                    if m2.occurs_on_day_from_and_up_to_and_including(applicable_date) {
                        if let Some(p) = m2
                            .tax_component
                            .get_applicable_percentage(applicable_date)
                        {
                            total = total + p;
                        }
                    }
                }
                if total > hundred {
                    return Err(TaxError::ValidationError {
                        parameter: parameter.to_string(),
                        code: format!("validation.msg.tax.group.{parameter}.greater.than.max"),
                        message: "Total percentage exceeds 100".to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn validate_overlapping_components(mappings: &[TaxGroupMappings]) -> Result<(), TaxError> {
        for (i, m1) in mappings.iter().enumerate() {
            for (j, m2) in mappings.iter().enumerate() {
                if i == j {
                    continue;
                }
                if m1.tax_component == m2.tax_component {
                    if m1.end_date.is_none() && m2.end_date.is_none() {
                        return Err(TaxError::ValidationError {
                            parameter: "component".to_string(),
                            code: "dates.are.overlapping".to_string(),
                            message: "Dates are overlapping for same tax component".to_string(),
                        });
                    } else if m1.start_date > m2.start_date {
                        if let Some(m2_end) = m2.end_date {
                            if m1.start_date <= m2_end {
                                return Err(TaxError::ValidationError {
                                    parameter: "component.start.date".to_string(),
                                    code: "dates.are.overlapping".to_string(),
                                    message: "Component start date must be after previous end date"
                                        .to_string(),
                                });
                            }
                        }
                    } else if let Some(m1_end) = m1.end_date {
                        if m2.start_date <= m1_end {
                            return Err(TaxError::ValidationError {
                                parameter: "component.start.date".to_string(),
                                code: "dates.are.overlapping".to_string(),
                                message: "Component start date must be after previous end date"
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actual_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    }

    fn tax_component_with_rate(id: i64, percentage: &str, start_date: NaiveDate) -> TaxComponent {
        TaxComponent::new(
            Some(id),
            format!("Tax {id}"),
            BigDec::parse_lossy(percentage),
            start_date,
        )
    }

    fn active_mapping_for(
        component: TaxComponent,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> TaxGroupMappings {
        TaxGroupMappings::new(None, component, start_date, end_date)
    }

    fn tax_group_with(
        component: TaxComponent,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    ) -> TaxGroup {
        TaxGroup::new(
            Some(1),
            "Standard Tax Group",
            vec![active_mapping_for(component, start_date, end_date)],
        )
    }

    #[test]
    fn compute_tax_returns_empty_map_when_tax_group_is_null() {
        let service = ChargeTaxApplicationServiceImpl;
        let result = service.compute_tax(
            None,
            Some(BigDec::parse_lossy("100.00")),
            actual_date(),
            6,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn compute_tax_returns_empty_map_when_base_amount_is_null() {
        let service = ChargeTaxApplicationServiceImpl;
        let group = TaxGroup::new(Some(1), "Group", vec![]);
        let result = service.compute_tax(Some(&group), None, actual_date(), 6);
        assert!(result.is_empty());
    }

    #[test]
    fn compute_tax_returns_empty_map_when_base_amount_is_zero() {
        let service = ChargeTaxApplicationServiceImpl;
        let group = TaxGroup::new(Some(1), "Group", vec![]);
        let result = service.compute_tax(Some(&group), Some(BigDec::ZERO), actual_date(), 6);
        assert!(result.is_empty());
    }

    #[test]
    fn compute_tax_calculates_tax_correctly_for_single_component() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let start = effective_date.checked_sub_days(Days::new(1)).unwrap();
        let component = tax_component_with_rate(1, "16", start);
        let group = tax_group_with(component.clone(), start, None);

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("1000.00")),
            effective_date,
            6,
        );
        assert_eq!(result.len(), 1);
        let tax = result.get(&component).unwrap();
        assert_eq!(
            tax.set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("160.00")
        );
    }

    #[test]
    fn compute_tax_calculates_tax_correctly_for_small_amount() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let start = effective_date.checked_sub_days(Days::new(1)).unwrap();
        let component = tax_component_with_rate(1, "10", start);
        let group = tax_group_with(component.clone(), start, None);

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("50.00")),
            effective_date,
            6,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result
                .get(&component)
                .unwrap()
                .set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("5.00")
        );
    }

    #[test]
    fn compute_tax_returns_tax_for_each_active_component_when_multiple_components_configured() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let start = effective_date.checked_sub_days(Days::new(10)).unwrap();
        let comp1 = tax_component_with_rate(1, "10", start);
        let comp2 = tax_component_with_rate(2, "5", start);

        let group = TaxGroup::new(
            Some(1),
            "Multi Group",
            vec![
                active_mapping_for(comp1.clone(), start, None),
                active_mapping_for(comp2.clone(), start, None),
            ],
        );

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("200.00")),
            effective_date,
            6,
        );
        assert_eq!(result.len(), 2);
        assert_eq!(
            result
                .get(&comp1)
                .unwrap()
                .set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("20.00")
        );
        assert_eq!(
            result
                .get(&comp2)
                .unwrap()
                .set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("10.00")
        );
        assert_eq!(
            TaxUtils::total_tax_amount(&result).set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("30.00")
        );
    }

    #[test]
    fn compute_tax_includes_active_and_excludes_expired_when_mixed_mappings() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let start = effective_date.checked_sub_days(Days::new(20)).unwrap();
        let expired_end = effective_date.checked_sub_days(Days::new(1)).unwrap();

        let active_comp = tax_component_with_rate(1, "10", start);
        let expired_comp = tax_component_with_rate(2, "5", start);

        let group = TaxGroup::new(
            Some(1),
            "Mixed Group",
            vec![
                active_mapping_for(active_comp.clone(), start, None),
                active_mapping_for(expired_comp, start, Some(expired_end)),
            ],
        );

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("100.00")),
            effective_date,
            6,
        );
        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&active_comp));
        assert_eq!(
            result
                .get(&active_comp)
                .unwrap()
                .set_scale(2, RoundingMode::HalfEven),
            BigDec::parse_lossy("10.00")
        );
    }

    #[test]
    fn compute_tax_returns_empty_map_when_mapping_has_not_started_yet() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let tomorrow = effective_date.checked_add_days(Days::new(1)).unwrap();
        let component = tax_component_with_rate(1, "16", tomorrow);
        let group = tax_group_with(component, tomorrow, None);

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("1000.00")),
            effective_date,
            6,
        );
        assert!(result.is_empty());
    }

    #[test]
    fn compute_tax_respects_requested_scale_and_validates_group_total() {
        let service = ChargeTaxApplicationServiceImpl;
        let effective_date = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let start = effective_date.checked_sub_days(Days::new(1)).unwrap();
        let component = tax_component_with_rate(1, "7", start);
        let group = tax_group_with(component.clone(), start, None);

        let result = service.compute_tax(
            Some(&group),
            Some(BigDec::parse_lossy("333.33")),
            effective_date,
            2,
        );
        // 333.33 * 7 / 100 = 23.3331 -> 23.33 at scale 2
        assert_eq!(result.get(&component).unwrap().to_string_with_scale(2), "23.33");
        assert!(TaxValidator::validate_tax_group(&group).is_ok());

        // Group exceeding 100% fails validation
        let c1 = tax_component_with_rate(10, "60", start);
        let c2 = tax_component_with_rate(11, "50", start);
        let bad_group = TaxGroup::new(
            Some(2),
            "Over 100%",
            vec![
                active_mapping_for(c1, start, None),
                active_mapping_for(c2, start, None),
            ],
        );
        assert!(TaxValidator::validate_tax_group(&bad_group).is_err());
    }
}
