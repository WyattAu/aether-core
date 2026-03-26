/**
 * HTTP Client with Capability Enforcement.
 *
 * Provides an {@link HttpClient} class that wraps the Fetch API with Aether's
 * capability system, ensuring actors cannot make outbound HTTP requests without
 * the appropriate permissions.
 *
 * @module aether/http
 */

import { CapabilitySet, Capability } from './capabilities';
import { CapabilityDenied } from './errors';

/**
 * Represents an HTTP response.
 */
export interface HttpResponse {
    /** The HTTP status code (e.g., 200, 404). */
    status: number;
    /** Response headers as key-value pairs. */
    headers: Record<string, string>;
    /** The response body as a string. */
    body: any;
}

/**
 * Capability-guarded HTTP client for actor use.
 *
 * Requires {@link Capability.NETWORK_OUTBOUND} to be present in the provided
 * capability set. All requests are subject to a configurable timeout.
 *
 * @example
 * ```typescript
 * const client = new HttpClient(capabilities, 15000);
 *
 * // GET request
 * const res = await client.get('https://api.example.com/data');
 * console.log(res.status, res.body);
 *
 * // POST request
 * const postRes = await client.post(
 *   'https://api.example.com/items',
 *   { name: 'widget' },
 *   { 'X-Custom-Header': 'value' }
 * );
 * ```
 */
export class HttpClient {
    /**
     * Create a new HttpClient.
     *
     * @param capabilities - The actor's capability set; must include
     *                      {@link Capability.NETWORK_OUTBOUND}.
     * @param timeout      - Request timeout in milliseconds (default: 30 000).
     * @throws CapabilityDenied If NETWORK_OUTBOUND capability is not present.
     */
    constructor(
        private capabilities: CapabilitySet,
        private timeout: number = 30000
    ) {
        if (!capabilities.has(Capability.NETWORK_OUTBOUND)) {
            throw new CapabilityDenied('HTTP client requires NETWORK_OUTBOUND capability');
        }
    }

    /**
     * Perform an HTTP GET request.
     *
     * @param url     - The URL to request.
     * @param headers - Optional request headers.
     * @returns The HTTP response.
     */
    async get(url: string, headers?: Record<string, string>): Promise<HttpResponse> {
        const response = await fetch(url, {
            method: 'GET',
            headers,
            signal: AbortSignal.timeout(this.timeout),
        });
        return this.toResponse(response);
    }

    /**
     * Perform an HTTP POST request with a JSON body.
     *
     * @param url     - The URL to request.
     * @param body    - The request body; serialized as JSON.
     * @param headers - Optional additional request headers. The
     *                 `Content-Type: application/json` header is set
     *                 automatically.
     * @returns The HTTP response.
     */
    async post(url: string, body?: any, headers?: Record<string, string>): Promise<HttpResponse> {
        const response = await fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                ...headers,
            },
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(this.timeout),
        });
        return this.toResponse(response);
    }

    /**
     * Convert a native `Response` into an {@link HttpResponse}.
     *
     * @param response - The native Fetch API response.
     * @returns The converted HTTP response.
     */
    private async toResponse(response: globalThis.Response): Promise<HttpResponse> {
        const headers: Record<string, string> = {};
        response.headers.forEach((value, key) => {
            headers[key] = value;
        });

        return {
            status: response.status,
            headers,
            body: await response.text(),
        };
    }
}
