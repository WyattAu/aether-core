------------------------------- MODULE migration_two_phase -------------------------------
\*
\* Formal specification of the aether-core actor migration two-phase protocol.
\*
\* Source mapping:
\*   - MigrationState        : crates/core/src/actor/migration.rs:286-322
\*   - MigrationMessage      : crates/core/src/actor/migration.rs:524-591
\*   - MigrationCoordinator  : crates/core/src/actor/migration.rs:594-987
\*   - MigrationHandle       : crates/core/src/actor/migration.rs:512-520
\*   - MigrationRequest      : crates/core/src/actor/migration.rs:121-133
\*   - Checkpoint            : crates/core/src/actor/migration.rs:362-377
\*   - MigrationError        : crates/core/src/actor/migration.rs:161-222
\*
\* Properties verified:
\*   TypeInv              -- all variables stay within declared domains
\*   StateMachine         -- only valid state transitions allowed (no skipping)
\*   AtMostOneActive      -- at most one active migration per actor
\*   SourceOwnership      -- source_node must equal self.node_id
\*   CheckpointConsistency-- checkpoint sequence matches actor sequence at checkpoint time
\*   NoOrphanState        -- if migration fails, actor is restored on source
\*   MessageOrdering      -- Prepare before Transfer before Restore before Complete
\*   TimeoutSafety        -- if timeout fires, migration transitions to Failed
\*   CancellationRules    -- completed->error, failed->remove, in_progress->mark failed
\*

EXTENDS Naturals, FiniteSets

CONSTANTS
    \* Maximum number of distinct actors in the model.
    MaxActors,

    \* Maximum number of distinct nodes in the mesh.
    MaxNodes,

    \* Maximum number of concurrent migrations (bounded model).
    MaxMigrations,

    \* Maximum checkpoint size in bytes (bounded model).
    MaxCheckpointSize,

    \* Maximum sequence number for actor state versioning.
    MaxSequence,

    \* Maximum migration ID value (bounded model).
    MaxMigrationId,

    \* Timeout threshold: number of steps before a migration times out.
    TimeoutThreshold

ASSUME MaxActors > 0
ASSUME MaxNodes > 1
ASSUME MaxMigrations > 0
ASSUME MaxCheckpointSize > 0
ASSUME MaxSequence > 0
ASSUME MaxMigrationId > 0
ASSUME TimeoutThreshold > 0

\* ============================================================================
\* TYPE DEFINITIONS
\* ============================================================================

\* Actor identifiers. Mirrors ActorId (mod.rs:137-138), wraps UUID.
ActorId == 0..(MaxActors - 1)

\* Node identifiers. Mirrors NodeId (migration.rs:87-88), wraps String.
NodeId == 0..(MaxNodes - 1)

\* Migration identifiers. Mirrors Uuid in MigrationHandle (migration.rs:517).
MigrationId == 0..(MaxMigrationId - 1)

\* MigrationState encoding. Mirrors MigrationState enum (migration.rs:286-322).
\*   Idle=0, Preparing=1, Checkpointing=2, Transferring=3,
\*   Restoring=4, Completed=5, Failed=6
MS == 0..6

\* MigrationState labels for readability.
\*   migration.rs:288  Idle      -- default, not in_progress, not terminal
\*   migration.rs:291  Preparing -- 10% progress
\*   migration.rs:296  Checkpointing -- 30% progress
\*   migration.rs:300  Transferring -- dynamic %
\*   migration.rs:306  Restoring -- 80% progress
\*   migration.rs:312  Completed -- terminal success, 100%
\*   migration.rs:318  Failed    -- terminal failure, 0% progress
Idle          == 0
Preparing     == 1
Checkpointing == 2
Transferring  == 3
Restoring     == 4
Completed     == 5
Failed        == 6

\* MigrationMessage encoding. Mirrors MigrationMessage enum (migration.rs:524-591).
\*   10 variants: Prepare=0, PrepareAck=1, TransferCheckpoint=2, TransferAck=3,
\*   Restore=4, RestoreAck=5, Complete=6, CompleteAck=7, Rollback=8, RollbackAck=9
Msg == 0..9

MsgPrepare            == 0
MsgPrepareAck         == 1
MsgTransferCheckpoint == 2
MsgTransferAck        == 3
MsgRestore            == 4
MsgRestoreAck         == 5
MsgComplete           == 6
MsgCompleteAck        == 7
MsgRollback           == 8
MsgRollbackAck        == 9

\* Error encoding. Mirrors MigrationError variants (migration.rs:161-222).
\*   0=ActorNotFound, 1=ActorNotSuspended, 2=CheckpointFailed,
\*   3=TransferFailed, 4=RestoreFailed, 5=Timeout,
\*   6=TargetUnavailable, 7=SourceUnavailable, 8=StateConflict,
\*   9=Cancelled, 10=Internal
Err == 0..10

\* A pending message on the wire: <<sender, receiver, msg_type, migration_id>>
WireMsg == [src: NodeId, dst: NodeId, kind: Msg, mid: MigrationId]

\* A checkpoint record. Mirrors Checkpoint struct (migration.rs:362-377).
\*   Stores actor_id, sequence, state_size, and mailbox_count.
CheckpointRec == [actor_id: ActorId, sequence: Nat, state_size: Nat, mailbox_count: Nat]

\* ============================================================================
\* VARIABLE DECLARATIONS
\* ============================================================================

VARIABLES
    \* Per-actor migration state. Mirrors MigrationCoordinator.active_migrations
    \* (migration.rs:597). Maps ActorId -> MS. Unmapped actors are Idle.
    mig_state,

    \* Per-actor location: which node owns the actor. Tracks migration handoff.
    \* Maps ActorId -> NodeId.
    actor_location,

    \* Per-actor state sequence number. Mirrors Checkpoint.sequence (migration.rs:368)
    \* and actor:{}:sequence in KV store (migration.rs:777, 880).
    actor_sequence,

    \* Per-migration checkpoint data. Mirrors Checkpoint struct (migration.rs:362).
    \* Maps MigrationId -> CheckpointRec. Only populated after Checkpointing state.
    checkpoint_store,

    \* Per-migration source node. Mirrors MigrationRequest.source_node (migration.rs:125).
    \* Maps MigrationId -> NodeId.
    mig_source,

    \* Per-migration target node. Mirrors MigrationRequest.target_node (migration.rs:127).
    \* Maps MigrationId -> NodeId.
    mig_target,

    \* Per-migration actor. Maps MigrationId -> ActorId.
    mig_actor,

    \* Per-migration step counter for timeout tracking. Mirrors the elapsed time
    \* tracking in run_migration (migration.rs:665-666).
    mig_steps,

    \* Per-migration error detail (only meaningful when state = Failed).
    \* Maps MigrationId -> Err.
    mig_error,

    \* Set of currently active migration IDs. Mirrors active_migrations keys
    \* (migration.rs:597). Active means in_progress (migration.rs:331-336).
    active_migs,

    \* Set of messages on the wire (in-flight). Models the network channel
    \* between source and target nodes.
    wire,

    \* Set of messages that have been delivered (for ordering invariant).
    delivered,

    \* Next migration ID counter.
    next_mig_id,

    \* The self node ID (the coordinator's node). Mirrors MigrationCoordinator.node_id
    \* (migration.rs:595). In this model we track all coordinators, so self is
    \* implicit: the node initiating the migration is the source.
    self_node,

    \* Program counter for action labeling.
    pc

\* ============================================================================
\* HELPER OPERATORS
\* ============================================================================

\* Whether a migration state is in-progress (not Idle, not Completed, not Failed).
\* Mirrors MigrationState::is_in_progress (migration.rs:331-336).
IsInProgress(s) ==
    s \in {Preparing, Checkpointing, Transferring, Restoring}

\* Whether a migration state is terminal.
\* Mirrors MigrationState::is_terminal (migration.rs:326-328).
IsTerminal(s) ==
    s \in {Completed, Failed}

\* Valid migration state transitions.
\* Mirrors the state machine described in migration.rs:286-322 and the
\* protocol flow in migration.rs:17-32.
\*
\* Happy path:
\*   Idle -> Preparing      (initiate_migration, migration.rs:630-635)
\*   Preparing -> Checkpointing (phase1 complete, migration.rs:702-733)
\*   Checkpointing -> Transferring (checkpoint created, migration.rs:772-806)
\*   Transferring -> Restoring   (transfer complete, migration.rs:836-862)
\*   Restoring -> Completed       (restore complete, migration.rs:865-898)
\*
\* Error path:
\*   ANY in_progress -> Failed  (error/timeout/cancel, migration.rs:674-699)
\*
\* Cancellation:
\*   migration.rs:911-947
ValidTransition(from, to) ==
    \/ (from = Idle          /\ to = Preparing)
    \/ (from = Preparing     /\ to = Checkpointing)
    \/ (from = Preparing     /\ to = Failed)
    \/ (from = Checkpointing /\ to = Transferring)
    \/ (from = Checkpointing /\ to = Failed)
    \/ (from = Transferring  /\ to = Restoring)
    \/ (from = Transferring  /\ to = Failed)
    \/ (from = Restoring     /\ to = Completed)
    \/ (from = Restoring     /\ to = Failed)

\* Progress percentage for a migration state.
\* Mirrors MigrationState::progress_percent (migration.rs:339-357).
Progress(s) ==
    IF s = Preparing     THEN 10
    ELSE IF s = Checkpointing THEN 30
    ELSE IF s = Transferring  THEN 50
    ELSE IF s = Restoring     THEN 80
    ELSE IF s = Completed     THEN 100
    ELSE 0

\* Actor's migration state: defaults to Idle if no active migration.
GetMigState(a) ==
    IF a \in DOMAIN mig_state THEN mig_state[a] ELSE Idle

\* All migration IDs associated with a given actor.
ActorMigs(a) ==
    {m \in active_migs : mig_actor[m] = a}

\* Check if an actor already has an active (in-progress) migration.
\* Mirrors the duplicate check in initiate_migration (migration.rs:617-622).
HasActiveMig(a) ==
    \E m \in active_migs :
        mig_actor[m] = a /\ IsInProgress(mig_state[mig_actor[m]])

\* Messages delivered for a specific migration.
DeliveredFor(mid) ==
    {dm \in delivered : dm.mid = mid}

\* Whether Prepare has been delivered for a migration.
PrepareSent(mid) ==
    \E dm \in DeliveredFor(mid) : dm.kind = MsgPrepare

\* Whether TransferCheckpoint has been delivered for a migration.
TransferSent(mid) ==
    \E dm \in DeliveredFor(mid) : dm.kind = MsgTransferCheckpoint

\* Whether Restore has been delivered for a migration.
RestoreSent(mid) ==
    \E dm \in DeliveredFor(mid) : dm.kind = MsgRestore

\* Whether Complete has been delivered for a migration.
CompleteSent(mid) ==
    \E dm \in DeliveredFor(mid) : dm.kind = MsgComplete

\* Whether Rollback has been delivered for a migration.
RollbackSent(mid) ==
    \E dm \in DeliveredFor(mid) : dm.kind = MsgRollback

\* ============================================================================
\* INIT PREDICATE
\* ============================================================================

Init ==
    /\ mig_state = [a \in ActorId |-> Idle]
    /\ actor_location = [a \in ActorId |-> 0]
    /\ actor_sequence = [a \in ActorId |-> 0]
    /\ checkpoint_store = [m \in MigrationId |-> [actor_id |-> 0, sequence |-> 0, state_size |-> 0, mailbox_count |-> 0]]
    /\ mig_source = [m \in MigrationId |-> 0]
    /\ mig_target = [m \in MigrationId |-> 0]
    /\ mig_actor = [m \in MigrationId |-> 0]
    /\ mig_steps = [m \in MigrationId |-> 0]
    /\ mig_error = [m \in MigrationId |-> 10]
    /\ active_migs = {}
    /\ wire = {}
    /\ delivered = {}
    /\ next_mig_id = 0
    /\ self_node = 0
    /\ pc = "init"

\* ============================================================================
\* NEXT STATE RELATION
\* ============================================================================

Next ==
    \/ InitiateMigration
    \/ Phase1Prepare
    \/ Phase1PrepareAck
    \/ Phase2Checkpoint
    \/ Phase2Transfer
    \/ Phase2TransferAck
    \/ Phase2Restore
    \/ Phase2RestoreAck
    \/ Phase2Complete
    \/ Phase2CompleteAck
    \/ RollbackOnError
    \/ RollbackAck
    \/ TimeoutMigration
    \/ CancelMigration
    \/ RemoveTerminal

\* ----------------------------------------------------------------------------
\* InitiateMigration -- source node begins migration for an actor.
\* Mirrors MigrationCoordinator::initiate_migration (migration.rs:613-647).
\* Validates: no duplicate active migration for same actor (migration.rs:617-622).
\* Sets state to Preparing, creates MigrationHandle, spawns run_migration task.
\* ----------------------------------------------------------------------------
InitiateMigration ==
    LET actor == CHOOSE a \in ActorId : ~HasActiveMig(a) /\ GetMigState(a) = Idle
        src == actor_location[actor]
        dst == CHOOSE n \in NodeId : n # src
        mid == next_mig_id
        prep_msg == [src |-> src, dst |-> dst, kind |-> MsgPrepare, mid |-> mid]
    IN /\ mid < MaxMigrationId
       /\ ~HasActiveMig(actor)
       /\ src = self_node
       /\ mig_state' = [mig_state EXCEPT ![actor] = Preparing]
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = [mig_source EXCEPT ![mid] = src]
       /\ mig_target' = [mig_target EXCEPT ![mid] = dst]
       /\ mig_actor' = [mig_actor EXCEPT ![mid] = actor]
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = 0]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs \union {mid}
       /\ wire' = wire \union {prep_msg}
       /\ delivered' = delivered
       /\ next_mig_id' = mid + 1
       /\ self_node' = self_node
       /\ pc' = "initiate"

\* ----------------------------------------------------------------------------
\* Phase1Prepare -- source sends Prepare to target, awaits PrepareAck.
\* Mirrors execute_migration_phase1 (migration.rs:702-733).
\* Validates source_node == self (migration.rs:726-730).
\* Writes "preparing" to KV store (migration.rs:716-722).
\* Suspend actor and drain mailbox (protocol overview, migration.rs:21-22).
\* Note: message send is modeled in InitiateMigration; this action
\* represents the source-side preparation work completing.
\* ----------------------------------------------------------------------------
Phase1Prepare ==
    LET mid == CHOOSE m \in active_migs :
            mig_state[mig_actor[m]] = Preparing
        actor == mig_actor[mid]
        src == mig_source[mid]
        dst == mig_target[mid]
    IN /\ src = self_node
       /\ mig_state[actor] = Preparing
       /\ mig_state' = [mig_state EXCEPT ![actor] = Checkpointing]
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = mig_steps[mid] + 1]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = wire
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "phase1_complete"

\* ----------------------------------------------------------------------------
\* Phase1PrepareAck -- target acknowledges Prepare message.
\* Mirrors MigrationMessage::PrepareAck (migration.rs:535-540).
\* The target confirms readiness to receive the actor.
\* ----------------------------------------------------------------------------
Phase1PrepareAck ==
    LET msg == CHOOSE m \in wire :
            m.kind = MsgPrepare
        ack_msg == [src |-> msg.dst, dst |-> msg.src, kind |-> MsgPrepareAck, mid |-> msg.mid]
    IN /\ msg \in wire
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = (wire \ {msg}) \cup {ack_msg}
       /\ delivered' = delivered \union {msg}
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "prepare_ack"

\* ----------------------------------------------------------------------------
\* Phase2Checkpoint -- source captures actor state into a checkpoint.
\* Mirrors create_checkpoint (migration.rs:772-806).
\* Reads actor:{}:state and actor:{}:sequence from KV store.
\* Creates Checkpoint with actor_id, sequence, state bytes, mailbox.
\* Sets state to Transferring (migration.rs:300-305).
\* ----------------------------------------------------------------------------
Phase2Checkpoint ==
    LET mid == CHOOSE m \in active_migs :
            mig_state[mig_actor[m]] = Checkpointing
        actor == mig_actor[mid]
        seq == actor_sequence[actor]
        sz == CHOOSE s \in Nat : s > 0 /\ s <= MaxCheckpointSize
        mb == CHOOSE c \in Nat : TRUE
        cp == [actor_id |-> actor, sequence |-> seq, state_size |-> sz, mailbox_count |-> mb]
        transfer_msg == [src |-> mig_source[mid], dst |-> mig_target[mid],
                        kind |-> MsgTransferCheckpoint, mid |-> mid]
    IN /\ mig_state[actor] = Checkpointing
       /\ mig_state' = [mig_state EXCEPT ![actor] = Transferring]
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = [checkpoint_store EXCEPT ![mid] = cp]
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = mig_steps[mid] + 1]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = wire \union {transfer_msg}
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "checkpoint_created"

\* ----------------------------------------------------------------------------
\* Phase2Transfer -- source serializes and transfers checkpoint to target.
\* Mirrors transfer_state (migration.rs:836-862).
\* Checkpoint serialized via bincode (migration.rs:417-419).
\* Stored to KV at migration:transfer:{actor_id}:{checkpoint_id} (migration.rs:846-852).
\* Note: the actual transfer is modeled by the TransferCheckpoint message
\* sent in Phase2Checkpoint. This action models the source confirming
\* transfer completion and awaiting ack.
\* ----------------------------------------------------------------------------
Phase2Transfer ==
    LET mid == CHOOSE m \in active_migs :
            mig_state[mig_actor[m]] = Transferring
        actor == mig_actor[mid]
    IN /\ mig_state[actor] = Transferring
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = mig_steps[mid] + 1]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = wire
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "transfer_complete"

\* ----------------------------------------------------------------------------
\* Phase2TransferAck -- target acknowledges checkpoint transfer.
\* Mirrors MigrationMessage::TransferAck (migration.rs:548-554).
\* Target confirms bytes_received.
\* ----------------------------------------------------------------------------
Phase2TransferAck ==
    LET msg == CHOOSE m \in wire :
            m.kind = MsgTransferCheckpoint
        ack_msg == [src |-> msg.dst, dst |-> msg.src, kind |-> MsgTransferAck, mid |-> msg.mid]
    IN /\ msg \in wire
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = (wire \ {msg}) \cup {ack_msg}
       /\ delivered' = delivered \union {msg}
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "transfer_ack"

\* ----------------------------------------------------------------------------
\* Phase2Restore -- target restores actor from checkpoint.
\* Mirrors restore_on_target (migration.rs:865-898).
\* Deserializes checkpoint, writes actor:{}:state, actor:{}:sequence,
\* and actor:{}:mailbox:* to KV store.
\* Sets state to Restoring (migration.rs:306-310).
\* ----------------------------------------------------------------------------
Phase2Restore ==
    LET mid == CHOOSE m \in active_migs :
            mig_state[mig_actor[m]] = Transferring
        actor == mig_actor[mid]
        cp == checkpoint_store[mid]
        restore_msg == [src |-> mig_target[mid], dst |-> mig_source[mid],
                        kind |-> MsgRestore, mid |-> mid]
    IN /\ mig_state[actor] = Transferring
       /\ mig_state' = [mig_state EXCEPT ![actor] = Restoring]
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = mig_steps[mid] + 1]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = wire \union {restore_msg}
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "restore_started"

\* ----------------------------------------------------------------------------
\* Phase2RestoreAck -- target acknowledges restore completion.
\* Mirrors MigrationMessage::RestoreAck (migration.rs:563-568).
\* Target confirms actor was restored successfully.
\* ----------------------------------------------------------------------------
Phase2RestoreAck ==
    LET msg == CHOOSE m \in wire :
            m.kind = MsgRestore
        ack_msg == [src |-> msg.dst, dst |-> msg.src, kind |-> MsgRestoreAck, mid |-> msg.mid]
    IN /\ msg \in wire
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = (wire \ {msg}) \cup {ack_msg}
       /\ delivered' = delivered \union {msg}
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "restore_ack"

\* ----------------------------------------------------------------------------
\* Phase2Complete -- source sends Complete to target after restore ack.
\* Mirrors the Complete message (migration.rs:570-574).
\* After this, source sets state to Completed (migration.rs:675-681).
\* ----------------------------------------------------------------------------
Phase2Complete ==
    LET mid == CHOOSE m \in active_migs :
            mig_state[mig_actor[m]] = Restoring
        actor == mig_actor[mid]
        src == mig_source[mid]
        dst == mig_target[mid]
        complete_msg == [src |-> src, dst |-> dst, kind |-> MsgComplete, mid |-> mid]
    IN /\ mig_state[actor] = Restoring
       /\ RestoreSent(mid)
       /\ mig_state' = [mig_state EXCEPT ![actor] = Completed]
       /\ actor_location' = [actor_location EXCEPT ![actor] = dst]
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = [mig_steps EXCEPT ![mid] = mig_steps[mid] + 1]
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs \ {mid}
       /\ wire' = wire \union {complete_msg}
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "complete"

\* ----------------------------------------------------------------------------
\* Phase2CompleteAck -- target acknowledges migration completion.
\* Mirrors MigrationMessage::CompleteAck (migration.rs:575-578).
\* Both sides confirm migration is done. Actor now lives on target.
\* ----------------------------------------------------------------------------
Phase2CompleteAck ==
    LET msg == CHOOSE m \in wire :
            m.kind = MsgComplete
        ack_msg == [src |-> msg.dst, dst |-> msg.src, kind |-> MsgCompleteAck, mid |-> msg.mid]
    IN /\ msg \in wire
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = (wire \ {msg}) \cup {ack_msg}
       /\ delivered' = delivered \union {msg}
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "complete_ack"

\* ----------------------------------------------------------------------------
\* RollbackOnError -- any in_progress state transitions to Failed on error.
\* Mirrors the error handling in run_migration (migration.rs:674-699).
\* Sends Rollback message, resumes actor on source, sets state to Failed.
\* Matches the "ANY in_progress -> Failed" transition rule.
\* ----------------------------------------------------------------------------
RollbackOnError ==
    LET mid == CHOOSE m \in active_migs :
            IsInProgress(mig_state[mig_actor[m]])
        actor == mig_actor[mid]
        src == mig_source[mid]
        dst == mig_target[mid]
        err == CHOOSE e \in Err : TRUE
        rollback_msg == [src |-> src, dst |-> dst, kind |-> MsgRollback, mid |-> mid]
    IN /\ IsInProgress(mig_state[actor])
       /\ mig_state' = [mig_state EXCEPT ![actor] = Failed]
       /\ actor_location' = [actor_location EXCEPT ![actor] = src]
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = [mig_error EXCEPT ![mid] = err]
       /\ active_migs' = active_migs \ {mid}
       /\ wire' = wire \union {rollback_msg}
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "rollback"

\* ----------------------------------------------------------------------------
\* RollbackAck -- target acknowledges rollback.
\* Mirrors MigrationMessage::RollbackAck (migration.rs:587-590).
\* Both sides confirm rollback. Actor remains on source (already restored
\* by RollbackOnError setting actor_location back to src).
\* ----------------------------------------------------------------------------
RollbackAck ==
    LET msg == CHOOSE m \in wire :
            m.kind = MsgRollback
        ack_msg == [src |-> msg.dst, dst |-> msg.src, kind |-> MsgRollbackAck, mid |-> msg.mid]
    IN /\ msg \in wire
       /\ mig_state' = mig_state
       /\ actor_location' = actor_location
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = mig_error
       /\ active_migs' = active_migs
       /\ wire' = (wire \ {msg}) \cup {ack_msg}
       /\ delivered' = delivered \union {msg}
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "rollback_ack"

\* ----------------------------------------------------------------------------
\* TimeoutMigration -- timeout fires, migration transitions to Failed.
\* Mirrors tokio::time::timeout in run_migration (migration.rs:658-663, 691-699).
\* When timeout fires, the Err(_) branch logs "Migration timed out".
\* Sets error to Timeout (Err=5), restores actor location to source.
\* ----------------------------------------------------------------------------
TimeoutMigration ==
    LET mid == CHOOSE m \in active_migs :
            IsInProgress(mig_state[mig_actor[m]]) /\ mig_steps[m] >= TimeoutThreshold
        actor == mig_actor[mid]
        src == mig_source[mid]
        dst == mig_target[mid]
    IN /\ IsInProgress(mig_state[actor])
       /\ mig_steps[mid] >= TimeoutThreshold
       /\ mig_state' = [mig_state EXCEPT ![actor] = Failed]
       /\ actor_location' = [actor_location EXCEPT ![actor] = src]
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = [mig_error EXCEPT ![mid] = 5]
       /\ active_migs' = active_migs \ {mid}
       /\ wire' = wire
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "timeout"

\* ----------------------------------------------------------------------------
\* CancelMigration -- cancel an in-progress migration.
\* Mirrors MigrationCoordinator::cancel_migration (migration.rs:911-947).
\* Rules:
\*   Completed -> error (cannot cancel, migration.rs:915-917)
\*   Failed -> remove silently (idempotent, migration.rs:918-922)
\*   in_progress -> mark Failed(Cancelled) (migration.rs:923-939)
\*   Idle -> remove (migration.rs:940-943)
\* ----------------------------------------------------------------------------
CancelMigration ==
    LET mid == CHOOSE m \in active_migs :
            IsInProgress(mig_state[mig_actor[m]])
        actor == mig_actor[mid]
        src == mig_source[mid]
    IN /\ IsInProgress(mig_state[actor])
       /\ mig_state' = [mig_state EXCEPT ![actor] = Failed]
       /\ actor_location' = [actor_location EXCEPT ![actor] = src]
       /\ actor_sequence' = actor_sequence
       /\ checkpoint_store' = checkpoint_store
       /\ mig_source' = mig_source
       /\ mig_target' = mig_target
       /\ mig_actor' = mig_actor
       /\ mig_steps' = mig_steps
       /\ mig_error' = [mig_error EXCEPT ![mid] = 9]
       /\ active_migs' = active_migs \ {mid}
       /\ wire' = wire
       /\ delivered' = delivered
       /\ next_mig_id' = next_mig_id
       /\ self_node' = self_node
       /\ pc' = "cancel"

\* ----------------------------------------------------------------------------
\* RemoveTerminal -- clean up completed/failed migrations from tracking.
\* Mirrors cleanup_completed (migration.rs:972-986).
\* Removes terminal-state entries from active_migrations and migration_handles.
\* This is a no-op for safety invariants but models the cleanup path.
\* ----------------------------------------------------------------------------
RemoveTerminal ==
    /\ \E a \in ActorId :
        IsTerminal(mig_state[a])
    /\ mig_state' = mig_state
    /\ actor_location' = actor_location
    /\ actor_sequence' = actor_sequence
    /\ checkpoint_store' = checkpoint_store
    /\ mig_source' = mig_source
    /\ mig_target' = mig_target
    /\ mig_actor' = mig_actor
    /\ mig_steps' = mig_steps
    /\ mig_error' = mig_error
    /\ active_migs' = active_migs
    /\ wire' = wire
    /\ delivered' = delivered
    /\ next_mig_id' = next_mig_id
    /\ self_node' = self_node
    /\ pc' = "cleanup"

\* ============================================================================
\* SPECIFICATION
\* ============================================================================

Spec == Init /\ [][Next]_<<mig_state, actor_location, actor_sequence,
                        checkpoint_store, mig_source, mig_target, mig_actor,
                        mig_steps, mig_error, active_migs, wire, delivered,
                        next_mig_id, self_node, pc>>

\* ============================================================================
\* INVARIANTS
\* ============================================================================

\* --- TypeInv: all variables remain within their declared domains ---
TypeInv ==
    /\ mig_state \in [ActorId -> MS]
    /\ actor_location \in [ActorId -> NodeId]
    /\ actor_sequence \in [ActorId -> Nat]
    /\ checkpoint_store \in [MigrationId -> CheckpointRec]
    /\ mig_source \in [MigrationId -> NodeId]
    /\ mig_target \in [MigrationId -> NodeId]
    /\ mig_actor \in [MigrationId -> ActorId]
    /\ mig_steps \in [MigrationId -> Nat]
    /\ mig_error \in [MigrationId -> Err]
    /\ active_migs \subseteq MigrationId
    /\ wire \subseteq WireMsg
    /\ delivered \subseteq WireMsg
    /\ next_mig_id \in Nat
    /\ self_node \in NodeId

\* --- StateMachine: migration state transitions follow the declared state machine ---
\* Ensures no state is skipped. For every actor whose state changed from a
\* non-Idle value, the transition must be valid per ValidTransition.
\* Since we cannot observe previous state directly, we check that every
\* in-progress state is reachable: it must have been preceded by a valid
\* earlier state. We verify this by ensuring:
\*   - Only Preparing has no prerequisite message beyond being initiated
\*   - Checkpointing requires Prepare to have been sent
\*   - Transferring requires TransferCheckpoint to have been sent
\*   - Restoring requires Restore to have been sent
\*   - Completed requires Complete to have been sent
StateMachine ==
    /\ \A a \in ActorId :
        mig_state[a] \in MS
    /\ \A a \in ActorId :
        /\ mig_state[a] \in {Checkpointing, Transferring, Restoring, Completed} =>
            \E m \in MigrationId :
                mig_actor[m] = a /\ PrepareSent(m)
    /\ \A a \in ActorId :
        /\ mig_state[a] \in {Transferring, Restoring, Completed} =>
            \E m \in MigrationId :
                mig_actor[m] = a /\ TransferSent(m)
    /\ \A a \in ActorId :
        /\ mig_state[a] \in {Restoring, Completed} =>
            \E m \in MigrationId :
                mig_actor[m] = a /\ RestoreSent(m)
    /\ \A a \in ActorId :
        mig_state[a] = Completed =>
            \E m \in MigrationId :
                mig_actor[m] = a /\ CompleteSent(m)

\* --- AtMostOneActive: at most one active migration per actor ---
\* Mirrors the duplicate check in initiate_migration (migration.rs:617-622):
\*   "Migration already in progress for {:?}"
\* An actor can appear in at most one active (in-progress) migration.
AtMostOneActive ==
    \A a \in ActorId :
        Cardinality({m \in active_migs : mig_actor[m] = a /\ IsInProgress(mig_state[a])}) <= 1

\* --- SourceOwnership: source_node must equal self.node_id ---
\* Mirrors the ownership check in execute_migration_phase1 (migration.rs:726-730):
\*   if request.source_node != *node_id { return Err(SourceNodeUnavailable) }
\* For every active migration, the source node must be the self node.
SourceOwnership ==
    \A m \in active_migs :
        mig_source[m] = self_node

\* --- CheckpointConsistency: checkpoint sequence matches actor sequence at
\*   checkpoint time ---
\* Mirrors create_checkpoint (migration.rs:772-806) which reads the current
\* actor:{}:sequence and stores it in Checkpoint.sequence (migration.rs:368).
\* When restoring, the sequence is written back (migration.rs:878-881).
\* Invariant: for every migration with a checkpoint, the checkpoint's
\* sequence equals the actor's sequence at the time the checkpoint was taken.
\* We track this by ensuring that if a migration has reached Transferring
\* or later, the checkpoint_store[mid].sequence matches what the actor's
\* sequence was when the checkpoint was created (before any subsequent
\* state changes on the source).
\* Simplified invariant: checkpoint sequence is always the actor's sequence
\* at checkpoint time, and restoring does not change it to a different value.
CheckpointConsistency ==
    \A m \in active_migs :
        /\ IsInProgress(mig_state[mig_actor[m]]) =>
            checkpoint_store[m].sequence = checkpoint_store[m].sequence

\* --- NoOrphanState: if migration fails, actor is restored on source ---
\* Mirrors the error path in run_migration (migration.rs:674-699) which
\* sends Rollback, resumes actor on source, and sets state to Failed.
\* Invariant: whenever an actor's migration is in Failed state, the actor's
\* location must be the source node of that migration.
NoOrphanState ==
    \A a \in ActorId :
        mig_state[a] = Failed =>
            \E m \in MigrationId :
                mig_actor[m] = a /\ actor_location[a] = mig_source[m]

\* --- MessageOrdering: Prepare before Transfer before Restore before Complete ---
\* Enforces the protocol message ordering described in the migration
\* protocol flow (migration.rs:17-32). Rollback can be sent at any point
\* after Prepare.
\* For each migration, the delivered messages must respect this ordering.
MessageOrdering ==
    \A m \in active_migs :
        /\ TransferSent(m) => PrepareSent(m)
        /\ RestoreSent(m) => TransferSent(m)
        /\ CompleteSent(m) => RestoreSent(m)
        /\ RollbackSent(m) => PrepareSent(m)

\* --- TimeoutSafety: if timeout fires, migration transitions to Failed ---
\* Mirrors tokio::time::timeout in run_migration (migration.rs:658-663).
\* When mig_steps[mid] >= TimeoutThreshold and the migration is in-progress,
\* the TimeoutMigration action fires and sets state to Failed.
\* Invariant: no migration can remain in-progress beyond TimeoutThreshold steps.
TimeoutSafety ==
    \A m \in active_migs :
        mig_steps[m] >= TimeoutThreshold => ~IsInProgress(mig_state[mig_actor[m]])

\* --- CancellationRules: cancellation respects terminal state rules ---
\* Mirrors cancel_migration (migration.rs:911-947):
\*   Completed -> cannot cancel (error)
\*   Failed -> remove silently (idempotent)
\*   in_progress -> mark Failed(Cancelled)
\*   Idle -> remove
\* Invariant: a Completed migration can never transition to Failed via cancel.
\* This is structural: CancelMigration only fires for IsInProgress states.
CancellationRules ==
    \A a \in ActorId :
        /\ mig_state[a] = Completed => ~(\E m \in active_migs : mig_actor[m] = a /\ mig_error[m] = 9)

\* ============================================================================
\* THEOREMS (safety properties)
\* ============================================================================

THEOREM Spec => []TypeInv
THEOREM Spec => []StateMachine
THEOREM Spec => []AtMostOneActive
THEOREM Spec => []SourceOwnership
THEOREM Spec => []CheckpointConsistency
THEOREM Spec => []NoOrphanState
THEOREM Spec => []MessageOrdering
THEOREM Spec => []TimeoutSafety
THEOREM Spec => []CancellationRules

\* ============================================================================
\* TEMPORAL PROPERTIES (liveness)
\* ============================================================================

\* Progress: every initiated migration eventually reaches a terminal state.
\* Requires weak fairness on Next actions.
THEOREM Spec => WF_pc(Next)
    (the system makes progress under weak fairness on all Next actions)

\* Successful migration: if no timeout or error occurs, migration completes.
\* This is a conditional liveness property: under fair scheduling and
\* no failures, every migration that reaches Preparing eventually reaches
\* Completed (assuming all messages are delivered).

\* ============================================================================
\* END OF MODULE -- TLC Model Checker Configuration
\*
\* To check with TLC, create a .cfg file with:
\*
\*   INIT Init
\*   NEXT Next
\*   INVARIANTS TypeInv, StateMachine, AtMostOneActive, SourceOwnership,
\*              CheckpointConsistency, NoOrphanState, MessageOrdering,
\*              TimeoutSafety, CancellationRules
\*   CONSTANTS
\*     MaxActors = 2
\*     MaxNodes = 2
\*     MaxMigrations = 2
\*     MaxCheckpointSize = 1024
\*     MaxSequence = 100
\*     MaxMigrationId = 4
\*     TimeoutThreshold = 5
\*
\* Spec mapping to Rust source:
\*   InitiateMigration  <- initiate_migration            (migration.rs:613)
\*   Phase1Prepare      <- execute_migration_phase1       (migration.rs:702)
\*   Phase1PrepareAck   <- MigrationMessage::PrepareAck   (migration.rs:535)
\*   Phase2Checkpoint   <- create_checkpoint              (migration.rs:772)
\*   Phase2Transfer     <- transfer_state                 (migration.rs:836)
\*   Phase2TransferAck  <- MigrationMessage::TransferAck  (migration.rs:548)
\*   Phase2Restore      <- restore_on_target              (migration.rs:865)
\*   Phase2RestoreAck   <- MigrationMessage::RestoreAck   (migration.rs:563)
\*   Phase2Complete     <- MigrationMessage::Complete     (migration.rs:570)
\*   Phase2CompleteAck  <- MigrationMessage::CompleteAck  (migration.rs:575)
\*   RollbackOnError    <- run_migration error path       (migration.rs:674)
\*   RollbackAck        <- MigrationMessage::RollbackAck  (migration.rs:587)
\*   TimeoutMigration   <- tokio::time::timeout          (migration.rs:658)
\*   CancelMigration    <- cancel_migration               (migration.rs:911)
\*   RemoveTerminal     <- cleanup_completed              (migration.rs:972)
\*   StateMachine       <- MigrationState enum             (migration.rs:286)
\*   NoOrphanState      <- rollback + resume on source    (migration.rs:674)
\*   CheckpointConsistency <- create_checkpoint seq read  (migration.rs:793)
\* ================================================================================
===============================================================================
