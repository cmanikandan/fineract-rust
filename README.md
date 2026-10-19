# fineract-rust

A high-performance **Rust Edition 2024** migration of [Apache Fineract](https://fineract.apache.org/) modules.

## License & Attribution

This project is a derivative work (Java-to-Rust translation and modernization) of [Apache Fineract](https://github.com/apache/fineract), licensed under the **Apache License, Version 2.0**.

Per **Sections 2 and 4 of the Apache License 2.0**:
- You are explicitly permitted to **modify, translate, fork, and redistribute** this software in source or binary form (including in private or commercial repositories).
- The original [LICENSE](./LICENSE) and [NOTICE](./NOTICE) files are preserved in the root of this repository, and all modified/translated files carry clear modification attribution notices.

---

## Workspace Architecture (Waves 1 & 2 — 14 Gradle Modules → 9 Rust Crates)

`fineract-rust` is organized as a Cargo workspace using **Rust Edition 2024** (`resolver = "3"`) and modern, latest-generation Rust dependencies (`rustc 1.99.1`, `axum 0.8`, `tokio 1.53`, `thiserror 2.0`, `chrono 0.4`, `serde 1.0`, `uuid 1.28`).

| Wave | Crate | Original Java Modules | Rust LOC | Tests |
| :---: | :--- | :--- | :---: | :---: |
| **Wave 1** | `crates/fineract-validation` | `fineract-validation` | 967 | 14 |
| **Wave 1** | `crates/fineract-schedule-generator` | `fineract-progressive-loan-embeddable-schedule-generator` (+ Progressive EMI & `BigDec`/`Money`) | 2,769 | 6 (+ CLI) |
| **Wave 1** | `crates/fineract-command` | `fineract-command`, `-async`, `-audit`, `-disruptor`, `-jdbc`, `-test` | 1,697 | 8 |
| **Wave 2** | `crates/fineract-events` | `fineract-avro-schemas` (`MessageV1`, `BulkMessagePayloadV1`, `ByteBufferSerializable`) | 362 | 2 |
| **Wave 2** | `crates/fineract-report` | `fineract-report` (`ReportingProcessServiceProvider`, `ReportParameterTypeResolver`) | 384 | 3 |
| **Wave 2** | `crates/fineract-rates` | `fineract-rates` (`FloatingRate`, BLR differential periods, validators, Axum API) | 941 | 15 |
| **Wave 2** | `crates/fineract-tax` | `fineract-tax` (`TaxComponent`, `TaxGroup`, `TaxUtils`, `ChargeTaxApplicationService`, `TaxValidator`) | 615 | 9 |
| **Wave 2** | `crates/fineract-branch` | `fineract-branch` (`Teller`, `Cashier`, `CashierTxnType`, `DateRange`, JSON deserializer) | 622 | 7 |
| **Wave 2** | `crates/fineract-charge` | `fineract-charge` (`Charge`, `ChargeAppliesTo`, `ChargeTimeType`, `ChargeCalculationType`, `ChargePaymentMode`) | 875 | 11 |
| **Total** | **9 Workspace Crates** | **14 Apache Fineract Gradle Modules** | **9,232 LOC** | **75 / 75 Passing** |

---

## Building, Running & Testing

```bash
# Run the complete unit and integration test suite (75 tests across all 9 crates)
cargo test

# Run the standalone Progressive Loan Schedule Generator CLI demo
cargo run --bin fineract-schedule-demo
```

See [CASE_STUDY_AND_MIGRATION_LOG.md](./CASE_STUDY_AND_MIGRATION_LOG.md) for the chronological module-by-module case study, token/timing telemetry ledger, and Waves 3–6 roadmap.
