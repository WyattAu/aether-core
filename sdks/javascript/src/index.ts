export { Actor, actor } from './actor';
export { Capability, CapabilitySet } from './capabilities';
export { Message, MessageType, Priority } from './messaging';
export { StateHandle } from './state';
export { HttpClient, HttpResponse } from './http';
export { AetherError, CapabilityDenied, ActorNotFound, RpcError } from './errors';
export type { ActorConfig, MessageHandler, RpcHandler } from './types';

export {
  AetherClient,
  AetherServerError,
} from './client';
export type {
  ActorInfo,
  MessageEnvelope,
  DeliveryReceipt,
  StateEntry,
  EventRecord,
  ServerInfo,
} from './client';

// Resilience Module
export * from './resilience';

// Validation Module
export * from './validation';
