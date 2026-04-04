# OWASP Secure Coding Practices - Quick Reference Checklist

Source: https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/stable-en/02-checklist/05-checklist

## Input Validation

- Execute all input validation on trusted systems (server-side, not client-side)
- Identify and categorize all data sources as trusted or untrusted
- Validate all data originating from untrusted sources
- Use centralized input validation across the application
- Specify character sets (e.g., UTF-8) for all input sources
- Encode input to common character sets before validation
- Reject all validation failures
- Validate after UTF-8 decoding when extended character sets are supported
- Validate all client-provided data before processing
- Ensure protocol header values contain only ASCII characters
- Validate data from redirects
- Use allowlists rather than denylists for expected data types
- Validate data range and length
- Implement additional controls for potentially hazardous inputs
- Use discrete checks when standard validation cannot address certain inputs
- Apply canonicalization to prevent obfuscation attacks

## Output Encoding

- Conduct all output encoding on trusted systems (server-side)
- Use standard, tested routines for each encoding type
- Specify character sets like UTF-8 for all outputs
- Contextually encode all untrusted data returned to clients
- Ensure output encoding works safely across all target systems
- Sanitize SQL, XML, and LDAP queries containing untrusted data
- Sanitize operating system command outputs with untrusted data

## Authentication and Password Management

- Require authentication for all non-public pages and resources
- Enforce all authentication controls on trusted systems
- Use standard, tested authentication services
- Centralize authentication implementation across the application
- Segregate authentication logic and use redirection to centralized controls
- Ensure authentication controls fail securely
- Secure administrative and account management functions equivalently to primary authentication
- Use cryptographically strong, salted, one-way hashes for credential stores
- Implement password hashing server-side on trusted systems
- Validate authentication data only after all input is complete
- Avoid disclosing which authentication element was incorrect
- Use authentication for external system connections involving sensitive data
- Store external service credentials in secure stores
- Transmit authentication credentials only via HTTP POST
- Send non-temporary passwords only over encrypted connections
- Enforce policy-based password complexity requirements
- Enforce policy-based password length requirements
- Obscure password entry on user screens
- Disable accounts after specified invalid login attempts
- Apply same controls to password reset/change as account creation
- Use sufficiently random password reset questions
- Send email-based resets only to pre-registered addresses with temporary links
- Set short expiration times for temporary passwords and links
- Force temporary password changes on first use
- Notify users of password resets
- Prevent password reuse
- Require passwords to be at least one day old before changing
- Enforce policy-based password change intervals
- Disable "remember me" for password fields
- Report last account use (successful or failed) at next successful login
- Monitor for attacks using identical passwords across multiple accounts
- Change or disable all vendor-supplied default credentials
- Re-authenticate before critical operations
- Implement multi-factor authentication for sensitive or high-value accounts
- Inspect third-party authentication code for malicious content

## Session Management

- Use server or framework session management controls exclusively
- Create session identifiers on trusted systems (server-side)
- Use well-vetted algorithms ensuring sufficiently random identifiers
- Set appropriate domain and path restrictions for authenticated session cookies
- Fully terminate associated sessions during logout
- Provide logout functionality on all protected pages
- Establish shortest practical session inactivity timeout
- Disallow persistent logins and enforce periodic terminations
- Close pre-login sessions and establish new ones after successful authentication
- Generate new identifiers upon re-authentication
- Prevent concurrent logins with identical user IDs
- Never expose session identifiers in URLs, error messages, or logs
- Implement access controls protecting server-side session data from unauthorized access
- Periodically generate new session identifiers
- Generate new identifiers if connection changes from HTTP to HTTPS during authentication
- Consistently use HTTPS rather than switching between protocols
- Supplement standard session management for account operations with per-session random tokens
- Supplement standard session management for critical operations with per-request random tokens
- Set "secure" attribute for cookies transmitted over TLS
- Set HttpOnly attribute on cookies unless client-side script access is required

## Access Control

- Use only trusted server-side objects for authorization decisions
- Employ single site-wide component for access authorization checks
- Ensure access controls fail securely
- Deny all access if security configuration is unavailable
- Enforce authorization on every request, including server-side scripts
- Segregate privileged logic from other application code
- Restrict file and resource access to authorized users only
- Restrict protected URL access to authorized users
- Restrict protected function access to authorized users
- Restrict direct object references to authorized users
- Restrict service access to authorized users
- Restrict application data access to authorized users
- Restrict access to user attributes and policy information
- Restrict access to security configuration information
- Align server-side and presentation layer access control rules
- Use encryption and integrity checking for client-stored state data
- Enforce application logic flows complying with business rules
- Limit transaction rates per user/device to deter automated attacks
- Use referrer headers as supplemental checks only
- Periodically re-validate user authorization for long sessions
- Implement account auditing and disable unused accounts
- Support account disabling and session termination when authorization ends
- Minimize privileges for service and external system accounts
- Document access control policies covering business rules and authorization processes

## Cryptographic Practices

- Implement all cryptographic secret-protection functions on trusted systems
- Protect secrets from unauthorized access
- Design cryptographic modules to fail securely
- Use approved cryptographic random number generators for all random data
- Ensure cryptographic modules comply with FIPS 140-2 or equivalent standards
- Establish and follow documented key management policies

## Error Handling and Logging

- Avoid disclosing sensitive information in error responses
- Use error handlers that hide debugging and stack trace information
- Implement generic error messages and custom error pages
- Handle application errors without relying on server configuration
- Free allocated memory when errors occur
- Deny access by default in security-related error handling
- Implement all logging on trusted systems
- Support logging of both successful and failed security events
- Include important log event data in entries
- Prevent untrusted data in logs from executing as code
- Restrict log access to authorized individuals only
- Use central routine for all logging operations
- Exclude sensitive information from logs
- Establish mechanism for log analysis
- Log all input validation failures
- Log all authentication attempts, especially failures
- Log all access control failures
- Log apparent tampering events and unexpected state changes
- Log invalid or expired session token connection attempts
- Log all system exceptions
- Log all administrative functions and security configuration changes
- Log backend TLS connection failures
- Log cryptographic module failures
- Use cryptographic hash functions to validate log entry integrity

## Data Protection

- Implement least privilege, restricting user access to necessary functionality and data
- Protect cached and temporary sensitive data from unauthorized access; purge promptly
- Encrypt highly sensitive stored information including authentication data
- Prevent server-side source code downloads
- Never store passwords, connection strings, or sensitive data in cleartext on clients
- Remove comments in production code revealing backend or sensitive information
- Remove unnecessary application and system documentation
- Exclude sensitive information from HTTP GET parameters
- Disable auto-complete on forms containing sensitive information
- Disable client-side caching on sensitive pages
- Support removal of sensitive data when no longer required
- Implement appropriate access controls for server-side sensitive data

## Communication Security

- Encrypt all sensitive information transmission using TLS and supplemental methods
- Verify TLS certificates have correct domain names, validity, and required intermediates
- Prevent failed TLS connections from falling back to insecure connections
- Use TLS for all authenticated content and sensitive information
- Use TLS for external system connections involving sensitive data
- Standardize and appropriately configure TLS implementation
- Specify character encodings for all connections
- Filter sensitive parameters from HTTP referer headers when linking externally

## System Configuration

- Maintain latest approved versions of servers, frameworks, and components
- Apply all available patches for in-use versions
- Turn off directory listings
- Restrict web server, process, and service account privileges to minimum necessary
- Fail securely when exceptions occur
- Remove all unnecessary functionality and files
- Remove test code and non-production functionality before deployment
- Prevent directory structure disclosure in robots.txt using isolated parent directories
- Define supported HTTP methods and handling by page
- Disable unnecessary HTTP methods
- Configure different HTTP versions similarly and document differences
- Remove unnecessary OS, web-server, and framework information from response headers
- Enable human-readable security configuration output for auditing
- Implement asset management registering all components and software
- Isolate development environments from production networks
- Implement software change control for code modifications

## Database Security

- Use strongly typed parameterized queries
- Validate input and encode output; address meta characters before executing commands
- Ensure all variables are strongly typed
- Use lowest necessary privilege levels for database access
- Use secure credentials for database access
- Store connection strings in encrypted separate configuration files, not hardcoded
- Use stored procedures abstracting data access and removing base table permissions
- Close connections as soon as possible
- Change or remove default database administrative passwords
- Turn off unnecessary database functionality
- Remove unnecessary default vendor content and sample schemas
- Disable unneeded default accounts
- Connect using different credentials per trust level (user, read-only, guest, admin)

## File Management

- Never pass user-supplied data directly to dynamic include functions
- Require authentication before file uploads
- Limit uploadable file types to business requirements only
- Validate file types by checking headers, not extensions
- Don't save files in application web context
- Prevent uploading files interpretable by the web server
- Turn off execution privileges on upload directories
- Implement safe UNIX uploads using logical drives or chrooted environments
- Use allowlists of permitted file names and types for references
- Never pass user-supplied data to dynamic redirects
- Use index values mapped to predefined paths instead of file paths
- Never send absolute file paths to clients
- Ensure application files and resources are read-only
- Scan uploaded files for viruses and malware

## Memory Management

- Utilize input and output controls for untrusted data
- Verify buffer sizes match specifications
- Handle NULL termination correctly in byte-counting functions
- Check buffer boundaries in loops and prevent overflow
- Truncate input strings to reasonable length before passing to functions
- Explicitly close resources without relying on garbage collection
- Use non-executable stacks when available
- Avoid known vulnerable functions
- Free allocated memory at all function exit points
- Overwrite sensitive allocated memory information at all exit points

## General Coding Practices

- Use tested, approved managed code for common tasks rather than unmanaged code
- Use task-specific built-in APIs for OS tasks; prevent direct OS command execution
- Use checksums or hashes to verify integrity of code, libraries, and configuration
- Use locking or synchronization to prevent concurrent request conflicts and race conditions
- Protect shared variables and resources from inappropriate concurrent access
- Explicitly initialize all variables and data stores at declaration or before first use
- Raise privileges as late as possible and drop them as soon as possible
- Understand programming language representations to avoid calculation errors
- Never pass user-supplied data to dynamic execution functions
- Restrict users from generating or altering code
- Review secondary applications, third-party code, and libraries for necessity and safety
- Implement safe updates using encrypted channels
