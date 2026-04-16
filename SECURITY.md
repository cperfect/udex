# Security Policy

## Supported Versions

Udex is currently in early development. Only the latest commit on `main` receives security fixes.

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ |
| older commits | ❌ |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Use [GitHub Private Security Advisories](https://github.com/cperfect/udex/security/advisories/new) to report vulnerabilities confidentially. This keeps details private until a fix is available.

Include as much of the following as you can:

- Type of vulnerability (e.g. authentication bypass, injection, information disclosure)
- The component affected (server, CLI, API crate, datastore)
- File paths and line numbers relevant to the issue
- Step-by-step instructions to reproduce
- Proof-of-concept or exploit code (if available)
- Impact assessment — what an attacker could achieve

## Response Timeline

Response times are best efforts only.

| Milestone | Target |
|-----------|--------|
| Acknowledgement | 3 business days |
| Initial assessment | 7 business days |
| Fix or workaround | Dependent on severity and complexity |

## Disclosure Policy

Once a fix is available and released, the vulnerability will be disclosed via a GitHub Security Advisory. Credit will be given to the reporter unless they prefer to remain anonymous.

## Scope

The following are **in scope**:

- `udex-server` — gRPC server, authentication, authorisation
- `udex-api` — protobuf types, JWT claims, authorisation logic
- `udex-datastore` — PostgreSQL datastore implementation
- `udex` CLI — token handling, configuration, server communication

The following are **out of scope**:

- Vulnerabilities in third-party dependencies (report these upstream; see `.trivyignore` for acknowledged findings with rationale)
- Issues requiring physical access to the host
- Social engineering attacks
