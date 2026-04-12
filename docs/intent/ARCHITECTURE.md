# Udex Architecture Intent

## Overview

Udex is a universal lookup directory for entities that is intended to be lightweight, fast and efficient for high transaction volumes, possibly across organisational and regulatory boundaries. Udex is intended to be used in complex distributed integration scenarios where it is desirable for internal representations of entities, potentially including the primary keys, not to be exposed across the integration boundaries so as to prevent tight coupling across those boundaries and limit the exposure to [Hyrum's Law](https://www.hyrumslaw.com/).

Udex holds one or more Indices which consist of entries from globally unique keys to contexts: globally unique keys are just that and may be realised as UUIDs or another suitable standard and are allocated by Udex and are intended to be truly globally unique, whereas contexts are just one or more key value pairs, plus a fast hash of the key value pairs that are intended to be the minimum to uniquely and reliably map the entity in some other context (e.g. a source database or registry, or a different Udex index): essentially a Udex entry is Globally Unique Key <-> Context Hash -> Context Key/Value Pairs and an Index is a namespace of Contexts.

Indices will have a name that is a string of lower case latin letters with the option of underscore infixes. Index names must be unique within a given Udex system and are immutable. Indices will also have a human friendly but short description (default empty) and a configurable max number of bulk operations (default 100) per client.

A Globally Unique Key is intended to be just that: unique not only within an index but across all indices (and ideally across everything — i.e. truly global).

A context is uniquely determined by its hash, with the hash determined by Udex by applying its hash function against the key value pairs. There could however be more than one unique key for any context in an Index. The same context can appear only once in an Index but could appear in multiple indices. A context is not necessarily one to one with an entity: the same entity could be mapped to different contexts with different key value pairs. This is opaque to Udex and is entirely up to how clients use it.

A context, including its hash, will be *immutable* and cannot be updated, though it can be deleted and replaced.

## Components

The system has two server side components, which combined support one or more indices.

* **Server**: a stateless application component that provides the APIs and business logic for Udex. It is intended that the application component can be horizontally scaled in and out. The server is largely generated from API specifications.
* **Configuration**: determines which datastore to use (and any configuration required for that datastore) and which indices exist and their configurations. Application components share the same configuration. Configuration can be static (e.g. file) or dynamic (e.g. via something like etcd).
* **Datastore**: holds the Udex index state. Transactions, distribution and scaling are handled by the datastore implementation and are opaque to the application and, as far as possible, the configuration.
* **CLI** _(Deferred)_: the CLI can be used to generate or update the configuration, start and stop the server and as a client for test and operational purposes.
* **SDKs** _(Deferred)_: to enable clients to be built for various technologies and frameworks. SDKs will largely be generated from API specifications and will support generating JWTs in the right format for the server.

## Operations

Udex has two types of operations: Index and Admin.

### Index Operations

Index operations, including bulk operations, are transactional.

* **Create** an entry for a context, returning a unique key. If the context already exists this is an error.
* **Replace** a context for an existing key or keys: the key will now map to the new context. If this change would result in no keys being mapped to the old context, the old context will be deleted. If the key does not exist or already maps to the context then this is an error.
* **Add** an additional unique key for a context: creates and returns a new unique key entry for the context. If the context does not exist then this is an error.
* **Delete** an entry for a context and one or more unique keys. If additional keys in Udex are mapped to the same context and were not asked to be deleted, the context will remain. If the context does not exist, or any of the requested keys do not exist or do not map to the context then this is an error.
* **Lookup** a context by its unique key. If the unique key does not exist or does not map to a context this is an error.
* **Reverse Lookup** unique key(s) by context. If more than one key exists for the context in the index then all are returned. If the context does not exist or does not map to any keys then this is an error.

All of these operations can be performed in bulk up to a configurable limit per index. Bulk operations are transactional — if any fail, the rest will be rolled back. Create and Lookup are the most commonly used operations and will be the most optimised.

### Admin Operations

_(Initially these may only be supported by static configuration.)_

* Create a new index with a bulk operation limit
* Update an index's bulk operation limit
* Delete an index
* List indices

## Usage

Udex is intended to be used by other systems. In general a "source" system will generate an entry by sending the desired context for the entity to Udex, and Udex will generate the entry with a key. The source then uses the key in integration for the entity with other parties. It may choose to use different keys and contexts for the same entity.

> **Example**: in an Open Banking environment, customer and account identifiers are required to be stable vs changes in the underlying systems but also not shared between different data holders/partners, so that the compromise of one data holder does not compromise the others. Udex provides the stable, opaque key that can be used across integration boundaries.

## What Udex Is Not

Udex is not intended to store entities themselves nor the relations between them. The context is only intended to capture the minimum required to perform further operations on the entity in other systems. Nor does Udex support search and query: clients are expected to know either the unique key(s) or the context of the entities they are interested in resolving.

Udex is not intended to be used directly by humans apart from specific admin operations.

## Security Model

### Authentication & Authorization

Udex initially only supports [OAuth 2.0 Client Credentials Flow](https://oauth.net/2/grant-types/client-credentials/). Supporting Authorization Code flow would require Udex to know about Authorization Servers and code exchanges, which conflicts with the simplicity principle.

Tokens are Json Web Tokens (JWTs) with the standard set of claims plus additional ones that map to the permissions model. Udex validates the token for every operation and will fail bulk transactions if any operation cannot be validated.

### Permissions

Udex does not support field-level permissions (e.g. on context keys or values). Where fine-grained permissions are required they must be provided by systems between the client and Udex.

#### Index Level

There is a permission per index per operation and each must be specifically enabled. There is also a permission for the maximum number of bulk operations per index. When performing bulk operations the lowest of the index's bulk limit and the client's permission limit applies.

#### Admin Level

No admin APIs are initially provided. Permissions for admin operations are assumed to be applied via other mechanisms (e.g. access to source control and infrastructure).

### Encryption

#### In Transit

The Udex server defaults to only exposing APIs via TLS 1.3 and clients are required to support this. Opting out of TLS requires explicit configuration by an administrator. Server–Datastore communication is also via TLS. Datastores that do not support TLS are not supported.

#### At Rest

Udex does not support encryption at rest directly, though supported datastores and/or their storage components are expected to. Udex supports encryption of context values by clients via envelope encryption: context key/value pairs can have the relevant metadata attached (i.e. the encrypted Data Encryption Key and the id of the Key Encryption Key). This metadata is not included in the context hash. Changing the encryption requires creation of a new context (as contexts are immutable).

> **Example**: Payment card numbers (PANs) are required by [PCI DSS](https://www.pcisecuritystandards.org/standards/pci-dss/) to be stored unreadably. An envelope-encrypted context value can hold the PAN, with a Udex key used externally to reference the card.

#### Secrets

Udex configuration only supports secrets by injection (e.g. environment variables) and will not support secrets directly in configuration files. The application component will only hold secrets in memory. Udex supports rolling out new secrets.

## Principles

* **Simplicity** — Udex should do one thing well rather than multiple things in a mediocre way.
* **Reliability** — Udex should be highly reliable, including in the event of node failures.
* **Performance** — Udex should handle many transactions concurrently. Ideally it will support 100–1000s of transactions per second with the right configuration and infrastructure.
* **Easy to develop and test** — Udex should be able to run locally for end-to-end testing. Project structure, usage and tooling should follow conventions and standards.
* **Easy to operate** — Udex should be simple to set up with sensible defaults and no more configuration than necessary. Udex should work well with standard tooling (HTTP, Kubernetes, \*Nix).
* **Minimise shell scripting** — prefer application code or established CLI tooling over shell scripts.
