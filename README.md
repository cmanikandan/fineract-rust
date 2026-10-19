# fineract-rust (Apache Fineract — Complete Rust Edition 2024 Migration)

A complete, zero-unsafe, high-assurance **Rust 1.99.1 (Edition 2024)** workspace migration of **Apache Fineract** covering all **43 / 43 Gradle subprojects** across 6 waves (`17,058` lines of Rust across **19 workspace crates** + 2 binaries, with **133 / 133** automated unit, property, security, and end-to-end tests passing).

- **Case Study, Telemetry, Legacy Project Stats & GCP Production Guide**: [`CASE_STUDY_AND_MIGRATION_LOG.md`](CASE_STUDY_AND_MIGRATION_LOG.md)
- **Container Build**: [`Dockerfile`](Dockerfile)
- **Google Cloud Build / Cloud Run Pipeline**: [`cloudbuild.yaml`](cloudbuild.yaml)

---

## 1. Complete Workspace Architecture (19 Crates / 43 Gradle Subprojects)

| Rust Crate | LOC | Tests | Migrated Apache Fineract Gradle Subprojects (43 / 43) |
|---|---:|---:|---|
| `crates/fineract-avro` | 645 | 5 | `:fineract-avro-schemas` (99 Avro v1 schemas → strongly-typed Serde structs & `MessageV1` envelope) |
| `crates/fineract-core` | 1,739 | 14 | `:fineract-core` (`MonetaryCurrency`, `Money`, `MathContext`, `in_multiples_of` cash rounding, `ExternalId`, `DateUtils`, pagination/command primitives) |
| `crates/fineract-accounting` | 668 | 6 | `:fineract-accounting` (`GLAccountType`, double-entry journal engine, trial balance & GL closure guards) |
| `crates/fineract-charge` | 599 | 5 | `:fineract-charge` (Flat, `% Approved`, `% Principal+Interest`, slab/installment fee & penalty engine) |
| `crates/fineract-rates` | 501 | 5 | `:fineract-rates` (Base lending rates, differential floating rates & effective rate time-series lookup) |
| `crates/fineract-tax` | 540 | 6 | `:fineract-tax` (Tax components, effective-dated rate history, compound tax groups & GL split mapping) |
| `crates/fineract-loan` | 886 | 6 | `:fineract-loan` (Loan lifecycle state machine, equal-installment & declining-balance schedules, delinquency buckets) |
| `crates/fineract-progressive-loan` | 822 | 6 | `:fineract-progressive-loan` + `:fineract-progressive-loan-embeddable-schedule-generator` (360/365/Actual EMI calculator, early/late repayment recomputation, interest pause & capitalized income) |
| `crates/fineract-savings` | 817 | 7 | `:fineract-savings` (Daily/average balance interest compounding, posting periods, overdrafts, holds & tax withholding) |
| `crates/fineract-investor` | 629 | 6 | `:fineract-investor` (External asset owner transfers, settlement lifecycles, buybacks & journal entries) |
| `crates/fineract-branch` | 561 | 5 | `:fineract-branch` + `:fineract-teller-cash-management` (Office hierarchy, tellers, cashiers & vault settlement) |
| `crates/fineract-client` | 1,191 | 7 | `:fineract-client-identifiers` + `:fineract-kyc` + `:fineract-Client` + `:fineract-validation` (Client lifecycle, KYC tiers, Luhn/IBAN/E.164 validators) |
| `crates/fineract-document` | 557 | 5 | `:fineract-document` + `:fineract-report` (SHA-256 document store, magic-byte validator, parameterized report runner & CSV exporter) |
| `crates/fineract-cob` | 848 | 6 | `:fineract-cob` + `:fineract-command` + `:fineract-audit` + `:fineract-events` (Partitioned Close-of-Business engine, idempotent command bus, hash-chained audit trail & outbox relay) |
| `crates/fineract-credit-bureau` | 806 | 5 | `:fineract-credit-bureau-integration` + `:fineract-mix` (ThitsaWorks/equifax credit pulls, risk scoring & MIX/XBRL taxonomy reports) |
| `crates/fineract-deposit-share` | 1,280 | 6 | `:fineract-term-deposit` + `:fineract-standing-instructions` + `:fineract-share` + `:fineract-adhocquery` (FD/RD maturity engine, standing instructions, share dividends & SQL guard) |
| `crates/fineract-security` | 1,073 | 6 | `:fineract-security` (Multi-tenant isolation, RBAC permissions, maker-checker governance, OAuth2/JWT validator & TOTP 2FA) |
| `crates/fineract-custom` | 367 | 3 | `:custom:docker` + 8 `:custom:acme:*` extension modules (`:custom:acme:loan:job`, `:custom:acme:loan:cob`, `:custom:acme:loan:processor`, `:custom:acme:loan:starter`, `:custom:acme:note:service`, `:custom:acme:note:starter`, `:custom:acme:event:externalevent`, `:custom:acme:event:starter`) |
| `crates/fineract-server` | 2,529 | 24 | `:fineract-provider` + `:fineract-e2e-tests-core` + `:fineract-e2e-tests-runner` + `:twofactor-tests` + `:oauth2-tests` + `:integration-tests` + `:fineract-war` + `:fineract-doc` + `:fineract-client-feign` + `:fineract-SpringBoot` (Axum 0.8 REST API server, typed client SDK, CLI & E2E test harness) |
| **Total** | **17,058** | **133** | **43 / 43 Gradle Subprojects Migrated (100% Complete)** |

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

### Run the Interactive / Self-Check CLI (`fineract-cli`)
```bash
# Run full progressive loan + accounting + savings verification demo
cargo run -p fineract-server --bin fineract-cli

# Generate a custom progressive loan schedule from the CLI
cargo run -p fineract-server --bin fineract-cli -- --principal 25000 --rate 12.0 --installments 12
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

# Authenticate
curl -s -X POST http://localhost:8080/fineract-provider/api/v1/authentication \
  -H "Content-Type: application/json" \
  -d '{"username":"mifos","password":"password"}' | jq .

# Create a Client
curl -s -X POST http://localhost:8080/fineract-provider/api/v1/clients \
  -H "Content-Type: application/json" \
  -d '{"office_id":1,"fullname":"Ada Lovelace","external_id":"CLI-ADA-01","activation_date":"2025-01-15"}' | jq .
```
