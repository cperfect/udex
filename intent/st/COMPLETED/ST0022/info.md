---
verblock: "29 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: jwks-refresh
created: 20260529
completed: 20260529
---

# ST0022: JWKS refresh

## Objective

Make the server refresh JWKS information for validation without re-start.

## Context

Currently when a JWKS url is configured for authorization the url is fetched and the keys cached at start-up and never refreshed otherwise.

We want to handle 2 mechanisms of refresh:
1. **Cache Miss**: When the server attempts to validate JWT with Kid that is not in the cache, refresh.
2. **Configured Expiry**: A max-age seconds for the cache can be configured to trigger a regular cache refresh attempt. The default for this will 86400 seconds (1 day) as it is expected that keys won't be rotated often.

In order to avoid a DoS via this mechanism we will support the following controls:
1. **Max Failed Refresh Attempts**: A configurable number of successive failed refresh attempts will be allowed - the default will be 5.
2. **Exponential Backoff with Equal Jitter**: Retries will back off exponential based on a configurable factor (default 3) with a small random ammount applied.
3. **Expiry Jitter**: A small random amount will be applied when the next expiry time is calculated if a max-age has been set.

A server that cannot refresh its cache should retain its current one and continue to serve as there might be still be valid keys in its cache but it should log errors regularly.

### Out of scope
We might in future respect cache control headers sent by the JWKS endpoint.

## Related Steel Threads

- ST0007 Integrated OAuth2 Authorization Server - added JWT validation with JWKS.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
