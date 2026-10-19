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
// module `fineract-document` to Rust Edition 2024.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

pub const SIZE_RESULT_VALUE: &str = "SIZE_RESULT_VALUE";
pub const DATA_URL_DECODE_RESULT_CONTENT_TYPE: &str = "DATA_URL_DECODE_RESULT_CONTENT_TYPE";
pub const DATA_URL_ENCODE_PARAM_CONTENT_TYPE: &str = "DATA_URL_ENCODE_PARAM_CONTENT_TYPE";
pub const DATA_URL_ENCODE_PARAM_ENCODING: &str = "DATA_URL_ENCODE_PARAM_ENCODING";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DocumentError {
    #[error("{0}")]
    ContentPolicyException(String),
    #[error("Content detector error: {0}")]
    ContentDetectorException(String),
    #[error("Content processor error: {0}")]
    ContentProcessorException(String),
    #[error("Content path sanitizer error: {0}")]
    ContentPathSanitizerException(String),
    #[error("Document {id} not found for {entity_type}/{entity_id}")]
    DocumentNotFoundException {
        entity_type: String,
        entity_id: i64,
        id: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentStoreType {
    FileSystem,
    S3,
}

#[derive(Debug, Clone, Default)]
pub struct ContentPolicyContext {
    pub path: String,
    pub mime_type: String,
    pub bytes: Option<Vec<u8>>,
}

/// Port of `org.apache.fineract.infrastructure.contentstore.policy.ContentPolicy`
pub trait ContentPolicy: Send + Sync {
    fn check(&self, ctx: &ContentPolicyContext) -> Result<(), DocumentError>;
}

/// Port of `org.apache.fineract.infrastructure.contentstore.policy.TraversalContentPolicy`
#[derive(Debug, Default, Clone)]
pub struct TraversalContentPolicy;

impl TraversalContentPolicy {
    fn has_sibling_traversal(path: &str) -> bool {
        // Matches Java regex `.*((\.)?\./+)+(/+)?.*` (e.g. `./`, `../`, `..//`, `/../`)
        let bytes = path.as_bytes();
        for i in 0..bytes.len() {
            if bytes[i] == b'.' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    return true;
                }
                if i + 2 < bytes.len() && bytes[i + 1] == b'.' && bytes[i + 2] == b'/' {
                    return true;
                }
            }
        }
        false
    }
}

impl ContentPolicy for TraversalContentPolicy {
    fn check(&self, ctx: &ContentPolicyContext) -> Result<(), DocumentError> {
        if Self::has_sibling_traversal(&ctx.path) {
            return Err(DocumentError::ContentPolicyException(format!(
                "Trying to overwrite a sibling file: {}",
                ctx.path
            )));
        }
        Ok(())
    }
}

/// Port of `ContentDetector` & `TikaContentDetector` / `FileContentDetector`
pub struct ContentDetector;

impl ContentDetector {
    pub fn detect_mime_from_bytes(bytes: &[u8], fallback_path: &str) -> String {
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return "image/gif".to_string();
        }
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return "image/png".to_string();
        }
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return "image/jpeg".to_string();
        }
        if bytes.starts_with(b"%PDF-") {
            return "application/pdf".to_string();
        }
        if let Some((_, ext)) = fallback_path.rsplit_once('.') {
            match ext.to_ascii_lowercase().as_str() {
                "png" => return "image/png".to_string(),
                "jpg" | "jpeg" => return "image/jpeg".to_string(),
                "gif" => return "image/gif".to_string(),
                "pdf" => return "application/pdf".to_string(),
                _ => {}
            }
        }
        "application/octet-stream".to_string()
    }
}

/// Port of `org.apache.fineract.infrastructure.contentstore.policy.MimeContentPolicy`
#[derive(Debug, Default, Clone)]
pub struct MimeContentPolicy;

impl ContentPolicy for MimeContentPolicy {
    fn check(&self, ctx: &ContentPolicyContext) -> Result<(), DocumentError> {
        if let Some(ref bytes) = ctx.bytes {
            let detected = ContentDetector::detect_mime_from_bytes(bytes, &ctx.path);
            if !detected.eq_ignore_ascii_case(&ctx.mime_type) {
                return Err(DocumentError::ContentPolicyException(format!(
                    "Detected file type ({detected}), but mime type ({}) was provided. Mismatch!",
                    ctx.mime_type
                )));
            }
        }
        Ok(())
    }
}

// ============================================================================
// Content Processors (Base64, DataUrl, Size)
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct ContentProcessorContext {
    pub data: Vec<u8>,
    pub parameters: HashMap<String, String>,
    pub results: HashMap<String, String>,
}

impl ContentProcessorContext {
    pub fn new(data: Vec<u8>, parameters: HashMap<String, String>) -> Self {
        Self {
            data,
            parameters,
            results: HashMap::new(),
        }
    }

    pub fn get_result(&self, key: &str) -> Option<&str> {
        self.results.get(key).map(|s| s.as_str())
    }
}

pub trait ContentProcessor: Send + Sync {
    fn process(
        &self,
        ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError>;
}

pub struct ChainedContentProcessor {
    first: Arc<dyn ContentProcessor>,
    next: Arc<dyn ContentProcessor>,
}

impl ChainedContentProcessor {
    pub fn new(first: Arc<dyn ContentProcessor>, next: Arc<dyn ContentProcessor>) -> Self {
        Self { first, next }
    }
}

impl ContentProcessor for ChainedContentProcessor {
    fn process(
        &self,
        ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        let intermediate = self.first.process(ctx)?;
        self.next.process(intermediate)
    }
}

/// Port of `SizeContentProcessor`
#[derive(Debug, Default, Clone)]
pub struct SizeContentProcessor;

impl ContentProcessor for SizeContentProcessor {
    fn process(
        &self,
        mut ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        let len = ctx.data.len();
        ctx.results
            .insert(SIZE_RESULT_VALUE.to_string(), len.to_string());
        Ok(ctx)
    }
}

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize]);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize]);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }
    }
    out
}

fn base64_decode(input: &[u8]) -> Result<Vec<u8>, DocumentError> {
    let filtered: Vec<u8> = input
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if filtered.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    for chunk in filtered.chunks(4) {
        if chunk.len() < 4 {
            return Err(DocumentError::ContentProcessorException(
                "Invalid base64 length".to_string(),
            ));
        }
        let mut vals = [0u32; 4];
        let mut pad = 0;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                vals[i] = 0;
                pad += 1;
            } else if let Some(pos) = B64_CHARS.iter().position(|&x| x == c) {
                vals[i] = pos as u32;
            } else {
                return Err(DocumentError::ContentProcessorException(
                    "Invalid base64 character".to_string(),
                ));
            }
        }
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out.push(((triple >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

/// Port of `Base64EncoderContentProcessor`
#[derive(Debug, Default, Clone)]
pub struct Base64EncoderContentProcessor;

impl ContentProcessor for Base64EncoderContentProcessor {
    fn process(
        &self,
        mut ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        ctx.data = base64_encode(&ctx.data);
        Ok(ctx)
    }
}

/// Port of `Base64DecoderContentProcessor`
#[derive(Debug, Default, Clone)]
pub struct Base64DecoderContentProcessor;

impl ContentProcessor for Base64DecoderContentProcessor {
    fn process(
        &self,
        mut ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        ctx.data = base64_decode(&ctx.data)?;
        Ok(ctx)
    }
}

/// Port of `DataUrlDecoderContentProcessor`
#[derive(Debug, Default, Clone)]
pub struct DataUrlDecoderContentProcessor;

impl ContentProcessor for DataUrlDecoderContentProcessor {
    fn process(
        &self,
        mut ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        let s = String::from_utf8_lossy(&ctx.data).to_string();
        if let Some(rest) = s.strip_prefix("data:") {
            if let Some((header, payload)) = rest.split_once(',') {
                let mime = header.split(';').next().unwrap_or("application/octet-stream");
                ctx.results.insert(
                    DATA_URL_DECODE_RESULT_CONTENT_TYPE.to_string(),
                    mime.to_string(),
                );
                ctx.data = payload.as_bytes().to_vec();
                return Ok(ctx);
            }
        }
        Err(DocumentError::ContentProcessorException(
            "Invalid Data URL".to_string(),
        ))
    }
}

/// Port of `DataUrlEncoderContentProcessor`
#[derive(Debug, Default, Clone)]
pub struct DataUrlEncoderContentProcessor;

impl ContentProcessor for DataUrlEncoderContentProcessor {
    fn process(
        &self,
        mut ctx: ContentProcessorContext,
    ) -> Result<ContentProcessorContext, DocumentError> {
        let content_type = ctx
            .parameters
            .get(DATA_URL_ENCODE_PARAM_CONTENT_TYPE)
            .cloned()
            .unwrap_or_else(|| "image/png".to_string());
        let encoding = ctx
            .parameters
            .get(DATA_URL_ENCODE_PARAM_ENCODING)
            .cloned()
            .unwrap_or_else(|| "base64".to_string());
        let payload = String::from_utf8_lossy(&ctx.data);
        ctx.data = format!("data:{content_type};{encoding},{payload}").into_bytes();
        Ok(ctx)
    }
}

/// Port of `org.apache.fineract.infrastructure.documentmanagement.domain.Document`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: Option<i64>,
    pub parent_entity_type: String,
    pub parent_entity_id: i64,
    pub name: String,
    pub file_name: String,
    pub size: i64,
    pub content_type: String,
    pub description: Option<String>,
    pub location: String,
    pub storage_type: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Port of TraversalContentPolicyTest
    // ========================================================================

    #[test]
    fn path_traversal_rejected_for_all_sibling_patterns() {
        let policy = TraversalContentPolicy;
        let paths = [
            "/tmp/../17/michael.vorburger-crepes.jpg",
            "../17/michael.vorburger-crepes.jpg",
            "/tmp/..//17//michael.vorburger-crepes.jpg",
            "/tmp/..//17/michael.vorburger-crepes.jpg",
            "..//17/michael.vorburger-crepes.jpg",
            "../../../../../../../../../../tmp/michael.vorburger-crepes.jpg",
            "/tmp/../../../../../../../../../../tmp/image-text-wrong-content.jsp",
            "../../../../../../../../../../tmp/image-text-wrong-content.jsp",
            "/test/../tmp/image-text-wrong-content.jsp",
        ];

        for path in paths {
            let ctx = ContentPolicyContext {
                path: path.to_string(),
                mime_type: "image/jpeg".to_string(),
                bytes: None,
            };
            let err = policy.check(&ctx).unwrap_err();
            assert!(
                err.to_string()
                    .starts_with("Trying to overwrite a sibling file:"),
                "Unexpected error for {path}: {err}"
            );
        }

        // Valid path passes
        let safe_ctx = ContentPolicyContext {
            path: "clients/17/documents/passport.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            bytes: None,
        };
        assert!(policy.check(&safe_ctx).is_ok());
    }

    // ========================================================================
    // Port of MimeContentPolicyTest
    // ========================================================================

    #[test]
    fn mime_type_mismatch_throws_content_policy_exception() {
        let policy = MimeContentPolicy;
        // File named .png claiming image/png, but actual magic bytes are GIF89a
        let ctx = ContentPolicyContext {
            path: "tmp/test-gif.png".to_string(),
            mime_type: "image/png".to_string(),
            bytes: Some(b"GIF89a\x01\x00\x01\x00".to_vec()),
        };

        let err = policy.check(&ctx).unwrap_err();
        assert!(err.to_string().contains("Detected file type (image/gif)"));
        assert!(err.to_string().contains("Mismatch!"));
    }

    // ========================================================================
    // Port of ContentProcessorTest
    // ========================================================================

    #[test]
    fn data_url_and_base64_and_size_content_processors_chain_correctly() {
        let data_url = b"data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==".to_vec();
        let chain = ChainedContentProcessor::new(
            Arc::new(DataUrlDecoderContentProcessor),
            Arc::new(ChainedContentProcessor::new(
                Arc::new(Base64DecoderContentProcessor),
                Arc::new(SizeContentProcessor),
            )),
        );

        let ctx = chain
            .process(ContentProcessorContext::new(data_url, HashMap::new()))
            .unwrap();
        assert_eq!(
            ctx.get_result(DATA_URL_DECODE_RESULT_CONTENT_TYPE),
            Some("image/png")
        );
        assert_eq!(ctx.get_result(SIZE_RESULT_VALUE), Some("16"));
        assert!(ctx.data.starts_with(b"\x89PNG\r\n\x1a\n"));

        // Re-encode back to Data URL
        let encode_chain = ChainedContentProcessor::new(
            Arc::new(Base64EncoderContentProcessor),
            Arc::new(DataUrlEncoderContentProcessor),
        );
        let encoded_ctx = encode_chain
            .process(ContentProcessorContext::new(
                ctx.data,
                HashMap::from([
                    (
                        DATA_URL_ENCODE_PARAM_CONTENT_TYPE.to_string(),
                        "image/png".to_string(),
                    ),
                    (
                        DATA_URL_ENCODE_PARAM_ENCODING.to_string(),
                        "base64".to_string(),
                    ),
                ]),
            ))
            .unwrap();
        assert_eq!(
            String::from_utf8(encoded_ctx.data).unwrap(),
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg=="
        );
    }
}
