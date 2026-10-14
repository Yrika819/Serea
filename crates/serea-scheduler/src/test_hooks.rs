//! Explicitly opt-in barriers for compiled Host process-crash acceptance tests.

use crate::runtime::ScheduleError;

pub(crate) fn barrier(point: &str) -> Result<(), ScheduleError> {
    if std::env::var("SEREA_SCHEDULER_TEST_BARRIER").as_deref() != Ok(point) {
        return Ok(());
    }
    let directory = std::env::var_os("SEREA_SCHEDULER_TEST_BARRIER_DIR")
        .map(std::path::PathBuf::from)
        .ok_or(ScheduleError::Invalid)?;
    std::fs::create_dir_all(&directory).map_err(|_| ScheduleError::Invalid)?;
    std::fs::write(directory.join(format!("{point}.reached")), b"ready")
        .map_err(|_| ScheduleError::Invalid)?;
    let release = directory.join(format!("{point}.release"));
    for _ in 0..24_000 {
        if release.exists() {
            return Ok(());
        }
        std::thread::park_timeout(std::time::Duration::from_millis(5));
    }
    Err(ScheduleError::Invalid)
}
