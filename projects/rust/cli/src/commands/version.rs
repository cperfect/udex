//! Handler for `udex version`.

use anyhow::Result;

use crate::cli::VersionArgs;

/// Print the CLI version and exit.
///
/// Default output is a human-readable line (`udex 0.1.0`).
/// Pass `--json` for a machine-readable JSON object (`{"version":"0.1.0"}`).
pub fn run(args: VersionArgs) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    if args.json {
        println!("{{\"version\":\"{version}\"}}");
    } else {
        println!("udex {version}");
    }
    Ok(())
}
