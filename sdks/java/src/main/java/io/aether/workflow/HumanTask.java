package io.aether.workflow;

import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;

/**
 * Human Task Integration
 * 
 * Provides human task management for workflow approvals and manual steps.
 */
public class HumanTask {
    
    /**
     * Create a new human task.
     */
    public static <T> HumanTaskContext<T> create(
        String taskType,
        String title,
        String description,
    ) {
        return new HumanTaskContext<T>(
            UUID.randomUUID().toString(),
            taskType,
            "", // workflowId
            "",
            title,
            description,
            null,
            Collections.emptyList(),
            Collections.emptyList(),
            5,
            null,
            new HashMap<>(),
            null,
            Types.HumanTaskStatus.PENDING,
            Instant.now(),
            null,
            null,
            null,
            new HashMap<>()
        );
    }
    
    /**
     * Claim a task (assign to self).
     */
    public static <T> HumanTaskContext<T> claim(
        HumanTaskContext<T> task,
        String userId
    ) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.PENDING && 
            task.status != Types.HumanTaskStatus.ASSIGNED) {
            throw new HumanTaskError("Task cannot be claimed in status: " + task.status);
        }
        
        // Check if user is eligible
        boolean isEligible = 
            task.candidateUsers.contains(userId) ||
            task.candidateGroups.isEmpty() || // Open to anyone
            task.assignee != null && task.assignee.equals(userId);
        
        if (!isEligible && task.assignee != null && !task.assignee.equals(userId)) {
            throw new HumanTaskError("Task is already assigned to: " + task.assignee);
        }
        
        task.assignee = userId;
        task.status = Types.HumanTaskStatus.ASSIGNED;
        task.updatedAt = Instant.now();
        
        return task;
    }
    
    /**
     * Release a task (unassign).
     */
    public static <T> HumanTaskContext<T> release(HumanTaskContext<T> task) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.ASSIGNED && 
            task.status != Types.HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError("Task cannot be released in status: " + task.status);
        }
        
        task.assignee = null;
        task.status = Types.HumanTaskStatus.PENDING;
        task.updatedAt = Instant.now();
        
        return task;
    }
    
    /**
     * Start working on a task.
     */
    public static <T> HumanTaskContext<T> start(HumanTaskContext<T> task) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.ASSIGNED) {
            throw new HumanTaskError("Task must be assigned before starting");
        }
        
        task.status = Types.HumanTaskStatus.IN_PROGRESS;
        task.updatedAt = Instant.now();
        
        return task;
    }
    
    /**
     * Complete a task with a result.
     */
    public static <T> HumanTaskContext<T> complete(
        HumanTaskContext<T> task,
        Map<String, Object> result,
        String completedBy
    ) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.ASSIGNED && 
            task.status != Types.HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError("Task cannot be completed in status: " + task.status);
        }
        
        task.result = result;
        task.status = Types.HumanTaskStatus.COMPLETED;
        task.completedAt = Instant.now();
        task.completedBy = completedBy;
        task.updatedAt = Instant.now();
        
        return task;
    }
    
    /**
     * Reject a task with a reason.
     */
    public static <T> HumanTaskContext<T> reject(
        HumanTaskContext<T> task,
        String reason,
        String rejectedBy
    ) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.ASSIGNED && 
            task.status != Types.HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError("Task cannot be rejected in status: " + task.status);
        }
        
        task.result = Map.of("rejected", true, "reason", reason);
        task.status = Types.HumanTaskStatus.REJECTED;
        task.completedAt = Instant.now();
        task.completedBy = rejectedBy;
        task.updatedAt = Instant.now();
        
        return task;
    }
    
    /**
     * Escalate a task.
     */
    public static <T> HumanTaskContext<T> escalate(HumanTaskContext<T> task, String reason) {
        task.status = Types.HumanTaskStatus.ESCALATED;
        task.metadata.put("escalationReason", reason);
        task.updatedAt = Instant.now();
        return task;
    }
    
    /**
     * Delegate a task to another user.
     */
    public static <T> HumanTaskContext<T> delegate(HumanTaskContext<T> task, String toUser) throws HumanTaskError {
        if (task.status != Types.HumanTaskStatus.ASSIGNED && 
            task.status != Types.HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError("Task cannot be delegated in status: " + task.status);
        }
        
        task.assignee = toUser;
        task.status = Types.HumanTaskStatus.ASSIGNED;
        task.updatedAt = Instant.now();
        
        return task;
    }
}

