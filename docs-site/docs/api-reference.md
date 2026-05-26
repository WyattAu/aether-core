# API Reference

Complete API reference for Project Aether SDKs.

## Go SDK API

### Package: `aether`

```go
import "github.com/WyattAu/aether-core/sdks/go/aether"
```

---

## Core Types

### Actor Interface

```go
type Actor interface {
    // Name returns the actor's unique identifier
    Name() string

    // HandleMessage processes an incoming message
    HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error)

    // OnStart is called when the actor starts
    OnStart(ctx context.Context) error

    // OnStop is called when the actor stops
    OnStop(ctx context.Context) error
}
```

### BaseActor

```go
type BaseActor struct {
    // contains filtered or unexported fields
}

// Constructor
func NewBaseActor(name string) *BaseActor

// Methods
func (a *BaseActor) Name() string
func (a *BaseActor) Capabilities() *CapabilitySet
func (a *BaseActor) State() *StateHandle
func (a *BaseActor) Require(capabilities ...Capability)
func (a *BaseActor) Send(ctx context.Context, target string, message *Message) error
func (a *BaseActor) Call(ctx context.Context, target string, request any, timeout time.Duration) (any, error)
func (a *BaseActor) Deliver(sender string, message *Message)
func (a *BaseActor) Run(ctx context.Context) error
func (a *BaseActor) Stop()
func (a *BaseActor) IsRunning() bool
```

### Message

```go
type Message struct {
    Type          MessageType         `json:"type"`
    Payload       any                 `json:"payload"`
    Sender        string              `json:"sender,omitempty"`
    CorrelationID string              `json:"correlation_id,omitempty"`
    Priority      Priority            `json:"priority,omitempty"`
    Timestamp     time.Time           `json:"timestamp"`
    Metadata      map[string]string   `json:"metadata,omitempty"`
}

// Constructors
func NewMessage(msgType MessageType, payload any) *Message
func NewResponse(request *Message, payload any) *Message
func NewRPCRequest(sender string, payload any, correlationID string) *Message
func NewRPCResponse(request *Message, payload any) *Message

// Methods
func (m *Message) WithPriority(p Priority) *Message
func (m *Message) WithMetadata(key, value string) *Message
func (m *Message) ToJSON() ([]byte, error)
func FromJSON(data []byte) (*Message, error)
func (m *Message) IsRPC() bool
func (m *Message) IsRequest() bool
func (m *Message) IsResponse() bool
```

### MessageType

```go
type MessageType string

const (
    MessageTypeRequest      MessageType = "request"
    MessageTypeResponse     MessageType = "response"
    MessageTypeEvent        MessageType = "event"
    MessageTypeRPCRequest   MessageType = "rpc_request"
    MessageTypeRPCResponse  MessageType = "rpc_response"
    MessageTypeError        MessageType = "error"
)
```

### Priority

```go
type Priority int

const (
    PriorityLow      Priority = iota
    PriorityNormal
    PriorityHigh
    PriorityCritical
)
```

---

## Capabilities

### Capability Type

```go
type Capability int

const (
    CapabilityNetworkOutbound Capability = iota
    CapabilityNetworkInbound
    CapabilityStateRead
    CapabilityStateWrite
    CapabilityFSRead
    CapabilityFSWrite
    CapabilityActorMessaging
    CapabilityLog
    CapabilityTime
    CapabilityRandom
    CapabilityEnvironment
    CapabilityHTTPClient
    CapabilityHTTPServer
    CapabilityProcessSpawn
)

func (c Capability) String() string
```

### CapabilitySet

```go
type CapabilitySet struct {
    // contains filtered or unexported fields
}

func NewCapabilitySet(capabilities ...Capability) *CapabilitySet
func EmptyCapabilitySet() *CapabilitySet
func AllCapabilities() *CapabilitySet

func (cs *CapabilitySet) Add(cap Capability)
func (cs *CapabilitySet) Has(cap Capability) bool
func (cs *CapabilitySet) HasNetwork() bool
func (cs *CapabilitySet) HasState() bool
func (cs *CapabilitySet) HasFS() bool
func (cs *CapabilitySet) HasHTTP() bool
func (cs *CapabilitySet) All() []Capability
```

---

## State Management

### StateHandle

```go
type StateHandle struct {
    // contains filtered or unexported fields
}

func NewStateHandle() *StateHandle

func (s *StateHandle) Read(ctx context.Context, key string) ([]byte, error)
func (s *StateHandle) Write(ctx context.Context, key string, value []byte) error
func (s *StateHandle) Delete(ctx context.Context, key string) error
func (s *StateHandle) ListKeys(ctx context.Context, prefix string) ([]string, error)
func (s *StateHandle) Clear(ctx context.Context) error
func (s *StateHandle) Exists(ctx context.Context, key string) (bool, error)
```

---

## Error Handling

### Error Types

```go
type ErrorCode string

const (
    ErrCodeInternal          ErrorCode = "INTERNAL"
    ErrCodeInvalidArgument   ErrorCode = "INVALID_ARGUMENT"
    ErrCodeNotFound          ErrorCode = "NOT_FOUND"
    ErrCodeTimeout           ErrorCode = "TIMEOUT"
    ErrCodePermissionDenied  ErrorCode = "PERMISSION_DENIED"
    ErrCodeRpcError          ErrorCode = "RPC_ERROR"
)

type Error struct {
    Code    ErrorCode
    Message string
    Cause   error
}

func NewError(code ErrorCode, message string, cause error) *Error
func (e *Error) Error() string
func (e *Error) Unwrap() error

// Helper constructors
func InvalidArgument(message string) *Error
func NotFound(message string) *Error
func Timeout(message string) *Error
func PermissionDenied(message string) *Error
func Internal(message string) *Error
```

---

## Version

```go
const (
    Version   = "2.0.0"
    VersionMajor = 2
    VersionMinor = 0
    VersionPatch = 0
)

func GetVersion() string
```

---

## Helpers

```go
// Generate a unique ID
func GenerateID() string

// Safe JSON marshal with error handling
func SafeMarshal(v any) []byte

// Safe JSON unmarshal with error handling
func SafeUnmarshal(data []byte, v any) error

// Truncate string to max length
func Truncate(s string, maxLen int) string
```

---

## Python SDK API

### Package: `aether_sdk`

```python
from aether_sdk import Actor, Message, Capability
```

### Actor Class

```python
class Actor(ABC):
    def __init__(self, name: str):
        ...
    
    @abstractmethod
    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        ...
    
    async def on_start(self) -> None:
        ...
    
    async def on_stop(self) -> None:
        ...
    
    def require(self, *capabilities: str) -> None:
        ...
    
    async def send(self, target: str, message: Message) -> None:
        ...
    
    async def call(self, target: str, payload: Any, timeout: float) -> Any:
        ...
    
    @property
    def name(self) -> str:
        ...
    
    @property
    def state(self) -> StateHandle:
        ...
```

### Message Class

```python
class Message:
    def __init__(
        self,
        type: MessageType,
        payload: Any,
        sender: Optional[str] = None,
        correlation_id: Optional[str] = None,
        priority: Priority = Priority.NORMAL,
    ):
        ...
    
    @classmethod
    def response(cls, payload: Any, request: Optional['Message'] = None) -> 'Message':
        ...
    
    @classmethod
    def from_json(cls, data: str) -> 'Message':
        ...
    
    def to_json(self) -> str:
        ...
    
    def with_metadata(self, key: str, value: str) -> 'Message':
        ...
```

---

## JavaScript SDK API

### Package: `@aether/sdk`

```javascript
import { Actor, Message, Capability } from '@aether/sdk';
```

### Actor Class

```typescript
abstract class Actor {
    constructor(name: string);
    
    abstract async handleMessage(
        sender: string,
        message: Message
    ): Promise<Message | null>;
    
    async onStart(): Promise<void>;
    async onStop(): Promise<void>;
    
    require(...capabilities: Capability[]): void;
    async send(target: string, message: Message): Promise<void>;
    async call(target: string, payload: any, timeout: number): Promise<any>;
    
    readonly name: string;
    readonly state: StateHandle;
    readonly capabilities: CapabilitySet;
}
```

### Message Class

```typescript
class Message {
    constructor(
        type: MessageType,
        payload: any,
        options?: MessageOptions
    );
    
    static response(payload: any, request?: Message): Message;
    static fromJSON(data: string): Message;
    
    toJSON(): string;
    withMetadata(key: string, value: string): Message;
    withPriority(priority: Priority): Message;
    
    readonly type: MessageType;
    readonly payload: any;
    readonly sender?: string;
    readonly correlationId?: string;
    readonly priority: Priority;
    readonly timestamp: Date;
    readonly metadata: Record<string, string>;
}
```

---

## Common Patterns

### Request-Response

```go
// Go
response, err := actor.Call(ctx, "service", payload, 5*time.Second)
```

```python
# Python
response = await actor.call("service", payload, timeout=5.0)
```

```javascript
// JavaScript
const response = await actor.call('service', payload, 5000);
```

### Fire-and-Forget

```go
// Go
err := actor.Send(ctx, "target", message)
```

```python
# Python
await actor.send("target", message)
```

```javascript
// JavaScript
await actor.send('target', message);
```

### State Persistence

```go
// Go
data, err := actor.State().Read(ctx, "key")
err = actor.State().Write(ctx, "key", []byte("value"))
```

```python
# Python
data = await actor.state.read("key")
await actor.state.write("key", b"value")
```

```javascript
// JavaScript
const data = await actor.state.read('key');
await actor.state.write('key', Buffer.from('value'));
```
