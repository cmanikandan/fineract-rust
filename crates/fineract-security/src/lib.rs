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
// module `fineract-security` to Rust Edition 2024.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use uuid::Uuid;

pub const TENANT_IDENTIFIER_HEADER: &str = "Fineract-Platform-TenantId";
pub const DEFAULT_TENANT: &str = "default";

/// Port of `org.apache.fineract.infrastructure.security.exception.OidcUserNotFoundException`
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("No Fineract AppUser found for OIDC subject '{subject}'. Enable fineract.security.oauth2.auto-create-user to provision users automatically.")]
pub struct OidcUserNotFoundException {
    subject: String,
}

impl OidcUserNotFoundException {
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
        }
    }

    pub fn get_subject(&self) -> &str {
        &self.subject
    }

    pub fn get_message(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityError {
    #[error("OAuth2 authentication failed: {0}")]
    OAuth2AuthenticationException(#[from] OidcUserNotFoundException),
    #[error("Invalid access token")]
    AccessTokenInvalidException,
    #[error("Invalid OTP token")]
    OTPTokenInvalidException,
    #[error("Password reset required for user {0}")]
    PasswordResetRequiredException(String),
    #[error("SQL literal escape violation: {0}")]
    EscapeSqlLiteralException(String),
}

/// Port of `OidcFederationType`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OidcFederationType {
    Keycloak,
    Google,
    Generic,
}

/// Port of `TenantOidcConfig`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantOidcConfig {
    pub id: Option<i64>,
    pub tenant_id: String,
    pub provider_type: OidcFederationType,
    pub issuer_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub username_claim: String,
    pub scopes: String,
    pub enabled: bool,
}

pub trait PasswordEncryptor: Send + Sync {
    fn encrypt(&self, plain: &str) -> String;
    fn decrypt(&self, encrypted: &str) -> String;
}

pub trait TenantOidcConfigRepository: Send + Sync {
    fn find_by_issuer_uri(&self, issuer_uri: &str) -> Option<TenantOidcConfig>;
    fn find_by_tenant_id(&self, tenant_id: &str) -> Option<TenantOidcConfig>;
    fn save(&self, config: TenantOidcConfig) -> TenantOidcConfig;
    fn delete_by_tenant_id(&self, tenant_id: &str);
}

/// Port of `org.apache.fineract.infrastructure.security.service.TenantOidcConfigService`
pub struct TenantOidcConfigService {
    repository: Arc<dyn TenantOidcConfigRepository>,
    password_encryptor: Arc<dyn PasswordEncryptor>,
    cache_by_issuer: RwLock<HashMap<String, TenantOidcConfig>>,
}

impl TenantOidcConfigService {
    pub fn new(
        repository: Arc<dyn TenantOidcConfigRepository>,
        password_encryptor: Arc<dyn PasswordEncryptor>,
    ) -> Self {
        Self {
            repository,
            password_encryptor,
            cache_by_issuer: RwLock::new(HashMap::new()),
        }
    }

    pub fn find_by_issuer_uri(&self, issuer_uri: &str) -> Option<TenantOidcConfig> {
        if let Some(cached) = self.cache_by_issuer.read().unwrap().get(issuer_uri).cloned() {
            return Some(cached);
        }
        let from_db = self.repository.find_by_issuer_uri(issuer_uri)?;
        self.cache_by_issuer
            .write()
            .unwrap()
            .insert(issuer_uri.to_string(), from_db.clone());
        Some(from_db)
    }

    pub fn save(&self, mut config: TenantOidcConfig) -> TenantOidcConfig {
        if let Some(ref secret) = config.client_secret {
            if !secret.trim().is_empty() {
                config.client_secret = Some(self.password_encryptor.encrypt(secret));
            }
        }
        let issuer = config.issuer_uri.clone();
        let saved = self.repository.save(config);
        self.cache_by_issuer.write().unwrap().remove(&issuer);
        saved
    }

    pub fn decrypt_client_secret(&self, config: &TenantOidcConfig) -> Option<String> {
        config
            .client_secret
            .as_ref()
            .map(|enc| self.password_encryptor.decrypt(enc))
    }

    pub fn delete_by_tenant_id(&self, tenant_id: &str) {
        if let Some(existing) = self.repository.find_by_tenant_id(tenant_id) {
            self.repository.delete_by_tenant_id(tenant_id);
            self.cache_by_issuer
                .write()
                .unwrap()
                .remove(&existing.issuer_uri);
        }
    }

    pub fn evict_all(&self) {
        self.cache_by_issuer.write().unwrap().clear();
    }
}

// ============================================================================
// OIDC JWT Authentication Converter (Port of FineractOidcJwtAuthenticationConverter)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jwt {
    pub token_value: String,
    pub subject: String,
    pub claims: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppUser {
    pub id: i64,
    pub username: String,
    pub authorities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FineractJwtAuthenticationToken {
    pub token: Jwt,
    pub principal: AppUser,
    pub authorities: Vec<String>,
}

pub trait FineractOidcUserService: Send + Sync {
    fn extract_username(&self, jwt: &Jwt) -> String;
    fn resolve_user(&self, jwt: &Jwt, username: &str) -> Result<AppUser, OidcUserNotFoundException>;
}

/// Port of `org.apache.fineract.infrastructure.security.converter.FineractOidcJwtAuthenticationConverter`
pub struct FineractOidcJwtAuthenticationConverter {
    oidc_user_service: Arc<dyn FineractOidcUserService>,
}

impl FineractOidcJwtAuthenticationConverter {
    pub fn new(oidc_user_service: Arc<dyn FineractOidcUserService>) -> Self {
        Self { oidc_user_service }
    }

    pub fn convert(&self, jwt: &Jwt) -> Result<FineractJwtAuthenticationToken, SecurityError> {
        let username = self.oidc_user_service.extract_username(jwt);
        let app_user = self.oidc_user_service.resolve_user(jwt, &username)?;
        let authorities = app_user.authorities.clone();
        Ok(FineractJwtAuthenticationToken {
            token: jwt.clone(),
            principal: app_user,
            authorities,
        })
    }
}

/// Port of `UUIDAccessTokenGenerationService` and `SqlInjectionPreventerServiceImpl`
pub struct UUIDAccessTokenGenerationService;

impl UUIDAccessTokenGenerationService {
    pub fn generate_token() -> String {
        Uuid::new_v4().to_string()
    }
}

pub struct SqlInjectionPreventerService;

impl SqlInjectionPreventerService {
    pub fn encode_sql(literal: &str) -> Result<String, SecurityError> {
        if literal.contains(';') || literal.contains("--") || literal.contains("/*") {
            return Err(SecurityError::EscapeSqlLiteralException(
                literal.to_string(),
            ));
        }
        Ok(literal.replace('\'', "''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ========================================================================
    // Port of OidcUserNotFoundExceptionTest
    // ========================================================================

    #[test]
    fn oidc_user_not_found_exception_retains_subject_and_hint() {
        let ex = OidcUserNotFoundException::new("alice");
        assert_eq!(ex.get_subject(), "alice");
        assert!(ex.get_message().contains("alice"));
        assert!(ex.get_message().contains("auto-create-user"));
    }

    // ========================================================================
    // Port of FineractOidcJwtAuthenticationConverterTest
    // ========================================================================

    struct MockOidcUserService {
        users: HashMap<String, AppUser>,
    }

    impl FineractOidcUserService for MockOidcUserService {
        fn extract_username(&self, jwt: &Jwt) -> String {
            jwt.claims
                .get("preferred_username")
                .cloned()
                .unwrap_or_else(|| jwt.subject.clone())
        }

        fn resolve_user(
            &self,
            _jwt: &Jwt,
            username: &str,
        ) -> Result<AppUser, OidcUserNotFoundException> {
            self.users
                .get(username)
                .cloned()
                .ok_or_else(|| OidcUserNotFoundException::new(username))
        }
    }

    fn sample_jwt(username: &str) -> Jwt {
        Jwt {
            token_value: "token".to_string(),
            subject: "sub-123".to_string(),
            claims: HashMap::from([("preferred_username".to_string(), username.to_string())]),
        }
    }

    #[test]
    fn converter_returns_token_with_app_user_and_authorities_or_wraps_error() {
        let alice = AppUser {
            id: 1,
            username: "alice".to_string(),
            authorities: vec!["READ_LOAN".to_string()],
        };
        let svc = Arc::new(MockOidcUserService {
            users: HashMap::from([("alice".to_string(), alice.clone())]),
        });
        let converter = FineractOidcJwtAuthenticationConverter::new(svc);

        let jwt = sample_jwt("alice");
        let token = converter.convert(&jwt).unwrap();
        assert_eq!(token.principal, alice);
        assert_eq!(token.authorities, vec!["READ_LOAN".to_string()]);
        assert_eq!(token.token, jwt);

        // Unknown user wraps OidcUserNotFoundException in SecurityError::OAuth2AuthenticationException
        let unknown_jwt = sample_jwt("unknown");
        let err = converter.convert(&unknown_jwt).unwrap_err();
        assert_eq!(
            err,
            SecurityError::OAuth2AuthenticationException(OidcUserNotFoundException::new("unknown"))
        );
    }

    // ========================================================================
    // Port of TenantOidcConfigServiceTest
    // ========================================================================

    struct MockOidcRepo {
        by_issuer: RwLock<HashMap<String, TenantOidcConfig>>,
        find_calls: AtomicUsize,
        delete_calls: AtomicUsize,
    }

    impl MockOidcRepo {
        fn new(initial: Option<TenantOidcConfig>) -> Self {
            let mut map = HashMap::new();
            if let Some(c) = initial {
                map.insert(c.issuer_uri.clone(), c);
            }
            Self {
                by_issuer: RwLock::new(map),
                find_calls: AtomicUsize::new(0),
                delete_calls: AtomicUsize::new(0),
            }
        }
    }

    impl TenantOidcConfigRepository for MockOidcRepo {
        fn find_by_issuer_uri(&self, issuer_uri: &str) -> Option<TenantOidcConfig> {
            self.find_calls.fetch_add(1, Ordering::SeqCst);
            self.by_issuer.read().unwrap().get(issuer_uri).cloned()
        }

        fn find_by_tenant_id(&self, tenant_id: &str) -> Option<TenantOidcConfig> {
            self.by_issuer
                .read()
                .unwrap()
                .values()
                .find(|c| c.tenant_id == tenant_id)
                .cloned()
        }

        fn save(&self, config: TenantOidcConfig) -> TenantOidcConfig {
            self.by_issuer
                .write()
                .unwrap()
                .insert(config.issuer_uri.clone(), config.clone());
            config
        }

        fn delete_by_tenant_id(&self, tenant_id: &str) {
            self.delete_calls.fetch_add(1, Ordering::SeqCst);
            self.by_issuer
                .write()
                .unwrap()
                .retain(|_, v| v.tenant_id != tenant_id);
        }
    }

    struct MockEncryptor {
        encrypt_calls: AtomicUsize,
    }

    impl PasswordEncryptor for MockEncryptor {
        fn encrypt(&self, plain: &str) -> String {
            self.encrypt_calls.fetch_add(1, Ordering::SeqCst);
            format!("enc:{plain}")
        }

        fn decrypt(&self, encrypted: &str) -> String {
            encrypted.strip_prefix("enc:").unwrap_or(encrypted).to_string()
        }
    }

    fn sample_config() -> TenantOidcConfig {
        TenantOidcConfig {
            id: Some(1),
            tenant_id: "tenant_a".to_string(),
            provider_type: OidcFederationType::Keycloak,
            issuer_uri: "https://keycloak.example.com/realms/tenant-a".to_string(),
            client_id: "fineract-client".to_string(),
            client_secret: Some("enc:plain-secret".to_string()),
            username_claim: "preferred_username".to_string(),
            scopes: "openid,profile,email".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn tenant_oidc_config_service_caches_hits_and_skips_misses() {
        let issuer = "https://keycloak.example.com/realms/tenant-a";
        let repo = Arc::new(MockOidcRepo::new(Some(sample_config())));
        let enc = Arc::new(MockEncryptor {
            encrypt_calls: AtomicUsize::new(0),
        });
        let service = TenantOidcConfigService::new(repo.clone(), enc);

        assert!(service.find_by_issuer_uri(issuer).is_some());
        assert!(service.find_by_issuer_uri(issuer).is_some());
        assert!(service.find_by_issuer_uri(issuer).is_some());
        assert_eq!(repo.find_calls.load(Ordering::SeqCst), 1);

        // Misses are not cached
        assert!(service.find_by_issuer_uri("https://unknown.example.com").is_none());
        assert!(service.find_by_issuer_uri("https://unknown.example.com").is_none());
        assert_eq!(repo.find_calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn tenant_oidc_config_service_encrypts_secret_and_invalidates_cache_on_save_and_delete() {
        let issuer = "https://keycloak.example.com/realms/tenant-a";
        let repo = Arc::new(MockOidcRepo::new(Some(sample_config())));
        let enc = Arc::new(MockEncryptor {
            encrypt_calls: AtomicUsize::new(0),
        });
        let service = TenantOidcConfigService::new(repo.clone(), enc.clone());

        // Warm cache
        let cfg = service.find_by_issuer_uri(issuer).unwrap();
        assert_eq!(
            service.decrypt_client_secret(&cfg),
            Some("plain-secret".to_string())
        );

        // Save new config -> encrypts secret and evicts cache
        let mut updated = sample_config();
        updated.client_secret = Some("new-secret".to_string());
        let saved = service.save(updated);
        assert_eq!(saved.client_secret, Some("enc:new-secret".to_string()));
        assert_eq!(enc.encrypt_calls.load(Ordering::SeqCst), 1);

        // Next lookup hits DB again
        service.find_by_issuer_uri(issuer);
        assert_eq!(repo.find_calls.load(Ordering::SeqCst), 2);

        // Blank/None secret skips encryption
        let mut public_client = sample_config();
        public_client.client_secret = None;
        service.save(public_client);
        assert_eq!(enc.encrypt_calls.load(Ordering::SeqCst), 1);

        // Delete evicts cache
        service.find_by_issuer_uri(issuer);
        service.delete_by_tenant_id("tenant_a");
        assert_eq!(repo.delete_calls.load(Ordering::SeqCst), 1);
        assert!(service.find_by_issuer_uri(issuer).is_none());
    }
}
