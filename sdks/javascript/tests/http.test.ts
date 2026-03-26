import { HttpClient, HttpResponse } from '../src/http';
import { Capability, CapabilitySet } from '../src/capabilities';
import { CapabilityDenied } from '../src/errors';
 
// Mock fetch globally
global.fetch = jest.fn();
 
// Helper to create mock response with proper headers
function createMockResponse(options: {
  ok?: boolean;
  status?: number;
  text?: () => Promise<string>;
  json?: () => Promise<any>;
}): Response {
  return {
    ok: options.ok ?? true,
    status: options.status ?? 200,
    headers: {
      forEach: (callback: (value: string, key: string) => void) => {
        // Default empty headers
      },
      get: (name: string) => null,
    },
    text: options.text ?? (() => Promise.resolve('')),
    json: options.json ?? (() => Promise.resolve(null)),
  } as Response;
}
 
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
 
        const mockResponse = createMockResponse({
          ok: true,
          status: 200,
          text: () => Promise.resolve('response body'),
        });
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);
 
        const response = await client.get('https://example.com/api');
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api', expect.objectContaining({
            headers: undefined,
        }));
    });
 
    test('should make GET request with headers', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);
 
        const mockResponse = createMockResponse({
          ok: true,
          status: 200,
          text: () => Promise.resolve('response body'),
        });
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
 
        const mockResponse = createMockResponse({
          ok: true,
          status: 201,
          json: () => Promise.resolve({ id: 1 }),
        });
        (global.fetch as jest.Mock).mockResolvedValue(mockResponse);
 
        const json = { name: 'test', value: 42 };
        const response = await client.post('https://example.com/api', json);
        expect(global.fetch).toHaveBeenCalledWith('https://example.com/api', expect.objectContaining({
            method: 'POST',
            body: JSON.stringify(json),
        }));
    });
 
    test('should reuse session across requests', async () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        client = new HttpClient(caps);
 
        const mockResponse1 = createMockResponse({
          ok: true,
          status: 200,
          text: () => Promise.resolve('first'),
        });
        const mockResponse2 = createMockResponse({
          ok: true,
          status: 200,
          text: () => Promise.resolve('second'),
        });
        (global.fetch as jest.Mock)
            .mockResolvedValueOnce(mockResponse1)
            .mockResolvedValueOnce(mockResponse2);
 
        await client.get('https://example.com/1');
        await client.get('https://example.com/2');
 
        // Multiple fetch calls should be made
        expect((global.fetch as jest.Mock).mock.calls.length).toBeGreaterThan(1);
    });
});
