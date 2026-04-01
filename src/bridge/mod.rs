//! ThunderCode bridge mode -- remote control of ThunderCode sessions.
//!
//! Provides the bridge server that connects to the environments API,
//! polls for work, and spawns ThunderCode sessions as child processes. This
//! enables remote control of local sessions from primary.ai.
//!
//! # Architecture
//!
//! - **`api`** -- Bridge API client for environment registration, polling, and
//!   session lifecycle.
//! - **`config`** -- Bridge configuration (auth, URLs, tokens).
//! - **`messaging`** -- Message filtering, transformation, and echo-dedup.
//! - **`runner`** -- Child process management for spawned sessions.
//! - **`poll`** -- Poll interval configuration and defaults.
//! - **`types`** -- Protocol types (work responses, session handles, etc.).
//! - **`bridge_loop`** -- The main polling loop that ties everything together.
//!
//! Ported from:
//! - `ref/bridge/bridgeMain.ts`
//! - `ref/bridge/bridgeApi.ts`
//! - `ref/bridge/bridgeConfig.ts`
//! - `ref/bridge/bridgeMessaging.ts`
//! - `ref/bridge/sessionRunner.ts`
//! - `ref/bridge/types.ts`

pub mod api;
pub mod bridge_loop;
pub mod config;
pub mod messaging;
pub mod poll;
pub mod runner;
pub mod types;

pub use api::{BridgeApiClient, BridgeFatalError};
pub use bridge_loop::{run_bridge_loop, BackoffConfig};
pub use config::{get_bridge_access_token, get_bridge_base_url};
pub use messaging::BoundedUuidSet;
pub use runner::{SessionHandle, SessionSpawner};
pub use types::{BridgeConfig, BridgeState, SpawnMode, WorkResponse};
