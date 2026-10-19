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

//! Rust migration of Apache Fineract's command processing architecture:
//! - `fineract-command` (Core CQRS command, hooks, handler manager, synchronous dispatcher)
//! - `fineract-command-async` (`AsyncCommandDispatcher`)
//! - `fineract-command-audit` (`AuditCommandHookBefore`, `AuditCommandHookAfter`, `AuditCommandHookError`)
//! - `fineract-command-disruptor` (`DisruptorCommandDispatcher` ring-buffer pipeline)
//! - `fineract-command-jdbc` (`CommandEntity`, `CommandMapper`, `CommandRepository`, `JdbcCommandStore` with DLQ fallback)
//! - `fineract-command-test` (`DummyCommand`, `DummyCommandHandler`, `DefaultDummyService`, Axum REST API `/test/dummy/sync` & `/test/dummy/async`)

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicI64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// 1. Core Constants, State, Command, and Properties (`fineract-command`)
// ============================================================================

pub mod constants {
    pub const COMMAND_JSON_CLASS_ATTRIBUTE: &str = "@class";
    pub const COMMAND_HTTP_HEADER_REQUEST_ID: &str = "x-fineract-request-id";
    pub const COMMAND_HTTP_HEADER_TENANT_ID: &str = "Fineract-Platform-TenantId";
    pub const COMMAND_HTTP_HEADER_IP: &str = "IP";
    pub const COMMAND_HOOK_ORDER_HEADERS: i32 = 10;
    pub const COMMAND_HOOK_ORDER_TIMESTAMP: i32 = 11;
    pub const COMMAND_HOOK_ORDER_USERNAME: i32 = 12;
}

/// Rust migration of `org.apache.fineract.command.core.CommandState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandState {
    Invalid,
    Processed,
    AwaitingApproval,
    Rejected,
    UnderProcessing,
    Error,
    Unknown,
}

/// Metadata shared across typed [`Command<T>`] and type-erased hooks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandMeta {
    pub command_id: Option<i64>,
    pub idempotency_key: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub initiated_by_username: Option<String>,
    pub executed_by_username: Option<String>,
    pub approved_by_username: Option<String>,
    pub rejected_by_username: Option<String>,
    pub error: Option<String>,
}

/// Rust migration of `org.apache.fineract.command.core.Command<T>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Command<T> {
    #[serde(flatten)]
    pub meta: CommandMeta,
    pub payload: Option<T>,
}

impl<T> Command<T> {
    pub fn new(payload: T) -> Self {
        Self {
            meta: CommandMeta::default(),
            payload: Some(payload),
        }
    }

    pub fn empty() -> Self {
        Self {
            meta: CommandMeta::default(),
            payload: None,
        }
    }

    pub fn command_id(&self) -> Option<i64> {
        self.meta.command_id
    }

    pub fn set_command_id(&mut self, id: i64) {
        self.meta.command_id = Some(id);
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.meta.idempotency_key.as_deref()
    }

    pub fn set_idempotency_key(&mut self, key: impl Into<String>) {
        self.meta.idempotency_key = Some(key.into());
    }

    pub fn ip_address(&self) -> Option<&str> {
        self.meta.ip_address.as_deref()
    }

    pub fn set_ip_address(&mut self, ip: impl Into<String>) {
        self.meta.ip_address = Some(ip.into());
    }

    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        self.meta.created_at
    }

    pub fn set_created_at(&mut self, ts: DateTime<Utc>) {
        self.meta.created_at = Some(ts);
    }

    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        self.meta.updated_at
    }

    pub fn set_updated_at(&mut self, ts: DateTime<Utc>) {
        self.meta.updated_at = Some(ts);
    }

    pub fn executed_at(&self) -> Option<DateTime<Utc>> {
        self.meta.executed_at
    }

    pub fn set_executed_at(&mut self, ts: DateTime<Utc>) {
        self.meta.executed_at = Some(ts);
    }

    pub fn approved_at(&self) -> Option<DateTime<Utc>> {
        self.meta.approved_at
    }

    pub fn set_approved_at(&mut self, ts: DateTime<Utc>) {
        self.meta.approved_at = Some(ts);
    }

    pub fn rejected_at(&self) -> Option<DateTime<Utc>> {
        self.meta.rejected_at
    }

    pub fn set_rejected_at(&mut self, ts: DateTime<Utc>) {
        self.meta.rejected_at = Some(ts);
    }

    pub fn initiated_by_username(&self) -> Option<&str> {
        self.meta.initiated_by_username.as_deref()
    }

    pub fn set_initiated_by_username(&mut self, user: impl Into<String>) {
        self.meta.initiated_by_username = Some(user.into());
    }

    pub fn executed_by_username(&self) -> Option<&str> {
        self.meta.executed_by_username.as_deref()
    }

    pub fn set_executed_by_username(&mut self, user: impl Into<String>) {
        self.meta.executed_by_username = Some(user.into());
    }

    pub fn approved_by_username(&self) -> Option<&str> {
        self.meta.approved_by_username.as_deref()
    }

    pub fn set_approved_by_username(&mut self, user: impl Into<String>) {
        self.meta.approved_by_username = Some(user.into());
    }

    pub fn rejected_by_username(&self) -> Option<&str> {
        self.meta.rejected_by_username.as_deref()
    }

    pub fn set_rejected_by_username(&mut self, user: impl Into<String>) {
        self.meta.rejected_by_username = Some(user.into());
    }

    pub fn error(&self) -> Option<&str> {
        self.meta.error.as_deref()
    }

    pub fn set_error(&mut self, err: impl Into<String>) {
        self.meta.error = Some(err.into());
    }

    pub fn payload(&self) -> Option<&T> {
        self.payload.as_ref()
    }

    pub fn set_payload(&mut self, payload: T) {
        self.payload = Some(payload);
    }
}

/// Rust migration of `org.apache.fineract.command.core.CommandProperties`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandProperties {
    pub enabled: bool,
    pub hooks: HashMap<String, bool>,
    pub idempotency_key_header_name: String,
}

impl Default for CommandProperties {
    fn default() -> Self {
        Self {
            enabled: true,
            hooks: HashMap::new(),
            idempotency_key_header_name: "Idempotency-Key".to_string(),
        }
    }
}

/// Rust migration of `org.apache.fineract.command.core.exception.*`.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum CommandError {
    #[error("Multiple handlers found for {request_type} command payload: Conflict found for {handler_name}")]
    CommandHandlerDuplicateException {
        request_type: String,
        handler_name: String,
    },

    #[error("CommandHandler [{handler_name}] is malformed: It does not explicitly define its generic REQ payload type parameters.")]
    CommandHandlerInvalidRequestTypeException { handler_name: String },

    #[error("Cannot find a matching handler for {command_name} command")]
    CommandHandlerNotFoundException { command_name: String },

    #[error("Illegal argument for {command_name} command: {message}")]
    CommandIllegalArgumentException {
        command_name: String,
        message: String,
    },

    #[error("Policy exception for {command_name} command: {message}")]
    CommandPolicyException {
        command_name: String,
        message: String,
    },

    #[error("{message}")]
    DummyException {
        request_content: String,
        message: String,
    },

    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
}

// ============================================================================
// 2. CommandHandler & DefaultCommandHandlerManager
// ============================================================================

/// Strongly-typed command handler trait matching `CommandHandler<REQ, RES>`.
pub trait CommandHandler<Req, Res>: Send + Sync + 'static
where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn handle(&self, command: &mut Command<Req>) -> Result<Res, CommandError>;

    fn fallback(&self, _command: &mut Command<Req>, err: CommandError) -> Result<Res, CommandError> {
        Err(err)
    }

    fn handler_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Type-erased handler registration so `DefaultCommandHandlerManager` can hold heterogeneous handlers
/// and verify uniqueness during `init()`.
pub trait ErasedCommandHandler: Send + Sync {
    fn handler_name(&self) -> &'static str;
    fn request_type_id(&self) -> Option<TypeId>;
    fn request_type_name(&self) -> Option<&'static str>;
    fn handle_erased(
        &self,
        meta: &mut CommandMeta,
        payload: &dyn Any,
    ) -> Result<Box<dyn Any + Send>, CommandError>;
}

pub struct TypedHandlerWrapper<H, Req, Res> {
    inner: H,
    _marker: std::marker::PhantomData<fn(Req) -> Res>,
}

impl<H, Req, Res> TypedHandlerWrapper<H, Req, Res>
where
    H: CommandHandler<Req, Res>,
    Req: Clone + Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    pub fn new(inner: H) -> Self {
        Self {
            inner,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<H, Req, Res> ErasedCommandHandler for TypedHandlerWrapper<H, Req, Res>
where
    H: CommandHandler<Req, Res>,
    Req: Clone + Send + Sync + 'static,
    Res: Send + Sync + 'static,
{
    fn handler_name(&self) -> &'static str {
        self.inner.handler_name()
    }

    fn request_type_id(&self) -> Option<TypeId> {
        Some(TypeId::of::<Req>())
    }

    fn request_type_name(&self) -> Option<&'static str> {
        Some(std::any::type_name::<Req>())
    }

    fn handle_erased(
        &self,
        meta: &mut CommandMeta,
        payload: &dyn Any,
    ) -> Result<Box<dyn Any + Send>, CommandError> {
        let req = payload.downcast_ref::<Req>().ok_or_else(|| {
            CommandError::CommandHandlerNotFoundException {
                command_name: std::any::type_name::<Req>().to_string(),
            }
        })?;
        let mut cmd = Command {
            meta: meta.clone(),
            payload: Some(req.clone()),
        };
        let res = match self.inner.handle(&mut cmd) {
            Ok(r) => Ok(r),
            Err(e) => self.inner.fallback(&mut cmd, e),
        };
        *meta = cmd.meta;
        res.map(|r| Box::new(r) as Box<dyn Any + Send>)
    }
}

/// Untyped/malformed handler used to test `CommandHandlerInvalidRequestTypeException`.
pub struct UntypedCommandHandler {
    pub name: &'static str,
}

impl ErasedCommandHandler for UntypedCommandHandler {
    fn handler_name(&self) -> &'static str {
        self.name
    }

    fn request_type_id(&self) -> Option<TypeId> {
        None
    }

    fn request_type_name(&self) -> Option<&'static str> {
        None
    }

    fn handle_erased(
        &self,
        _meta: &mut CommandMeta,
        _payload: &dyn Any,
    ) -> Result<Box<dyn Any + Send>, CommandError> {
        Err(CommandError::CommandHandlerInvalidRequestTypeException {
            handler_name: self.name.to_string(),
        })
    }
}

/// Rust migration of `org.apache.fineract.command.implementation.DefaultCommandHandlerManager`.
pub struct DefaultCommandHandlerManager {
    handlers: Vec<Arc<dyn ErasedCommandHandler>>,
}

impl DefaultCommandHandlerManager {
    pub fn new(handlers: Vec<Arc<dyn ErasedCommandHandler>>) -> Self {
        Self { handlers }
    }

    /// Validates handler uniqueness and generic request payload type resolution,
    /// matching `DefaultCommandHandlerManager.init()`.
    pub fn init(&self) -> Result<(), CommandError> {
        let mut registered_request_types = HashSet::new();
        for handler in &self.handlers {
            let Some(req_type_id) = handler.request_type_id() else {
                return Err(CommandError::CommandHandlerInvalidRequestTypeException {
                    handler_name: handler.handler_name().to_string(),
                });
            };
            if !registered_request_types.insert(req_type_id) {
                return Err(CommandError::CommandHandlerDuplicateException {
                    request_type: handler.request_type_name().unwrap_or("Unknown").to_string(),
                    handler_name: handler.handler_name().to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn handle<Req, Res>(&self, command: &mut Command<Req>) -> Result<Res, CommandError>
    where
        Req: Clone + Send + Sync + 'static,
        Res: Send + Sync + 'static,
    {
        let Some(ref payload) = command.payload else {
            return Err(CommandError::CommandHandlerNotFoundException {
                command_name: "UNKNOWN".to_string(),
            });
        };
        let target_id = TypeId::of::<Req>();
        let handler = self
            .handlers
            .iter()
            .find(|h| h.request_type_id() == Some(target_id))
            .ok_or_else(|| CommandError::CommandHandlerNotFoundException {
                command_name: std::any::type_name::<Req>().to_string(),
            })?;

        let boxed = handler.handle_erased(&mut command.meta, payload)?;
        boxed
            .downcast::<Res>()
            .map(|b| *b)
            .map_err(|_| CommandError::ExecutionFailed("Response type mismatch".to_string()))
    }
}

// ============================================================================
// 3. Command Hooks & DefaultCommandHookManager
// ============================================================================

pub trait CommandHookBefore: Send + Sync {
    fn on_before(&self, meta: &mut CommandMeta, payload_json: &JsonValue);
}

pub trait CommandHookAfter: Send + Sync {
    fn on_after(&self, meta: &mut CommandMeta, payload_json: &JsonValue, response_json: &JsonValue);
}

pub trait CommandHookError: Send + Sync {
    fn on_error(&self, meta: &mut CommandMeta, payload_json: &JsonValue, error: &CommandError);
}

/// Rust migration of `org.apache.fineract.command.implementation.DefaultCommandHookManager`.
#[derive(Default, Clone)]
pub struct DefaultCommandHookManager {
    before_hooks: Vec<Arc<dyn CommandHookBefore>>,
    after_hooks: Vec<Arc<dyn CommandHookAfter>>,
    error_hooks: Vec<Arc<dyn CommandHookError>>,
}

impl DefaultCommandHookManager {
    pub fn new(
        before_hooks: Vec<Arc<dyn CommandHookBefore>>,
        after_hooks: Vec<Arc<dyn CommandHookAfter>>,
        error_hooks: Vec<Arc<dyn CommandHookError>>,
    ) -> Self {
        Self {
            before_hooks,
            after_hooks,
            error_hooks,
        }
    }

    pub fn before(&self, meta: &mut CommandMeta, payload_json: &JsonValue) {
        for hook in &self.before_hooks {
            hook.on_before(meta, payload_json);
        }
    }

    pub fn after(
        &self,
        meta: &mut CommandMeta,
        payload_json: &JsonValue,
        response_json: &JsonValue,
    ) {
        for hook in &self.after_hooks {
            hook.on_after(meta, payload_json, response_json);
        }
    }

    pub fn error(&self, meta: &mut CommandMeta, payload_json: &JsonValue, error: &CommandError) {
        for hook in &self.error_hooks {
            hook.on_error(meta, payload_json, error);
        }
    }
}

/// Rust migration of `TimestampCommandHook`.
pub struct TimestampCommandHook;

impl CommandHookBefore for TimestampCommandHook {
    fn on_before(&self, meta: &mut CommandMeta, _payload_json: &JsonValue) {
        if meta.created_at.is_none() {
            meta.created_at = Some(Utc::now());
        }
    }
}

/// Rust migration of `UsernameCommandHook`.
pub struct UsernameCommandHook {
    pub default_username: String,
}

impl Default for UsernameCommandHook {
    fn default() -> Self {
        Self {
            default_username: "unknown".to_string(),
        }
    }
}

impl CommandHookBefore for UsernameCommandHook {
    fn on_before(&self, meta: &mut CommandMeta, _payload_json: &JsonValue) {
        if meta
            .initiated_by_username
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            meta.initiated_by_username = Some(self.default_username.clone());
        }
    }
}

/// Rust migration of `ServletHeadersCommandHook`.
pub struct ServletHeadersCommandHook {
    pub properties: CommandProperties,
    pub headers: HashMap<String, String>,
}

impl CommandHookBefore for ServletHeadersCommandHook {
    fn on_before(&self, meta: &mut CommandMeta, _payload_json: &JsonValue) {
        if meta.ip_address.as_deref().unwrap_or("").is_empty() {
            if let Some(ip) = self
                .headers
                .get(&constants::COMMAND_HTTP_HEADER_IP.to_ascii_lowercase())
            {
                meta.ip_address = Some(ip.clone());
            }
        }
        if meta.idempotency_key.as_deref().unwrap_or("").is_empty() {
            let key_name = self
                .properties
                .idempotency_key_header_name
                .to_ascii_lowercase();
            if let Some(idem) = self.headers.get(&key_name) {
                meta.idempotency_key = Some(idem.clone());
            }
        }
    }
}

// ============================================================================
// 4. Command Dispatchers: Synchronous, Async, and Disruptor
// ============================================================================

pub type CommandSupplier<Res> = Box<dyn FnOnce() -> Result<Res, CommandError> + Send>;

pub trait CommandDispatcher: Send + Sync {
    fn dispatch<Req, Res>(&self, command: &mut Command<Req>) -> CommandSupplier<Res>
    where
        Req: Clone + Serialize + Send + Sync + 'static,
        Res: Clone + Serialize + Send + Sync + 'static;
}

/// Rust migration of `org.apache.fineract.command.implementation.SynchronousCommandDispatcher`.
#[derive(Clone)]
pub struct SynchronousCommandDispatcher {
    pub handler_manager: Arc<DefaultCommandHandlerManager>,
    pub hook_manager: Arc<DefaultCommandHookManager>,
}

impl SynchronousCommandDispatcher {
    pub fn new(
        handler_manager: Arc<DefaultCommandHandlerManager>,
        hook_manager: Arc<DefaultCommandHookManager>,
    ) -> Self {
        Self {
            handler_manager,
            hook_manager,
        }
    }
}

impl CommandDispatcher for SynchronousCommandDispatcher {
    fn dispatch<Req, Res>(&self, command: &mut Command<Req>) -> CommandSupplier<Res>
    where
        Req: Clone + Serialize + Send + Sync + 'static,
        Res: Clone + Serialize + Send + Sync + 'static,
    {
        let handler_manager = Arc::clone(&self.handler_manager);
        let hook_manager = Arc::clone(&self.hook_manager);
        let mut cmd_clone = command.clone();

        Box::new(move || {
            let payload_json = serde_json::to_value(&cmd_clone.payload).unwrap_or(JsonValue::Null);
            hook_manager.before(&mut cmd_clone.meta, &payload_json);

            match handler_manager.handle::<Req, Res>(&mut cmd_clone) {
                Ok(response) => {
                    let resp_json = serde_json::to_value(&response).unwrap_or(JsonValue::Null);
                    hook_manager.after(&mut cmd_clone.meta, &payload_json, &resp_json);
                    Ok(response)
                }
                Err(err) => {
                    hook_manager.error(&mut cmd_clone.meta, &payload_json, &err);
                    Err(err)
                }
            }
        })
    }
}

/// Rust migration of `org.apache.fineract.command.async.implementation.AsyncCommandDispatcher`.
#[derive(Clone)]
pub struct AsyncCommandDispatcher {
    pub handler_manager: Arc<DefaultCommandHandlerManager>,
    pub hook_manager: Arc<DefaultCommandHookManager>,
    pub timeout: Duration,
}

impl AsyncCommandDispatcher {
    pub fn new(
        handler_manager: Arc<DefaultCommandHandlerManager>,
        hook_manager: Arc<DefaultCommandHookManager>,
    ) -> Self {
        Self {
            handler_manager,
            hook_manager,
            timeout: Duration::from_secs(3),
        }
    }
}

impl CommandDispatcher for AsyncCommandDispatcher {
    fn dispatch<Req, Res>(&self, command: &mut Command<Req>) -> CommandSupplier<Res>
    where
        Req: Clone + Serialize + Send + Sync + 'static,
        Res: Clone + Serialize + Send + Sync + 'static,
    {
        let handler_manager = Arc::clone(&self.handler_manager);
        let hook_manager = Arc::clone(&self.hook_manager);
        let timeout = self.timeout;
        let mut cmd_clone = command.clone();

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let payload_json = serde_json::to_value(&cmd_clone.payload).unwrap_or(JsonValue::Null);
            hook_manager.before(&mut cmd_clone.meta, &payload_json);
            let result = match handler_manager.handle::<Req, Res>(&mut cmd_clone) {
                Ok(res) => {
                    let resp_json = serde_json::to_value(&res).unwrap_or(JsonValue::Null);
                    hook_manager.after(&mut cmd_clone.meta, &payload_json, &resp_json);
                    Ok(res)
                }
                Err(e) => {
                    hook_manager.error(&mut cmd_clone.meta, &payload_json, &e);
                    Err(e)
                }
            };
            let _ = tx.send(result);
        });

        Box::new(move || {
            rx.recv_timeout(timeout)
                .map_err(|e| CommandError::ExecutionFailed(format!("Async timeout/error: {}", e)))?
        })
    }
}

/// Rust migration of `org.apache.fineract.command.disruptor.implementation.DisruptorCommandDispatcher`.
/// Uses a bounded ring-buffer work queue with a dedicated background consumer thread.
pub struct DisruptorCommandDispatcher {
    sender: Option<mpsc::SyncSender<Box<dyn FnOnce() + Send>>>,
    worker: Option<thread::JoinHandle<()>>,
    sequence: AtomicUsize,
    ring_buffer_size: usize,
    handler_manager: Arc<DefaultCommandHandlerManager>,
    hook_manager: Arc<DefaultCommandHookManager>,
}

impl DisruptorCommandDispatcher {
    pub fn new(
        ring_buffer_size: usize,
        handler_manager: Arc<DefaultCommandHandlerManager>,
        hook_manager: Arc<DefaultCommandHookManager>,
    ) -> Self {
        assert!(
            ring_buffer_size.is_power_of_two(),
            "Ring buffer size must be a power of 2"
        );
        let (tx, rx) = mpsc::sync_channel::<Box<dyn FnOnce() + Send>>(ring_buffer_size);
        let worker = thread::spawn(move || {
            while let Ok(task) = rx.recv() {
                task();
            }
        });
        Self {
            sender: Some(tx),
            worker: Some(worker),
            sequence: AtomicUsize::new(0),
            ring_buffer_size,
            handler_manager,
            hook_manager,
        }
    }

    pub fn ring_buffer_size(&self) -> usize {
        self.ring_buffer_size
    }

    pub fn published_sequences(&self) -> usize {
        self.sequence.load(Ordering::SeqCst)
    }

    pub fn close(&mut self) {
        self.sender.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DisruptorCommandDispatcher {
    fn drop(&mut self) {
        self.close();
    }
}

impl CommandDispatcher for DisruptorCommandDispatcher {
    fn dispatch<Req, Res>(&self, command: &mut Command<Req>) -> CommandSupplier<Res>
    where
        Req: Clone + Serialize + Send + Sync + 'static,
        Res: Clone + Serialize + Send + Sync + 'static,
    {
        let handler_manager = Arc::clone(&self.handler_manager);
        let hook_manager = Arc::clone(&self.hook_manager);
        let mut cmd_clone = command.clone();
        self.sequence.fetch_add(1, Ordering::SeqCst);

        let (res_tx, res_rx) = mpsc::channel();
        if let Some(ref sender) = self.sender {
            let _ = sender.send(Box::new(move || {
                let payload_json =
                    serde_json::to_value(&cmd_clone.payload).unwrap_or(JsonValue::Null);
                hook_manager.before(&mut cmd_clone.meta, &payload_json);
                let res = match handler_manager.handle::<Req, Res>(&mut cmd_clone) {
                    Ok(r) => {
                        let resp_json = serde_json::to_value(&r).unwrap_or(JsonValue::Null);
                        hook_manager.after(&mut cmd_clone.meta, &payload_json, &resp_json);
                        Ok(r)
                    }
                    Err(e) => {
                        hook_manager.error(&mut cmd_clone.meta, &payload_json, &e);
                        Err(e)
                    }
                };
                let _ = res_tx.send(res);
            }));
        }

        Box::new(move || {
            res_rx.recv().map_err(|e| {
                CommandError::ExecutionFailed(format!("Disruptor channel closed: {}", e))
            })?
        })
    }
}

// ============================================================================
// 5. Command Store, JDBC Persistence & Audit Hooks (`fineract-command-jdbc`, `fineract-command-audit`)
// ============================================================================

/// Rust migration of `org.apache.fineract.command.jdbc.store.domain.CommandEntity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntity {
    pub id: Option<i64>,
    pub command_id: Option<i64>,
    pub idempotency_key: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub initiated_by_username: Option<String>,
    pub executed_by_username: Option<String>,
    pub approved_by_username: Option<String>,
    pub rejected_by_username: Option<String>,
    pub error: Option<String>,
    pub state: CommandState,
    pub request: Option<JsonValue>,
    pub response: Option<JsonValue>,
}

/// Rust migration of `org.apache.fineract.command.jdbc.store.mapping.CommandMapper`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CommandMapper;

impl CommandMapper {
    pub fn new() -> Self {
        Self
    }

    pub fn map_request<T: Serialize>(&self, command: &Command<T>) -> CommandEntity {
        let req_val = command
            .payload
            .as_ref()
            .and_then(|p| serde_json::to_value(p).ok())
            .map(|v| attach_class_attribute(v, std::any::type_name::<T>()));
        self.map_raw(&command.meta, req_val, None)
    }

    pub fn map_request_response<T: Serialize, R: Serialize>(
        &self,
        command: &Command<T>,
        response: &R,
    ) -> CommandEntity {
        let req_val = command
            .payload
            .as_ref()
            .and_then(|p| serde_json::to_value(p).ok())
            .map(|v| attach_class_attribute(v, std::any::type_name::<T>()));
        let res_val = serde_json::to_value(response)
            .ok()
            .map(|v| attach_class_attribute(v, std::any::type_name::<R>()));
        self.map_raw(&command.meta, req_val, res_val)
    }

    pub fn map_raw(
        &self,
        meta: &CommandMeta,
        request: Option<JsonValue>,
        response: Option<JsonValue>,
    ) -> CommandEntity {
        CommandEntity {
            id: meta.command_id,
            command_id: meta.command_id,
            idempotency_key: meta.idempotency_key.clone(),
            ip_address: meta.ip_address.clone(),
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            executed_at: meta.executed_at,
            approved_at: meta.approved_at,
            rejected_at: meta.rejected_at,
            initiated_by_username: meta.initiated_by_username.clone(),
            executed_by_username: meta.executed_by_username.clone(),
            approved_by_username: meta.approved_by_username.clone(),
            rejected_by_username: meta.rejected_by_username.clone(),
            error: meta.error.clone(),
            state: CommandState::Unknown,
            request,
            response,
        }
    }
}

fn attach_class_attribute(mut val: JsonValue, type_name: &str) -> JsonValue {
    if let JsonValue::Object(ref mut map) = val {
        map.insert(
            constants::COMMAND_JSON_CLASS_ATTRIBUTE.to_string(),
            JsonValue::String(type_name.to_string()),
        );
    }
    val
}

/// Rust migration of `org.apache.fineract.command.jdbc.store.domain.CommandRepository`.
#[derive(Default)]
pub struct CommandRepository {
    next_id: AtomicI64,
    entities: RwLock<HashMap<i64, CommandEntity>>,
    fail_saves: Mutex<bool>,
}

impl CommandRepository {
    pub fn new() -> Self {
        Self {
            next_id: AtomicI64::new(1),
            entities: RwLock::new(HashMap::new()),
            fail_saves: Mutex::new(false),
        }
    }

    pub fn set_fail_saves(&self, fail: bool) {
        *self.fail_saves.lock().unwrap() = fail;
    }

    pub fn save(&self, mut entity: CommandEntity) -> Result<CommandEntity, String> {
        if *self.fail_saves.lock().unwrap() {
            return Err("Simulated database failure".to_string());
        }
        let id = entity
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        entity.id = Some(id);
        entity.command_id = Some(id);
        self.entities.write().unwrap().insert(id, entity.clone());
        Ok(entity)
    }

    pub fn find_by_id(&self, id: i64) -> Option<CommandEntity> {
        self.entities.read().unwrap().get(&id).cloned()
    }

    pub fn find_one_by_idempotency_key(&self, key: &str) -> Option<CommandEntity> {
        self.entities
            .read()
            .unwrap()
            .values()
            .find(|e| e.idempotency_key.as_deref() == Some(key))
            .cloned()
    }
}

/// Rust migration of `org.apache.fineract.command.jdbc.JdbcCommandProperties`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdbcCommandProperties {
    pub enabled: bool,
    pub file_dead_letter_queue_enabled: bool,
    pub file_dead_letter_queue_path: PathBuf,
}

impl Default for JdbcCommandProperties {
    fn default() -> Self {
        Self {
            enabled: true,
            file_dead_letter_queue_enabled: true,
            file_dead_letter_queue_path: PathBuf::from("/tmp/fineract-dlq"),
        }
    }
}

/// Rust migration of `org.apache.fineract.command.core.CommandStore` and `JdbcCommandStore`.
pub struct JdbcCommandStore {
    pub mapper: CommandMapper,
    pub repository: Arc<CommandRepository>,
    pub properties: JdbcCommandProperties,
}

impl JdbcCommandStore {
    pub fn new(repository: Arc<CommandRepository>, properties: JdbcCommandProperties) -> Self {
        let store = Self {
            mapper: CommandMapper::new(),
            repository,
            properties,
        };
        store.on_startup();
        store
    }

    pub fn on_startup(&self) {
        let _ = fs::create_dir_all(&self.properties.file_dead_letter_queue_path);
    }

    pub fn get_request_by_id<T: DeserializeOwned>(&self, id: i64) -> Option<T> {
        let entity = self.repository.find_by_id(id)?;
        serde_json::from_value(entity.request?).ok()
    }

    pub fn get_response_by_id<T: DeserializeOwned>(&self, id: i64) -> Option<T> {
        let entity = self.repository.find_by_id(id)?;
        serde_json::from_value(entity.response?).ok()
    }

    pub fn get_state_by_id(&self, id: i64) -> CommandState {
        self.repository
            .find_by_id(id)
            .map(|e| e.state)
            .unwrap_or(CommandState::Unknown)
    }

    pub fn get_request_by_key<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let entity = self.repository.find_one_by_idempotency_key(key)?;
        serde_json::from_value(entity.request?).ok()
    }

    pub fn get_response_by_key<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let entity = self.repository.find_one_by_idempotency_key(key)?;
        serde_json::from_value(entity.response?).ok()
    }

    pub fn get_state_by_key(&self, key: &str) -> CommandState {
        self.repository
            .find_one_by_idempotency_key(key)
            .map(|e| e.state)
            .unwrap_or(CommandState::Unknown)
    }

    pub fn store_raw(
        &self,
        meta: &mut CommandMeta,
        request_json: Option<JsonValue>,
        response_json: Option<JsonValue>,
        state: CommandState,
    ) {
        let mut entity = self.mapper.map_raw(meta, request_json.clone(), response_json);
        entity.state = state;

        match self.repository.save(entity) {
            Ok(saved) => {
                meta.command_id = saved.id;
            }
            Err(_err) => {
                // Fallback: write to file dead-letter queue if enabled
                if self.properties.file_dead_letter_queue_enabled {
                    let _ = fs::create_dir_all(&self.properties.file_dead_letter_queue_path);
                    let ts_millis = meta
                        .created_at
                        .unwrap_or_else(Utc::now)
                        .timestamp_millis();
                    let idem = meta
                        .idempotency_key
                        .clone()
                        .unwrap_or_else(|| Uuid::new_v4().to_string());
                    let filename = format!("{}-{}.json", ts_millis, idem);
                    let path = self.properties.file_dead_letter_queue_path.join(filename);
                    let dlq_payload = serde_json::json!({
                        "meta": meta,
                        "payload": request_json,
                        "state": state,
                    });
                    let _ = fs::write(
                        path,
                        serde_json::to_string_pretty(&dlq_payload).unwrap_or_default(),
                    );
                }
            }
        }
    }
}

/// Audit hook before execution (`org.apache.fineract.command.audit.hook.AuditCommandHookBefore`).
pub struct AuditCommandHookBefore {
    pub store: Arc<JdbcCommandStore>,
}

impl CommandHookBefore for AuditCommandHookBefore {
    fn on_before(&self, meta: &mut CommandMeta, payload_json: &JsonValue) {
        let now = Utc::now();
        meta.executed_by_username = meta.initiated_by_username.clone();
        meta.updated_at = Some(now);
        meta.executed_at = Some(now);
        self.store.store_raw(
            meta,
            Some(payload_json.clone()),
            None,
            CommandState::UnderProcessing,
        );
    }
}

/// Audit hook after execution (`org.apache.fineract.command.audit.hook.AuditCommandHookAfter`).
pub struct AuditCommandHookAfter {
    pub store: Arc<JdbcCommandStore>,
}

impl CommandHookAfter for AuditCommandHookAfter {
    fn on_after(&self, meta: &mut CommandMeta, payload_json: &JsonValue, response_json: &JsonValue) {
        let now = Utc::now();
        meta.executed_by_username = meta.initiated_by_username.clone();
        meta.updated_at = Some(now);
        meta.executed_at = Some(now);
        self.store.store_raw(
            meta,
            Some(payload_json.clone()),
            Some(response_json.clone()),
            CommandState::Processed,
        );
    }
}

/// Audit hook on error (`org.apache.fineract.command.audit.hook.AuditCommandHookError`).
pub struct AuditCommandHookError {
    pub store: Arc<JdbcCommandStore>,
}

impl CommandHookError for AuditCommandHookError {
    fn on_error(&self, meta: &mut CommandMeta, payload_json: &JsonValue, error: &CommandError) {
        let now = Utc::now();
        meta.error = Some(error.to_string());
        meta.updated_at = Some(now);
        meta.executed_at = Some(now);
        self.store.store_raw(
            meta,
            Some(payload_json.clone()),
            None,
            CommandState::Error,
        );
    }
}

// ============================================================================
// 6. Sample Commands, Handlers & Axum REST API (`fineract-command-test`)
// ============================================================================

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DummyRequest {
    pub content: Option<String>,
}

impl DummyRequest {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DummyResponse {
    pub content: Option<String>,
    pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DummyErrorRequest {
    pub content: Option<String>,
}

impl DummyErrorRequest {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DummyErrorResponse {
    pub content: Option<String>,
}

pub type DummyCommand = Command<DummyRequest>;
pub type DummyErrorCommand = Command<DummyErrorRequest>;

pub trait DummyService: Send + Sync {
    fn process(&self, request: &DummyRequest) -> Result<DummyResponse, CommandError>;
    fn process_error(&self, request: &DummyErrorRequest) -> Result<DummyErrorResponse, CommandError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultDummyService {
    pub tenant_id: Option<&'static str>,
}

impl DummyService for DefaultDummyService {
    fn process(&self, request: &DummyRequest) -> Result<DummyResponse, CommandError> {
        let content = request.content.as_deref().unwrap_or("");
        if content.trim().is_empty() {
            return Err(CommandError::CommandIllegalArgumentException {
                command_name: "DummyCommand".to_string(),
                message: "must not be blank".to_string(),
            });
        }
        Ok(DummyResponse {
            content: Some(content.to_ascii_uppercase()),
            tenant_id: Some(self.tenant_id.unwrap_or("dummy").to_string()),
        })
    }

    fn process_error(
        &self,
        request: &DummyErrorRequest,
    ) -> Result<DummyErrorResponse, CommandError> {
        let content = request.content.clone().unwrap_or_default();
        Err(CommandError::DummyException {
            request_content: content.clone(),
            message: format!("Error processing dummy request: {}", content),
        })
    }
}

pub struct DummyCommandHandler<S: DummyService = DefaultDummyService> {
    pub service: S,
}

impl<S: DummyService> DummyCommandHandler<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S: DummyService + 'static> CommandHandler<DummyRequest, DummyResponse>
    for DummyCommandHandler<S>
{
    fn handle(&self, command: &mut Command<DummyRequest>) -> Result<DummyResponse, CommandError> {
        let req = command
            .payload()
            .ok_or_else(|| CommandError::CommandIllegalArgumentException {
                command_name: "DummyCommand".to_string(),
                message: "Missing payload".to_string(),
            })?;
        self.service.process(req)
    }
}

pub struct DummyErrorCommandHandler<S: DummyService = DefaultDummyService> {
    pub service: S,
}

impl<S: DummyService> DummyErrorCommandHandler<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S: DummyService + 'static> CommandHandler<DummyErrorRequest, DummyErrorResponse>
    for DummyErrorCommandHandler<S>
{
    fn handle(
        &self,
        command: &mut Command<DummyErrorRequest>,
    ) -> Result<DummyErrorResponse, CommandError> {
        let req = command
            .payload()
            .ok_or_else(|| CommandError::CommandIllegalArgumentException {
                command_name: "DummyErrorCommand".to_string(),
                message: "Missing payload".to_string(),
            })?;
        self.service.process_error(req)
    }
}

/// RFC 7807 ProblemDetail response matching Spring's `org.springframework.http.ProblemDetail`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDetail {
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(default)]
    pub properties: JsonMap<String, JsonValue>,
}

/// Shared state for the Axum `DummyApiController`.
#[derive(Clone)]
pub struct DummyApiState {
    pub dispatcher: Arc<SynchronousCommandDispatcher>,
    pub properties: CommandProperties,
    pub seen_request_ids: Arc<Mutex<HashSet<String>>>,
}

/// Builds the Axum router equivalent to `org.apache.fineract.command.test.sample.api.DummyApiController`.
pub fn build_dummy_api_router(state: DummyApiState) -> Router {
    Router::new()
        .route("/test/dummy/sync", post(dummy_sync_handler))
        .route("/test/dummy/async", post(dummy_async_handler))
        .with_state(state)
}

async fn dummy_sync_handler(
    State(state): State<DummyApiState>,
    headers: HeaderMap,
    Json(request): Json<DummyRequest>,
) -> Response {
    execute_dummy_endpoint(state, headers, request).await
}

async fn dummy_async_handler(
    State(state): State<DummyApiState>,
    headers: HeaderMap,
    Json(request): Json<DummyRequest>,
) -> Response {
    tokio::task::spawn(execute_dummy_endpoint(state, headers, request))
        .await
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn execute_dummy_endpoint(
    state: DummyApiState,
    headers: HeaderMap,
    request: DummyRequest,
) -> Response {
    // Check request ID idempotency if provided
    if let Some(req_id) = headers
        .get(constants::COMMAND_HTTP_HEADER_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
    {
        let mut seen = state.seen_request_ids.lock().unwrap();
        if !seen.insert(req_id.to_string()) {
            let problem = ProblemDetail {
                title: "Internal Server Error".to_string(),
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                detail: "Duplicate request ID".to_string(),
                properties: JsonMap::new(),
            };
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/problem+json")],
                Json(problem),
            )
                .into_response();
        }
    }

    let mut command = DummyCommand::new(request);
    if let Some(idem) = headers
        .get(&state.properties.idempotency_key_header_name)
        .and_then(|v| v.to_str().ok())
    {
        command.set_idempotency_key(idem);
    }

    let supplier = state.dispatcher.dispatch::<DummyRequest, DummyResponse>(&mut command);
    match supplier() {
        Ok(mut response) => {
            if let Some(tenant) = headers
                .get(constants::COMMAND_HTTP_HEADER_TENANT_ID)
                .and_then(|v| v.to_str().ok())
            {
                response.tenant_id = Some(tenant.to_string());
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => {
            let mut props = JsonMap::new();
            props.insert("error".to_string(), JsonValue::String(err.to_string()));
            let problem = ProblemDetail {
                title: "Bad Request".to_string(),
                status: StatusCode::BAD_REQUEST.as_u16(),
                detail: err.to_string(),
                properties: props,
            };
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/problem+json")],
                Json(problem),
            )
                .into_response()
        }
    }
}

// ============================================================================
// 7. Unit & Integration Tests (Migrated from fineract-command, fineract-command-jdbc,
//    fineract-command-disruptor, fineract-command-async, fineract-command-audit)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn create_default_test_dispatcher(
        store: Option<Arc<JdbcCommandStore>>,
    ) -> Arc<SynchronousCommandDispatcher> {
        let dummy_handler = Arc::new(TypedHandlerWrapper::new(DummyCommandHandler::new(
            DefaultDummyService::default(),
        )));
        let error_handler = Arc::new(TypedHandlerWrapper::new(DummyErrorCommandHandler::new(
            DefaultDummyService::default(),
        )));
        let handler_manager = Arc::new(DefaultCommandHandlerManager::new(vec![
            dummy_handler,
            error_handler,
        ]));
        handler_manager.init().expect("handler manager init");

        let mut before_hooks: Vec<Arc<dyn CommandHookBefore>> = vec![
            Arc::new(TimestampCommandHook),
            Arc::new(UsernameCommandHook::default()),
        ];
        let mut after_hooks: Vec<Arc<dyn CommandHookAfter>> = Vec::new();
        let mut error_hooks: Vec<Arc<dyn CommandHookError>> = Vec::new();

        if let Some(s) = store {
            before_hooks.push(Arc::new(AuditCommandHookBefore {
                store: Arc::clone(&s),
            }));
            after_hooks.push(Arc::new(AuditCommandHookAfter {
                store: Arc::clone(&s),
            }));
            error_hooks.push(Arc::new(AuditCommandHookError { store: s }));
        }

        let hook_manager = Arc::new(DefaultCommandHookManager::new(
            before_hooks,
            after_hooks,
            error_hooks,
        ));
        Arc::new(SynchronousCommandDispatcher::new(
            handler_manager,
            hook_manager,
        ))
    }

    // ------------------------------------------------------------------------
    // 1. CommandDispatcherTest (Migrated from fineract-command)
    // ------------------------------------------------------------------------

    #[test]
    fn command_dispatcher_process() {
        let dispatcher = create_default_test_dispatcher(None);
        let content = "hello";
        let mut command = DummyCommand::new(DummyRequest::new(content));
        command.set_idempotency_key("1234567890");
        command.set_ip_address("127.0.0.1");
        command.set_initiated_by_username("abc");

        let result = dispatcher.dispatch::<DummyRequest, DummyResponse>(&mut command);
        let response = result().expect("Response should not be error");

        assert_eq!(
            Some(content.to_ascii_uppercase()),
            response.content,
            "Wrong response content."
        );
    }

    #[test]
    fn command_dispatcher_error() {
        let dispatcher = create_default_test_dispatcher(None);
        let content = "hello error";
        let mut command = DummyErrorCommand::new(DummyErrorRequest::new(content));

        let result = dispatcher.dispatch::<DummyErrorRequest, DummyErrorResponse>(&mut command);
        let err = result().expect_err("Expected DummyException!");
        assert!(matches!(err, CommandError::DummyException { .. }));
    }

    // ------------------------------------------------------------------------
    // 2. DefaultCommandHandlerManagerTest (Migrated from fineract-command)
    // ------------------------------------------------------------------------

    struct DummyCommandHandlerOriginal;
    impl CommandHandler<DummyRequest, DummyResponse> for DummyCommandHandlerOriginal {
        fn handle(&self, _command: &mut Command<DummyRequest>) -> Result<DummyResponse, CommandError> {
            Ok(DummyResponse {
                content: Some("res1".to_string()),
                tenant_id: None,
            })
        }
    }

    struct DummyCommandHandlerDuplicate;
    impl CommandHandler<DummyRequest, DummyResponse> for DummyCommandHandlerDuplicate {
        fn handle(&self, _command: &mut Command<DummyRequest>) -> Result<DummyResponse, CommandError> {
            Ok(DummyResponse {
                content: Some("res2".to_string()),
                tenant_id: None,
            })
        }
    }

    #[test]
    fn handler_manager_process_init() {
        let handlers: Vec<Arc<dyn ErasedCommandHandler>> =
            vec![Arc::new(TypedHandlerWrapper::new(DummyCommandHandlerOriginal))];
        let manager = DefaultCommandHandlerManager::new(handlers);
        assert!(manager.init().is_ok());
    }

    #[test]
    fn handler_manager_error_duplicate_command_handlers() {
        let handlers: Vec<Arc<dyn ErasedCommandHandler>> = vec![
            Arc::new(TypedHandlerWrapper::new(DummyCommandHandlerOriginal)),
            Arc::new(TypedHandlerWrapper::new(DummyCommandHandlerDuplicate)),
        ];
        let manager = DefaultCommandHandlerManager::new(handlers);
        let err = manager.init().expect_err("Expected duplicate exception");
        assert!(matches!(
            err,
            CommandError::CommandHandlerDuplicateException { .. }
        ));
    }

    #[test]
    fn handler_manager_error_invalid_command_request_type() {
        let handlers: Vec<Arc<dyn ErasedCommandHandler>> = vec![
            Arc::new(TypedHandlerWrapper::new(DummyCommandHandlerDuplicate)),
            Arc::new(UntypedCommandHandler {
                name: "DummyCommandHandlerNoType",
            }),
        ];
        let manager = DefaultCommandHandlerManager::new(handlers);
        let err = manager
            .init()
            .expect_err("Expected invalid request type exception");
        assert!(matches!(
            err,
            CommandError::CommandHandlerInvalidRequestTypeException { .. }
        ));
    }

    // ------------------------------------------------------------------------
    // 3. CommandSampleApiTest (Migrated from fineract-command with Axum 0.8)
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn command_sample_api_validation_and_sync_and_idempotency() {
        let repo = Arc::new(CommandRepository::new());
        let store = Arc::new(JdbcCommandStore::new(
            repo,
            JdbcCommandProperties::default(),
        ));
        let dispatcher = create_default_test_dispatcher(Some(Arc::clone(&store)));
        let state = DummyApiState {
            dispatcher,
            properties: CommandProperties::default(),
            seen_request_ids: Arc::new(Mutex::new(HashSet::new())),
        };
        let app = build_dummy_api_router(state);

        // 1. Validation error test (empty content)
        let req_validation = Request::builder()
            .method("POST")
            .uri("/test/dummy/sync")
            .header(constants::COMMAND_HTTP_HEADER_REQUEST_ID, Uuid::new_v4().to_string())
            .header(constants::COMMAND_HTTP_HEADER_TENANT_ID, "dummy")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&DummyRequest::default()).unwrap()))
            .unwrap();

        let resp_validation = app.clone().oneshot(req_validation).await.unwrap();
        assert_eq!(StatusCode::BAD_REQUEST, resp_validation.status());
        let body_bytes = axum::body::to_bytes(resp_validation.into_body(), 1024 * 16)
            .await
            .unwrap();
        let problem: ProblemDetail = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!problem.detail.is_empty());

        // 2. dummyApiSync test + audit persistence verification
        let content = "test-sync";
        let idempotency_key = Uuid::new_v4().to_string();
        let req_sync = Request::builder()
            .method("POST")
            .uri("/test/dummy/sync")
            .header(constants::COMMAND_HTTP_HEADER_REQUEST_ID, Uuid::new_v4().to_string())
            .header(constants::COMMAND_HTTP_HEADER_TENANT_ID, "dummy")
            .header("Idempotency-Key", &idempotency_key)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&DummyRequest::new(content)).unwrap(),
            ))
            .unwrap();

        let resp_sync = app.clone().oneshot(req_sync).await.unwrap();
        assert_eq!(StatusCode::OK, resp_sync.status());
        let sync_bytes = axum::body::to_bytes(resp_sync.into_body(), 1024 * 16)
            .await
            .unwrap();
        let result: DummyResponse = serde_json::from_slice(&sync_bytes).unwrap();
        assert_eq!(Some("TEST-SYNC".to_string()), result.content);
        assert_eq!(Some("dummy".to_string()), result.tenant_id);

        // Verify Audit hooks stored the command in JdbcCommandStore as PROCESSED
        assert_eq!(
            CommandState::Processed,
            store.get_state_by_key(&idempotency_key)
        );
        let stored_resp: DummyResponse = store
            .get_response_by_key(&idempotency_key)
            .expect("Stored response should exist");
        assert_eq!(Some("TEST-SYNC".to_string()), stored_resp.content);

        // 3. dummyApiIdempotencySync test (same request ID twice -> second returns 500)
        let dup_request_id = Uuid::new_v4().to_string();
        let make_idem_req = || {
            Request::builder()
                .method("POST")
                .uri("/test/dummy/sync")
                .header(constants::COMMAND_HTTP_HEADER_REQUEST_ID, &dup_request_id)
                .header(constants::COMMAND_HTTP_HEADER_TENANT_ID, "dummy")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&DummyRequest::new("test-idempotency")).unwrap(),
                ))
                .unwrap()
        };

        let first_resp = app.clone().oneshot(make_idem_req()).await.unwrap();
        assert_eq!(StatusCode::OK, first_resp.status());

        let second_resp = app.oneshot(make_idem_req()).await.unwrap();
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, second_resp.status());
    }

    // ------------------------------------------------------------------------
    // 4. CommandRepositoryTest (Migrated from fineract-command-jdbc)
    // ------------------------------------------------------------------------

    #[test]
    fn command_repository_save_and_find() {
        let repository = CommandRepository::new();
        let mapper = CommandMapper::new();

        let content = "hello";
        let now = Utc::now();
        let mut command = DummyCommand::new(DummyRequest::new(content));
        command.set_idempotency_key("1234567890");
        command.set_ip_address("127.0.0.1");
        command.set_created_at(now);
        command.set_executed_at(now);
        command.set_approved_at(now);
        command.set_rejected_at(now);
        command.set_initiated_by_username("abc");
        command.set_executed_by_username("abc");
        command.set_approved_by_username("abc");
        command.set_rejected_by_username("abc");

        let response = DummyResponse {
            content: Some(content.to_string()),
            tenant_id: None,
        };

        let mut command_entity = mapper.map_request_response(&command, &response);
        command_entity.state = CommandState::Error;
        command_entity.error = Some("Some error message".to_string());

        let result = repository.save(command_entity).expect("save entity");
        assert!(result.id.is_some());

        let found = repository
            .find_by_id(result.id.unwrap())
            .expect("entity should be found");
        assert_eq!(CommandState::Error, found.state);
        assert_eq!(Some("Some error message".to_string()), found.error);
        assert_eq!(Some("1234567890".to_string()), found.idempotency_key);
    }

    // ------------------------------------------------------------------------
    // 5. Async & Disruptor Dispatchers Tests
    // ------------------------------------------------------------------------

    #[test]
    fn async_and_disruptor_command_dispatchers_work() {
        let dummy_handler = Arc::new(TypedHandlerWrapper::new(DummyCommandHandler::new(
            DefaultDummyService::default(),
        )));
        let handler_manager = Arc::new(DefaultCommandHandlerManager::new(vec![dummy_handler]));
        let hook_manager = Arc::new(DefaultCommandHookManager::default());

        // AsyncCommandDispatcher
        let async_dispatcher = AsyncCommandDispatcher::new(
            Arc::clone(&handler_manager),
            Arc::clone(&hook_manager),
        );
        let mut cmd1 = DummyCommand::new(DummyRequest::new("async hello"));
        let res1 = async_dispatcher.dispatch::<DummyRequest, DummyResponse>(&mut cmd1)().unwrap();
        assert_eq!(Some("ASYNC HELLO".to_string()), res1.content);

        // DisruptorCommandDispatcher
        let mut disruptor_dispatcher =
            DisruptorCommandDispatcher::new(2048, handler_manager, hook_manager);
        let mut cmd2 = DummyCommand::new(DummyRequest::new("disruptor hello"));
        let res2 = disruptor_dispatcher
            .dispatch::<DummyRequest, DummyResponse>(&mut cmd2)()
            .unwrap();
        assert_eq!(Some("DISRUPTOR HELLO".to_string()), res2.content);
        assert_eq!(1, disruptor_dispatcher.published_sequences());
        disruptor_dispatcher.close();
    }
}
