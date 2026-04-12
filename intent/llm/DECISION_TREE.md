# Decision Tree - Where Does This Code Belong?

> Use this tree when you're about to write new code. Walk through the questions to find the right location.
> Always cross-reference MODULES.md — if a module already owns that concern, put the code there.

## Rust Decision Tree

### Step 1: Which layer is it?

**Does it define the shape of messages, permissions, or hashing?**
→ `udex-api` crate  
- Protobuf schema changes go in `../protobuf/` and regenerate via `build.rs`
- New auth/permission logic goes in `udex_api::authz`
- New hashing logic goes in `udex_api::hash`
- Do NOT put business logic or I/O here

**Does it touch the database or own the datastore contract?**
→ `udex-datastore` crate  
- Changes to the `Datastore` or `Migrator` trait go in `lib.rs`
- All SQL goes in `udex_datastore::postgres`
- New domain types (structs, enums used across the datastore) go in `lib.rs`
- Do NOT expose `sqlx` types outside this crate

**Is it a gRPC handler, server config, auth enforcement, or observability?**
→ `udex-server` crate  
- gRPC handlers go in the relevant service module (`entry`, `index`, `healthz`)
- Handlers MUST be thin: validate input → call datastore → map result → return
- Authentication logic goes in `authn` (JWT enforcement only — policy is in `udex-api::authz`)
- Logging/tracing setup goes in `logging`
- Do NOT put SQL or protobuf-generation here

### Step 2: Does a module already own this?

1. Check MODULES.md
2. If yes: add code to that module
3. If no: register in MODULES.md first, then create the module

### Step 3: Anti-patterns

| Temptation | Correct Location |
| ---------- | ---------------- |
| SQL in a gRPC handler | `udex_datastore::postgres` |
| Business logic in a gRPC handler | Service module or datastore layer |
| `sqlx::Error` in server code | Wrap in `udex_datastore::Error` first |
| New protobuf type defined in Rust | Define in `.proto`, regenerate |
| Auth policy logic in `authn.rs` | `udex_api::authz` (policy), `authn.rs` (enforcement only) |
| Timestamp utils duplicated per crate | `udex_api` lib.rs — already has `now_timestamp` etc. |
| Second module for the same concern | Use the existing one (Highlander Rule) |

---

## Generic Decision Tree (Non-Rust)

### Where does it go?

1. **Data access** → Repository/data layer (not in handlers/controllers)
2. **Business logic** → Service/domain layer (not in UI or API layer)
3. **Request handling** → Controller/handler (thin: parse, delegate, respond)
4. **UI rendering** → View/template layer (no business logic)
5. **Background work** → Worker/job layer (delegates to services)
6. **Shared utilities** → Only if used by 3+ callers; otherwise inline

### The same rules apply everywhere

- Thin handlers, fat services
- One module per concern (check the registry)
- Register before you create
