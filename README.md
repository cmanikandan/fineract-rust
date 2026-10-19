# fineract-rust

A modern **Rust (Edition 2024)** migration of [Apache Fineract](https://fineract.apache.org/), licensed under the **Apache License, Version 2.0**.

## Workspace Crates (Waves 1, 2 & 3 — 19 Gradle Modules Migrated into 13 Rust Crates)

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

**Total Automated Tests Passing:** `93 / 93` (0 warnings)

## Quick Start

```bash
# Run all 93 unit and integration tests across all 13 workspace crates
cargo test

# Run the standalone loan schedule generator demo
cargo run -p fineract-schedule-generator --bin fineract-schedule-demo
```

## Documentation

- [CASE_STUDY_AND_MIGRATION_LOG.md](./CASE_STUDY_AND_MIGRATION_LOG.md) — Running case study, chronological module migration log, and token/timing telemetry across Waves 1, 2, and 3.
- [MIGRATION_REPORT.md](./MIGRATION_REPORT.md) — Initial Wave 1 architectural benchmark report.
