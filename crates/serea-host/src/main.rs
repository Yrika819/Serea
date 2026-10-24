#![forbid(unsafe_code)]

#[cfg(feature = "synthetic")]
use std::error::Error;
use std::process::ExitCode;
#[cfg(feature = "synthetic")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "synthetic")]
mod host_clock;
#[cfg(feature = "synthetic")]
mod synthetic_ids;
#[cfg(feature = "synthetic")]
mod synthetic_model;
#[cfg(feature = "synthetic-test-hooks")]
mod test_readiness;

#[cfg(feature = "synthetic")]
static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "doctor".into());
    match command.as_str() {
        "doctor" => {
            println!("Serea Core development host");
            println!("os={}", std::env::consts::OS);
            println!("arch={}", std::env::consts::ARCH);
            println!("external_providers=disabled");
            println!("capability_invocation=disabled");
            ExitCode::SUCCESS
        }
        "status" => {
            println!("Serea Core host status");
            println!("os={}", std::env::consts::OS);
            println!("arch={}", std::env::consts::ARCH);
            println!("external_providers=disabled");
            ExitCode::SUCCESS
        }
        #[cfg(feature = "synthetic")]
        "run" if std::env::args().any(|arg| arg == "--synthetic") => match run_synthetic_host() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_error) => {
                #[cfg(feature = "synthetic-test-hooks")]
                test_readiness::report_host_error(&_error);
                eprintln!("{{\"event\":\"host_error\",\"code\":\"runtime_failed\"}}");
                ExitCode::FAILURE
            }
        },
        #[cfg(feature = "synthetic")]
        "stop" if std::env::args().any(|arg| arg == "--synthetic") => {
            let Some(directory) = option_value("--data-dir") else {
                eprintln!("usage: serea-host stop --synthetic --data-dir PATH");
                return ExitCode::from(2);
            };
            match std::fs::write(std::path::Path::new(&directory).join("host.stop"), b"stop") {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::FAILURE,
            }
        }
        #[cfg(feature = "synthetic")]
        "smoke" if std::env::args().any(|arg| arg == "--synthetic") => match synthetic_smoke() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("synthetic smoke failed: {error}");
                ExitCode::FAILURE
            }
        },
        #[cfg(feature = "synthetic")]
        "model-config-check" if std::env::args().any(|arg| arg == "--synthetic") => {
            match synthetic_model::config_check() {
                Ok(()) => {
                    println!(
                        "{{\"event\":\"synthetic_model_configured\",\"provider\":\"in_process_fake\",\"dispatch\":false}}"
                    );
                    ExitCode::SUCCESS
                }
                Err(_) => ExitCode::FAILURE,
            }
        }
        #[cfg(feature = "synthetic")]
        "scheduler-seed" if std::env::args().any(|arg| arg == "--synthetic") => {
            let Some(directory) = option_value("--data-dir") else {
                eprintln!(
                    "usage: serea-host scheduler-seed --synthetic --data-dir PATH [--count N]"
                );
                return ExitCode::from(2);
            };
            let count = match option_value("--count") {
                None => 1,
                Some(value) => match value.parse::<u16>() {
                    Ok(count) => count,
                    Err(_) => {
                        eprintln!("scheduler seed count must be an integer");
                        return ExitCode::from(2);
                    }
                },
            };
            if count == 0 || count > 256 {
                eprintln!("scheduler seed count must be between 1 and 256");
                return ExitCode::from(2);
            }
            match seed_synthetic_schedule(std::path::Path::new(&directory), count) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => {
                    eprintln!("{{\"event\":\"scheduler_seed_error\",\"code\":\"seed_failed\"}}");
                    ExitCode::FAILURE
                }
            }
        }
        #[cfg(feature = "synthetic")]
        "model-smoke" if std::env::args().any(|arg| arg == "--synthetic") => {
            let Some(directory) = option_value("--data-dir") else {
                eprintln!(
                    "usage: serea-host model-smoke --synthetic --data-dir PATH [--task-id TASK_ID]"
                );
                return ExitCode::from(2);
            };
            #[cfg(feature = "synthetic-test-hooks")]
            let result = {
                let scenario = option_value("--scenario").unwrap_or_else(|| "success".into());
                synthetic_model::execute_once_with_test_barrier(
                    std::path::Path::new(&directory),
                    &scenario,
                    option_value("--test-barrier").as_deref(),
                    option_value("--test-barrier-dir")
                        .as_deref()
                        .map(std::path::Path::new),
                    option_value("--task-id").as_deref(),
                )
            };
            #[cfg(not(feature = "synthetic-test-hooks"))]
            let result = {
                let failure = match option_value("--scenario").as_deref() {
                    None | Some("success") => false,
                    Some("failure") => true,
                    Some(_) => {
                        eprintln!(
                            "usage: serea-host model-smoke --synthetic --data-dir PATH [--scenario success|failure] [--task-id TASK_ID]"
                        );
                        return ExitCode::from(2);
                    }
                };
                if option_value("--test-barrier").is_some()
                    || option_value("--test-barrier-dir").is_some()
                {
                    eprintln!("test barriers require the synthetic-test-hooks feature");
                    return ExitCode::from(2);
                }
                synthetic_model::execute_once(
                    std::path::Path::new(&directory),
                    failure,
                    option_value("--task-id").as_deref(),
                )
            };
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("synthetic model check failed: {error}");
                    eprintln!(
                        "{{\"event\":\"synthetic_model_error\",\"code\":\"dispatch_failed\"}}"
                    );
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "usage: serea-host [doctor|status|smoke --synthetic|scheduler-seed --synthetic --data-dir PATH [--count N]|model-config-check --synthetic|model-smoke --synthetic --data-dir PATH|run --synthetic --data-dir PATH|stop --synthetic --data-dir PATH]"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(feature = "synthetic")]
fn option_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

#[cfg(feature = "synthetic")]
fn run_synthetic_host() -> Result<(), Box<dyn std::error::Error>> {
    use crate::synthetic_ids::SyntheticHostUlidSource;
    use host_clock::WallClock;
    use serea_core::Core;
    use serea_event_bus::EventBus;
    use serea_protocol::{Clock, TimestampMs};
    use std::fs::OpenOptions;
    use std::time::Duration;

    let directory = option_value("--data-dir").ok_or("--data-dir is required")?;
    let root = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&root)?;
    let lock_path = root.join("host.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "synthetic host is already running")?;
    let _lock_guard = HostLock(lock);
    let stop = root.join("host.stop");
    // A stop request belongs to the previous process lifetime. The lock is
    // held before removing it, so concurrent starts cannot consume it.
    match std::fs::remove_file(&stop) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let db = root.join("core.sqlite");
    let clock = WallClock;
    let epoch = clock.now_ms()?;
    // OS entropy separates process lifetimes even with same-millisecond
    // restarts, clock rollback, or PID reuse.
    let ulid_ms = u64::try_from(epoch.get()).map_err(|_| "clock before ULID epoch")?;
    let ulid_timestamp = TimestampMs::new(ulid_ms)?;
    let ids = SyntheticHostUlidSource::new(ulid_timestamp)
        .map_err(|_| "synthetic identifier entropy unavailable")?;
    let events = EventBus::new(ids);
    let mut core = Core::open(&db, &clock, events.clone())?;
    if core.schema_version()? != 8 {
        return Err("unexpected canonical schema version".into());
    }
    let recovered = core.recover(epoch, "synthetic-host")?;
    // Task/Scheduler repair runs first, then unresolved dispatch intents are
    // terminally reconciled without registering or invoking a provider.
    // Any failure is fatal and readiness is withheld.
    let model_recovered = synthetic_model::recover_unresolved(&root, &events, &clock)
        .map_err(|_| "synthetic model startup recovery failed")?;
    let shutdown_requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal_flag = std::sync::Arc::clone(&shutdown_requested);
    let host_thread = std::thread::current();
    ctrlc::set_handler(move || {
        signal_flag.store(true, std::sync::atomic::Ordering::Release);
        host_thread.unpark();
    })
    .map_err(|_| "termination handler unavailable")?;
    {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        writeln!(
            output,
            "{{\"event\":\"host_ready\",\"schema_version\":8,\"mode\":\"synthetic\"}}"
        )?;
        output.flush()?;
    }
    // Test-only readiness evidence stays behind the test-hooks feature so the
    // normal development Host has no environment-variable configuration path.
    #[cfg(feature = "synthetic-test-hooks")]
    test_readiness::write_ready_marker(model_recovered)?;
    if model_recovered > 0 {
        println!("{{\"event\":\"model_recovery\",\"ambiguous_attempts\":{model_recovered}}}");
    }
    if recovered.calendar_tasks_created_or_reused > 0
        || recovered.host_event_tasks_created_or_reused > 0
    {
        println!(
            "{{\"event\":\"scheduler_recovery\",\"calendar_tasks_created_or_reused\":{},\"host_event_tasks_created_or_reused\":{}}}",
            recovered.calendar_tasks_created_or_reused,
            recovered.host_event_tasks_created_or_reused
        );
    }
    loop {
        if stop.exists() || shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        let now = clock.now_ms()?;
        let (event_tasks, calendar_tasks) = core.process_scheduler_wake(now, "synthetic-host")?;
        if event_tasks > 0 || calendar_tasks > 0 {
            println!(
                "{{\"event\":\"scheduler_wake\",\"host_event_tasks_created_or_reused\":{event_tasks},\"calendar_tasks_created_or_reused\":{calendar_tasks}}}"
            );
        }
        // Bounded wake cadence; due schedules are rechecked after every wake.
        std::thread::park_timeout(Duration::from_secs(1));
    }
    let _ = std::fs::remove_file(&stop);
    core.shutdown()?;
    println!("{{\"event\":\"host_stopped\",\"mode\":\"synthetic\"}}");
    Ok(())
}

#[cfg(feature = "synthetic")]
struct HostLock(std::fs::File);

#[cfg(feature = "synthetic")]
impl Drop for HostLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

#[cfg(feature = "synthetic")]
fn synthetic_smoke() -> Result<(), Box<dyn Error>> {
    use serea_core::Core;
    use serea_event_bus::EventBus;
    use serea_protocol::*;
    use serea_task_engine::{NewTask, TransitionContext};
    use serea_testkit::{DeterministicUlidSource, TestClock};

    let clock = TestClock::at_epoch();
    let now = clock.now_ms()?;
    let root = loop {
        let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "serea-synthetic-smoke-{}-{}-{nonce}",
            std::process::id(),
            now.get()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };
    let path = root.join("core.sqlite");
    let events = EventBus::new(DeterministicUlidSource::new());
    let fixture_generation = install_synthetic_registry_fixture(&path, &clock, &events, now)?;
    if install_synthetic_registry_fixture(&path, &clock, &events, now).is_ok() {
        return Err("synthetic setup replaced an already active registry generation".into());
    }
    let task_id = TaskId::new("tsk_00000000000000000000000001")?;
    let actor = ActorId::new("serea-synthetic-host")?;
    let actor_version = SemVer::new("0.1.0")?;
    let cause = EventId::new("evt_00000000000000000000000001")?;
    let context = TransitionContext {
        actor_kind: ActorKind::Host,
        actor_id: &actor,
        actor_version: &actor_version,
        causation_id: Some(&cause),
    };

    let initial_event_count = {
        let mut core = Core::open(&path, &clock, events.clone())?;
        if core.schema_version()? != 8 {
            return Err("unexpected canonical schema version".into());
        }
        core.tasks().create_task(
            NewTask {
                task_id: task_id.clone(),
                kind: TaskKind::UserRequest,
                title: TaskTitle::new("Synthetic core lifecycle")?,
                origin: TaskOrigin {
                    kind: TaskOriginKind::new("SYNTHETIC")?,
                    device_id: None,
                    message_id: None,
                    extensions: Extensions::new(),
                },
                data_class: DataClass::Personal,
                policy_class: RiskClass::Observe,
                attempt_budget: AttemptBudget {
                    max_model_calls: 1,
                    max_tool_calls: 0,
                    max_attempts_per_step: 1,
                    extensions: Extensions::new(),
                },
                created_at: now,
                deadline_at: None,
                extensions: Extensions::new(),
            },
            &context,
        )?;
        core.start_planning(task_id.clone(), TaskState::Received, 0, now, &context)?;
        {
            let store = serea_storage::Store::open(&path, &clock)?;
            if store.get_task_registry_generation(&task_id)? != Some(fixture_generation) {
                return Err("synthetic task did not pin the installed fixture generation".into());
            }
        }
        let audit = core.audit_sequence_summary(64)?;
        if audit.task_created_events != 1 || !audit.strictly_ordered || audit.event_count < 3 {
            return Err("synthetic audit sequence was incomplete or out of order".into());
        }
        let event_count = audit.event_count;
        core.shutdown()?;
        event_count
    };

    {
        let events = EventBus::new(DeterministicUlidSource::starting_at(1_800_000_000_000)?);
        let mut core = Core::open(&path, &clock, events)?;
        core.recover(now, "synthetic-recovery")?;
        let recovered = core.load_task(task_id.clone())?;
        if recovered.task.state != TaskState::Planning {
            return Err("synthetic task state did not survive restart".into());
        }
        let audit = core.audit_sequence_summary(64)?;
        if audit.task_created_events != 1
            || !audit.strictly_ordered
            || audit.event_count < initial_event_count
        {
            return Err("restart recovery duplicated or reordered durable audit events".into());
        }
        core.shutdown()?;
    }

    std::fs::remove_file(&path)?;
    std::fs::remove_dir(&root)?;
    println!("synthetic_task={}", task_id.as_str());
    println!("schema_version=8");
    println!("restart_recovery=passed");
    println!("external_providers=unreachable");
    println!("capability_invocation=disabled");
    Ok(())
}

#[cfg(feature = "synthetic")]
fn install_synthetic_registry_fixture(
    path: &std::path::Path,
    clock: &dyn serea_protocol::Clock,
    events: &serea_event_bus::EventBus,
    now: serea_protocol::EpochMillis,
) -> Result<i64, Box<dyn std::error::Error>> {
    use serea_capability::CapabilityRegistry;
    use serea_protocol::{
        Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId, DataClass,
        DescriptorDescription, DescriptorTitle, IdempotencySupport, JsonSchemaRef, ReplaySafety,
        RiskClass, RootRequirement, SemVer, SideEffectClass, digest_of,
    };
    use serea_storage::{
        DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store,
    };

    let store = Store::open(path, clock)?;
    if CapabilityRegistry::current_generation(&store)?.is_some() {
        return Err("refusing to replace an active registry generation".into());
    }
    let generation = CapabilityRegistry::create_generation(
        &store,
        RegistryGenerationDraft {
            manifest_digest: digest_of("\"serea-empty-synthetic-manifest-v1\"")?,
            schema_catalog_digest: digest_of("\"serea-empty-synthetic-catalog-v1\"")?,
        },
    )?;
    let descriptor = CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new("synthetic.echo.read")?,
        version: SemVer::new("1.0.0")?,
        title: DescriptorTitle::new("Synthetic no-op")?,
        description: DescriptorDescription::new("Synthetic-only no-op fixture")?,
        provider_id: serea_protocol::ProviderId::new("synthetic")?,
        implementation_id: None,
        input_schema: JsonSchemaRef::new("serea://synthetic/input")?,
        output_schema: JsonSchemaRef::new("serea://synthetic/output")?,
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Public,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 1_000,
        cost_class: serea_protocol::CostClass::Free,
        experimental: true,
    })?;
    let descriptor_digest = digest_of("\"serea-synthetic-no-op-descriptor-v1\"")?;
    let schema_digest = digest_of("\"serea-synthetic-no-op-schema-v1\"")?;
    CapabilityRegistry::insert_descriptor_revision(
        &store,
        DescriptorRevisionDraft {
            descriptor_digest: descriptor_digest.clone(),
            descriptor: descriptor.clone(),
            input_schema_digest: schema_digest.clone(),
            output_schema_digest: schema_digest,
        },
    )?;
    CapabilityRegistry::add_generation_member(
        &store,
        GenerationMemberDraft {
            generation_id: generation.generation_id(),
            descriptor_digest,
            candidate_priority: 0,
        },
    )?;
    CapabilityRegistry::set_default_version(
        &store,
        generation.generation_id(),
        descriptor.id().clone(),
        descriptor.version().clone(),
    )?;
    CapabilityRegistry::activate_generation(&store, events, generation.generation_id(), now)?;
    Ok(generation.generation_id())
}

#[cfg(feature = "synthetic")]
fn seed_synthetic_schedule(
    root: &std::path::Path,
    count: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    use host_clock::WallClock;
    use serea_event_bus::EventBus;
    use serea_protocol::{Clock, DataClass, Digest, EventId, ScheduleId};
    use serea_scheduler::{ScheduleDefinition, Scheduler};
    use serea_storage::{MissedOccurrencePolicy, ScheduleOwnerKind, ScheduleTriggerKind, Store};
    use serea_task_engine::TaskEngine;
    use serea_testkit::DeterministicUlidSource;
    use std::fs::OpenOptions;

    std::fs::create_dir_all(root)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("host.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| "synthetic host is already running")?;
    let _lock_guard = HostLock(lock);
    let path = root.join("core.sqlite");
    let clock = WallClock;
    let now = clock.now_ms()?;
    let events = EventBus::new(DeterministicUlidSource::new());
    install_synthetic_registry_fixture(&path, &clock, &events, now)?;
    let mut scheduler = Scheduler::new(
        Store::open(&path, &clock)?,
        TaskEngine::new(Store::open(&path, &clock)?, events.clone()),
        events,
    );
    for index in 1..=count {
        let suffix = format!("{:026}", u32::from(index) + 40);
        scheduler.create_schedule(
            ScheduleDefinition {
                schedule_id: ScheduleId::new(format!("sch_{suffix}"))?,
                owner_kind: ScheduleOwnerKind::Host,
                owner_id: "synthetic-host".to_owned(),
                trigger_kind: ScheduleTriggerKind::Calendar,
                recurrence_json: Some(
                    r#"{"version":"1","kind":"ONCE","anchor_local":"2020-01-02T03:04"}"#
                        .to_owned(),
                ),
                event_predicate_json: None,
                template_json: br#"{"version":"1","title":"Synthetic scheduled task","intent":"PUBLIC synthetic scheduled work."}"#.to_vec(),
                title_data_class: DataClass::Public,
                intent_data_class: DataClass::Public,
                policy_class_rank: 0,
                approval_policy: serde_json::json!({}),
                timezone: Some("UTC".to_owned()),
                missed_policy: MissedOccurrencePolicy::RunOnce,
            },
            &EventId::new(format!("evt_{suffix}"))?,
            &Digest::new(format!("sha256:{}", "b".repeat(64)))?,
            now,
        )?;
    }
    println!(
        "{{\"event\":\"scheduler_seeded\",\"trigger\":\"calendar_once\",\"count\":{count},\"data_class\":\"PUBLIC\"}}"
    );
    Ok(())
}
