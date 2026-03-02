//! CogWorks CLI entry point.
//!
//! This binary is the composition root for the entire system. Responsibilities:
//!
//! 1. **Parse configuration** — load `.cogworks/config.toml` and validate it.
//! 2. **Wire observability** — configure `tracing-subscriber` with a JSON layer
//!    and an OpenTelemetry OTLP exporter. All `tracing` spans and structured
//!    events emitted by every crate in the workspace flow through this layer.
//! 3. **Construct infrastructure** — create concrete instances of all
//!    infrastructure types (`GithubClient`, `AnthropicProvider`,
//!    `ExtensionApiClient`, event source) and inject them into `PipelineExecutor`.
//! 4. **Select trigger mode** — based on `CliConfig.trigger_mode`:
//!    - `SingleShot` — synthesise one [`pipeline::GitHubEvent`] from `--issue-url`
//!      and call `run_step` once (Phase 1 CLI).
//!    - `Webhook` — construct a `GitHubWebhookEventSource` and run the event loop.
//!    - `Queue` — construct a `QueueEventSource` and run the event loop.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/infrastructure.md` §cli for the full contract.
//!
//! *This binary is a skeleton. Implementation is added in PR 10.*

fn main() {
    todo!("CLI entry point — see docs/spec/interfaces/infrastructure.md §cli")
}
