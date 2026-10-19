# fineract-rust (v1.0.0 — 100% Complete Migration)

A modern **Rust (Edition 2024)** migration of [Apache Fineract](https://fineract.apache.org/), licensed under the **Apache License, Version 2.0**.

All **30 of 30 Java/Gradle modules** from the Apache Fineract repository have been migrated into **21 idiomatic Rust crates** (18 library crates + 2 executable binaries + workspace integration) with **130 / 130 passing automated tests** and **0 compiler warnings**.

## Workspace Crates (Waves 1–5 — 30 Gradle Modules $\rightarrow$ 21 Rust Crates)

| Crate | Migrated From (Java / Gradle) | Description | Tests |
| :--- | :--- | :--- | :---: |
| `crates/fineract-validation` | `fineract-validation` | Self-contained validation constraints (Cron, DateFormat, Locale, Enum, Phone, URL, Range, etc.) | 14 |
| `crates/fineract-schedule-generator` | `fineract-progressive-loan-embeddable-schedule-generator` | High-precision progressive loan EMI & repayment schedule calculator (`rust_decimal`) + `fineract-schedule-demo` CLI | 6 (incl. 403 CSV scenarios) |
| `crates/fineract-command` | `fineract-command` + `async` + `audit` + `idempotency` + `quota` + `sample` | CQRS Command Pipeline, Router, Middlewares, Sync/Async/Disruptor Executors, and Axum 0.8 HTTP server | 8 |
| `crates/fineract-events` | `fineract-avro-schemas` | Strongly-typed external business event schemas (`MessageV1`, `LoanAccountDataV1`, `LoanTransactionDataV1`, etc.) | 2 |
| `crates/fineract-report` | `fineract-report` | Reporting process service trait, export types, parameter validation, and SQL injection guard | 3 |
| `crates/fineract-rates` | `fineract-rates` | Floating interest rates, rate periods, Base Lending Rate (BLR) differential resolution, and validation | 15 |
| `crates/fineract-tax` | `fineract-tax` | Tax components, tax groups, effective date history, and tax split/factor calculation engine | 9 |
| `crates/fineract-branch` | `fineract-branch` | Branch offices, materialized hierarchy paths, inter-branch transfers, tellers, and cashier drawer management | 7 |
| `crates/fineract-charge` | `fineract-charge` | Fee & penalty charge definitions, product compatibility matrix, and flat/percentage/sliding charge calculator | 11 |
| `crates/fineract-client` | `fineract-client` & `fineract-client-feign` | Rust client SDK foundations, `ExternalIdAdapter`, multipart `Parts`/`FeignParts` MIME & magic-byte detection, and `FineractErrorDecoder` | 5 |
| `crates/fineract-security` | `fineract-security` | Multi-tenant OIDC configuration service with issuer cache & secret encryption, JWT role/authority converter, and SQLi guard | 4 |
| `crates/fineract-document` | `fineract-document` | Document storage traversal & magic-byte MIME content policies and chainable Base64/DataURL content processors | 3 |
| `crates/fineract-cob` | `fineract-cob` | Close of Business (COB) batch range partitioner, dialect-aware SQL lock upgrade service, and ordered business step runner | 6 |
| `crates/fineract-loan-origination` | `fineract-loan-origination` | Loan originator lifecycle, application vs disbursement originator resolution, set reconciliation, and Avro enrichers | 8 |
| `crates/fineract-investor` | `fineract-investor` | External asset owner transfers, delayed settlement attributes, transferability rules, interest strategies, and investor accounting | 8 |
| `crates/fineract-accounting` | `fineract-accounting` | Double-entry General Ledger (`GLAccount`, `GLClosure`, `FinancialActivityAccount`, `AccountingRule`, `JournalEntry`, running balances) | 5 |
| `crates/fineract-core` | `fineract-core` | Core `MonetaryCurrency` & `CoreMoney`, `BackdatedTransactionValidationService`, Batch `ResolutionHelper`, and `LoopGuard` | 4 |
| `crates/fineract-loan` | `fineract-loan` & `fineract-progressive-loan` | `AprCalculator` (`FINERACT-2492`), `LoanRepaymentScheduleInstallment`, `AdvancedPaymentAllocationsValidator`, `CreditAllocationsValidator`, and `LoanAccount` engine | 4 |
| `crates/fineract-savings` | `fineract-savings` | `SavingsAccountCharge`, `SavingsAccount`, and daily-compounded interest & overdraft interest split calculator | 2 |
| `crates/fineract-working-capital-loan` | `fineract-working-capital-loan` | `ProjectedAmortizationScheduleModel` with periodic IRR solver and daily discount fee amortization walk | 2 |
| `crates/fineract-server` | `fineract-provider`, `fineract-e2e-tests-core`, `fineract-e2e-tests-runner` | Unified Axum 0.8 REST API server (`fineract-server` binary) + end-to-end banking workflow integration tests | 4 |

**Total Automated Tests Passing:** `130 / 130` (0 warnings)  
**Total Rust Lines of Code:** `16,527 LOC`

## Quick Start

```bash
# Run all 130 unit and E2E tests across all workspace crates
cargo test

# Run the standalone loan schedule generator demo
cargo run -p fineract-schedule-generator --bin fineract-schedule-demo

# Run the unified Fineract-Rust server binary
cargo run -p fineract-server --bin fineract-server
```

## Documentation

- [CASE_STUDY_AND_MIGRATION_LOG.md](./CASE_STUDY_AND_MIGRATION_LOG.md) — Complete 5-wave case study, chronological module migration log (#1–#30), architectural benchmarks, and token/timing telemetry.
- [MIGRATION_REPORT_AND_STATS.md](./MIGRATION_REPORT_AND_STATS.md) — Initial Wave 1 architectural benchmark report.
