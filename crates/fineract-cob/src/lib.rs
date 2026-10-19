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
// module `fineract-cob` to Rust Edition 2024.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, RwLock};
use thiserror::Error;

pub const PARTITION_PREFIX: &str = "partition_";
pub const PARTITION_COUNT_KEY: &str = "partitionCount";
pub const BUSINESS_STEPS_KEY: &str = "businessSteps";
pub const COB_PARAMETER_KEY: &str = "cobParameter";
pub const PARTITION_KEY: &str = "partition";
pub const BUSINESS_DATE_PARAMETER_NAME: &str = "businessDate";
pub const IS_CATCH_UP_PARAMETER_NAME: &str = "isCatchUp";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CobError {
    #[error("Account {0} lock cannot be overruled")]
    AccountLockCannotBeOverruledException(i64),
    #[error("Business step exception: {0}")]
    BusinessStepException(String),
    #[error("Business step does not belong to job")]
    BusinessStepNotBelongsToJobException,
    #[error("Lock cannot be applied to loan {0}")]
    LockCannotBeAppliedException(i64),
    #[error("{0}")]
    IllegalStateException(String),
}

/// Port of `org.apache.fineract.cob.domain.LockOwner`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LockOwner {
    LoanCobChunkProcessing,
    LoanCobPartitioning,
    LoanInlineCobProcessing,
}

impl LockOwner {
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoanCobChunkProcessing => "LOAN_COB_CHUNK_PROCESSING",
            Self::LoanCobPartitioning => "LOAN_COB_PARTITIONING",
            Self::LoanInlineCobProcessing => "LOAN_INLINE_COB_PROCESSING",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
    MySql,
    PostgreSql,
}

/// Port of `DatabaseSpecificSQLGenerator` for COB lock queries
#[derive(Debug, Clone)]
pub struct DatabaseSpecificSQLGenerator {
    pub database_type: DatabaseType,
}

impl DatabaseSpecificSQLGenerator {
    pub fn new(database_type: DatabaseType) -> Self {
        Self { database_type }
    }

    pub fn in_clause(&self, column: &str, count: usize) -> String {
        match self.database_type {
            DatabaseType::PostgreSql => format!("{column} = ANY (?)"),
            DatabaseType::MySql => {
                let placeholders = vec!["?"; count].join(",");
                format!("{column} IN ({placeholders})")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSqlCall {
    pub sql: String,
    pub bind_params: Vec<String>,
}

/// Port of `org.apache.fineract.cob.domain.AbstractLockingService`
pub struct AbstractLockingService {
    pub table_name: String,
    pub sql_generator: DatabaseSpecificSQLGenerator,
    pub recorded_queries: RwLock<Vec<RecordedSqlCall>>,
}

impl AbstractLockingService {
    pub fn new(table_name: impl Into<String>, database_type: DatabaseType) -> Self {
        Self {
            table_name: table_name.into(),
            sql_generator: DatabaseSpecificSQLGenerator::new(database_type),
            recorded_queries: RwLock::new(Vec::new()),
        }
    }

    fn build_in_params(&self, loan_ids: &[i64]) -> Vec<String> {
        match self.sql_generator.database_type {
            DatabaseType::PostgreSql => {
                let joined = loan_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                vec![format!("ARRAY[{joined}]::bigint[]")]
            }
            DatabaseType::MySql => loan_ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    pub fn find_lock_ids_by_loan_id_in(&self, loan_ids: &[i64]) -> Vec<i64> {
        if loan_ids.is_empty() {
            return Vec::new();
        }
        let sql = format!(
            "SELECT loan_id FROM {} WHERE {}",
            self.table_name,
            self.sql_generator.in_clause("loan_id", loan_ids.len())
        );
        let bind_params = self.build_in_params(loan_ids);
        self.recorded_queries
            .write()
            .unwrap()
            .push(RecordedSqlCall { sql, bind_params });
        loan_ids.to_vec()
    }

    pub fn find_lock_ids_by_loan_id_in_and_lock_owner(
        &self,
        loan_ids: &[i64],
        lock_owner: LockOwner,
    ) -> Vec<i64> {
        if loan_ids.is_empty() {
            return Vec::new();
        }
        let sql = format!(
            "SELECT loan_id FROM {} WHERE {} AND lock_owner = ?",
            self.table_name,
            self.sql_generator.in_clause("loan_id", loan_ids.len())
        );
        let mut bind_params = self.build_in_params(loan_ids);
        bind_params.push(lock_owner.name().to_string());
        self.recorded_queries
            .write()
            .unwrap()
            .push(RecordedSqlCall { sql, bind_params });
        loan_ids.to_vec()
    }

    pub fn delete_by_loan_id_in_and_lock_owner(&self, loan_ids: &[i64], lock_owner: LockOwner) {
        if loan_ids.is_empty() {
            return;
        }
        let sql = format!(
            "DELETE FROM {} WHERE {} AND lock_owner = ?",
            self.table_name,
            self.sql_generator.in_clause("loan_id", loan_ids.len())
        );
        let mut bind_params = self.build_in_params(loan_ids);
        bind_params.push(lock_owner.name().to_string());
        self.recorded_queries
            .write()
            .unwrap()
            .push(RecordedSqlCall { sql, bind_params });
    }
}

// ============================================================================
// Partitioning & Business Step Execution (`CommonPartitioner` & `COBBusinessStepService`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusinessStepNameAndOrder {
    pub step_order: i64,
    pub step_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct COBParameter {
    pub min_account_id: i64,
    pub max_account_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct COBPartition {
    pub min_id: i64,
    pub max_id: i64,
    pub page_no: i64,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionExecutionContext {
    pub business_steps: BTreeSet<BusinessStepNameAndOrder>,
    pub cob_parameter: COBParameter,
    pub partition_key: String,
    pub business_date: String,
    pub is_catch_up: String,
}

/// Port of `org.apache.fineract.cob.common.CommonPartitioner`
pub struct CommonPartitioner {
    pub step_name: String,
    pub manager_execution_context: RwLock<HashMap<String, i32>>,
    pub job_stopped: RwLock<bool>,
}

impl CommonPartitioner {
    pub fn new(step_name: impl Into<String>) -> Self {
        Self {
            step_name: step_name.into(),
            manager_execution_context: RwLock::new(HashMap::new()),
            job_stopped: RwLock::new(false),
        }
    }

    pub fn get_partitions(
        &self,
        _partition_size: usize,
        cob_business_steps: &BTreeSet<BusinessStepNameAndOrder>,
        retrieved_partitions: Vec<COBPartition>,
        business_date: NaiveDate,
        is_catch_up: bool,
    ) -> HashMap<String, PartitionExecutionContext> {
        if cob_business_steps.is_empty() {
            *self.job_stopped.write().unwrap() = true;
            return HashMap::new();
        }

        let mut partitions = retrieved_partitions;
        if partitions.is_empty() {
            partitions.push(COBPartition {
                min_id: 0,
                max_id: 0,
                page_no: 0,
                count: 0,
            });
        }

        self.manager_execution_context
            .write()
            .unwrap()
            .insert(PARTITION_COUNT_KEY.to_string(), partitions.len() as i32);

        let mut map = HashMap::new();
        for p in partitions {
            let key = format!("{PARTITION_PREFIX}{}", p.page_no);
            map.insert(
                key.clone(),
                PartitionExecutionContext {
                    business_steps: cob_business_steps.clone(),
                    cob_parameter: COBParameter {
                        min_account_id: p.min_id,
                        max_account_id: p.max_id,
                    },
                    partition_key: key,
                    business_date: business_date.to_string(),
                    is_catch_up: is_catch_up.to_string(),
                },
            );
        }
        map
    }

    pub fn get_partition_names(&self, _grid_size: usize) -> Result<Vec<String>, CobError> {
        let ctx = self.manager_execution_context.read().unwrap();
        let Some(&partition_count) = ctx.get(PARTITION_COUNT_KEY) else {
            return Err(CobError::IllegalStateException(format!(
                "Cannot restart partitioned step '{}': the partition count is missing from its execution context, so the original partition names cannot be reproduced. Run the loan COB catch-up endpoint instead of restarting this job.",
                self.step_name
            )));
        };

        Ok((0..partition_count)
            .map(|page_no| format!("{PARTITION_PREFIX}{page_no}"))
            .collect())
    }
}

/// Port of `org.apache.fineract.cob.COBBusinessStep`
pub trait COBBusinessStep<T>: Send + Sync {
    fn get_enum_styled_name(&self) -> &'static str;
    fn get_human_readable_name(&self) -> &'static str;
    fn execute(&self, item: T) -> Result<T, CobError>;
}

/// Port of `org.apache.fineract.cob.COBBusinessStepServiceImpl`
pub struct COBBusinessStepService<T> {
    steps: HashMap<String, Arc<dyn COBBusinessStep<T>>>,
}

impl<T> COBBusinessStepService<T> {
    pub fn new(registered: Vec<Arc<dyn COBBusinessStep<T>>>) -> Self {
        let mut steps = HashMap::new();
        for s in registered {
            steps.insert(s.get_enum_styled_name().to_string(), s);
        }
        Self { steps }
    }

    pub fn run(
        &self,
        execution_map: &BTreeSet<BusinessStepNameAndOrder>,
        mut item: T,
    ) -> Result<T, CobError> {
        if execution_map.is_empty() {
            return Err(CobError::BusinessStepException(
                "Execution map is empty".to_string(),
            ));
        }
        for step_meta in execution_map {
            let Some(step) = self.steps.get(&step_meta.step_name) else {
                return Err(CobError::BusinessStepNotBelongsToJobException);
            };
            item = step.execute(item)?;
        }
        Ok(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOAN_IDS: [i64; 3] = [10, 20, 30];
    const TABLE_NAME: &str = "test_locks";

    // ========================================================================
    // Port of AbstractLockingServiceTest
    // ========================================================================

    #[test]
    fn find_lock_ids_by_loan_id_in_on_mysql_uses_in_clause_with_individual_bind_parameters() {
        let under_test = AbstractLockingService::new(TABLE_NAME, DatabaseType::MySql);
        under_test.find_lock_ids_by_loan_id_in(&LOAN_IDS);

        let queries = under_test.recorded_queries.read().unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(
            queries[0].sql,
            "SELECT loan_id FROM test_locks WHERE loan_id IN (?,?,?)"
        );
        assert_eq!(queries[0].bind_params, vec!["10", "20", "30"]);
    }

    #[test]
    fn find_lock_ids_by_loan_id_in_on_postgres_uses_any_clause_with_array_bind_parameter() {
        let under_test = AbstractLockingService::new(TABLE_NAME, DatabaseType::PostgreSql);
        under_test.find_lock_ids_by_loan_id_in(&LOAN_IDS);

        let queries = under_test.recorded_queries.read().unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(
            queries[0].sql,
            "SELECT loan_id FROM test_locks WHERE loan_id = ANY (?)"
        );
        assert_eq!(queries[0].bind_params, vec!["ARRAY[10,20,30]::bigint[]"]);
    }

    #[test]
    fn find_lock_ids_by_loan_id_in_and_lock_owner_on_postgres_uses_any_clause_with_array_bind_parameter(
    ) {
        let under_test = AbstractLockingService::new(TABLE_NAME, DatabaseType::PostgreSql);
        under_test.find_lock_ids_by_loan_id_in_and_lock_owner(
            &LOAN_IDS,
            LockOwner::LoanCobChunkProcessing,
        );

        let queries = under_test.recorded_queries.read().unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(
            queries[0].sql,
            "SELECT loan_id FROM test_locks WHERE loan_id = ANY (?) AND lock_owner = ?"
        );
        assert_eq!(
            queries[0].bind_params,
            vec!["ARRAY[10,20,30]::bigint[]", "LOAN_COB_CHUNK_PROCESSING"]
        );
    }

    #[test]
    fn delete_by_loan_id_in_and_lock_owner_on_mysql_uses_in_clause_with_individual_bind_parameters()
    {
        let under_test = AbstractLockingService::new(TABLE_NAME, DatabaseType::MySql);
        under_test
            .delete_by_loan_id_in_and_lock_owner(&LOAN_IDS, LockOwner::LoanCobChunkProcessing);

        let queries = under_test.recorded_queries.read().unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(
            queries[0].sql,
            "DELETE FROM test_locks WHERE loan_id IN (?,?,?) AND lock_owner = ?"
        );
        assert_eq!(
            queries[0].bind_params,
            vec!["10", "20", "30", "LOAN_COB_CHUNK_PROCESSING"]
        );
    }

    #[test]
    fn find_lock_ids_by_loan_id_in_with_no_ids_skips_query() {
        let under_test = AbstractLockingService::new(TABLE_NAME, DatabaseType::PostgreSql);
        assert!(under_test.find_lock_ids_by_loan_id_in(&[]).is_empty());
        assert!(under_test.recorded_queries.read().unwrap().is_empty());
    }

    // ========================================================================
    // Port of CommonPartitioner & COBBusinessStepService Tests
    // ========================================================================

    #[test]
    fn common_partitioner_creates_fallback_partition_and_reproduces_names_on_restart() {
        let partitioner = CommonPartitioner::new("loanCobPartitionStep");
        let bdate = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();

        // Missing partitionCount on restart fails loudly
        assert!(partitioner.get_partition_names(4).is_err());

        let steps = BTreeSet::from([BusinessStepNameAndOrder {
            step_order: 1,
            step_name: "APPLY_CHARGE_TO_OVERDUE_LOANS".to_string(),
        }]);

        let partitions = partitioner.get_partitions(100, &steps, vec![], bdate, false);
        assert_eq!(partitions.len(), 1);
        assert!(partitions.contains_key("partition_0"));
        assert_eq!(
            partitioner.get_partition_names(4).unwrap(),
            vec!["partition_0".to_string()]
        );
    }
}
