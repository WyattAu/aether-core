"""
Distributed Workflow Orchestrator Example

Demonstrates:
- State machine for document approval workflow
- Saga pattern for multi-step post-approval processing
- Human task integration (review step)
- Event sourcing for full audit trail

Workflow:
    Draft -> Pending Review -> [Human Approval] -> Approved -> [Saga] -> Completed
                              -> [Human Rejection]  -> Rejected
    Saga steps: Notify Team -> Archive Document -> Send Confirmation
    If saga fails -> Compensation: Undo each completed step in reverse
"""

import asyncio
import uuid
from datetime import datetime

from aether_sdk.workflow.human_task import HumanTask, HumanTaskManager, TaskForm
from aether_sdk.workflow.saga import Saga, SagaExecutor
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
from aether_sdk.workflow.types import Duration, RetryConfig, RetryPolicy, SagaStatus

AUDIT_LOG = []


def log_event(msg: str):
    ts = datetime.utcnow().strftime("%H:%M:%S.%f")[:-3]
    AUDIT_LOG.append(f"[{ts}] {msg}")
    print(f"  [{ts}] {msg}")


async def run_approval_flow(simulate_saga_failure: bool = False):
    print("=" * 70)
    print("  AETHER WORKFLOW ORCHESTRATOR - Document Approval Example")
    print("=" * 70)
    print()

    human_task_manager = HumanTaskManager()

    print("--- Step 1: Define the approval state machine ---")
    approval_workflow = (
        Workflow("document-approval")
        .state("draft", is_initial=True)
        .state("pending_review")
        .state("approved")
        .state("rejected", is_final=True)
        .state("completed", is_final=True)
        .state("failed", is_final=True)
        .transition("submit", "draft", "pending_review")
        .transition("approve", "pending_review", "approved")
        .transition("reject", "pending_review", "rejected")
        .transition("saga_success", "approved", "completed")
        .transition("saga_failed", "approved", "failed")
        .on_enter(
            "pending_review",
            lambda ctx: log_event(
                f"ENTER pending_review: Document '{ctx.input['title']}' sent for review"
            ),
        )
        .on_enter(
            "approved",
            lambda ctx: log_event(
                "ENTER approved: Document has been approved by reviewer"
            ),
        )
        .on_enter(
            "rejected",
            lambda ctx: log_event("ENTER rejected: Document has been rejected"),
        )
        .on_enter(
            "completed",
            lambda ctx: log_event(
                "ENTER completed: All post-approval steps finished successfully"
            ),
        )
        .on_enter(
            "failed",
            lambda ctx: log_event(
                "ENTER failed: Post-approval saga failed, compensation applied"
            ),
        )
        .build()
    )
    print(f"  States: {list(approval_workflow.states.keys())}")
    print(f"  Initial: {approval_workflow.initial_state}")
    print()

    print("--- Step 2: Define the post-approval saga ---")

    async def notify_team(ctx):
        log_event("  [SAGA] Notifying team about approved document...")
        await asyncio.sleep(0.1)
        ctx.set_state("notification_sent", True)
        log_event("  [SAGA] Team notification sent successfully")
        return "notification_ok"

    async def undo_notify_team(ctx):
        log_event("  [COMPENSATE] Withdrawing team notification...")
        ctx.set_state("notification_sent", False)
        log_event("  [COMPENSATE] Team notification withdrawn")

    async def archive_document(ctx):
        log_event("  [SAGA] Archiving document to storage...")
        await asyncio.sleep(0.1)
        if simulate_saga_failure and not ctx.is_step_completed("archive-document"):
            raise RuntimeError("Storage service unavailable during archival")
        ctx.set_state("archived", True)
        log_event("  [SAGA] Document archived successfully")
        return "archive_ok"

    async def undo_archive_document(ctx):
        log_event("  [COMPENSATE] Un-archiving document from storage...")
        ctx.set_state("archived", False)
        log_event("  [COMPENSATE] Document un-archived")

    async def send_confirmation(ctx):
        log_event("  [SAGA] Sending confirmation to submitter...")
        await asyncio.sleep(0.1)
        ctx.set_state("confirmation_sent", True)
        log_event("  [SAGA] Confirmation sent successfully")
        return "confirmation_ok"

    async def undo_send_confirmation(ctx):
        log_event("  [COMPENSATE] Withdrawing confirmation...")
        ctx.set_state("confirmation_sent", False)
        log_event("  [COMPENSATE] Confirmation withdrawn")

    approval_saga = (
        Saga("post-approval")
        .step("notify-team")
        .action(notify_team)
        .compensate(undo_notify_team)
        .step("archive-document")
        .action(archive_document)
        .compensate(undo_archive_document)
        .retry(
            RetryConfig(
                max_attempts=2,
                policy=RetryPolicy.FIXED,
                initial_delay=Duration.from_seconds(0.1),
            )
        )
        .step("send-confirmation")
        .action(send_confirmation)
        .compensate(undo_send_confirmation)
        .build()
    )
    print(f"  Saga: {approval_saga.name} ({len(approval_saga.steps)} steps)")
    for step in approval_saga.steps:
        has_comp = step.compensate is not None
        print(f"    - {step.name} (compensation: {'yes' if has_comp else 'no'})")
    print()

    print("--- Step 3: Create the review task form ---")
    review_form = TaskForm()
    review_form.add_field("approved", "boolean", required=True, label="Approve?")
    review_form.add_field("comments", "text", label="Review Comments")
    print(f"  Form fields: {[f.name for f in review_form.fields]}")
    print()

    workflow_executor = WorkflowExecutor()
    saga_executor = SagaExecutor()

    for run_num, should_fail in enumerate([False, simulate_saga_failure], 1):
        doc_title = f"Q4 Report {run_num} {'(failure scenario)' if should_fail else ''}"
        print(f"{'=' * 70}")
        print(f"  RUN {run_num}: Document '{doc_title}'")
        print(f"{'=' * 70}")

        doc_id = str(uuid.uuid4())[:8]
        log_event(f"Submitting document '{doc_title}' (id={doc_id})")

        print("\n  >> Transition: draft -> pending_review")
        result = await workflow_executor.start(
            approval_workflow,
            {"title": doc_title, "id": doc_id},
        )
        wf_id = result.workflow_id
        log_event(f"Workflow started: {wf_id}, state={result.current_state}")

        await workflow_executor.transition(wf_id, "submit")
        status = await workflow_executor.get_status(wf_id)
        log_event(f"Current state: {status.current_state}")

        print("\n  >> Creating human review task...")
        task = (
            HumanTask(
                task_type="document_review",
                title=f"Review: {doc_title}",
                description=f"Please review document {doc_id}",
            )
            .with_assignee("reviewer@company.com")
            .with_priority(2)
            .with_form(review_form)
        )
        created_task = await human_task_manager.create_task(task, wf_id, "review-step")
        log_event(f"Review task created: {created_task.task_id}")
        log_event(f"  Assigned to: {created_task.assignee}")

        await asyncio.sleep(0.05)

        print("\n  >> Human reviewer approves the document...")
        await human_task_manager.claim_task(
            created_task.task_id, "reviewer@company.com"
        )
        await human_task_manager.complete_task(
            created_task.task_id,
            {"approved": True, "comments": "Looks good, approved."},
            user="reviewer@company.com",
        )
        log_event(f"Task completed by {created_task.completed_by}")
        log_event(f"  Result: {created_task.result}")

        print("\n  >> Transition: pending_review -> approved")
        await workflow_executor.transition(wf_id, "approve")
        status = await workflow_executor.get_status(wf_id)
        log_event(f"Current state: {status.current_state}")

        print("\n  >> Executing post-approval saga...")
        saga_result = await saga_executor.execute(
            approval_saga,
            {"title": doc_title, "id": doc_id, "workflow_id": wf_id},
        )
        log_event(f"Saga result: status={saga_result.status.value}")
        log_event(f"  Completed steps: {saga_result.completed_steps}")
        if saga_result.compensated_steps:
            log_event(f"  Compensated steps: {saga_result.compensated_steps}")
        if saga_result.error:
            log_event(f"  Error: {saga_result.error}")
        if saga_result.duration_ms:
            log_event(f"  Duration: {saga_result.duration_ms}ms")

        if saga_result.status == SagaStatus.COMPLETED:
            print("\n  >> Transition: approved -> completed")
            await workflow_executor.transition(wf_id, "saga_success")
        else:
            print("\n  >> Transition: approved -> failed")
            await workflow_executor.transition(wf_id, "saga_failed")

        final_status = await workflow_executor.get_status(wf_id)
        log_event(
            f"Final state: {final_status.current_state} (status={final_status.status.value})"
        )

        print(f"\n  >> Audit trail for workflow {wf_id}:")
        for event in final_status.history:
            detail = event.get("details", {})
            _ = event.get("timestamp", "?")
            evt_type = event.get("type", "?")
            detail_str = ", ".join(f"{k}={v}" for k, v in detail.items())
            log_event(f"  AUDIT: {evt_type} ({detail_str})")

        print()

    print("=" * 70)
    print("  FULL AUDIT LOG")
    print("=" * 70)
    for entry in AUDIT_LOG:
        print(entry)
    print(f"\nTotal audit entries: {len(AUDIT_LOG)}")


if __name__ == "__main__":
    asyncio.run(run_approval_flow(simulate_saga_failure=True))
