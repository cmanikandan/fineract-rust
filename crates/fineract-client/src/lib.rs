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
// modules `fineract-client` and `fineract-client-feign` to Rust Edition 2024.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

pub const TENANT_HEADER_NAME: &str = "Fineract-Platform-TenantId";

/// Port of `org.apache.fineract.client.models.ExternalId` and `ExternalIdAdapter`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalId {
    value: Option<String>,
}

impl ExternalId {
    pub fn new() -> Self {
        Self { value: None }
    }

    pub fn value(mut self, val: Option<impl Into<String>>) -> Self {
        self.value = val.map(|v| v.into());
        self
    }

    pub fn get_value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

impl Serialize for ExternalId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.value {
            Some(v) => serializer.serialize_str(v),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for ExternalId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let opt = Option::<String>::deserialize(deserializer)?;
        Ok(Self { value: opt })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdModule {
    pub module_name: &'static str,
}

pub struct ExternalIdAdapter;

impl ExternalIdAdapter {
    pub fn create_module() -> ExternalIdModule {
        ExternalIdModule {
            module_name: "ExternalIdModule",
        }
    }

    pub fn write_value_as_string(ext: Option<&ExternalId>) -> Result<String, serde_json::Error> {
        match ext {
            Some(e) => serde_json::to_string(e),
            None => Ok("null".to_string()),
        }
    }

    pub fn read_value(json: &str) -> Result<Option<ExternalId>, serde_json::Error> {
        if json.trim() == "null" {
            return Ok(None);
        }
        let parsed: ExternalId = serde_json::from_str(json)?;
        if parsed.value.is_none() {
            Ok(None)
        } else {
            Ok(Some(parsed))
        }
    }
}

/// Port of `org.apache.fineract.client.util.Parts` and `FeignParts`
pub struct Parts;

impl Parts {
    pub fn media_type(file_name: Option<&str>) -> Option<&'static str> {
        let name = file_name?;
        if name.is_empty() {
            return None;
        }
        let dot_pos = name.rfind('.')?;
        if dot_pos == name.len() - 1 {
            return None;
        }
        let ext = name[dot_pos + 1..].to_ascii_lowercase();
        let mime = match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "tif" | "tiff" => "image/tiff",
            "pdf" => "application/pdf",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "doc" => "application/msword",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xls" => "application/vnd.ms-excel",
            "odt" => "application/vnd.oasis.opendocument.text",
            "ods" => "application/vnd.oasis.opendocument.spreadsheet",
            "txt" => "text/plain",
            _ => "application/octet-stream",
        };
        Some(mime)
    }

    pub fn probe_content_type(path: &Path) -> String {
        if let Ok(bytes) = fs::read(path) {
            if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return "image/jpeg".to_string();
            }
            if bytes.starts_with(b"%PDF-") {
                return "application/pdf".to_string();
            }
            if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return "image/png".to_string();
            }
            if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                return "image/gif".to_string();
            }
        }
        let file_name = path.file_name().and_then(|s| s.to_str());
        Self::media_type(file_name)
            .unwrap_or("application/octet-stream")
            .to_string()
    }

    pub fn file_name(headers: Option<&HashMap<String, Vec<String>>>) -> Option<String> {
        let map = headers?;
        let values = map
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Content-Disposition"))
            .map(|(_, v)| v)?;
        let header_val = values.first()?;
        for part in header_val.split(';') {
            let trimmed = part.trim();
            if let Some(rest) = trimmed.strip_prefix("filename=") {
                let unquoted = rest.trim_matches('"').trim();
                if !unquoted.is_empty() {
                    return Some(unquoted.to_string());
                }
            }
        }
        None
    }
}

pub type FeignParts = Parts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestInfo {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponseInfo {
    pub status: u16,
    pub reason: String,
    pub request: HttpRequestInfo,
    pub headers: Option<HashMap<String, Vec<String>>>,
    pub body: Option<Vec<u8>>,
}

/// Port of `org.apache.fineract.client.feign.FeignException`
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{}", self.get_message())]
pub struct FeignException {
    status: u16,
    base_message: String,
    request: HttpRequestInfo,
    response_body: Option<Vec<u8>>,
    developer_message: Option<String>,
    user_message: Option<String>,
}

impl FeignException {
    pub fn new(
        status: u16,
        base_message: impl Into<String>,
        request: HttpRequestInfo,
        response_body: Option<Vec<u8>>,
        developer_message: Option<String>,
        user_message: Option<String>,
    ) -> Self {
        Self {
            status,
            base_message: base_message.into(),
            request,
            response_body,
            developer_message,
            user_message,
        }
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn get_developer_message(&self) -> Option<&str> {
        self.developer_message.as_deref()
    }

    pub fn get_user_message(&self) -> Option<&str> {
        self.user_message.as_deref()
    }

    pub fn request(&self) -> &HttpRequestInfo {
        &self.request
    }

    pub fn response_body(&self) -> Option<&[u8]> {
        self.response_body.as_deref()
    }

    pub fn response_body_as_string(&self) -> Option<String> {
        self.response_body
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).to_string())
    }

    pub fn get_message(&self) -> String {
        let mut msg = format!("[{}] {}", self.status, self.base_message);
        if let Some(um) = &self.user_message {
            msg.push_str(&format!(" | userMessage: {um}"));
        }
        if let Some(dm) = &self.developer_message {
            msg.push_str(&format!(" | developerMessage: {dm}"));
        }
        msg
    }
}

/// Port of `org.apache.fineract.client.feign.FineractErrorDecoder`
#[derive(Debug, Default, Clone)]
pub struct FineractErrorDecoder;

impl FineractErrorDecoder {
    pub fn decode(&self, method_key: &str, response: &HttpResponseInfo) -> FeignException {
        let mut dev_msg = None;
        let mut user_msg = None;

        if let Some(body_bytes) = &response.body {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body_bytes) {
                dev_msg = json
                    .get("developerMessage")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                user_msg = json
                    .get("userMessage")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }

        FeignException::new(
            response.status,
            format!("Error in {method_key}: {}", response.reason),
            response.request.clone(),
            response.body.clone(),
            dev_msg,
            user_msg,
        )
    }
}

/// Port of `BasicAuthRequestInterceptor` and `TenantIdRequestInterceptor`
#[derive(Debug, Clone)]
pub struct FineractClientConfig {
    pub base_url: String,
    pub tenant_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
}

impl Default for FineractClientConfig {
    fn default() -> Self {
        Self {
            base_url: "https://localhost:8443/fineract-provider/api/v1".to_string(),
            tenant_id: "default".to_string(),
            username: None,
            password: None,
            connect_timeout_ms: 10_000,
            read_timeout_ms: 60_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> HttpRequestInfo {
        HttpRequestInfo {
            method: "GET".to_string(),
            url: "/api/test".to_string(),
            headers: HashMap::new(),
        }
    }

    // ========================================================================
    // Port of PartsTest & FeignPartsTest
    // ========================================================================

    #[test]
    fn valid_and_edge_case_media_types() {
        assert_eq!(Parts::media_type(Some("test.jpg")), Some("image/jpeg"));
        assert_eq!(Parts::media_type(Some("test.")), None);
        assert_eq!(Parts::media_type(Some("")), None);
        assert_eq!(Parts::media_type(None), None);

        // Supported image formats
        assert_eq!(FeignParts::media_type(Some("image.jpg")), Some("image/jpeg"));
        assert_eq!(FeignParts::media_type(Some("image.jpeg")), Some("image/jpeg"));
        assert_eq!(FeignParts::media_type(Some("image.png")), Some("image/png"));
        assert_eq!(FeignParts::media_type(Some("image.gif")), Some("image/gif"));
        assert_eq!(FeignParts::media_type(Some("image.tif")), Some("image/tiff"));
        assert_eq!(FeignParts::media_type(Some("image.tiff")), Some("image/tiff"));

        // Supported document formats
        assert_eq!(
            FeignParts::media_type(Some("doc.pdf")),
            Some("application/pdf")
        );
        assert_eq!(
            FeignParts::media_type(Some("doc.doc")),
            Some("application/msword")
        );
        assert_eq!(
            FeignParts::media_type(Some("doc.docx")),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(
            FeignParts::media_type(Some("doc.odt")),
            Some("application/vnd.oasis.opendocument.text")
        );
        assert_eq!(FeignParts::media_type(Some("doc.txt")), Some("text/plain"));

        // Supported spreadsheet formats
        assert_eq!(
            FeignParts::media_type(Some("sheet.xls")),
            Some("application/vnd.ms-excel")
        );
        assert_eq!(
            FeignParts::media_type(Some("sheet.xlsx")),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        assert_eq!(
            FeignParts::media_type(Some("sheet.ods")),
            Some("application/vnd.oasis.opendocument.spreadsheet")
        );
    }

    #[test]
    fn file_name_extraction_from_content_disposition() {
        let headers = HashMap::from([(
            "Content-Disposition".to_string(),
            vec!["attachment; filename=\"report-2024-01-15.xlsx\"".to_string()],
        )]);
        assert_eq!(
            FeignParts::file_name(Some(&headers)),
            Some("report-2024-01-15.xlsx".to_string())
        );

        let empty_headers: HashMap<String, Vec<String>> = HashMap::new();
        assert_eq!(FeignParts::file_name(Some(&empty_headers)), None);
        assert_eq!(FeignParts::file_name(None), None);

        let inline_headers = HashMap::from([(
            "Content-Disposition".to_string(),
            vec!["inline".to_string()],
        )]);
        assert_eq!(FeignParts::file_name(Some(&inline_headers)), None);
    }

    // ========================================================================
    // Port of ExternalIdAdapterTest
    // ========================================================================

    #[derive(Serialize, Deserialize)]
    struct TestObject {
        #[serde(rename = "externalId")]
        external_id: ExternalId,
    }

    #[test]
    fn external_id_adapter_serialization_and_roundtrip() {
        let ext = ExternalId::new().value(Some("EXT-12345"));
        assert_eq!(
            ExternalIdAdapter::write_value_as_string(Some(&ext)).unwrap(),
            "\"EXT-12345\""
        );

        let special = ExternalId::new().value(Some("EXT-ID-2024/01/15"));
        assert_eq!(
            ExternalIdAdapter::write_value_as_string(Some(&special)).unwrap(),
            "\"EXT-ID-2024/01/15\""
        );

        let null_val = ExternalId::new().value(None::<String>);
        assert_eq!(
            ExternalIdAdapter::write_value_as_string(Some(&null_val)).unwrap(),
            "null"
        );
        assert_eq!(
            ExternalIdAdapter::write_value_as_string(None).unwrap(),
            "null"
        );

        let deser = ExternalIdAdapter::read_value("\"EXT-67890\"")
            .unwrap()
            .unwrap();
        assert_eq!(deser.get_value(), Some("EXT-67890"));
        assert!(ExternalIdAdapter::read_value("null").unwrap().is_none());

        let obj = TestObject {
            external_id: ExternalId::new().value(Some("TEST-123")),
        };
        let json = serde_json::to_string(&obj).unwrap();
        assert!(json.contains("\"externalId\":\"TEST-123\""));
        let parsed: TestObject = serde_json::from_str("{\"externalId\":\"TEST-456\"}").unwrap();
        assert_eq!(parsed.external_id.get_value(), Some("TEST-456"));
        assert_eq!(ExternalIdAdapter::create_module().module_name, "ExternalIdModule");
    }

    // ========================================================================
    // Port of FeignExceptionTest & FineractErrorDecoderTest
    // ========================================================================

    #[test]
    fn feign_exception_details_and_body_string() {
        let req = sample_request();
        let body = b"Error response body".to_vec();
        let ex = FeignException::new(
            400,
            "Error message",
            req.clone(),
            Some(body),
            Some("Technical error details".to_string()),
            Some("User-friendly error message".to_string()),
        );

        assert_eq!(ex.status(), 400);
        assert_eq!(ex.get_developer_message(), Some("Technical error details"));
        assert_eq!(ex.get_user_message(), Some("User-friendly error message"));
        assert_eq!(
            ex.response_body_as_string(),
            Some("Error response body".to_string())
        );
        assert!(ex.get_message().contains("400"));
        assert!(ex.get_message().contains("User-friendly error message"));
        assert!(ex.get_message().contains("Technical error details"));

        let null_body_ex = FeignException::new(404, "Not found", req, None, None, None);
        assert_eq!(null_body_ex.response_body_as_string(), None);
    }

    #[test]
    fn fineract_error_decoder_parses_structured_and_fallback_errors() {
        let decoder = FineractErrorDecoder;
        let resp = HttpResponseInfo {
            status: 400,
            reason: "Bad Request".to_string(),
            request: sample_request(),
            headers: None,
            body: Some(
                br#"{"developerMessage":"Developer error message","userMessage":"User error message"}"#
                    .to_vec(),
            ),
        };

        let ex = decoder.decode("Test#method", &resp);
        assert_eq!(ex.status(), 400);
        assert_eq!(ex.get_user_message(), Some("User error message"));
        assert_eq!(ex.get_developer_message(), Some("Developer error message"));

        // Invalid JSON still produces a valid FeignException
        let invalid_resp = HttpResponseInfo {
            status: 500,
            reason: "Internal Server Error".to_string(),
            request: sample_request(),
            headers: None,
            body: Some(b"This is not valid JSON".to_vec()),
        };
        let ex2 = decoder.decode("Test#method", &invalid_resp);
        assert_eq!(ex2.status(), 500);
        assert_eq!(ex2.get_user_message(), None);
    }
}
