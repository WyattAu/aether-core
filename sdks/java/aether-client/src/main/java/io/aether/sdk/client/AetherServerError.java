package io.aether.sdk.client;

/**
 * Exception thrown when the Aether server returns an error response.
 */
public class AetherServerError extends RuntimeException {

    private final int statusCode;

    /**
     * Create a server error with status code and detail message.
     *
     * @param statusCode the HTTP status code
     * @param detail     the error detail message
     */
    public AetherServerError(int statusCode, String detail) {
        super("HTTP " + statusCode + ": " + detail);
        this.statusCode = statusCode;
    }

    /**
     * Get the HTTP status code.
     *
     * @return the status code, or {@code -1} for connection errors
     */
    public int getStatusCode() {
        return statusCode;
    }

    /**
     * Get the error detail message.
     *
     * @return the detail string
     */
    public String getDetail() {
        return getMessage().replace("HTTP " + statusCode + ": ", "");
    }
}
