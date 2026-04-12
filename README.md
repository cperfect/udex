Udex
=====

## Overview

Udex is a universal lookup directory for entities that is intended to be lighweight, fast and efficient for high transaction volumes, possibly across organisational and regulatory boundaries. Udex is intended to be used in complex distributed integration scenarios where it is desirably for internal representations of entities, potentially including the primary keys, not be exposed across the integration boundaries so are prevent tight coupling across those boundaries and limit the exposure to [Hyrum's Law](https://www.hyrumslaw.com/)!.

Udex holds one or more Indeces which consist of entries from globally unique keys to contexts: globally unique keys are just that and may be realised as UUIDs or another suitable standard and are allocated by Udex and are intended to be trully globally unique, whereas contexts are just one or more key value pairs, plus a fast hash of the key value pairs that are intended to be the minimum to uniquely and reliably map the entity in some other context (e.g. a source database or registry, or a differen Udex index): essential a Udex entry is Globally Unique Key <-> Context Hash -> Context Key/Value Pairs and an Index is a namespace of Contexts. 

Indeces will have a name that is a string of lower case latin letters with the option of underscore infixes. Index names must be unique within a give Udex system and are immutable. Indeces will also have a human friendly but short description (default empty) and a configurable max number of bulk operations (default 100) per client. These can be changed.

A Globally Unique Key is intended to be just that: unique not only within an index (at least on the key side of the entry) but across all indeces (and ideally across everything - i.e. truly global).

A context is uniquely determined by its hash, with the hash determined by Udex by applying its hash function agains the key value pairs. There could however be more than one unique key for any context in an Index. The same context can appear only once in an Index but could appear in multiple indeces. Note that, however, a context is not necessarily one to one with an entity: the same entity could be mapped to different contexts with different key value pairs. This is opaque to Udex and is entirely up to how clients use. 

A context, including its hash will be *immutable* and cannot be updated, though they can be deleted and replaceds.

### Operations
Udex has two types of operations: Index  and Admin

#### Index
Index operations, including bulk operations, are transactional.
* Create an entry for a context, returning a unique key. If the context already exists this is an error.
* Replace a context for an existing key or keys: the key will now map to the context (which will be created if it does not already exist). Any other keys not specified will still map to the old context. If this change would result in no keys being mapped to the old context the old context will be deleted. If the key does not exists or already maps to the context then this will be treated as an error.
* Add an additional unique key for a context: this creates and returns a new unique key entry for the context and returns the key. If the context does not exist then this is an error.
* Delete an entry for a context and one or more unique keys. If there are additional keys in Udex mapped to the same context which the client has not asked to delete then the context will remain in Udex mapped to these "surviing keys". If the context does not exist, or any of the requested keys do not exist or do not map to the context then this is an error. The actual operation of the delete on the underlying datastore is opaque to clients (e.g. soft vs hard) and it might vary by datastore and configuration.
* Lookup a context by its unique key. If the unique key does not exist or does not map to a context this is an error.
* Reverse Lookup a unique key(s) by its context. If more than one key exits for the context in the index then all are returned. If the context does not exist or does not map to any keys then this is an error. In general use of this is only intended to support the creation or deletion of entries.

All of these operations can be performed in bulk (i.e. many at once) up to some configurable limit per index and bulk operations can combine a mix of different individual operations. Where bulk operations are performed the client can expect them to be performed in the exact order given and that if any fail rest will be rolled back (i.e. bulk operations are transactional). It is intended that the Create and Lookup operations will be the ones most utilised and these will be the ones most optimised for. Note that as contexts are immutable no update is provided. 

#### Admin Operations
The admin operations are (initially these might only be supported by static configuration):
* Create a new index with a bulk operation limit
* Update an index with new bulk operational limit
* Delete an index
* List indeces
* Various other configuration operations.

### Usage
Udex is intended to be used by other systems.

In general a "source" system will generate an entry by sending the desired context for the entity to Udex, and Udex will generate a the entry with a key. THe source then uses the key in integration for the entity with other parties. It may choose to use different keys and contexts for the same entity.

> An example of usage of Udex might be an Open Banking environment where customer and account identifiers are required to be stable vs changes in the underlying customer and account systems but also not shared between different data holders/partners so that the compromise of one data holder does not compromise the other data holders.

### What Udex is not
Udex is not intended to store the entities themselves nor the relations between them and deliberately does not provide a flexible data model for this: the context is only intended to be able to capture the minimum required to perform further operations on the entity in other systems. Note that context could itself just be another unique key. Nor does Udex support mechanisms for search and query: it is assumed that Udex clients know either the unique key(s) or the context (or at least the context hash) of the entities they are interested in resolving and, once the entry has been resolved they know how to use the entry to perform further operations. 

Udex is not intended to be used directly by humans apart from specific admin operations (which will still be ideally automated in some way). Other systems might provide a human interface for Udex, and if so must provide for human level (i.e. fine-grained) authentication and authorization. 

## Prinicples
* Simplicity - Udex should be simple to use and understand and try and do one thing well rather than multiple things in a mediocre way.
* Reliability - Udex should highly reliable, including in the event of node failures. 
* Performance - Udex should be able to handle many transaction concurrently and respond quickly. Ideally it will support 100 or 1000s of transactions per second with the right configuration and infrastructure.
* Easy to develop and test both in local development and shared test environments. Udex should be able to run locally for end to end testing. Project structure, usage and tooling should follow conventions and standards.
* Easy to operate - Udex should be simple to setup and get running and should provide no more configuration options than necessary, and provide sensible defaults. Udex should make use of and work well with standard, both actual and defacto, such as HTTP, Kubernetes, *Nix. 
* Minimise use of shell scripting

## Architecture
The system has two server side components, which combined support one or more indeces.
* Server: a stateless application component that provides the APIs and business logic for Udex. It is intended that the application component can be horizontally scaled in and out. It is intended that the server is largely generated from API specifications.
* Configuration: the configuration determines which datastore to use (and any configuration required for that datastore) and which indeces exist and their configurations. Application components share the same configuration. Configuration can be static (e.g. file) or dynamic (e.g. via something like etcd).
* Datastore: holds the udex index state. Transactons, distribution and scaling are intended to be handled by the datastore implementation and should be opaque to the application and, as far as possible, the configuration.
* CLI: the CLI can be used to generate or update the configuration, start and stop the server and as a client for test and operational purposes, plus other useful tasks.
* SDKs to enable clients to be built for various technologies and frameworks. SDKs will largely be generated from API specifications. The SDKS will also support useful things like generating JWTs in the right format for the server.
* Testing: any test harnesses and fixtures that might be desirable.

## Security Model

### Authentication & Authorization
Udex will initially only support [OAuth 2.0 Client Credentials Flow](https://oauth.net/2/grant-types/client-credentials/). The reasoning for this is to keep Udex simple: supporting Authorization Code flow would require Udex to know about Authorization Servers and code exchanges.

Tokens will be in form of Json Web Tokens (JWTs) with the standard set of claims plus additional ones that map to the permissions model.

Udex will validate the token for every operation and reject operations that cannot be validated against the JWT and permissions. Where bulk operations are performed Udex requires that all operations are validated and will fail the bulk transaction if it any of the operations cannot be validated (respecting transactional boundaries).

### Permissions
Udex will not support fine-grained "for level" permissions (e.g. on context keys or values) and does not, in any case, support operations for which these would likely apply (e.g. queries across contexts). Where such might be required they must be provided by other systems between the client/user and Udex.

#### Index level 
There will be a permission per index per operation and each must be specifically enabled. There will also be a permission for the maximum number of bulk operations per index with an a positive integer value of 1 or greater. When performing bulk operations the lowest of the index's bulk limit and the client's permission limit.

#### Admin Level
No admin APIs will be initially provided so no permissions will initially exist for this. It is assumed that permissions will be applied via other mechanisms - e.g. access to source control and infrastructure.

### Encryption

#### In Transit
The Udex server will default to only exposing APIs via TLS 1.3 and clients will be required to support this. The Udex CLI will provide to easily configure TLS even for local development. Opting out of TLS will require explicit configuration by an administrator. Server - Datastore communication will also be via TLS with the implementation determined by the datastore. Datastores that don't support TLS will not be supported.

#### At Rest
Udex will not support encryption at rest directly though it is anticipated that the support datastores and/or their storage components will. Udex will support encryption of context values by clients via envelope encrytion and context key/value pairs can have the relevant metadata attached (i.e. the encrypted Data Encryption Key and the id of the Key Encryption Key). This metadata will not be included in the context hash, though the encrypted value will be. Therefore changing the encryption will require the creation of a new context (as contexts are immutable). Note that the context hash is intended to be fast and not strongly encrypted and the client should not apply envelope encryption to the sensitive context values if they do not believe the datastore's and/or transport security are strong enough for their needs. 

> One scenario for the use of envelope encryption would be for payment cards: often the payment card number (Primary Account Number "PAN") is used as a key in legacy systems but [Payment Card Industry Data Security Standard (PCI DSS)](https://www.pcisecuritystandards.org/standards/pci-dss/) requires that the number be unreadable where it is stored and it is highly desirable that the number is not required to be used as a key unless it is necessary for a transaction. In Udex an envelope encrypted context value could be used for the number and a Udex key then used for external system to reference the card. 

#### Secrets
Udex configuration will only support secrets by injection (e.g. Datastore credentials, JWT validation keys, TLS private keys) using standard patterns (e.g. env vars) and will not support secrets directly in the condiguration. The application component will only hold secrets in memory. Udex will support rolling out of new secrets.

## Developer Guides

- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — general development principles, guidelines, and testing standards for all contributors
- [projects/rust/DEVELOPMENT.md](projects/rust/DEVELOPMENT.md) — Rust-specific coding standards, error conventions, and local check commands

## Development & Implementation
* Development will be API spec driven using Protobuf version 3: server, client, datamodels and SDKs will be generated from this and then extended and elaborated as necessary.
* _(Deferred)_ Operations will be CLI driven: the server will be stopped and started via CLI and the configuration created and managed via CLI.
* Udex CLI and Server and the initial client will be written in Rust. _(Deferred)_ The CLI will used the [clap create using the build pattern](https://docs.rs/clap/latest/clap/_tutorial/index.html). The Server will be built using the [tokio runtime](https://docs.rs/tokio/latest/tokio/). APIs will be exposed via gRPC by default (via tonic). _(Deferred)_ An optional REST interface (via Hyper) may be added in future.
* The Datastores supported initially will be Postgres. _(Deferred)_ SQLite support may be added in future. The choice of datastore will be at compile time and the Udex CLI and Server binaries and install packaing will need to be available in different versions for the different datastores supported.
* The Configurations supported will initially be a yaml file
* Udex will be semantically versioned.
* _(Deferred)_ Udex will support Open Telemetry tracing and metric standards.
* Udex deployments will be containerized with the primary deployment pattern being Kubernetes: Udex can be run either as its own service or as a sidecar. The sidecar pattern might be the only valid use case for disabling transport level encryption outside of local development.

### Generative AI
This project is developed using [Claude Code](https://claude.ai/code) (Anthropic) with [Intent v2.8.0](https://github.com/matthewsinclair/intent) for steel thread and work package management.

**Plugins**
- [`rust-analyzer-lsp`](https://github.com/anthropics/claude-code-plugins) — provides live Rust diagnostics and code intelligence to Claude Code via the rust-analyzer language server.

**Skills**
- [`in-essentials`](https://github.com/matthewsinclair/intent) — core Intent workflow skill providing steel thread and work package management conventions.

## Workspace
Udex will be developed in a git monorepo. Some kind of build tooling will be required that support polyglot projects - e.g. Nx. The entire workspaces will be used via a vscode devcontainer.


## Questions/Issues
Should many keys to one context per index be allowed or should there only be a one to one entry? The latter would map better to a KV style datastore and semantics but require the creation of more contexts (and entries) and for "sources" to distinguish between them (e.g. based on integration party?)
