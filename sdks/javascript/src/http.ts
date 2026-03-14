import { CapabilitySet, Capability } from './capabilities';
import { CapabilityDenied } from './errors';

export interface HttpResponse {
    status: number;
    headers: Record<string, string>;
    body: any;
}

export class HttpClient {
    constructor(
        private capabilities: CapabilitySet,
        private timeout: number = 30000
    ) {
        if (!capabilities.has(Capability.NETWORK_OUTBOUND)) {
            throw new CapabilityDenied('HTTP client requires NETWORK_OUTBOUND capability');
        }
    }

    async get(url: string, headers?: Record<string, string>): Promise<HttpResponse> {
        const response = await fetch(url, {
            method: 'GET',
            headers,
            signal: AbortSignal.timeout(this.timeout),
        });
        return this.toResponse(response);
    }

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
