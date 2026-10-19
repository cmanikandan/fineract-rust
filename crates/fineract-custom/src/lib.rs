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

//! `fineract-custom` — Rust port of Apache Fineract's dynamic `:custom:*` extension modules:
//! - `:custom:acme:loan:job` (`AcmeJobName`, `AcmeNoopJobTasklet`)
//! - `:custom:acme:loan:cob` (`AcmeNoopBusinessStep`)
//! - `:custom:acme:loan:processor` (`AcmeLoanRepaymentScheduleTransactionProcessor`)
//! - `:custom:acme:loan:starter` (`AcmeLoanAutoConfiguration`)
//! - `:custom:acme:note:service` & `:custom:acme:note:starter` (`AcmeNoteReadPlatformService`, `AcmeNoteWritePlatformService`)
//! - `:custom:acme:event:externalevent` & `:custom:acme:event:starter` (`AcmeLoanExternalEvent`)

use fineract_cob::{COBBusinessStep, CobError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcmeJobName {
    AcmeNoopJob,
}

impl AcmeJobName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AcmeNoopJob => "ACME Noop Job",
        }
    }
}

/// Port of `com.acme.fineract.loan.job.AcmeNoopJobTasklet`.
#[derive(Debug, Default, Clone)]
pub struct AcmeNoopJobTasklet {
    pub executions: usize,
}

impl AcmeNoopJobTasklet {
    pub fn execute(&mut self) -> &'static str {
        self.executions += 1;
        "FINISHED"
    }
}

/// Port of `com.acme.fineract.loan.cob.AcmeNoopBusinessStep`.
#[derive(Debug, Default, Clone)]
pub struct AcmeNoopBusinessStep;

impl COBBusinessStep<i64> for AcmeNoopBusinessStep {
    fn execute(&self, item: i64) -> Result<i64, CobError> {
        Ok(item)
    }

    fn get_enum_styled_name(&self) -> &'static str {
        "ACME_NOOP_BUSINESS_STEP"
    }

    fn get_human_readable_name(&self) -> &'static str {
        "Execute ACME Noop Business Step"
    }
}

/// Port of `com.acme.fineract.loan.processor.AcmeLoanRepaymentScheduleTransactionProcessor`.
#[derive(Debug, Default, Clone)]
pub struct AcmeLoanRepaymentScheduleTransactionProcessor;

impl AcmeLoanRepaymentScheduleTransactionProcessor {
    pub const CODE: &'static str = "acme-loan-transaction-strategy-code";
    pub const NAME: &'static str = "ACME Loan Transaction Strategy";

    pub fn code(&self) -> &'static str {
        Self::CODE
    }

    pub fn name(&self) -> &'static str {
        Self::NAME
    }
}

/// Port of `com.acme.fineract.portfolio.note.service.AcmeNoteReadPlatformService` and `AcmeNoteWritePlatformService`.
#[derive(Debug, Default, Clone)]
pub struct AcmeNotePlatformService {
    notes: HashMap<i64, String>,
    next_id: i64,
}

impl AcmeNotePlatformService {
    pub fn new() -> Self {
        Self {
            notes: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn create_note(&mut self, note_text: impl Into<String>) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.notes
            .insert(id, format!("ACME_CUSTOM_NOTE: {}", note_text.into()));
        id
    }

    pub fn retrieve_note(&self, id: i64) -> Option<&str> {
        self.notes.get(&id).map(|s| s.as_str())
    }
}

/// Port of `com.acme.fineract.event.externalevent.AcmeLoanExternalEvent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcmeLoanExternalEvent {
    pub loan_id: i64,
    pub category: String,
    pub event_type: String,
}

impl AcmeLoanExternalEvent {
    pub fn new(loan_id: i64) -> Self {
        Self {
            loan_id,
            category: "LOAN".to_string(),
            event_type: "AcmeLoanExternalEvent".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acme_custom_modules_execute_correctly() {
        let mut tasklet = AcmeNoopJobTasklet::default();
        assert_eq!(tasklet.execute(), "FINISHED");
        assert_eq!(tasklet.executions, 1);
        assert_eq!(AcmeJobName::AcmeNoopJob.as_str(), "ACME Noop Job");

        let step = AcmeNoopBusinessStep;
        assert_eq!(step.execute(42), Ok(42));
        assert_eq!(step.get_enum_styled_name(), "ACME_NOOP_BUSINESS_STEP");

        let proc = AcmeLoanRepaymentScheduleTransactionProcessor;
        assert_eq!(proc.code(), "acme-loan-transaction-strategy-code");

        let mut note_svc = AcmeNotePlatformService::new();
        let note_id = note_svc.create_note("Custom override test");
        assert_eq!(
            note_svc.retrieve_note(note_id),
            Some("ACME_CUSTOM_NOTE: Custom override test")
        );

        let evt = AcmeLoanExternalEvent::new(99);
        assert_eq!(evt.event_type, "AcmeLoanExternalEvent");
    }
}
