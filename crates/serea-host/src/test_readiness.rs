//! Synthetic-test-only readiness receipt for the Host lifecycle harness.
//!
//! This module is compiled only with `synthetic-test-hooks`. Production Host
//! builds do not read environment variables or write test markers.

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

pub(super) fn report_host_error(error: &dyn std::fmt::Debug) {
    if std::env::var_os("SEREA_TEST_READY_MARKER").is_some() {
        eprintln!("synthetic_host_error_detail={error:?}");
    }
}

pub(super) fn write_ready_marker(model_recovered: usize) -> Result<(), Box<dyn Error>> {
    let Some(marker) = std::env::var_os("SEREA_TEST_READY_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    let name = marker
        .file_name()
        .ok_or("invalid synthetic readiness marker path")?
        .to_string_lossy();
    let temporary = marker.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    let mut receipt = std::fs::File::create(&temporary)?;
    writeln!(receipt, "pid={}", std::process::id())?;
    writeln!(receipt, "schema_version=8")?;
    writeln!(receipt, "mode=synthetic")?;
    writeln!(receipt, "model_recovered={model_recovered}")?;
    receipt.sync_all()?;
    drop(receipt);
    std::fs::rename(temporary, marker)?;
    Ok(())
}

/// Test-only barrier used when a lifecycle test must crash the host before its
/// normal scheduler wake loop begins. This keeps process restart coverage from
/// racing an unrelated 120-second scheduler consumer lease.
pub(super) fn pause_after_ready_if_configured() -> Result<(), Box<dyn Error>> {
    let Some(marker) = std::env::var_os("SEREA_TEST_PAUSE_AFTER_READY_MARKER") else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    let name = marker
        .file_name()
        .ok_or("invalid synthetic pause marker path")?
        .to_string_lossy();
    let temporary = marker.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    let mut receipt = std::fs::File::create(&temporary)?;
    writeln!(receipt, "pid={}", std::process::id())?;
    receipt.sync_all()?;
    drop(receipt);
    std::fs::rename(temporary, marker)?;

    // The lifecycle test kills this child after observing the marker. Keeping
    // the host parked here prevents it from claiming the live scheduler lease.
    loop {
        std::thread::park();
    }
}
