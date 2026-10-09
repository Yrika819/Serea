use serea_capability::{
    AvailabilityError, CapabilityAvailabilitySnapshotV1, CapabilityManifestV1, CapabilityRegistry,
    HostEligibility, ProviderRegistry,
};
use serea_storage::{
    DescriptorRevisionDraft, GenerationMemberDraft, RegistryGenerationDraft, Store,
};
use std::sync::Mutex;

static GENERATION_SETUP: Mutex<()> = Mutex::new(());

/// Build the durable facts a snapshot names, then pass that exact ID to the
/// production constructor. Keeping this in tests avoids inferring identity in
/// the production API.
pub fn build_snapshot(
    manifest: CapabilityManifestV1,
    registry: &ProviderRegistry,
    store: &Store,
    eligibility: HostEligibility,
) -> Result<CapabilityAvailabilitySnapshotV1, AvailabilityError> {
    let _setup = GENERATION_SETUP.lock().unwrap();
    let generation = CapabilityRegistry::create_generation(
        store,
        RegistryGenerationDraft {
            manifest_digest: manifest.digest().clone(),
            schema_catalog_digest: manifest.catalog_digest().clone(),
        },
    )
    .unwrap();
    for entry in manifest.entries() {
        CapabilityRegistry::insert_descriptor_revision(
            store,
            DescriptorRevisionDraft {
                descriptor_digest: entry.descriptor_digest.clone(),
                descriptor: entry.descriptor.clone(),
                input_schema_digest: entry.input_schema_digest.clone(),
                output_schema_digest: entry.output_schema_digest.clone(),
            },
        )
        .unwrap();
        CapabilityRegistry::add_generation_member(
            store,
            GenerationMemberDraft {
                generation_id: generation.generation_id(),
                descriptor_digest: entry.descriptor_digest.clone(),
                candidate_priority: entry.candidate_priority,
            },
        )
        .unwrap();
    }
    for (id, version) in manifest.defaults() {
        CapabilityRegistry::set_default_version(
            store,
            generation.generation_id(),
            id.clone(),
            version.clone(),
        )
        .unwrap();
    }
    block_on(CapabilityAvailabilitySnapshotV1::build_for_generation(
        manifest,
        generation.generation_id(),
        registry,
        store,
        eligibility,
    ))
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(std::sync::Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}
