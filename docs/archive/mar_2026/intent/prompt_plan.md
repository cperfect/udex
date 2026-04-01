# Udex Implementation Plan Checklist

## Phase 1: Core Data Model and Foundation

### Step 1.1: Project Setup and Base Structure
- [ ] Create a new Rust project with Cargo workspace structure
- [ ] Set up essential dependencies (tokio, tonic, serde)
- [ ] Establish project layout with appropriate src folders
- [ ] Configure Cargo.toml with metadata

```
I'm building a project called Udex, a universal lookup directory for entities written in Rust. Let's start by setting up the project structure and basic dependencies.

Create a new Rust project with Cargo with the following features:
1. A workspace structure that will eventually accommodate both a server component and CLI
2. Essential dependencies including tokio, tonic (for gRPC), and serde for serialization
3. A basic project layout with src folders for the core components mentioned in the spec
4. A minimal but proper Cargo.toml file with version, authors, and description

The project should follow idiomatic Rust patterns and directory structure.
```

### Step 1.2: Define Core Data Types
- [ ] Implement Context struct with support for different value types
- [ ] Add support for envelope encryption data
- [ ] Include metadata fields
- [ ] Create ContextHash implementation
- [ ] Define GloballyUniqueKey type using UUIDv4
- [ ] Implement Index struct with configuration options

```
Now, let's define the core data types for Udex based on the specification. We need to implement:

1. The Context struct that represents a set of key-value pairs with the following:
   - Support for string, integer, float, and boolean values
   - Optional envelope encryption data (Data Encryption Key and Key Encryption Key Id)
   - Metadata fields (created_at, created_by)

2. The ContextHash implementation that:
   - Takes a Context and computes its hash using a configurable algorithm (default SHA-1)
   - Properly sorts and serializes key-value pairs for consistent hashing
   - Excludes metadata from the hash input

3. The GloballyUniqueKey type that:
   - Uses UUIDv4 format
   - Includes proper validation

4. The Index struct that defines:
   - Name, description, and configuration limits
   - Hash algorithm selection
   - Metadata fields

These types should be defined in src/model.rs with appropriate validation, constructor methods, and error handling.
```

### Step 1.3: Context Hash Implementation
- [ ] Create HashComputer trait
- [ ] Implement SHA-1 hashing
- [ ] Implement Blake3 hashing
- [ ] Build HashAlgorithmRegistry
- [ ] Write unit tests for consistent behavior

```
Let's implement the Context hash computation logic for Udex. This is a critical part as it defines the identity of contexts within an index.

Create a module in src/hash.rs that:

1. Implements a HashComputer trait with methods:
   - compute_hash(&self, context: &Context) -> Result<ContextHash, HashError>
   - algorithm_name(&self) -> &str

2. Concrete implementations for at least:
   - SHA-1 hash (the default)
   - Blake3 hash (as an alternative)

3. A HashAlgorithmRegistry that:
   - Allows registration of hash algorithms by name
   - Returns the appropriate implementation based on configuration

4. Unit tests that verify:
   - Consistent hashing regardless of key-value pair order
   - Different contexts produce different hashes
   - Same contexts produce the same hash
   - Proper handling of various data types

Make sure the implementation correctly excludes metadata from the hash computation, sorts key-value pairs, and follows the serialization approach specified.
```

### Step 1.4: Config Implementation
- [ ] Define Config struct with server, database, and other settings
- [ ] Implement YAML loading functionality
- [ ] Add validation logic
- [ ] Create sample configuration files
- [ ] Write unit tests for parsing and validation

```
Now, let's implement the configuration system for Udex using YAML.

Create a configuration module in src/config.rs that:

1. Defines a Config struct with:
   - Server configuration (host, port, max_connections, timeouts)
   - Database configuration (type, connection details)
   - Index configurations (including all fields from the spec)
   - Authentication settings
   - Telemetry settings

2. Implements functions to:
   - Load configuration from a YAML file
   - Validate the configuration (checking for required fields, valid values, etc.)
   - Provide sensible defaults where appropriate

3. Unit tests that verify:
   - Proper parsing of valid configuration files
   - Proper error handling for invalid configurations
   - Default values are applied correctly

Include sample configuration files in a config/ directory that demonstrate different setups (e.g., SQLite vs. Postgres, different authentication methods).
```

## Phase 2: Server Implementation

### Step 2.1: Proto Definitions
- [ ] Create proto/ directory structure
- [ ] Define Index service proto file
- [ ] Define Entry service proto file
- [ ] Include comprehensive documentation
- [ ] Define error responses

```
Let's define the protocol buffer files for Udex's gRPC API based on the technical specification.

Create proto/ directory with the following files:

1. proto/udex/index/v1/index.proto:
   - Define the Index service with Healthz and Describe operations
   - Include message definitions for requests and responses

2. proto/udex/entry/v1/entry.proto:
   - Define the entry service with all operations specified (Createentry, ReplaceContextForKeys, etc.)
   - Include message definitions for requests and responses
   - Support for bulk operations

The proto files should:
- Use proper versioning (v1)
- Include comprehensive field documentation
- Follow Protocol Buffers best practices
- Define proper error responses
- Include any necessary common types in shared files

These files will be the foundation for both the server implementation and client SDK generation.
```

### Step 2.2: gRPC Service Implementation Setup
- [ ] Configure tonic-build in build.rs
- [ ] Create basic service structure
- [ ] Implement service structs with dependencies
- [ ] Add method stubs with validation
- [ ] Set up server.rs with gRPC server configuration

```
Now let's set up the gRPC service implementations for Udex based on our proto definitions.

1. Configure tonic-build in build.rs to generate Rust code from our proto files

2. Create the basic service structure in src/services/:
   - index_service.rs for the Index service
   - entry_service.rs for the entry service

3. Implement service structs with dependencies:
   - Make services depend on an abstract DataStore trait (we'll implement later)
   - Add configuration and metrics dependencies
   - Prepare for proper error handling

4. For each service, implement the trait generated by tonic with method stubs that:
   - Validate request inputs
   - Return appropriate gRPC status codes for errors
   - Log operations
   - For now, return "unimplemented" status to be filled in later

5. Create a server.rs module that:
   - Sets up and runs the gRPC server
   - Registers both services
   - Configures TLS (if enabled)
   - Handles graceful shutdown

The code should follow Rust's async/await patterns with tokio and properly handle errors throughout.
```

### Step 2.3: Service Implementation - Index Operations
- [ ] Implement Healthz operation
- [ ] Implement Describe operation
- [ ] Add error handling and logging
- [ ] Write unit tests

```
Let's implement the Index service operations defined in our proto files.

In src/services/index_service.rs:

1. Complete the implementation of the `Healthz` operation:
   - Check database connectivity
   - Return appropriate status based on system health
   - Include version and uptime information

2. Complete the implementation of the `Describe` operation:
   - Return detailed information about an index based on its name
   - Include all index configuration and metadata
   - Properly handle cases where the index doesn't exist

3. Add proper error handling and logging:
   - Use structured logging
   - Include correlation IDs in logs
   - Return appropriate gRPC status codes

4. Add unit tests that verify:
   - Proper responses for valid inputs
   - Proper error handling for invalid inputs
   - Correct behavior when indexes don't exist

The implementation should use the DataStore trait methods (which we'll implement later) and handle transactions appropriately.
```

### Step 2.4: Service Implementation - Basic entry Operations
- [ ] Implement Createentry
- [ ] Implement LookupContextByKey
- [ ] Implement LookupKeysByContext
- [ ] Add error handling and validation
- [ ] Write unit tests

```
Let's implement the core entry service operations for Udex.

In src/services/entry_service.rs:

1. Complete the implementation of `Createentry`:
   - Validate the context according to index configuration
   - Compute the context hash using the configured algorithm
   - Create a new globally unique key
   - Store the entry in the database
   - Return the created key

2. Complete the implementation of `LookupContextByKey`:
   - Validate the provided key
   - Retrieve the context from the database
   - Return the context with all key-value pairs and metadata

3. Complete the implementation of `LookupKeysByContext`:
   - Compute the hash for the provided context
   - Retrieve all keys associated with that context hash
   - Return the list of keys with their metadata

For all operations:
- Add proper error handling and validation
- Include correlation IDs in logs and responses
- Return appropriate gRPC status codes
- Ensure transactional integrity

Add unit tests that verify correct behavior for each operation, including edge cases and error conditions.
```

### Step 2.5: Service Implementation - Advanced entry Operations
- [ ] Implement ReplaceContextForKeys
- [ ] Implement AddKeyToContext
- [ ] Implement Deleteentry
- [ ] Add proper validation and error handling
- [ ] Write unit tests

```
Let's implement the more complex entry service operations for Udex.

In src/services/entry_service.rs:

1. Complete the implementation of `ReplaceContextForKeys`:
   - Validate the new context and keys
   - Compute the context hash
   - Update the entrys in a single transaction
   - Handle errors if any keys don't exist

2. Complete the implementation of `AddKeyToContext`:
   - Validate the context
   - Compute the context hash
   - Create a new globally unique key
   - Add the entry to the database
   - Return the created key

3. Complete the implementation of `Deleteentry`:
   - Validate the provided key
   - Remove the entry from the database
   - Return success or appropriate error

4. For all operations:
   - Add proper validation and error handling
   - Ensure transactional integrity
   - Include correlation IDs and proper logging

Add unit tests that verify correct behavior for each operation, including edge cases and error conditions.
```

### Step 2.6: Bulk Operations Implementation
- [ ] Implement BulkOperation functionality
- [ ] Add support for all operation types
- [ ] Ensure proper error handling and transactional integrity
- [ ] Write unit tests

```
Let's implement the BulkOperation functionality for Udex, which allows multiple operations to be performed in a single transaction.

In src/services/entry_service.rs:

1. Complete the implementation of `BulkOperation` that:
   - Validates the request against the index's max_bulk_operations limit
   - Processes each operation in order within a single transaction
   - Rolls back on any failure
   - Returns appropriate results for each operation

2. Implement support for all operation types in bulk:
   - Createentry
   - ReplaceContextForKeys
   - AddKeyToContext
   - Deleteentry

3. Ensure proper error handling:
   - Validate each operation before execution
   - Return detailed error information for any failed operation
   - Maintain transactional integrity

4. Add unit tests that verify:
   - Successful execution of mixed operation types
   - Proper rollback on failure
   - Respect for max_bulk_operations limit
   - Ordering of operations is preserved

The implementation should reuse the logic from individual operation handlers while ensuring everything happens in a single transaction.
```

## Phase 3: Datastore Integration

### Step 3.1: Datastore Trait Definition
- [ ] Define DataStore trait with all required methods
- [ ] Create appropriate error types
- [ ] Define transaction support
- [ ] Add comprehensive documentation

```
Let's define the abstract DataStore trait for Udex that will allow plugging in different storage backends.

Create a module in src/datastore/mod.rs that:

1. Defines a DataStore trait with methods for all required operations:
   - create_index, describe_index, list_indices
   - create_entry, lookup_context_by_key, lookup_keys_by_context
   - replace_context_for_keys, delete_entry
   - transaction support (begin_transaction, commit, rollback)

2. Define appropriate error types and result types:
   - DataStoreError enum with variants for different error types
   - TransactionError for transaction-specific errors

3. Define helper types:
   - Transaction trait or type
   - Query options and filters

4. Add documentation that explains:
   - Expected behavior for each method
   - Transactional guarantees
   - Error handling expectations

The trait should be designed to be implementation-agnostic while supporting all the operations needed by the services we've already implemented.
```

### Step 3.2: SQLite Implementation
- [ ] Implement DataStore trait for SQLite
- [ ] Add connection pooling
- [ ] Create schema initialization
- [ ] Implement transaction handling
- [ ] Write unit tests

```
Let's implement the DataStore trait for SQLite, which will be the default storage backend for Udex.

Create a module in src/datastore/sqlite.rs that:

1. Implements the DataStore trait using rusqlite:
   - Connection pooling for efficiency
   - Proper handling of SQLite-specific features
   - Implementation of all required methods

2. Adds schema initialization and migration support:
   - Create tables for indices, contexts, and entrys
   - Add indices for efficient queries
   - Support for updating schema versions

3. Implements proper transaction handling:
   - Begin/commit/rollback support
   - Proper error propagation
   - Connection management

4. Includes comprehensive error handling:
   - entry SQLite errors to DataStoreError types
   - Providing context in error messages
   - Handling edge cases (constraints, duplicates)

5. Add unit tests that verify:
   - Correct operation of all methods
   - Proper transaction behavior
   - Error handling
   - Performance for common operations

The implementation should be production-ready, with proper connection pooling, prepared statements, and error handling.
```

### Step 3.3: PostgreSQL Implementation
- [ ] Implement DataStore trait for PostgreSQL
- [ ] Add async connection pooling
- [ ] Create schema initialization
- [ ] Implement transaction handling
- [ ] Write unit tests

```
Now, let's implement the DataStore trait for PostgreSQL as an alternative backend for Udex.

Create a module in src/datastore/postgres.rs that:

1. Implements the DataStore trait using tokio-postgres:
   - Asynchronous connection pooling
   - Proper handling of Postgres-specific features
   - Implementation of all required methods

2. Adds schema initialization and migration support:
   - Create tables for indices, contexts, and entrys
   - Add indices for efficient queries
   - Support for updating schema versions

3. Implements proper transaction handling:
   - Begin/commit/rollback support
   - Proper error propagation
   - Connection management

4. Includes comprehensive error handling:
   - entry Postgres errors to DataStoreError types
   - Providing context in error messages
   - Handling edge cases (constraints, duplicates)

5. Add unit tests that verify:
   - Correct operation of all methods
   - Proper transaction behavior
   - Error handling
   - Performance for common operations

Ensure the implementation takes advantage of Postgres-specific features where appropriate while maintaining compatibility with the DataStore trait.
```

### Step 3.4: Database Factory and Configuration
- [ ] Implement create_datastore factory function
- [ ] Add connection URL parsing
- [ ] Support configuration loading
- [ ] Add comprehensive error handling
- [ ] Write integration tests

```
Let's implement a factory pattern for creating the appropriate DataStore implementation based on configuration.

Create a module in src/datastore/factory.rs that:

1. Implements a create_datastore function:
   - Takes a DatabaseConfig as input
   - Returns the appropriate DataStore implementation based on the type field
   - Handles connection setup and pooling
   - Returns appropriate errors for invalid configurations

2. Add support for connection URL parsing:
   - SQLite file paths
   - PostgreSQL connection strings
   - Validation of connection parameters

3. Add configuration loading from the main Config struct:
   - Default to SQLite if not specified
   - Support for connection pools and timeouts
   - Environment variable substitution

4. Include comprehensive error handling:
   - Connection failures
   - Invalid configurations
   - Initialization errors

Add integration tests that verify proper datastore creation and operation with different configurations.
```

### Step 3.5: Migration System Implementation
- [ ] Create Migration trait
- [ ] Implement initial schema migrations
- [ ] Build MigrationRunner
- [ ] Add CLI commands for migrations
- [ ] Write integration tests

```
Let's implement a database migration system for Udex to support schema evolution over time.

Create a module in src/datastore/migrations.rs that:

1. Implements a Migration trait with:
   - version() method
   - up() method for applying the migration
   - down() method for rolling back (if supported)
   - description() for documentation

2. Create concrete migrations for the initial schema:
   - Tables for indices, contexts, and entrys
   - Indexes for performance
   - Constraints for data integrity

3. Implement a MigrationRunner that:
   - Detects the current database version
   - Applies needed migrations in order
   - Handles errors and rollbacks
   - Updates the schema version tracking

4. Add CLI commands in the factory module:
   - migrate: Apply pending migrations
   - rollback: Revert the latest migration
   - status: Show migration status

5. Include comprehensive error handling:
   - Migration failures
   - Version conflicts
   - Connection issues

Add integration tests that verify proper migration application and rollback functionality.
```

## Phase 4: CLI Development

### Step 4.1: CLI Framework Setup
- [ ] Define top-level CLI structure
- [ ] Implement command-line argument parsing
- [ ] Set up command dispatch logic
- [ ] Add proper exit codes
- [ ] Write unit tests

```
Let's set up the CLI framework for Udex using clap with the builder pattern.

Create a new crate in the workspace at cli/src/main.rs that:

1. Defines the top-level CLI structure with commands:
   - config (with subcommands: validate, test)
   - index (with subcommands for index operations)
   - entry (with subcommands for entry operations)
   - utility commands (hash-context, inspect-token, benchmark)

2. Implements the command-line argument parsing using clap:
   - Define all required arguments and options
   - Add help text and documentation
   - Support for different output formats (table, JSON, YAML)

3. Sets up the basic structure:
   - Command dispatch logic
   - Error handling and reporting
   - Configuration loading

4. Implements proper exit codes:
   - 0 for success
   - Non-zero for various error conditions

Add unit tests that verify proper command-line parsing and dispatch for different command combinations.
```

### Step 4.2: Config Command Implementation
- [ ] Implement validate subcommand
- [ ] Implement test subcommand
- [ ] Add support for different output formats
- [ ] Include comprehensive error handling
- [ ] Write integration tests

```
Let's implement the config-related commands for the Udex CLI.

In cli/src/commands/config.rs:

1. Implement the `validate` subcommand:
   - Load the specified config file
   - Validate all sections and fields
   - Report specific validation errors
   - Return appropriate exit code

2. Implement the `test` subcommand:
   - Load and validate the config
   - Test database connectivity
   - Verify index configurations
   - Check authentication settings

3. Add support for different output formats:
   - Human-readable tables/text (default)
   - JSON for machine processing
   - YAML for configuration examples

4. Include comprehensive error handling:
   - File not found
   - Parse errors
   - Validation failures
   - Connection issues

Add integration tests that verify the commands work with sample configuration files.
```

### Step 4.3: Index Command Implementation
- [ ] Implement healthz subcommand
- [ ] Implement describe subcommand
- [ ] Add support for authentication
- [ ] Include comprehensive error handling
- [ ] Write integration tests

```
Let's implement the index-related commands for the Udex CLI that interact with the server.

In cli/src/commands/index.rs:

1. Implement the `healthz` subcommand:
   - Connect to the server
   - Call the Healthz gRPC method
   - Display results in the selected format
   - Handle connectivity errors

2. Implement the `describe` subcommand:
   - Connect to the server
   - Call the Describe gRPC method
   - Format and display the index information
   - Handle cases where the index doesn't exist

3. Add support for authentication:
   - Read authentication settings from config or arguments
   - Obtain and include authentication tokens
   - Handle authentication failures

4. Include comprehensive error handling:
   - Connection issues
   - Authentication failures
   - Service errors
   - Display detailed error information

Add integration tests that verify the commands work correctly against a running server.
```

### Step 4.4: entry Command Implementation - Basic
- [ ] Implement create-entry subcommand
- [ ] Implement lookup-context subcommand
- [ ] Implement lookup-keys subcommand
- [ ] Add support for different input/output formats
- [ ] Write integration tests

```
Let's implement the basic entry-related commands for the Udex CLI.

In cli/src/commands/entry.rs:

1. Implement the `create-entry` subcommand:
   - Parse context from command line or input file
   - Connect to the server
   - Call the Createentry gRPC method
   - Display the resulting key

2. Implement the `lookup-context` subcommand:
   - Accept a key argument
   - Connect to the server
   - Call the LookupContextByKey gRPC method
   - Format and display the context

3. Implement the `lookup-keys` subcommand:
   - Parse context from command line or input file
   - Connect to the server
   - Call the LookupKeysByContext gRPC method
   - Format and display the keys

4. Add support for different input/output formats:
   - JSON input/output
   - YAML input/output
   - Table output for human readability

Include comprehensive error handling and add integration tests that verify the commands work correctly against a running server.
```

### Step 4.5: entry Command Implementation - Advanced
- [ ] Implement replace-context subcommand
- [ ] Implement add-key subcommand
- [ ] Implement delete-entry subcommand
- [ ] Implement bulk subcommand
- [ ] Write integration tests

```
Let's implement the more advanced entry-related commands for the Udex CLI.

In cli/src/commands/entry.rs:

1. Implement the `replace-context` subcommand:
   - Parse context and keys from command line or input file
   - Connect to the server
   - Call the ReplaceContextForKeys gRPC method
   - Display the results

2. Implement the `add-key` subcommand:
   - Parse context from command line or input file
   - Connect to the server
   - Call the AddKeyToContext gRPC method
   - Display the resulting key

3. Implement the `delete-entry` subcommand:
   - Accept a key argument
   - Connect to the server
   - Call the Deleteentry gRPC method
   - Confirm deletion

4. Implement the `bulk` subcommand:
   - Parse operations from an input file
   - Connect to the server
   - Call the BulkOperation gRPC method
   - Display results for each operation

5. Add support for different input/output formats:
   - JSON input/output
   - YAML input/output
   - Table output for human readability

Include comprehensive error handling and add integration tests.
```

### Step 4.6: Utility Command Implementation
- [ ] Implement hash-context command
- [ ] Implement inspect-token command
- [ ] Implement benchmark command
- [ ] Add comprehensive error handling
- [ ] Write unit tests

```
Let's implement the utility commands for the Udex CLI that don't require server connectivity.

In cli/src/commands/utility.rs:

1. Implement the `hash-context` command:
   - Parse context from command line or input file
   - Compute the hash using the specified algorithm
   - Display the resulting hash
   - Support different output formats

2. Implement the `inspect-token` command:
   - Parse a JWT token
   - Display the header and payload
   - Verify the signature if possible
   - Show expiration and other metadata

3. Implement the `benchmark` command:
   - Connect to the server
   - Perform a configurable number of operations
   - Measure and report throughput and latency
   - Generate summary statistics

4. Add comprehensive error handling:
   - Input parsing issues
   - Algorithm errors
   - Invalid tokens
   - Connection problems during benchmarking

Add unit tests to verify the correct operation of each utility command.
```

## Phase 5: Authentication and Security

### Step 5.1: JWT Authentication Implementation
- [ ] Implement JWT validation for multiple algorithms
- [ ] Add token validation logic
- [ ] Implement key management
- [ ] Create gRPC interceptor
- [ ] Write unit tests

```
Let's implement JWT authentication for Udex based on the OAuth 2.0 Client Credentials flow.

Create a module in src/auth/jwt.rs that:

1. Implements JWT validation with support for:
   - HS256 (HMAC + SHA256)
   - RS256 (RSA + SHA256)
   - ES256 (ECDSA + SHA256)

2. Add support for token validation:
   - Signature verification
   - Expiration and not-before time checks
   - Issuer validation
   - Audience validation

3. Implement key management:
   - Loading HMAC secrets from configuration
   - Loading RSA/ECDSA public keys from files or JWKs
   - Key rotation support

4. Create a gRPC interceptor that:
   - Extracts the token from the Authorization header
   - Validates the token
   - Extracts claims for authorization
   - Adds claims to the request context

Add comprehensive unit tests that verify token validation with different algorithms and error conditions.
```

### Step 5.2: Authorization Implementation
- [ ] Define permission types
- [ ] Implement Permission trait
- [ ] Create authorization interceptor
- [ ] Add index-level permission configuration
- [ ] Write unit tests

```
Let's implement authorization for Udex based on index-level operation permissions.

Create a module in src/auth/authorization.rs that:

1. Defines permission types:
   - IndexPermission enum (Read, Write, Admin)
   - Operation-specific permissions

2. Implements a Permission trait with:
   - check_permission method
   - is_admin helper

3. Creates an authorization interceptor that:
   - Uses JWT claims from the authentication step
   - Checks permissions against the requested operation
   - Returns appropriate gRPC status codes for denied access

4. Adds index-level permission configuration:
   - Map client IDs to permissions
   - Support for wildcard permissions
   - Default permissions

5. Implements bulk operation limits per client:
   - Configurable maximum operations
   - Override for admin clients

Add unit tests that verify proper permission checking and authorization for different operations and user types.
```

### Step 5.3: TLS Implementation
- [ ] Implement TLS configuration for gRPC server
- [ ] Add TLS for datastore connections
- [ ] Implement TLS for CLI client
- [ ] Add configuration options
- [ ] Write unit tests

```
Let's implement TLS support for Udex to secure all communications.

Create a module in src/security/tls.rs that:

1. Implements TLS configuration for the gRPC server:
   - Loading certificates and private keys
   - Configuring TLS versions and cipher suites
   - Client certificate validation (optional)

2. Adds TLS configuration for datastore connections:
   - Secure connections to PostgreSQL
   - Certificate validation
   - SNI support

3. Implements TLS for the CLI client:
   - Certificate validation
   - Support for custom CA certificates
   - Certificate pinning (optional)

4. Adds configuration options:
   - Enable/disable TLS
   - Certificate and key paths
   - Required/optional client verification

Add unit tests that verify proper TLS configuration and connection handling.
```

### Step 5.4: Envelope Encryption Support
- [ ] Define encryption metadata structures
- [ ] Implement handling for encrypted values
- [ ] Add KEK ID tracking
- [ ] Add validation and error handling
- [ ] Write unit tests

```
Let's implement envelope encryption support for Udex context values.

Create a module in src/security/encryption.rs that:

1. Defines encryption metadata structures:
   - Data Encryption Key (DEK) storage
   - Key Encryption Key (KEK) ID reference
   - Encryption algorithm and parameters

2. Implements handling for encrypted values:
   - Detection of encrypted values in contexts
   - Serialization format for encrypted data
   - Base64 encoding/decoding

3. Add support for KEK ID tracking:
   - Metadata storage
   - Reference tracking
   - Tagging of encrypted key-value pairs

4. Add validation and error handling:
   - Validate encryption metadata
   - Proper error reporting for encryption issues
   - Security-focused logging (without exposing keys)

Note that actual encryption/decryption will be performed by clients, but the server needs to properly store and handle the envelope encryption metadata.

Add unit tests that verify proper handling of encrypted values and metadata.
```

## Phase 6: Observability and Telemetry

### Step 6.1: Structured Logging Implementation
- [ ] Set up structured JSON logging
- [ ] Implement log sinks
- [ ] Add correlation ID tracking
- [ ] Create logging helpers
- [ ] Add log level configuration
- [ ] Write unit tests

```
Let's implement structured logging for Udex using a modern logging framework.

Create a module in src/telemetry/logging.rs that:

1. Sets up structured JSON logging with:
   - Timestamp in ISO 8601 format
   - Log level
   - Module path
   - Correlation ID
   - Context-specific fields

2. Implements log sinks:
   - stdout (default)
   - File output (configurable)
   - Support for log rotation

3. Adds correlation ID tracking:
   - Generation for new requests
   - Propagation through the call chain
   - Inclusion in all log messages

4. Creates logging macros or helpers:
   - Context-aware logging
   - Standard fields for different components
   - Performance-sensitive logging (sampled, etc.)

5. Configures log levels:
   - From configuration file
   - Runtime adjustment (if supported)
   - Default levels for different modules

Add unit tests that verify proper log formatting and correlation ID handling.
```

### Step 6.2: Metrics Implementation
- [ ] Set up OpenTelemetry metrics
- [ ] Implement metrics for key components
- [ ] Add dimension tagging
- [ ] Create metrics registry
- [ ] Implement exporters
- [ ] Write unit tests

```
Let's implement metrics collection for Udex using OpenTelemetry.

Create a module in src/telemetry/metrics.rs that:

1. Sets up OpenTelemetry metrics with:
   - Counter for request counts
   - Histogram for request durations
   - Gauge for active connections
   - Custom metrics for business operations

2. Implements metrics for key components:
   - Server request handling
   - Datastore operations
   - Authentication and authorization
   - Bulk operations

3. Adds dimension tagging:
   - Index name
   - Operation type
   - Client ID
   - Success/failure

4. Creates a metrics registry:
   - Central registration of metrics
   - Access to metrics from components
   - Default dimensions

5. Implements exporters:
   - OTLP exporter (for Prometheus, etc.)
   - Stdout for development
   - Periodic export scheduling

Add unit tests that verify proper metric recording and export formatting.
```

### Step 6.3: Distributed Tracing Implementation
- [ ] Set up OpenTelemetry tracing
- [ ] Implement tracing for key operations
- [ ] Add context propagation
- [ ] Create trace exporters
- [ ] Implement OTLP endpoint
- [ ] Write unit tests

```
Let's implement distributed tracing for Udex using OpenTelemetry.

Create a module in src/telemetry/tracing.rs that:

1. Sets up OpenTelemetry tracing with:
   - Span creation for requests
   - Nested spans for operations
   - Attribute tagging
   - Error marking

2. Implements tracing for key operations:
   - Request handling in gRPC services
   - Datastore operations
   - Validation steps
   - External service calls

3. Adds context propagation:
   - TraceContext extraction from gRPC metadata
   - Correlation with logging IDs
   - Propagation to downstream services

4. Creates trace exporters:
   - OTLP exporter (for Jaeger, Zipkin, etc.)
   - Stdout for development
   - Sampling configuration

5. Implements the OpenTelemetry Protocol (OTLP) endpoint:
   - Receive traces from clients
   - Forward to configured backends
   - Sample based on configuration

Add unit tests that verify proper span creation and context propagation.
```

### Step 6.4: Health Reporting Implementation
- [ ] Set up OpenTelemetry tracing
- [ ] Implement tracing for key operations
- [ ] Add context propagation
- [ ] Create trace exporters
- [ ] Implement OTLP endpoint
- [ ] Write unit tests

```
Let's implement comprehensive health reporting for Udex.

Create a module in src/telemetry/health.rs that:

1. Implements health check components:
   - Datastore connectivity check
   - Memory usage monitoring
   - System load monitoring
   - Dependent service checks

2. Creates a health aggregator:
   - Overall system health status
   - Component-level health statuses
   - Health history

3. Adds reporting endpoints:
   - Implementation for the gRPC Healthz method
   - Optional HTTP health endpoint
   - Detailed vs. simple health status

4. Implements alerting triggers:
   - Threshold-based alerts
   - State change detection
   - Alert suppression logic

5. Creates a self-test framework:
   - Startup self-test
   - Periodic testing
   - Deep vs. shallow tests

Add unit tests that verify proper health status reporting under various conditions.
```

---

## Phase 7: SDK Generation

### Step 7.1: Client SDK Generator Setup
- [ ] Set up SDK generator CLI tool
- [ ] Create language template structures
- [ ] Implement core proto parser and generator
- [ ] Add output formatting capabilities
- [ ] Add initial language support

```
Let's set up a framework for generating client SDKs for Udex based on our gRPC/Protobuf definitions.

Create a new crate in the workspace at sdk-generator/src/main.rs that:

1. Implements a CLI tool that:
   - Takes proto files as input
   - Generates client code in specified languages
   - Handles configuration and options

2. Sets up language template structures:
   - Template files for each supported language
   - Macro system for code generation
   - Configuration for language-specific options

3. Creates a core generator that:
   - Parses proto files using protoc
   - Extracts service and message definitions
   - Processes types and converts them to target language
   - Handles imports and dependencies

4. Adds output formatting:
   - Code style enforcement
   - Documentation generation
   - Package/module organization

Start with support for one language (e.g., Go or TypeScript) and add structure for expanding to others.
```

### Step 7.2: Go SDK Generation

```
Let's implement Go SDK generation for Udex.

In sdk-generator/src/languages/go.rs:

1. Implement Go-specific code generation:
   - Package structure and imports
   - Type conversions from proto to Go
   - Error handling patterns
   - Client interface and implementation

2. Add Go-specific features:
   - Context-aware methods
   - Error wrapping
   - Optional parameters via functional options
   - Retry and timeout handling

3. Implement authentication integration:
   - OAuth 2.0 client credentials
   - Token caching and refresh
   - Custom credentials providers

4. Generate documentation:
   - GoDoc compatible comments
   - Example usage
   - API reference

5. Create build and packaging:
   - Go module support
   - Version information
   - Example programs

Add tests that verify the generated code compiles and works correctly against the Udex server.
```

### Step 7.3: TypeScript SDK Generation

```
Let's implement TypeScript SDK generation for Udex.

In sdk-generator/src/languages/typescript.rs:

1. Implement TypeScript-specific code generation:
   - Module structure and imports
   - Type definitions and interfaces
   - Promise-based API
   - Client class implementation

2. Add TypeScript-specific features:
   - Type safety
   - JSDoc comments
   - Async/await support
   - Error classes and handling

3. Implement authentication integration:
   - OAuth 2.0 client credentials
   - Token management
   - Custom credentials providers

4. Generate documentation:
   - TSDoc compatible comments
   - Example usage
   - API reference

5. Create build and packaging:
   - npm package setup
   - TypeScript configuration
   - Webpack/Rollup bundling support
   - Example programs

Add tests that verify the generated code compiles and works correctly against the Udex server.
```

### Step 7.4: Python SDK Generation

```
Let's implement Python SDK generation for Udex.

In sdk-generator/src/languages/python.rs:

1. Implement Python-specific code generation:
   - Package structure and imports
   - Type conversions with type hints
   - Generator-based and async API options
   - Client class implementation

2