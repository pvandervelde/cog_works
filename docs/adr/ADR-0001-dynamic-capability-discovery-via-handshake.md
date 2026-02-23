# ADR-0001: Dynamic Capability Discovery via Handshake

**Status:** Accepted
**Date:** 2026-02-24
**Deciders:** Architecture

---

## Context

The original Extension API spec required domain service authors to statically declare their service's `domain`, `capabilities`, `artifact_types`, and `interface_types` in the `.cogworks/services.toml` registration file. CogWorks would read these declarations at startup and use them for routing.

This approach has several problems:

1. **Config / implementation drift** — The declared capabilities can become stale or incorrect relative to what the service actually implements. There is no automatic verification that the service supports what it claims in config.
2. **Deployment-time friction** — Every time a domain service adds or removes a capability, the operator must update `services.toml` before CogWorks will use the new capability. This is a manual step that is easy to forget and hard to validate in CI.
3. **Version mismatch risk** — If a rolled-back service version no longer supports a capability still declared in config, CogWorks will route requests to it and receive unexpected errors.
4. **Breaking change for existing services** — Any domain service that adds capabilities but does not update its config registration is silently not used for those capabilities.

The Extension API domain service template spec introduced a handshake protocol: a structured request/response that the domain service responds to with its full capability set, including `name`, `service_version`, `api_version`, `domain`, `capabilities`, `artifact_types`, and `interface_types`. This is already required of all conformant services.

---

## Decision

CogWorks will discover domain service capabilities dynamically at startup (and on reconnect) via the handshake protocol. The `.cogworks/services.toml` registration file contains only the information needed to establish a connection:

```toml
[[services]]
name = "firmware-service"
transport = "unix"
endpoint = "/var/run/cogworks/firmware.sock"
```

Capabilities, artifact types, interface types, and domain identity are read from the handshake response. They are not stored in config. CogWorks caches the handshake result in memory for the duration of a run and re-queries on connection error.

Services whose `api_version` is incompatible with CogWorks' supported range are rejected at startup and reported as unavailable. All remaining pipeline steps proceed without them, and the operator is notified via structured log output.

---

## Consequences

### Positive

- **No config drift** — The source of truth for a service's capabilities is the service itself. A rolled-back or mis-configured service cannot claim capabilities it does not have.
- **Zero operator friction on capability changes** — Operators only need to update `services.toml` when the service's connection endpoint changes. Capability changes are self-describing.
- **Automatic version gating** — API version incompatibility is detected at startup, not at first invocation, giving operators an early and clear failure signal.
- **Simpler config schema** — `services.toml` entries are minimal and stable; the schema is unlikely to change.

### Negative

- **Breaking change for existing services** — Domain services built against the old static-config spec must implement the handshake protocol (standardised in the domain service template) before they are usable. There is no backward compatibility path for services that do not respond to handshake requests.
- **Startup latency** — CogWorks must complete handshakes with all registered services before beginning pipeline processing. For services reached over HTTP this adds network round-trips.
- **Handshake implementation required** — Domain service authors must implement the handshake endpoint. The domain service template provides a reference implementation; minimal it is not optional.

### Migration Path

Domain service authors migrating from a static-config declaration must:

1. Implement the handshake protocol endpoint (see domain service template spec, `HANDSHAKE` method)
2. Return the canonical response fields: `name`, `service_version`, `api_version`, `domain`, `capabilities`, `artifact_types`, `interface_types`, `status`
3. Remove `domain`, `capabilities`, `artifact_types`, and `interface_types` from their `services.toml` entry (CogWorks ignores them after this change; leaving them is harmless but misleading)

---

## Alternatives Considered

### Alternative A: Keep static config, add optional validation handshake

Config declarations remain authoritative; a handshake is performed optionally to verify them. Mismatch produces a warning.

**Rejected because:** Warnings are ignored in practice. The config remains the source of truth and the drift problem is not resolved. Adds complexity without solving the core problem.

### Alternative B: Static config with schema validation in CI

Domain service authors publish a capability manifest file alongside their service binary; CogWorks CI validates the manifest matches the `services.toml` declaration.

**Rejected because:** Requires coordination between multiple repositories and does not catch production deployment mismatches. Runtime discovery is simpler and more reliable.

### Alternative C: Runtime capability negotiation per request

Rather than a startup handshake, CogWorks sends a capability probe as part of every method invocation (e.g., a header or preamble field). The service returns its current capabilities alongside the method response.

**Rejected because:** Adds latency to every request. Startup handshake is sufficient — operational teams need to know about capability availability before work items are processed, not after the first request fails.
