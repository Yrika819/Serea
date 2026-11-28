//! Feature-gated native synchronization point for synthetic diagnostics only.
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::BridgeError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Armed,
    Entered,
    Released,
}
static GATE: (Mutex<Phase>, Condvar) = (Mutex::new(Phase::Idle), Condvar::new());
const LIMIT: Duration = Duration::from_secs(15);

pub(crate) fn arm() -> Result<(), BridgeError> {
    let mut phase = GATE.0.lock().map_err(|_| BridgeError::Internal)?;
    if *phase != Phase::Idle {
        return Err(BridgeError::Internal);
    }
    *phase = Phase::Armed;
    Ok(())
}

pub(crate) fn wait_entered() -> Result<(), BridgeError> {
    let phase = GATE.0.lock().map_err(|_| BridgeError::Internal)?;
    let (mut phase, timeout) = GATE
        .1
        .wait_timeout_while(phase, LIMIT, |p| *p == Phase::Armed)
        .map_err(|_| BridgeError::Internal)?;
    if *phase == Phase::Entered {
        Ok(())
    } else {
        if timeout.timed_out() && *phase == Phase::Armed {
            *phase = Phase::Idle;
            GATE.1.notify_all();
        }
        Err(BridgeError::Internal)
    }
}

pub(crate) fn release() -> Result<(), BridgeError> {
    let mut phase = GATE.0.lock().map_err(|_| BridgeError::Internal)?;
    if *phase != Phase::Entered {
        return Err(BridgeError::Internal);
    }
    *phase = Phase::Released;
    GATE.1.notify_all();
    Ok(())
}

pub(crate) fn checkpoint() -> Result<(), BridgeError> {
    let mut phase = GATE.0.lock().map_err(|_| BridgeError::Internal)?;
    if *phase == Phase::Idle {
        return Ok(());
    }
    if *phase != Phase::Armed {
        return Err(BridgeError::Internal);
    }
    *phase = Phase::Entered;
    GATE.1.notify_all();
    let deadline = Instant::now() + LIMIT;
    while *phase == Phase::Entered {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            *phase = Phase::Idle;
            return Err(BridgeError::Internal);
        }
        let (next, _) = GATE
            .1
            .wait_timeout(phase, remaining)
            .map_err(|_| BridgeError::Internal)?;
        phase = next;
    }
    if *phase != Phase::Released {
        *phase = Phase::Idle;
        return Err(BridgeError::Internal);
    }
    *phase = Phase::Idle;
    Ok(())
}
