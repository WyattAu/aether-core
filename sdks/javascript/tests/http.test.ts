import { HttpClient, HttpResponse } from '../src/http';
import { Capability, CapabilitySet } from '../src/capabilities';
import { CapabilityDenied } from '../src/errors';

// Mock fetch globally
global.fetch = jest.fn();

describe('HttpClient', () => {
    let client: HttpClient;

    beforeEach(() => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);
        (global.fetch as jest.Mock).mockClear();
    });

    test('should require NETWORK_OUTBOUND capability', () => {
        const caps = new CapabilitySet();
        expect(() => new HttpClient(caps)).toThrow(CapabilityDenied);
    });

    test('should initialize with capability', () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        const httpClient = new HttpClient(caps);
        expect(httpClient).toBeDefined();
    });

    test('should make GET request', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse = { ok: true, status: 200, text: () => Promise.resolve('response body') };
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);

        const response = await client.get('https://example.com/api');
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api', expect.objectContaining({
            headers: undefined,
        }));
    });

    test('should make GET request with headers', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse = { ok: true, status: 200, text: () => Promise.resolve('response body') };
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);

        const headers = { 'Authorization': 'Bearer token' };
        await client.get('https://example.com/api', headers);
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api', expect.objectContaining({
            headers: headers,
        }));
    });

    test('should make POST request', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse = { ok: true, status: 201, json: () => Promise.resolve({ id: 1 }) };
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);

        const json = { name: 'test', value: 42 };
        const response = await client.post('https://example.com/api', json);
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api', expect.objectContaining({
            method: 'POST',
            body: JSON.stringify(json),
        }));
    });

    test('should make PUT request', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse = { ok: true, status: 200, json: () => Promise.resolve({ id: 1, updated: true }) };
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);

        const json = { id: 1, name: 'updated' };
        const response = await client.put('https://example.com/api/1', json);
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api/1', expect.objectContaining({
            method: 'PUT',
            body: JSON.stringify(json),
        }));
    });

    test('should make DELETE request', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse = { ok: true, status: 204 };
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);

        const response = await client.delete('https://example.com/api/1');
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api/1', expect.objectContaining({
            method: 'DELETE',
        }));
    });

    test('should close session', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        await client.close();
        // Session should be closed - no error means success
    });

    test('should close without session', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        await client.close();
        // Should not throw
    });

    test('should reuse session', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        const mockResponse1 = { ok: true, status: 200, text: () => Promise.resolve('first') };
        const mockResponse2 = { ok: true, status: 200, text: () => Promise.resolve('second') };
        (global.fetch as jest.Mock)
            .mockResolvedValueOnce(mockResponse1)
            .mockResolvedValueOnce(mockResponse2);

        await client.get('https://example.com/1');
        await client.get('https://example.com/2');

        // Session should be reused
        expect((global.fetch as jest.Mock).mock.calls.length).toBeGreaterThan(1);
    });

    test('should handle closed session', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);

        // Create session
        await client.get('https://example.com');

        // Close session
        await client.close();

        // Try to use after close - should create new session
        const response = await client.get('https://example.com/after-close');
        expect(response).toBeDefined();
    });

    test('should work as async context manager', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);

        const urls: string[] = [];

        // Use context manager pattern
        await using httpClient = new HttpClient(caps);
        {
            const response = await httpClient.get('https://example.com');
            expect(response.status).toBe(200);
        }
        // Auto-close on exit
    });
});
