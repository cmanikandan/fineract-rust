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
// MODIFICATION NOTICE: Translated and adapted from Apache Fineract (Java/Avro)
// module `fineract-avro-schemas` to Rust Edition 2024.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventSerializationError {
    #[error("Failed to serialize/deserialize event payload: {0}")]
    CodecError(#[from] serde_json::Error),
}

/// Port of `org.apache.fineract.avro.generator.ByteBufferSerializable`
pub trait ByteBufferSerializable: Serialize + DeserializeOwned {
    fn schema_fqn() -> &'static str;

    fn to_byte_buffer(&self) -> Result<Vec<u8>, EventSerializationError> {
        Ok(serde_json::to_vec(self)?)
    }

    fn from_byte_buffer(bytes: &[u8]) -> Result<Self, EventSerializationError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Port of `MessageV1.avsc` (`org.apache.fineract.avro.MessageV1`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageV1 {
    /// The ID of the message to be sent
    pub id: i64,
    /// The source of the event (e.g., fineract-loan)
    pub source: String,
    /// The type of event the payload refers to (e.g., LoanApprovedBusinessEvent)
    #[serde(rename = "type")]
    pub event_type: String,
    /// The category of event the payload refers to (e.g., LOAN)
    pub category: String,
    /// The UTC time of when the event has been raised in ISO_LOCAL_DATE_TIME format
    pub created_at: String,
    /// The business date when the event has been raised in ISO_LOCAL_DATE format
    pub business_date: String,
    /// The tenantId that the event has been sent from (e.g., default)
    pub tenant_id: String,
    /// The idempotency key for this particular event for consumer de-duplication
    pub idempotency_key: String,
    /// The fully qualified name of the schema of the event payload
    pub dataschema: String,
    /// The payload data serialized into bytes
    pub data: Vec<u8>,
}

impl MessageV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn from_payload<T: ByteBufferSerializable>(
        id: i64,
        source: impl Into<String>,
        event_type: impl Into<String>,
        category: impl Into<String>,
        created_at: impl Into<String>,
        business_date: impl Into<String>,
        tenant_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: &T,
    ) -> Result<Self, EventSerializationError> {
        Ok(Self {
            id,
            source: source.into(),
            event_type: event_type.into(),
            category: category.into(),
            created_at: created_at.into(),
            business_date: business_date.into(),
            tenant_id: tenant_id.into(),
            idempotency_key: idempotency_key.into(),
            dataschema: T::schema_fqn().to_string(),
            data: payload.to_byte_buffer()?,
        })
    }

    pub fn decode_payload<T: ByteBufferSerializable>(&self) -> Result<T, EventSerializationError> {
        T::from_byte_buffer(&self.data)
    }
}

impl ByteBufferSerializable for MessageV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.MessageV1"
    }
}

/// Port of `BulkMessageItemV1.avsc` (`org.apache.fineract.avro.BulkMessageItemV1`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkMessageItemV1 {
    pub id: i64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub category: String,
    pub dataschema: String,
    pub data: Vec<u8>,
}

impl ByteBufferSerializable for BulkMessageItemV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.BulkMessageItemV1"
    }
}

/// Port of `BulkMessagePayloadV1.avsc` (`org.apache.fineract.avro.BulkMessagePayloadV1`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkMessagePayloadV1 {
    pub datas: Vec<BulkMessageItemV1>,
}

impl ByteBufferSerializable for BulkMessagePayloadV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.BulkMessagePayloadV1"
    }
}

// ============================================================================
// Core Domain Event Schemas (Ported from fineract-avro-schemas/src/main/avro)
// ============================================================================

/// Port of `generic/v1/CurrencyDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyDataV1 {
    pub code: Option<String>,
    pub name: Option<String>,
    pub decimal_places: Option<i32>,
    pub in_multiples_of: Option<i32>,
    pub display_symbol: Option<String>,
    pub name_code: Option<String>,
    pub display_label: Option<String>,
}

impl ByteBufferSerializable for CurrencyDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.generic.v1.CurrencyDataV1"
    }
}

/// Port of `generic/v1/EnumOptionDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumOptionDataV1 {
    pub id: Option<i64>,
    pub code: Option<String>,
    pub value: Option<String>,
}

impl ByteBufferSerializable for EnumOptionDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.generic.v1.EnumOptionDataV1"
    }
}

/// Port of `generic/v1/CommandProcessingResultV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProcessingResultV1 {
    pub office_id: Option<i64>,
    pub group_id: Option<i64>,
    pub client_id: Option<i64>,
    pub loan_id: Option<i64>,
    pub savings_id: Option<i64>,
    pub sub_resource_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub changes: Option<HashMap<String, String>>,
    pub product_id: Option<i64>,
    pub gsim_id: Option<i64>,
    pub glim_id: Option<i64>,
    pub rollback_transaction: Option<bool>,
}

impl ByteBufferSerializable for CommandProcessingResultV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.generic.v1.CommandProcessingResultV1"
    }
}

/// Port of `office/v1/OfficeDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub name_decorated: Option<String>,
    pub external_id: Option<String>,
    pub opening_date: Option<String>,
    pub hierarchy: Option<String>,
    pub parent_id: Option<i64>,
    pub parent_name: Option<String>,
}

impl ByteBufferSerializable for OfficeDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.office.v1.OfficeDataV1"
    }
}

/// Port of `portfolio/v1/RateDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub percentage: Option<String>,
    pub product_apply: Option<EnumOptionDataV1>,
    pub active: Option<bool>,
}

impl ByteBufferSerializable for RateDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.portfolio.v1.RateDataV1"
    }
}

/// Port of `portfolio/v1/ChargeDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargeDataV1 {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub active: Option<bool>,
    pub penalty: Option<bool>,
    pub currency: Option<CurrencyDataV1>,
    pub amount: Option<String>,
    pub charge_time_type: Option<EnumOptionDataV1>,
    pub charge_applies_to: Option<EnumOptionDataV1>,
    pub charge_calculation_type: Option<EnumOptionDataV1>,
    pub charge_payment_mode: Option<EnumOptionDataV1>,
}

impl ByteBufferSerializable for ChargeDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.portfolio.v1.ChargeDataV1"
    }
}

/// Port of `loan/v1/LoanAmountDataV1.avsc`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanAmountDataV1 {
    pub total: Option<String>,
    pub principal: Option<String>,
    pub interest: Option<String>,
    pub fee_charges: Option<String>,
    pub penalty_charges: Option<String>,
    pub overpayment: Option<String>,
}

impl ByteBufferSerializable for LoanAmountDataV1 {
    fn schema_fqn() -> &'static str {
        "org.apache.fineract.avro.loan.v1.LoanAmountDataV1"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_v1_roundtrip_and_payload_decoding() {
        let charge_event = ChargeDataV1 {
            id: Some(42),
            name: Some("Disbursement Fee".to_string()),
            active: Some(true),
            penalty: Some(false),
            currency: Some(CurrencyDataV1 {
                code: Some("USD".to_string()),
                name: Some("US Dollar".to_string()),
                decimal_places: Some(2),
                in_multiples_of: Some(1),
                display_symbol: Some("$".to_string()),
                name_code: Some("currency.USD".to_string()),
                display_label: Some("US Dollar ($)".to_string()),
            }),
            amount: Some("25.00".to_string()),
            charge_time_type: Some(EnumOptionDataV1 {
                id: Some(1),
                code: Some("chargeTimeType.disbursement".to_string()),
                value: Some("Disbursement".to_string()),
            }),
            charge_applies_to: None,
            charge_calculation_type: None,
            charge_payment_mode: None,
        };

        let msg = MessageV1::from_payload(
            1001,
            "fineract-charge",
            "ChargeCreatedBusinessEvent",
            "CHARGE",
            "2026-10-19T01:20:00",
            "2026-10-19",
            "default",
            "idem-key-001",
            &charge_event,
        )
        .unwrap();

        assert_eq!(
            msg.dataschema,
            "org.apache.fineract.avro.portfolio.v1.ChargeDataV1"
        );

        let raw_bytes = msg.to_byte_buffer().unwrap();
        let decoded_msg = MessageV1::from_byte_buffer(&raw_bytes).unwrap();
        assert_eq!(decoded_msg, msg);

        let decoded_payload: ChargeDataV1 = decoded_msg.decode_payload().unwrap();
        assert_eq!(decoded_payload, charge_event);
    }

    #[test]
    fn bulk_message_payload_v1_roundtrip() {
        let office = OfficeDataV1 {
            id: Some(1),
            name: Some("Head Office".to_string()),
            name_decorated: Some("Head Office".to_string()),
            external_id: Some("HO-01".to_string()),
            opening_date: Some("2020-01-01".to_string()),
            hierarchy: Some(".".to_string()),
            parent_id: None,
            parent_name: None,
        };

        let item = BulkMessageItemV1 {
            id: 1,
            event_type: "OfficeCreatedBusinessEvent".to_string(),
            category: "OFFICE".to_string(),
            dataschema: OfficeDataV1::schema_fqn().to_string(),
            data: office.to_byte_buffer().unwrap(),
        };

        let bulk = BulkMessagePayloadV1 { datas: vec![item] };
        let bytes = bulk.to_byte_buffer().unwrap();
        let restored = BulkMessagePayloadV1::from_byte_buffer(&bytes).unwrap();
        assert_eq!(restored.datas.len(), 1);
        let restored_office = OfficeDataV1::from_byte_buffer(&restored.datas[0].data).unwrap();
        assert_eq!(restored_office, office);
    }
}
