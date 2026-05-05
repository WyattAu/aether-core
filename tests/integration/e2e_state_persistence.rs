//! End-to-End State Persistence Tests
//!
//! Validates state persistence and recovery:
//! - Create actor with state capability
//! - Write state via checkpoint manager
//! - Simulate actor restart
//! - Restore state
//! - Verify integrity

use aether_core::{
    Observability,
    actor::{ActorBuilder, ActorScheduler, SchedulerConfig},
    capability::CapabilitySet,
    state::{Checkpoint, CheckpointManager, CheckpointStore, InMemoryStore},
    wasi::StateHandle,
};
use std::sync::Arc;

#[tokio::test]
async fn test_e2e_state_checkpoint_create() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let actor_id = "checkpoint-create-actor";

    let state_data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

    let checkpoint = manager
        .checkpoint(actor_id, state_data.clone())
        .await
        .expect("Failed to create checkpoint");

    assert_eq!(checkpoint.sequence(), 1);
    assert_eq!(checkpoint.actor_id(), actor_id);
    assert!(!checkpoint.data.is_empty());
    assert_eq!(checkpoint.data, state_data);
}

#[tokio::test]
async fn test_e2e_state_checkpoint_restore() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let actor_id = "checkpoint-restore-actor";

    let initial_state = vec![0xDE, 0xAD, 0xBE, 0xEF];
    manager
        .checkpoint(actor_id, initial_state.clone())
        .await
        .expect("Failed to create initial checkpoint");

    let restored = manager
        .restore(actor_id)
        .await
        .expect("Failed to restore")
        .expect("No state found");

    assert_eq!(restored, initial_state);
}

#[tokio::test]
async fn test_e2e_state_versioning() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let actor_id = "versioning-actor";

    let v1 = vec![0x01];
    let v2 = vec![0x02];
    let v3 = vec![0x03];

    let cp1 = manager
        .checkpoint(actor_id, v1.clone())
        .await
        .expect("Failed to checkpoint v1");
    assert_eq!(cp1.sequence(), 1);

    let cp2 = manager
        .checkpoint(actor_id, v2.clone())
        .await
        .expect("Failed to checkpoint v2");
    assert_eq!(cp2.sequence(), 2);

    let cp3 = manager
        .checkpoint(actor_id, v3.clone())
        .await
        .expect("Failed to checkpoint v3");
    assert_eq!(cp3.sequence(), 3);

    let latest = manager
        .restore(actor_id)
        .await
        .expect("Failed to restore")
        .expect("No state");
    assert_eq!(latest, v3);

    let v1_restored = manager
        .restore_version(actor_id, 1)
        .await
        .expect("Failed to restore v1")
        .expect("No v1");
    assert_eq!(v1_restored, v1);

    let v2_restored = manager
        .restore_version(actor_id, 2)
        .await
        .expect("Failed to restore v2")
        .expect("No v2");
    assert_eq!(v2_restored, v2);
}

#[tokio::test]
async fn test_e2e_state_checksum_integrity() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let actor_id = "checksum-actor";

    let state_data = b"important state data that must not be corrupted".to_vec();

    let checkpoint = manager
        .checkpoint(actor_id, state_data.clone())
        .await
        .expect("Failed to create checkpoint");

    let expected_checksum = blake3::hash(&state_data);
    let actual_checksum = checkpoint.checksum();

    assert_eq!(expected_checksum.as_bytes(), &actual_checksum);

    let restored = manager
        .restore(actor_id)
        .await
        .expect("Failed to restore")
        .expect("No state");
    let restored_checksum = blake3::hash(&restored);

    assert_eq!(restored_checksum.as_bytes(), &actual_checksum);
}

#[tokio::test]
async fn test_e2e_state_multiple_actors() {
    let manager = CheckpointManager::new(InMemoryStore::new());

    let actors = vec![
        ("actor-a", vec![0x0A, 0x0A]),
        ("actor-b", vec![0x0B, 0x0B]),
        ("actor-c", vec![0x0C, 0x0C]),
    ];

    for (actor_id, state) in &actors {
        manager
            .checkpoint(actor_id, state.clone())
            .await
            .expect("Failed to checkpoint");
    }

    for (actor_id, expected_state) in &actors {
        let restored = manager
            .restore(actor_id)
            .await
            .expect("Failed to restore")
            .expect("No state found");
        assert_eq!(&restored, expected_state);
    }
}

#[tokio::test]
async fn test_e2e_state_with_actor_lifecycle() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let state_manager = Arc::new(CheckpointManager::new(InMemoryStore::new()));

    let handle = ActorBuilder::new()
        .name("stateful-lifecycle-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    let actor_id_str = handle.id().0.to_string();

    let initial_state = b"initial-actor-state".to_vec();
    state_manager
        .checkpoint(&actor_id_str, initial_state.clone())
        .await
        .expect("Failed to checkpoint initial state");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let updated_state = b"updated-actor-state".to_vec();
    state_manager
        .checkpoint(&actor_id_str, updated_state.clone())
        .await
        .expect("Failed to checkpoint updated state");

    handle.stop().await.expect("Failed to stop actor");

    let restored = state_manager
        .restore(&actor_id_str)
        .await
        .expect("Failed to restore")
        .expect("No state found");

    assert_eq!(restored, updated_state);

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_state_handle_api() {
    let state_caps = CapabilitySet::STATE_READ | CapabilitySet::STATE_WRITE;

    let state_handle =
        StateHandle::open("test-state-handle", &state_caps).expect("Failed to open state handle");

    let write_result = state_handle.write("counter", b"42");
    assert!(write_result.is_ok());

    let read_result = state_handle.read("counter");
    assert!(read_result.is_ok());

    let no_state_caps = CapabilitySet::LOG;
    let denied_result = StateHandle::open("denied-state", &no_state_caps);
    assert!(denied_result.is_err());
}

#[tokio::test]
async fn test_e2e_state_checkpoint_list() {
    let store = CheckpointStore::new(InMemoryStore::new());
    let actor_id = "list-checkpoints-actor";

    for i in 1..=5 {
        let checkpoint = Checkpoint::new(actor_id, i, vec![i as u8]);
        store
            .save(&checkpoint)
            .await
            .expect("Failed to save checkpoint");
    }

    let checkpoints = store
        .list(actor_id)
        .await
        .expect("Failed to list checkpoints");

    assert_eq!(checkpoints.len(), 5);

    assert_eq!(checkpoints[0].sequence, 5);
    assert_eq!(checkpoints[4].sequence, 1);

    for cp in &checkpoints {
        assert_eq!(cp.actor_id, actor_id);
        assert!(cp.size > 0);
    }
}

#[tokio::test]
async fn test_e2e_state_checkpoint_delete() {
    let store = CheckpointStore::new(InMemoryStore::new());
    let actor_id = "delete-checkpoint-actor";

    for i in 1..=3 {
        let checkpoint = Checkpoint::new(actor_id, i, vec![i as u8]);
        store
            .save(&checkpoint)
            .await
            .expect("Failed to save checkpoint");
    }

    let checkpoints = store.list(actor_id).await.expect("Failed to list");
    assert_eq!(checkpoints.len(), 3);

    store
        .delete(actor_id, 2)
        .await
        .expect("Failed to delete checkpoint 2");

    let checkpoints_after = store
        .list(actor_id)
        .await
        .expect("Failed to list after delete");
    assert_eq!(checkpoints_after.len(), 2);

    let sequences: Vec<u64> = checkpoints_after.iter().map(|c| c.sequence).collect();
    assert!(!sequences.contains(&2));
}

#[tokio::test]
async fn test_e2e_state_rollback() {
    let store = CheckpointStore::new(InMemoryStore::new());
    let actor_id = "rollback-actor";

    for i in 1..=5 {
        let checkpoint = Checkpoint::new(actor_id, i, vec![i as u8; 4]);
        store
            .save(&checkpoint)
            .await
            .expect("Failed to save checkpoint");
    }

    store
        .rollback(actor_id, 3)
        .await
        .expect("Failed to rollback");

    let checkpoints = store
        .list(actor_id)
        .await
        .expect("Failed to list after rollback");
    assert_eq!(checkpoints.len(), 3);

    let latest = store
        .load_latest(actor_id)
        .await
        .expect("Failed to load latest")
        .expect("No latest");
    assert_eq!(latest.sequence(), 3);
    assert_eq!(latest.data, vec![3u8; 4]);
}

#[tokio::test]
async fn test_e2e_state_max_checkpoints_limit() {
    let store = CheckpointStore::new(InMemoryStore::new());
    let actor_id = "max-limit-actor";

    for i in 1..=15 {
        let checkpoint = Checkpoint::new(actor_id, i, vec![i as u8]);
        store
            .save(&checkpoint)
            .await
            .expect("Failed to save checkpoint");
    }

    let checkpoints = store.list(actor_id).await.expect("Failed to list");

    assert!(checkpoints.len() <= aether_core::state::MAX_CHECKPOINTS_PER_ACTOR);
}

#[tokio::test]
async fn test_e2e_state_serialization() {
    let original = Checkpoint::new("serialization-test", 1, vec![0xDE, 0xAD, 0xBE, 0xEF]);

    let bytes = original.to_bytes().expect("Failed to serialize");
    assert!(!bytes.is_empty());

    let restored = Checkpoint::from_bytes(&bytes).expect("Failed to deserialize");

    assert_eq!(restored.actor_id(), original.actor_id());
    assert_eq!(restored.sequence(), original.sequence());
    assert_eq!(restored.data, original.data);
}

#[tokio::test]
async fn test_e2e_state_with_observability() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let obs = Observability::new();

    let actor_id = "obs-state-actor";

    obs.record_actor_start(actor_id, 75);

    let state = b"observable-state".to_vec();
    let start = std::time::Instant::now();
    manager
        .checkpoint(actor_id, state.clone())
        .await
        .expect("Failed to checkpoint");
    let checkpoint_time = start.elapsed().as_micros() as u64;

    obs.record_message_processed(checkpoint_time);

    let restored = manager
        .restore(actor_id)
        .await
        .expect("Failed to restore")
        .expect("No state");

    assert_eq!(restored, state);
    assert!(obs.metrics().messages_total() >= 1);

    obs.record_actor_stop();
    assert_eq!(obs.metrics().actors_running(), 0);
}

#[tokio::test]
async fn test_e2e_state_concurrent_actors() {
    let manager = Arc::new(CheckpointManager::new(InMemoryStore::new()));

    let mut handles = Vec::new();

    for i in 0..10 {
        let mgr = manager.clone();
        let handle = tokio::spawn(async move {
            let actor_id = format!("concurrent-actor-{}", i);
            let state = vec![i as u8; 100];

            mgr.checkpoint(&actor_id, state.clone())
                .await
                .expect("Failed to checkpoint");

            let restored = mgr
                .restore(&actor_id)
                .await
                .expect("Failed to restore")
                .expect("No state");

            assert_eq!(restored, state);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("Task failed");
    }
}

#[tokio::test]
async fn test_e2e_state_empty_restore() {
    let manager = CheckpointManager::new(InMemoryStore::new());

    let result = manager
        .restore("non-existent-actor")
        .await
        .expect("Restore should not error");

    assert!(
        result.is_none(),
        "Should return None for non-existent actor"
    );
}

#[tokio::test]
async fn test_e2e_state_checkpoint_metadata() {
    let store = CheckpointStore::new(InMemoryStore::new());
    let actor_id = "metadata-actor";

    let checkpoint = Checkpoint::new(actor_id, 1, vec![0x01, 0x02, 0x03, 0x04, 0x05]);
    store.save(&checkpoint).await.expect("Failed to save");

    let metadata_list = store.list(actor_id).await.expect("Failed to list");
    assert_eq!(metadata_list.len(), 1);

    let metadata = &metadata_list[0];
    assert_eq!(metadata.actor_id, actor_id);
    assert_eq!(metadata.sequence, 1);
    assert_eq!(metadata.size, 5);
    assert!(!metadata.checksum.iter().all(|&b| b == 0));
    assert_eq!(metadata.version, aether_core::state::CHECKPOINT_VERSION);
}

#[tokio::test]
async fn test_e2e_state_large_payload() {
    let manager = CheckpointManager::new(InMemoryStore::new());
    let actor_id = "large-payload-actor";

    let large_state = vec![0xAB; 1024 * 1024];

    manager
        .checkpoint(actor_id, large_state.clone())
        .await
        .expect("Failed to checkpoint large state");

    let restored = manager
        .restore(actor_id)
        .await
        .expect("Failed to restore")
        .expect("No state");

    assert_eq!(restored.len(), large_state.len());
    assert_eq!(restored, large_state);
}

#[tokio::test]
async fn test_e2e_state_checkpoint_storage_key() {
    let checkpoint = Checkpoint::new("test-actor", 42, vec![1, 2, 3]);
    let key = checkpoint.storage_key();

    assert!(key.starts_with(aether_core::state::CHECKPOINT_PREFIX));

    let (actor_id, sequence) =
        Checkpoint::parse_storage_key(&key).expect("Failed to parse storage key");

    assert_eq!(actor_id, "test-actor");
    assert_eq!(sequence, 42);
}
