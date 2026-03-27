package io.aether.sdk.validation;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.*;
import java.util.regex.Pattern;

class ValidatorTest {

    @Test
    @DisplayName("valid by default")
    void testValidByDefault() {
        Validator v = new Validator();
        assertTrue(v.isValid());
        assertTrue(v.getErrors().isEmpty());
    }

    @Test
    @DisplayName("addError adds error")
    void testAddError() {
        Validator v = new Validator();
        v.addError("field", "error message");
        assertFalse(v.isValid());
        assertEquals(1, v.getErrors().size());
        assertEquals("error message", v.getErrors().get("field").get(0));
    }

    @Test
    @DisplayName("clear removes all errors")
    void testClear() {
        Validator v = new Validator();
        v.addError("f", "e");
        v.clear();
        assertTrue(v.isValid());
    }

    @Test
    @DisplayName("required null fails")
    void testRequiredNull() {
        Validator v = new Validator();
        v.required("name", null);
        assertFalse(v.isValid());
        assertTrue(v.getErrors().containsKey("name"));
    }

    @Test
    @DisplayName("required blank string fails")
    void testRequiredBlank() {
        Validator v = new Validator();
        v.required("name", "   ");
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("required empty collection fails")
    void testRequiredEmptyCollection() {
        Validator v = new Validator();
        v.required("items", Collections.emptyList());
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("required empty map fails")
    void testRequiredEmptyMap() {
        Validator v = new Validator();
        v.required("data", Collections.emptyMap());
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("required passes for valid value")
    void testRequiredPass() {
        Validator v = new Validator();
        v.required("name", "Alice");
        assertTrue(v.isValid());
    }

    @Test
    @DisplayName("string type check")
    void testStringType() {
        Validator v = new Validator();
        v.string("name", 123);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("string type null passes")
    void testStringNullPasses() {
        Validator v = new Validator();
        v.string("name", null);
        assertTrue(v.isValid());
    }

    @Test
    @DisplayName("integer type check")
    void testIntegerType() {
        Validator v = new Validator();
        v.integer("age", "not-int");
        assertFalse(v.isValid());
        v.integer("count", 42);
        assertTrue(v.getErrors().getOrDefault("count", List.of()).isEmpty());
    }

    @Test
    @DisplayName("decimal type check")
    void testDecimalType() {
        Validator v = new Validator();
        v.decimal("price", "not-number");
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("bool type check")
    void testBoolType() {
        Validator v = new Validator();
        v.bool("active", "yes");
        assertFalse(v.isValid());
        v.bool("flag", true);
        assertTrue(v.getErrors().getOrDefault("flag", List.of()).isEmpty());
    }

    @Test
    @DisplayName("minLength")
    void testMinLength() {
        Validator v = new Validator();
        v.minLength("name", "ab", 3);
        assertFalse(v.isValid());
        v.minLength("name2", "abc", 3);
        assertTrue(v.getErrors().getOrDefault("name2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("maxLength")
    void testMaxLength() {
        Validator v = new Validator();
        v.maxLength("name", "abcdef", 5);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("pattern validation")
    void testPattern() {
        Validator v = new Validator();
        v.pattern("code", "abc", Pattern.compile("[0-9]+"));
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("minValue")
    void testMinValue() {
        Validator v = new Validator();
        v.minValue("age", 5, 18);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("maxValue")
    void testMaxValue() {
        Validator v = new Validator();
        v.maxValue("score", 150, 100);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("range validation")
    void testRange() {
        Validator v = new Validator();
        v.range("score", 50, 0, 100);
        assertTrue(v.getErrors().getOrDefault("score", List.of()).isEmpty());
        v.range("score2", 150, 0, 100);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("email validation")
    void testEmail() {
        Validator v = new Validator();
        v.email("email", "invalid");
        assertFalse(v.isValid());
        v.email("email2", "user@example.com");
        assertTrue(v.getErrors().getOrDefault("email2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("url validation")
    void testUrl() {
        Validator v = new Validator();
        v.url("site", "not-a-url");
        assertFalse(v.isValid());
        v.url("site2", "https://example.com");
        assertTrue(v.getErrors().getOrDefault("site2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("uuid validation")
    void testUuid() {
        Validator v = new Validator();
        v.uuid("id", "not-uuid");
        assertFalse(v.isValid());
        v.uuid("id2", "550e8400-e29b-41d4-a716-446655440000");
        assertTrue(v.getErrors().getOrDefault("id2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("phone validation")
    void testPhone() {
        Validator v = new Validator();
        v.phone("phone", "123");
        assertFalse(v.isValid());
        v.phone("phone2", "+1234567890123");
        assertTrue(v.getErrors().getOrDefault("phone2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("slug validation")
    void testSlug() {
        Validator v = new Validator();
        v.slug("slug", "Invalid Slug");
        assertFalse(v.isValid());
        v.slug("slug2", "valid-slug-123");
        assertTrue(v.getErrors().getOrDefault("slug2", List.of()).isEmpty());
    }

    @Test
    @DisplayName("minItems")
    void testMinItems() {
        Validator v = new Validator();
        v.minItems("tags", List.of("a"), 2);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("maxItems")
    void testMaxItems() {
        Validator v = new Validator();
        v.maxItems("tags", List.of("a", "b", "c"), 2);
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("custom predicate")
    void testCustom() {
        Validator v = new Validator();
        v.custom("value", "abc", s -> s.length() > 5, "too short");
        assertFalse(v.isValid());
    }

    @Test
    @DisplayName("when conditional validation")
    void testWhen() {
        Validator v = new Validator();
        v.when(true, val -> val.required("name", null));
        assertFalse(v.isValid());
        v.clear();
        v.when(false, val -> val.required("name", null));
        assertTrue(v.isValid());
    }

    @Test
    @DisplayName("getErrors is unmodifiable")
    void testGetErrorsUnmodifiable() {
        Validator v = new Validator();
        assertThrows(UnsupportedOperationException.class, () ->
            v.getErrors().put("k", List.of("v")));
    }

    @Test
    @DisplayName("ValidationException")
    void testValidationException() {
        Validator v = new Validator();
        v.required("name", null);
        Validator.ValidationException ex = assertThrows(
            Validator.ValidationException.class, () -> {
                if (!v.isValid()) throw new Validator.ValidationException(v.getErrors());
            });
        assertTrue(ex.getErrors().containsKey("name"));
    }

    @Test
    @DisplayName("fluent chaining")
    void testFluentChaining() {
        Validator v = new Validator()
            .required("name", "Alice")
            .minLength("name", "Alice", 3)
            .email("email", "a@b.com");
        assertTrue(v.isValid());
    }
}
