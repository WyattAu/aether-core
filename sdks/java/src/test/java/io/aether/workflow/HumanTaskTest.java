package io.aether.workflow;

import io.aether.workflow.Types.HumanTaskContext;
import io.aether.workflow.Types.HumanTaskStatus;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Instant;
import java.util.List;
import java.util.Map;

class HumanTaskTest {

    private HumanTaskContext<String> task;

    @BeforeEach
    void setUp() {
        task = HumanTask.create("approval", "Review Document", "Please review");
    }

    @Test
    @DisplayName("create initializes with PENDING status")
    void testCreate() {
        assertNotNull(task.getTaskId());
        assertEquals("approval", task.getTaskType());
        assertEquals("Review Document", task.getTitle());
        assertEquals(HumanTaskStatus.PENDING, task.getStatus());
        assertNull(task.getAssignee());
        assertNotNull(task.getCreatedAt());
    }

    @Test
    @DisplayName("claim assigns user")
    void testClaim() {
        task.getCandidateUsers().add("alice");
        HumanTaskContext<String> result = HumanTask.claim(task, "alice");
        assertEquals("alice", result.getAssignee());
        assertEquals(HumanTaskStatus.ASSIGNED, result.getStatus());
    }

    @Test
    @DisplayName("claim with empty candidate groups assigns anyone")
    void testClaimEmptyCandidateGroups() {
        HumanTaskContext<String> result = HumanTask.claim(task, "bob");
        assertEquals("bob", result.getAssignee());
        assertEquals(HumanTaskStatus.ASSIGNED, result.getStatus());
    }

    @Test
    @DisplayName("claim fails when already completed")
    void testClaimCompleted() {
        task.setStatus(HumanTaskStatus.COMPLETED);
        assertThrows(HumanTaskError.class, () -> HumanTask.claim(task, "alice"));
    }

    @Test
    @DisplayName("claim fails when assigned to another user")
    void testClaimAssignedToOther() {
        task.getCandidateUsers().add("alice");
        task.setAssignee("bob");
        assertThrows(HumanTaskError.class, () -> HumanTask.claim(task, "charlie"));
    }

    @Test
    @DisplayName("release unassigns task")
    void testRelease() {
        task.setAssignee("alice");
        task.setStatus(HumanTaskStatus.ASSIGNED);
        HumanTaskContext<String> result = HumanTask.release(task);
        assertNull(result.getAssignee());
        assertEquals(HumanTaskStatus.PENDING, result.getStatus());
    }

    @Test
    @DisplayName("release fails when PENDING")
    void testReleasePending() {
        assertThrows(HumanTaskError.class, () -> HumanTask.release(task));
    }

    @Test
    @DisplayName("start transitions to IN_PROGRESS")
    void testStart() {
        task.setAssignee("alice");
        task.setStatus(HumanTaskStatus.ASSIGNED);
        HumanTaskContext<String> result = HumanTask.start(task);
        assertEquals(HumanTaskStatus.IN_PROGRESS, result.getStatus());
    }

    @Test
    @DisplayName("start fails when not ASSIGNED")
    void testStartNotAssigned() {
        assertThrows(HumanTaskError.class, () -> HumanTask.start(task));
    }

    @Test
    @DisplayName("complete sets result and status")
    void testComplete() {
        task.setAssignee("alice");
        task.setStatus(HumanTaskStatus.ASSIGNED);
        Map<String, Object> result = Map.of("decision", "approved");
        HumanTaskContext<String> completed = HumanTask.complete(task, result, "alice");
        assertEquals(HumanTaskStatus.COMPLETED, completed.getStatus());
        assertEquals("approved", completed.getResult().get("decision"));
        assertNotNull(completed.getCompletedAt());
        assertEquals("alice", completed.getCompletedBy());
    }

    @Test
    @DisplayName("complete fails when PENDING")
    void testCompletePending() {
        assertThrows(HumanTaskError.class, () ->
            HumanTask.complete(task, Map.of(), "alice"));
    }

    @Test
    @DisplayName("reject sets rejection result")
    void testReject() {
        task.setAssignee("alice");
        task.setStatus(HumanTaskStatus.ASSIGNED);
        HumanTaskContext<String> rejected = HumanTask.reject(task, "not valid", "alice");
        assertEquals(HumanTaskStatus.REJECTED, rejected.getStatus());
        assertTrue((Boolean) rejected.getResult().get("rejected"));
        assertEquals("not valid", rejected.getResult().get("reason"));
        assertNotNull(rejected.getCompletedAt());
    }

    @Test
    @DisplayName("escalate sets ESCALATED status")
    void testEscalate() {
        HumanTaskContext<String> escalated = HumanTask.escalate(task, "needs manager review");
        assertEquals(HumanTaskStatus.ESCALATED, escalated.getStatus());
        assertEquals("needs manager review", escalated.getMetadata().get("escalationReason"));
    }

    @Test
    @DisplayName("delegate reassigns task")
    void testDelegate() {
        task.setAssignee("alice");
        task.setStatus(HumanTaskStatus.ASSIGNED);
        HumanTaskContext<String> delegated = HumanTask.delegate(task, "bob");
        assertEquals("bob", delegated.getAssignee());
        assertEquals(HumanTaskStatus.ASSIGNED, delegated.getStatus());
    }

    @Test
    @DisplayName("delegate fails when not ASSIGNED or IN_PROGRESS")
    void testDelegatePending() {
        assertThrows(HumanTaskError.class, () -> HumanTask.delegate(task, "bob"));
    }

    @Test
    @DisplayName("HumanTaskContext setters and getters")
    void testContextSettersGetters() {
        task.setWorkflowId("wf-1");
        task.setStepName("step-1");
        task.setDescription("desc");
        task.setPriority(10);
        task.setDueDate(Instant.now().plusSeconds(3600));
        task.setFormData(Map.of("field", "value"));
        assertEquals("wf-1", task.getWorkflowId());
        assertEquals("step-1", task.getStepName());
        assertEquals(10, task.getPriority());
        assertNotNull(task.getDueDate());
        assertEquals("value", task.getFormData().get("field"));
    }
}
