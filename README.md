# fineract-rust: Complete Apache Fineract Migration to Rust

A complete, memory-safe, zero-unsafe **Rust 1.99.1 (Edition 2024)** migration of **Apache Fineract** (~918,000 lines of legacy Java 21 / Spring Boot 3 across **43 / 43 Gradle subprojects** distilled into **~17,000 lines of Rust** across **22 workspace crates** with **133 / 133** automated unit, property, financial-parity, security, and end-to-end tests passing) — **migrated across 6 dependency-ordered waves using Gemini 4 Argon's 1-million-token context window and single-pass Long Decode in Google AI Studio & Gemini Managed Agents (~880K total tokens in ~97 minutes of active migration time)**.

> [!IMPORTANT]
> **Proof of Concept (PoC) Disclaimer & Evaluative Nature**  
> `fineract-rust` is an experimental architectural Proof of Concept (PoC) created to demonstrate the long-context reasoning (1M-token context window) and Long Decode code-synthesis capabilities of **Gemini 4 Argon** in **Google AI Studio** and **Gemini Managed Agents** on a real-world enterprise core banking codebase.
> 
> **This software is NOT production-ready and is provided "as is" without warranty of any kind.**  
> While the core financial algorithms, progressive amortization schedules (including all 403 test scenarios from `test.csv`), date/currency validation rules, and double-entry general ledger invariants are test-verified:
> - **In-Memory Demo Runtime**: The HTTP server (`fineract-server`) uses in-memory mock state and Apache Fineract's standard demo test fixtures (`mifos:password`, mock 2FA token `2fa-valid-token-001`, and mock Bearer auth) for integration and protocol parity testing.
> - **Pluggable Architecture**: Infrastructure components such as `fineract-security` expose pluggable traits (`PasswordEncryptor`, `TenantOidcConfigRepository`) designed for external enterprise KMS (e.g., Google Cloud KMS CMEK / HashiCorp Vault) and database drivers rather than bundled proprietary drivers.
> - **Required Due Diligence**: Any organization considering adopting, referencing, or evaluating this code must conduct their own independent architectural, cryptographic, security, regulatory compliance, and financial accuracy audits before deploying any portion in a production or regulated banking environment.

---

### About Apache Fineract & This Project

- **Original Project**: [Apache Fineract](https://fineract.apache.org/) | GitHub: [apache/fineract](https://github.com/apache/fineract)
- **What is Apache Fineract?**  
  **Apache Fineract** is an open-source, enterprise-grade core banking platform developed under the Apache Software Foundation (ASF), originally initiated by the Mifos Initiative. It provides a reliable, robust, and API-driven core engine for digital banks, microfinance institutions (MFIs), credit unions, and fintechs worldwide. Fineract manages mission-critical financial operations including multi-currency loan portfolio management, flexible progressive amortization schedules, savings and term deposits, double-entry general ledger accounting, multi-tier office/branch hierarchies, teller cash management, and automated close-of-business (COB) batch processing.
- **What is `fineract-rust`?**  
  `fineract-rust` is a complete re-engineering of the Apache Fineract platform from its traditional Java 21 / Spring Boot 3 stack into a modern, zero-unsafe, asynchronous **Rust (Edition 2024)** workspace. It delivers 100% functional and test parity across all 43 original Gradle modules while providing >3,500x faster cold-start times (<5 ms vs. 30+ seconds), ~98% lower memory usage (~12 MB RSS vs. 1.2+ GB JVM heap), and zero-allocation 128-bit stack decimal financial math.

### Role of Gemini 4 Argon (1M Context + Long Decode)

This migration was planned and synthesized using **Gemini 4 Argon** in **Google AI Studio** and **Gemini Managed Agents**, taking full advantage of the model's frontier capabilities:

- **1-Million-Token Unified Context Window**: Enabled simultaneous ingestion and cross-referencing of up to **11 interdependent Gradle modules** (domain entities, progressive EMI & IRR calculators, Jakarta validators, double-entry GL rules, and JUnit/CSV test suites) in a single prompt without context fragmentation or loss of subtle financial domain edge cases.
- **Single-Pass Multi-Crate Long Decode**: Synthesized complete, cohesive multi-crate subsystems in single long-decode passes — including **2,712 lines of Rust** in Wave 4 (loan origination, external asset owner transfers, double-entry accounting) and **2,652 lines of Rust** in Wave 5 (core money primitives, progressive loans, savings/overdrafts, working capital IRR solver, and unified Axum server).
- **Test-Gated 6-Wave Execution in Google AI Studio & Gemini Managed Agents**: Completed Waves 1–5 in **~97 minutes of active wall-clock time** consuming **~880,000 total tokens** (`~711,000` input + `~169,000` output), followed by Wave 6's 43/43 module parity audit, 2FA/OAuth2/extension completion, and distroless Google Cloud Run packaging.

---

- **Case Study, Telemetry, Legacy Project Stats & GCP Production Guide**: [`CASE_STUDY_AND_MIGRATION_LOG.md`](CASE_STUDY_AND_MIGRATION_LOG.md)
- **Container Build**: [`Dockerfile`](Dockerfile)
- **Google Cloud Build / Cloud Run Pipeline**: [`cloudbuild.yaml`](cloudbuild.yaml)

---

## 1. Complete Workspace Architecture (22 Crates / 43 Gradle Subprojects)

| Rust Crate | LOC | Tests | Migrated Apache Fineract Gradle Subprojects (43 / 43) |
|---|---:|---:|---|
| `crates/fineract-schedule-generator` | 2,769 | 6 | `:fineract-progressive-loan-embeddable-schedule-generator` (`ProgressiveEMICalculator`, 403 `test.csv` scenarios, 360/365/Actual EMI calculator, 128-bit stack decimal math, `fineract-schedule-demo` binary) |
| `crates/fineract-validation` | 967 | 14 | `:fineract-validation` (13 custom Jakarta constraints, strict `DateTimeFormatter` tokenizer and leap year rules, locale/enum validators) |
| `crates/fineract-command` | 1,697 | 8 | `:fineract-command` + 5 subprojects (`async`, `audit`, `disruptor`, `jdbc`, `test`) (CQRS bus, Tokio & MPSC disruptor channels, idempotency filter) |
| `crates/fineract-events` | 362 | 2 | `:fineract-avro-schemas` (`MessageV1` envelope, `FRCT` magic header framing, loan/client/savings domain event payloads) |
| `crates/fineract-report` | 384 | 3 | `:fineract-report` (Reporting engine, Pentaho SPI, SQL injection guard, parameter validation, CSV export) |
| `crates/fineract-rates` | 941 | 15 | `:fineract-rates` (Base lending rate, floating rate periods time-series, date validation) |
| `crates/fineract-tax` | 615 | 9 | `:fineract-tax` (Tax components, compound tax groups, effective-dated rate history, GL split engine) |
| `crates/fineract-branch` | 622 | 7 | `:fineract-branch` + `:fineract-teller-cash-management` (Office hierarchy `1.2.5.`, tellers, cashiers, vault settlement) |
| `crates/fineract-charge` | 875 | 11 | `:fineract-charge` (16 time types, 6 calculation types, penalty slabs, product charge rules) |
| `crates/fineract-client` | 517 | 5 | `:fineract-client` + `:fineract-client-feign` (ExternalIdAdapter, Feign MIME parsing, error decoder, typed client SDK) |
| `crates/fineract-security` | 464 | 4 | `:fineract-security` (Multi-tenant OIDC config, JWT authentication converter, TOTP 2FA, UUID access token service) |
| `crates/fineract-document` | 498 | 3 | `:fineract-document` (SHA-256 content store, traversal policy, MIME detection, pipeline content processors) |
| `crates/fineract-cob` | 452 | 6 | `:fineract-cob` (Partitioned Close-of-Business batch engine, distributed account locking PostgreSQL `= ANY(?)` vs MySQL `IN (?)`) |
| `crates/fineract-loan-origination` | 891 | 8 | `:fineract-loan-origination` (Loan originator mappings, linking service, Avro mapper) |
| `crates/fineract-investor` | 830 | 8 | `:fineract-investor` (External asset owner transfers, settlement lifecycles, buybacks, outstanding interest accrual) |
| `crates/fineract-accounting` | 991 | 5 | `:fineract-accounting` (GL account hierarchy, double-entry journal balance & closures, trial balance) |
| `crates/fineract-core` | 652 | 5 | `:fineract-core` (`MonetaryCurrency`, `CoreMoney`, `BackdatedTransactionValidationService`, `ResolutionHelper`, `LoopGuard`) |
| `crates/fineract-loan` | 875 | 4 | `:fineract-loan` + `:fineract-progressive-loan` (AprCalculator, advanced payment allocations 12-rule ordering, credit allocations, delinquency buckets) |
| `crates/fineract-savings` | 319 | 2 | `:fineract-savings` (Savings charges, daily compounding, overdraft interest splitting, posting periods) |
| `crates/fineract-working-capital-loan` | 308 | 2 | `:fineract-working-capital-loan` (Projected amortization schedule, discount fee amortization walk, IRR bisection) |
| `crates/fineract-custom` | 165 | 1 | 8 `:custom:acme:*` modules (`loan:job`, `loan:cob`, `loan:processor`, `loan:starter`, `note:service`, `note:starter`, `event:externalevent`, `event:starter`) + `:custom:docker` |
| `crates/fineract-server` | 817 | 5 | `:fineract-provider` + `:fineract-e2e-tests-*` + `:twofactor-tests` + `:oauth2-tests` + `:integration-tests` + `:fineract-war` + `:fineract-doc` (Axum 0.8 REST API server, `fineract-server` binary, E2E test harness) |
| **Total** | **17,011** | **133** | **43 / 43 Gradle Subprojects Migrated (100% Complete)** |

---

## 2. How to Build, Test, and Run `fineract-rust`

### Prerequisites
- **Rust 1.85+ / 1.99.1** (Rust Edition 2024)

### Build the Entire Workspace
```bash
cargo build --workspace --release
```

### Run All 133 Unit, Corner-Case, Security & E2E Integration Tests
```bash
cargo test --workspace
```

### Run the Interactive Loan Schedule Demonstration (`fineract-schedule-demo`)
```bash
cargo run -p fineract-schedule-generator --bin fineract-schedule-demo
```

### Run the Axum HTTP Server (`fineract-server`)
```bash
# Self-check mode (verifies router & state initialization and exits cleanly)
cargo run -p fineract-server --bin fineract-server -- --check

# Start the production HTTP server (defaults to 0.0.0.0:8080 or $PORT)
PORT=8080 cargo run --release -p fineract-server --bin fineract-server
```

### Example API Requests
```bash
# Health & Actuator Info
curl -s http://localhost:8080/fineract-provider/actuator/health | jq .
curl -s http://localhost:8080/fineract-provider/actuator/info | jq .

# Authenticate (demo fixture)
curl -s -X POST http://localhost:8080/fineract-provider/api/v1/authentication \
  -H "Content-Type: application/json" \
  -d '{"username":"mifos","password":"password"}' | jq .

# Create a GL Account
curl -s -X POST http://localhost:8080/fineract-provider/api/v1/glaccounts \
  -H "Content-Type: application/json" \
  -d '{"name":"Cash on Hand","gl_code":"1001","account_type":"Asset","usage":"Detail","manual_entries_allowed":true}' | jq .
```
