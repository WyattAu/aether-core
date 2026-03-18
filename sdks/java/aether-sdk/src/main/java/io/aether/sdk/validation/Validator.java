package io.aether.sdk.validation;

import java.util.*;
import java.util.function.Predicate;
import java.util.regex.Pattern;

/**
 * Fluent validator for building validation rules.
 * 
 * Example:
 * <pre>
 * Validator validator = new Validator();
 * validator.required("name", name);
 * validator.email("email", email);
 * validator.minLength("password", password, 8);
 * 
 * if (!validator.isValid()) {
 *     throw new ValidationException(validator.getErrors());
 * }
 * </pre>
 */
public class Validator {
    private final Map<String, List<String>> errors = new LinkedHashMap<>();
    
    /**
     * Add an error for a field.
     */
    public Validator addError(String field, String message) {
        errors.computeIfAbsent(field, k -> new ArrayList<>()).add(message);
        return this;
    }
    
    /**
     * Check if all validations passed.
     */
    public boolean isValid() {
        return errors.isEmpty();
    }
    
    /**
     * Clear all errors.
     */
    public Validator clear() {
        errors.clear();
        return this;
    }
    
    /**
     * Get all errors.
     */
    public Map<String, List<String>> getErrors() {
        return Collections.unmodifiableMap(errors);
    }
    
    // ========================================
    // Required Validation
    // ========================================
    
    public Validator required(String field, Object value) {
        return required(field, value, field + " is required");
    }
    
    public Validator required(String field, Object value, String message) {
        if (value == null) {
            addError(field, message);
        } else if (value instanceof String && ((String) value).isBlank()) {
            addError(field, message);
        } else if (value instanceof Collection && ((Collection<?>) value).isEmpty()) {
            addError(field, message);
        } else if (value instanceof Map && ((Map<?, ?>) value).isEmpty()) {
            addError(field, message);
        }
        return this;
    }
    
    // ========================================
    // Type Validations
    // ========================================
    
    public Validator string(String field, Object value) {
        return string(field, value, field + " must be a string");
    }
    
    public Validator string(String field, Object value, String message) {
        if (value != null && !(value instanceof String)) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator integer(String field, Object value) {
        return integer(field, value, field + " must be an integer");
    }
    
    public Validator integer(String field, Object value, String message) {
        if (value != null) {
            if (!(value instanceof Integer) && !(value instanceof Long)) {
                addError(field, message);
            }
        }
        return this;
    }
    
    public Validator decimal(String field, Object value) {
        return decimal(field, value, field + " must be a number");
    }
    
    public Validator decimal(String field, Object value, String message) {
        if (value != null) {
            if (!(value instanceof Number)) {
                addError(field, message);
            }
        }
        return this;
    }
    
    public Validator bool(String field, Object value) {
        return bool(field, value, field + " must be a boolean");
    }
    
    public Validator bool(String field, Object value, String message) {
        if (value != null && !(value instanceof Boolean)) {
            addError(field, message);
        }
        return this;
    }
    
    // ========================================
    // String Validations
    // ========================================
    
    public Validator minLength(String field, String value, int minLength) {
        return minLength(field, value, minLength, 
            field + " must be at least " + minLength + " characters");
    }
    
    public Validator minLength(String field, String value, int minLength, String message) {
        if (value != null && value.length() < minLength) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator maxLength(String field, String value, int maxLength) {
        return maxLength(field, value, maxLength, 
            field + " must be at most " + maxLength + " characters");
    }
    
    public Validator maxLength(String field, String value, int maxLength, String message) {
        if (value != null && value.length() > maxLength) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator pattern(String field, String value, Pattern pattern) {
        return pattern(field, value, pattern, field + " has invalid format");
    }
    
    public Validator pattern(String field, String value, Pattern pattern, String message) {
        if (value != null && !pattern.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    // ========================================
    // Numeric Validations
    // ========================================
    
    public Validator minValue(String field, Number value, Number minValue) {
        return minValue(field, value, minValue, 
            field + " must be at least " + minValue);
    }
    
    public Validator minValue(String field, Number value, Number minValue, String message) {
        if (value != null && value.doubleValue() < minValue.doubleValue()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator maxValue(String field, Number value, Number maxValue) {
        return maxValue(field, value, maxValue, 
            field + " must be at most " + maxValue);
    }
    
    public Validator maxValue(String field, Number value, Number maxValue, String message) {
        if (value != null && value.doubleValue() > maxValue.doubleValue()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator range(String field, Number value, Number min, Number max) {
        return range(field, value, min, max, 
            field + " must be between " + min + " and " + max);
    }
    
    public Validator range(String field, Number value, Number min, Number max, String message) {
        if (value != null) {
            double v = value.doubleValue();
            if (v < min.doubleValue() || v > max.doubleValue()) {
                addError(field, message);
            }
        }
        return this;
    }
    
    // ========================================
    // Format Validations
    // ========================================
    
    private static final Pattern EMAIL_PATTERN = 
        Pattern.compile("^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$");
    
    private static final Pattern UUID_PATTERN = 
        Pattern.compile("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", Pattern.CASE_INSENSITIVE);
    
    private static final Pattern URL_PATTERN = 
        Pattern.compile("^(https?|ftp)://[^\\s/$.?#].[^\\s]*$", Pattern.CASE_INSENSITIVE);
    
    private static final Pattern PHONE_PATTERN = 
        Pattern.compile("^\\+?[1-9]\\d{1,14}$");
    
    private static final Pattern SLUG_PATTERN = 
        Pattern.compile("^[a-z0-9]+(?:-[a-z0-9]+)*$");
    
    public Validator email(String field, String value) {
        return email(field, value, field + " must be a valid email");
    }
    
    public Validator email(String field, String value, String message) {
        if (value != null && !EMAIL_PATTERN.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator url(String field, String value) {
        return url(field, value, field + " must be a valid URL");
    }
    
    public Validator url(String field, String value, String message) {
        if (value != null && !URL_PATTERN.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator uuid(String field, String value) {
        return uuid(field, value, field + " must be a valid UUID");
    }
    
    public Validator uuid(String field, String value, String message) {
        if (value != null && !UUID_PATTERN.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator phone(String field, String value) {
        return phone(field, value, field + " must be a valid phone number");
    }
    
    public Validator phone(String field, String value, String message) {
        if (value != null && !PHONE_PATTERN.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator slug(String field, String value) {
        return slug(field, value, field + " must be a valid slug");
    }
    
    public Validator slug(String field, String value, String message) {
        if (value != null && !SLUG_PATTERN.matcher(value).matches()) {
            addError(field, message);
        }
        return this;
    }
    
    // ========================================
    // List Validations
    // ========================================
    
    public Validator minItems(String field, Collection<?> value, int minItems) {
        return minItems(field, value, minItems, 
            field + " must have at least " + minItems + " items");
    }
    
    public Validator minItems(String field, Collection<?> value, int minItems, String message) {
        if (value != null && value.size() < minItems) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator maxItems(String field, Collection<?> value, int maxItems) {
        return maxItems(field, value, maxItems, 
            field + " must have at most " + maxItems + " items");
    }
    
    public Validator maxItems(String field, Collection<?> value, int maxItems, String message) {
        if (value != null && value.size() > maxItems) {
            addError(field, message);
        }
        return this;
    }
    
    // ========================================
    // Custom Validation
    // ========================================
    
    public <T> Validator custom(String field, T value, Predicate<T> predicate, String message) {
        if (!predicate.test(value)) {
            addError(field, message);
        }
        return this;
    }
    
    public Validator when(boolean condition, java.util.function.Consumer<Validator> validation) {
        if (condition) {
            validation.accept(this);
        }
        return this;
    }
    
    /**
     * Exception thrown when validation fails.
     */
    public static class ValidationException extends RuntimeException {
        private final Map<String, List<String>> errors;
        
        public ValidationException(Map<String, List<String>> errors) {
            super("Validation failed: " + errors);
            this.errors = errors;
        }
        
        public Map<String, List<String>> getErrors() {
            return errors;
        }
    }
}
