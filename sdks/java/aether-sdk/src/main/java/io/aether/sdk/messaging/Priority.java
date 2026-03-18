package io.aether.sdk.messaging;

import java.time.Instant;
import java.util.*;

/**
 * Message priority levels.
 */
public enum Priority {
    /** Low priority - background processing */
    LOW(0),
    /** Normal priority - default */
    NORMAL(1),
    /** High priority - urgent processing */
    HIGH(2),
    /** Critical priority - must't be delayed */
    CRITICAL(3);
    
    private final int value;
    
    Priority(int value) {
        this.value = value;
    }
    
    public int getValue() {
        return value;
    }
    
    public static Priority fromValue(int value) {
        for (Priority p : values()) {
            if (p.value == value) {
                return p;
            }
        }
        throw new IllegalArgumentException("Unknown priority: " + value);
    }
}
