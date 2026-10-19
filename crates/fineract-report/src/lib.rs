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
// module `fineract-report` to Rust Edition 2024.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

pub const NUMERIC_FORMAT_TYPE: &str = "number";
pub const DATE_FORMAT_TYPE: &str = "date";
pub const SERVICE_MISSING: &str = "There is no ReportingProcessService registered in the ReportingProcessServiceProvider for this report type: ";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReportError {
    #[error("unknown report parameter '{param}' is not registered for report '{report}'")]
    UnknownReportParameter { param: String, report: String },
    #[error("Invalid '{format_type}' value '{value}' for report parameter '{param}'")]
    InvalidParameterFormat {
        param: String,
        value: String,
        format_type: String,
    },
    #[error("{SERVICE_MISSING}{0}")]
    ServiceMissing(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportExportType {
    pub key: String,
    pub query_parameter: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportResponse {
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Port of `org.apache.fineract.infrastructure.security.service.InputValidator`
pub trait InputValidator: Send + Sync {
    fn validate(&self, param_name: &str, format_type: &str, value: &str) -> Result<(), ReportError>;
}

#[derive(Debug, Default, Clone)]
pub struct DefaultInputValidator;

impl InputValidator for DefaultInputValidator {
    fn validate(&self, param_name: &str, format_type: &str, value: &str) -> Result<(), ReportError> {
        if format_type.eq_ignore_ascii_case(NUMERIC_FORMAT_TYPE) {
            if value.trim().parse::<f64>().is_err() {
                return Err(ReportError::InvalidParameterFormat {
                    param: param_name.to_string(),
                    value: value.to_string(),
                    format_type: format_type.to_string(),
                });
            }
        } else if format_type.eq_ignore_ascii_case(DATE_FORMAT_TYPE) {
            // Accept ISO YYYY-MM-DD or reject SQL injection / non-date tokens
            if NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").is_err() {
                return Err(ReportError::InvalidParameterFormat {
                    param: param_name.to_string(),
                    value: value.to_string(),
                    format_type: format_type.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Port of `org.apache.fineract.infrastructure.report.service.ReportParameterTypeResolver`
pub trait ReportParameterTypeResolver: Send + Sync {
    fn load_param_format_types(&self, report_name: &str) -> HashMap<String, String>;
}

/// Port of `org.apache.fineract.infrastructure.report.service.ReportParameterTypeResolverImpl`
pub struct SqlReportParameterTypeResolver {
    pub is_postgresql: bool,
    pub registered_reports: HashMap<String, HashMap<String, String>>,
}

impl SqlReportParameterTypeResolver {
    pub const PARAM_TYPE_SQL_PREFIX: &'static str = "SELECT sp.parameter_variable, sp.";
    pub const PARAM_TYPE_SQL_SUFFIX: &'static str = " AS format_type\nFROM stretchy_report_parameter srp\nJOIN stretchy_parameter sp ON sp.id = srp.parameter_id\nWHERE srp.report_id = (SELECT id FROM stretchy_report WHERE report_name = ?)\n";

    pub fn new(is_postgresql: bool) -> Self {
        Self {
            is_postgresql,
            registered_reports: HashMap::new(),
        }
    }

    pub fn register_report_params(&mut self, report_name: impl Into<String>, params: HashMap<String, String>) {
        self.registered_reports.insert(report_name.into(), params);
    }

    pub fn get_quoted_column_name(&self, column_name: &str) -> String {
        if self.is_postgresql {
            format!("\"{column_name}\"")
        } else {
            column_name.to_string()
        }
    }

    pub fn build_sql(&self) -> String {
        let quoted_col = self.get_quoted_column_name("parameter_FormatType");
        format!(
            "{}{}{}",
            Self::PARAM_TYPE_SQL_PREFIX,
            quoted_col,
            Self::PARAM_TYPE_SQL_SUFFIX
        )
    }
}

impl ReportParameterTypeResolver for SqlReportParameterTypeResolver {
    fn load_param_format_types(&self, report_name: &str) -> HashMap<String, String> {
        self.registered_reports
            .get(report_name)
            .cloned()
            .unwrap_or_default()
    }
}

/// Port of `org.apache.fineract.infrastructure.report.service.ReportingProcessService`
pub trait ReportingProcessService: Send + Sync {
    fn report_types(&self) -> &'static [&'static str];

    fn process_request(
        &self,
        report_name: &str,
        query_params: &HashMap<String, Vec<String>>,
    ) -> Result<ReportResponse, ReportError>;

    fn get_available_export_targets(&self) -> Vec<ReportExportType>;

    fn get_report_params(
        &self,
        report_name: &str,
        query_params: &HashMap<String, Vec<String>>,
    ) -> Result<HashMap<String, String>, ReportError>;
}

/// Port of `org.apache.fineract.infrastructure.report.service.AbstractReportingProcessService`
pub struct BaseReportingProcessService {
    pub input_validator: Arc<dyn InputValidator>,
    pub param_type_resolver: Arc<dyn ReportParameterTypeResolver>,
    pub supported_types: &'static [&'static str],
}

impl BaseReportingProcessService {
    pub fn new(
        input_validator: Arc<dyn InputValidator>,
        param_type_resolver: Arc<dyn ReportParameterTypeResolver>,
        supported_types: &'static [&'static str],
    ) -> Self {
        Self {
            input_validator,
            param_type_resolver,
            supported_types,
        }
    }

    pub fn extract_and_validate_report_params(
        &self,
        report_name: &str,
        query_params: &HashMap<String, Vec<String>>,
    ) -> Result<HashMap<String, String>, ReportError> {
        let param_format_types = self.param_type_resolver.load_param_format_types(report_name);
        let has_registered_params = !param_format_types.is_empty();
        let mut report_params = HashMap::new();

        for (key, values) in query_params {
            if let Some(param_variable) = key.strip_prefix("R_") {
                let Some(p_value) = values.first() else {
                    continue;
                };
                let p_key = format!("${{{param_variable}}}");

                if has_registered_params {
                    let Some(format_type) = param_format_types.get(param_variable) else {
                        return Err(ReportError::UnknownReportParameter {
                            param: param_variable.to_string(),
                            report: report_name.to_string(),
                        });
                    };
                    self.validate_param_by_type(param_variable, p_value, format_type)?;
                }

                report_params.insert(p_key, p_value.clone());
            }
        }

        Ok(report_params)
    }

    fn validate_param_by_type(
        &self,
        param_name: &str,
        value: &str,
        format_type: &str,
    ) -> Result<(), ReportError> {
        if format_type.eq_ignore_ascii_case(NUMERIC_FORMAT_TYPE) {
            self.input_validator
                .validate(param_name, NUMERIC_FORMAT_TYPE, value)?;
        } else if format_type.eq_ignore_ascii_case(DATE_FORMAT_TYPE) {
            self.input_validator
                .validate(param_name, DATE_FORMAT_TYPE, value)?;
        }
        Ok(())
    }
}

impl ReportingProcessService for BaseReportingProcessService {
    fn report_types(&self) -> &'static [&'static str] {
        self.supported_types
    }

    fn process_request(
        &self,
        report_name: &str,
        query_params: &HashMap<String, Vec<String>>,
    ) -> Result<ReportResponse, ReportError> {
        let params = self.get_report_params(report_name, query_params)?;
        let body = serde_json::to_vec(&params).unwrap_or_default();
        Ok(ReportResponse {
            content_type: "application/json".to_string(),
            body,
        })
    }

    fn get_available_export_targets(&self) -> Vec<ReportExportType> {
        vec![
            ReportExportType {
                key: "CSV".to_string(),
                query_parameter: "exportCSV".to_string(),
            },
            ReportExportType {
                key: "PDF".to_string(),
                query_parameter: "exportPDF".to_string(),
            },
        ]
    }

    fn get_report_params(
        &self,
        report_name: &str,
        query_params: &HashMap<String, Vec<String>>,
    ) -> Result<HashMap<String, String>, ReportError> {
        self.extract_and_validate_report_params(report_name, query_params)
    }
}

/// Port of `org.apache.fineract.infrastructure.report.provider.ReportingProcessServiceProvider`
pub struct ReportingProcessServiceProvider {
    services: HashMap<String, Arc<dyn ReportingProcessService>>,
}

impl ReportingProcessServiceProvider {
    pub fn new(reporting_process_services: Vec<Arc<dyn ReportingProcessService>>) -> Self {
        let mut services = HashMap::new();
        for service in reporting_process_services {
            for &report_type in service.report_types() {
                services.insert(report_type.to_string(), Arc::clone(&service));
            }
        }
        Self { services }
    }

    pub fn find_reporting_process_service(
        &self,
        report_type: &str,
    ) -> Option<Arc<dyn ReportingProcessService>> {
        self.services.get(report_type).cloned()
    }

    pub fn find_all_reporting_types(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.services.keys().cloned().collect();
        keys.sort();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_column_quoting_differs_between_postgres_and_mysql() {
        let pg_resolver = SqlReportParameterTypeResolver::new(true);
        assert_eq!(
            pg_resolver.get_quoted_column_name("parameter_FormatType"),
            "\"parameter_FormatType\""
        );
        assert!(pg_resolver.build_sql().contains("sp.\"parameter_FormatType\" AS format_type"));

        let mysql_resolver = SqlReportParameterTypeResolver::new(false);
        assert_eq!(
            mysql_resolver.get_quoted_column_name("parameter_FormatType"),
            "parameter_FormatType"
        );
        assert!(mysql_resolver.build_sql().contains("sp.parameter_FormatType AS format_type"));
    }

    #[test]
    fn get_report_params_validates_registered_types_and_rejects_unknown_params() {
        let mut resolver = SqlReportParameterTypeResolver::new(true);
        resolver.register_report_params(
            "Active Loans",
            HashMap::from([
                ("officeId".to_string(), "number".to_string()),
                ("startDate".to_string(), "date".to_string()),
            ]),
        );

        let service = Arc::new(BaseReportingProcessService::new(
            Arc::new(DefaultInputValidator),
            Arc::new(resolver),
            &["Table", "SMS"],
        ));

        let valid_query = HashMap::from([
            ("R_officeId".to_string(), vec!["10".to_string()]),
            ("R_startDate".to_string(), vec!["2026-01-15".to_string()]),
            ("exportCSV".to_string(), vec!["true".to_string()]),
        ]);

        let params = service.get_report_params("Active Loans", &valid_query).unwrap();
        assert_eq!(params.get("${officeId}"), Some(&"10".to_string()));
        assert_eq!(params.get("${startDate}"), Some(&"2026-01-15".to_string()));
        assert!(!params.contains_key("${exportCSV}"));

        // Unknown R_ param is rejected
        let unknown_query = HashMap::from([("R_hackParam".to_string(), vec!["1".to_string()])]);
        let err = service.get_report_params("Active Loans", &unknown_query).unwrap_err();
        assert_eq!(
            err,
            ReportError::UnknownReportParameter {
                param: "hackParam".to_string(),
                report: "Active Loans".to_string(),
            }
        );

        // Invalid number format is rejected
        let bad_number = HashMap::from([("R_officeId".to_string(), vec!["10; DROP TABLE".to_string()])]);
        assert!(matches!(
            service.get_report_params("Active Loans", &bad_number),
            Err(ReportError::InvalidParameterFormat { .. })
        ));
    }

    #[test]
    fn reporting_process_service_provider_registers_and_looks_up_types() {
        let resolver = SqlReportParameterTypeResolver::new(false);
        let svc = Arc::new(BaseReportingProcessService::new(
            Arc::new(DefaultInputValidator),
            Arc::new(resolver),
            &["Table", "Pentaho"],
        ));

        let provider = ReportingProcessServiceProvider::new(vec![svc]);
        assert_eq!(provider.find_all_reporting_types(), vec!["Pentaho", "Table"]);
        assert!(provider.find_reporting_process_service("Table").is_some());
        assert!(provider.find_reporting_process_service("Unknown").is_none());
    }
}
