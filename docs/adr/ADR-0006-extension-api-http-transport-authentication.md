# ADR-0006: Extension API HTTP Transport Authentication (Deferred)

**Status:** Proposed
**Date:** 2026-03-29
**Deciders:** Architecture

---

## Context

The Extension API client (`crates/extension-api`) supports two transport modes for
communicating with domain services:

1. **Unix domain socket** (default) — access control is provided by the operating
   system's file-system permissions on the socket file. Only processes running as
   the same user (or group) as the domain service can connect. No additional
   authentication layer is required for the local, same-host case.

2. **HTTP/1.1** — intended for domain services running on a remote host or in a
   container where Unix sockets are unavailable. The HTTP transport currently sends
   unauthenticated requests.

The HTTP transport without authentication is safe only when the network path between
CogWorks and the domain service is trusted (e.g. a private pod network or loopback).
Deploying it across a network boundary without authentication would expose domain
service operations to any network-adjacent caller.

The correct authentication mechanism is not yet decided. The candidates are:

| Mechanism | Pros | Cons |
|---|---|---|
| Mutual TLS (mTLS) | Strong identity on both sides; no shared secrets | PKI complexity; cert rotation overhead |
| Bearer token (JWT/opaque) | Simple to implement; stateless | Requires secure secret distribution |
| HMAC-signed request headers | No PKI; tamper-evident; per-request | Shared secret management; clock skew |

The choice depends on the deployment model (single-host vs. multi-host,
Kubernetes vs. bare-metal) and the operational capabilities of the target
environment, which are not yet fully defined.

---

## Decision

**Deferred.** The HTTP transport authentication mechanism is explicitly left
unspecified until the deployment model is clarified. In the interim:

1. The HTTP transport sends unauthenticated requests.
2. The `extension-api` crate documentation and the infrastructure spec
   (`docs/spec/interfaces/infrastructure.md` §HttpTransport) carry a prominent
   warning: _"Do not deploy the HTTP transport on a network boundary without
   adding authentication."_
3. This ADR is the tracking artifact for the open decision so it does not fall
   through the cracks.

The Unix socket transport is the recommended default for all deployments until
this decision is resolved. HTTP transport should only be used in development or
in deployments where the network path is provably trusted (e.g. localhost only).

---

## Consequences

- **Positive**: Unblocks the implementation phase without prematurely committing
  to an authentication mechanism that may not fit the deployment model.
- **Negative**: HTTP transport cannot be safely deployed across a network boundary
  until this decision is resolved. Teams that need remote domain services must
  use a network-layer control (VPN, private subnet, firewall rules) as a
  compensating control in the interim.
- **Follow-up required**: Before the `extension-api` HTTP transport is used in
  production across a network boundary, this ADR must be updated to `Accepted`
  with a specific authentication mechanism selected, and the `HttpTransport`
  implementation must be updated accordingly. A corresponding issue should be
  opened to track the implementation work.

---

## References

- `crates/extension-api/src/http.rs` — HTTP transport implementation stub
- `docs/spec/interfaces/infrastructure.md` §HttpTransport — transport spec
- ADR-0001 — Extension API handshake protocol (related context on the Extension API design)
