# fineract-rust

A high-performance Rust 2024 migration of the standalone and core pipeline modules of [Apache Fineract](https://fineract.apache.org/).

## License & Attribution

This project is a derivative work (Java-to-Rust translation and modernization) of [Apache Fineract](https://github.com/apache/fineract), licensed under the **Apache License, Version 2.0**.

Per **Sections 2 and 4 of the Apache License 2.0**:
- You are explicitly permitted to **modify, translate, fork, and redistribute** this software in source or binary form (including in private or commercial repositories).
- The original [LICENSE](./LICENSE) and [NOTICE](./NOTICE) files are preserved in the root of this repository, and all modified/translated files carry clear modification attribution notices.

---

## Workspace Architecture

`fineract-rust` is organized as a Cargo workspace using **Rust Edition 2024** (`resolver = "3"`) and modern, latest-generation Rust dependencies (`rustc 1.99.1`, `axum 0.8`, `tokio 1.53`, `thiserror 2.0`, `chrono 0.4`, `serde 1.0`, `uuid 1.28`).

| Crate | Original Java Modules | Description |
| :--- | :--- | :--- |
| `crates/fineract-schedule-generator` | `fineract-progressive-loan-embeddable-schedule-generator`, `fineract-progressive-loan`, `fineract-loan`, `fineract-core` | Exact fixed-point decimal (`BigDec` / `MathContext`), `Money` & `CurrencyData`, `DefaultScheduledDateGenerator`, `ProgressiveEMICalculator`, `EmbeddableProgressiveLoanScheduleGenerator`, and CLI demo binary (`fineract-schedule-demo`). |
| `crates/fineract-validation` | `fineract-validation` | Zero-allocation validator suite: `DateFormatValidator` (full `java.time.format.DateTimeFormatter` pattern parser), `EnumValueValidator`, `LocaleValidator` (ISO 639/3166 & BCP-47), and `LocalDateValidator` (strict `ResolverStyle.STRICT` calendar resolver including leap-year rules). |
| `crates/fineract-command` | `fineract-command`, `fineract-command-async`, `fineract-command-audit`, `fineract-command-disruptor`, `fineract-command-jdbc`, `fineract-command-test` | Full CQRS command pipeline: `Command<Req, Res>`, `DefaultCommandHandlerManager`, `DefaultCommandHookManager`, `SynchronousCommandDispatcher` (with idempotency and fallback error recovery), `AsyncCommandDispatcher` (Tokio work-stealing pool), `DisruptorCommandDispatcher` (bounded high-throughput MPSC ring buffer), lifecycle hooks (`Timestamp`, `Username`, `ServletHeaders`, `Audit`), `JdbcCommandStore`, and an **Axum 0.8** REST API (`DummyApiController`). |

---

## Building, Running & Testing

```bash
# Run the complete unit and integration test suite (28 tests across all 3 crates)
cargo test

# Run the standalone Progressive Loan Schedule Generator CLI demo
cargo run --bin fineract-schedule-demo
```

### Sample CLI Output (`cargo run --bin fineract-schedule-demo`)

```text
#------ Loan Schedule -----------------#
  Number of Periods: 6
  Loan Term in Days: 182
  Total Disbursed Amount: 100.00
  Total Interest Amount: 2.05
  Total Repayment Amount: 102.05
#------ Repayment Schedule ------------#
  Disbursement - Date: 2024-01-01, Amount: 100.00
  Repayment Period: #1, Due Date: 2024-02-01, Balance: 83.57, Principal: 16.43, Interest: 0.58, Total: 17.01, Total Outstanding Balance: 85.04
  Repayment Period: #2, Due Date: 2024-03-01, Balance: 67.05, Principal: 16.52, Interest: 0.49, Total: 17.01, Total Outstanding Balance: 68.03
  Repayment Period: #3, Due Date: 2024-04-01, Balance: 50.43, Principal: 16.62, Interest: 0.39, Total: 17.01, Total Outstanding Balance: 51.02
  Repayment Period: #4, Due Date: 2024-05-01, Balance: 33.71, Principal: 16.72, Interest: 0.29, Total: 17.01, Total Outstanding Balance: 34.01
  Repayment Period: #5, Due Date: 2024-06-01, Balance: 16.90, Principal: 16.81, Interest: 0.20, Total: 17.01, Total Outstanding Balance: 17.00
  Repayment Period: #6, Due Date: 2024-07-01, Balance: 0.00, Principal: 16.90, Interest: 0.10, Total: 17.00, Total Outstanding Balance: 0.00
```

See [MIGRATION_REPORT_AND_STATS.md](./MIGRATION_REPORT_AND_STATS.md) for the complete migration telemetry, token/timing statistics, and architectural comparison between Java 21 / Spring Boot 3 and Rust 2024.
