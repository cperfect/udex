fn main() {
    // forces re-run if migrations directory changes
    // see https://docs.rs/sqlx/latest/sqlx/macro.migrate.html#triggering-recompilation-on-migration-changes
    println!("cargo:rerun-if-changed=migrations");
}