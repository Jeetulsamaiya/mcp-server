# SSE Connection Flow

This diagram shows the Server-Sent Events (SSE) connection establishment and message handling using the `/sse` endpoint.

```mermaid
sequenceDiagram
    participant Client as MCP Client
    participant ActixWeb as Actix Web Server
    participant Auth as Auth Middleware
    participant Session as Session Manager
    participant Stream as SSE Stream
    participant Handler as Protocol Handler
    participant Feature as Feature Manager

    Note over Client, Feature: SSE Connection Establishment

    %% SSE Connection Request
    Client->>+ActixWeb: GET /sse<br/>Accept: text/event-stream
    
    %% CORS Validation
    ActixWeb->>ActixWeb: Validate CORS Origin
    alt Invalid Origin
        ActixWeb-->>Client: 403 Forbidden
    end
    
    %% Accept Header Validation
    ActixWeb->>ActixWeb: Check Accept Header
    alt Not text/event-stream
        ActixWeb-->>Client: 405 Method Not Allowed
    end
    
    %% Authentication
    ActixWeb->>+Auth: Validate Request
    alt Authentication Enabled
        Auth->>Auth: Check Credentials
        alt Invalid Credentials
            Auth-->>ActixWeb: 401 Unauthorized
            ActixWeb-->>Client: 401 Unauthorized
        end
    end
    Auth->>-ActixWeb: Authentication OK
    
    %% Session Management
    ActixWeb->>+Session: Get/Create Session
    Session->>Session: Check Session ID
    alt No Session
        Session->>Session: Generate New Session
        Session->>Session: Store Session
    else Existing Session
        Session->>Session: Update Activity
        Session->>Session: Mark as SSE Connected
    end
    Session->>-ActixWeb: Session ID
    
    %% SSE Stream Setup
    ActixWeb->>+Stream: Create SSE Stream
    Stream->>Stream: Set Headers:<br/>Content-Type: text/event-stream<br/>Cache-Control: no-cache<br/>Connection: keep-alive
    Stream->>Stream: Add Session ID Header
    
    %% Initial Connection Event
    Stream->>Client: data: {"type":"connected"}\n\n
    ActixWeb->>-Client: 200 OK<br/>SSE Stream Established
    
    Note over Client, Feature: Bidirectional Communication

    %% Client sends message via POST
    Client->>+ActixWeb: POST /message<br/>JSON-RPC Request
    ActixWeb->>ActixWeb: Extract Session ID
    ActixWeb->>+Session: Validate Session
    alt Session Invalid/Expired
        Session-->>ActixWeb: Session Error
        ActixWeb-->>Client: 401 Session Expired
    end
    Session->>-ActixWeb: Session Valid
    
    %% Process Message
    ActixWeb->>+Handler: Handle Message
    Handler->>Handler: Parse JSON-RPC
    Handler->>Handler: Route Method
    
    alt Notification (no response needed)
        Handler->>+Feature: Process Notification
        Feature->>Feature: Update State
        Feature->>-Handler: Processing Complete
        Handler->>-ActixWeb: No Response
        ActixWeb->>-Client: 204 No Content
    else Request (response needed)
        Handler->>+Feature: Process Request
        Feature->>Feature: Execute Logic
        Feature->>-Handler: Response Data
        Handler->>-ActixWeb: JSON-RPC Response
        
        %% Send response via SSE
        ActixWeb->>+Stream: Send Response
        Stream->>Client: data: {"jsonrpc":"2.0","result":{...},"id":1}\n\n
        Stream->>-ActixWeb: Response Sent
        ActixWeb->>-Client: 200 OK
    end
    
    Note over Client, Feature: Server-Initiated Events

    %% Resource subscription updates
    rect rgb(240, 255, 240)
        Feature->>Feature: Resource Changed
        Feature->>+Stream: Send Notification
        Stream->>Client: data: {"jsonrpc":"2.0","method":"notifications/resources/updated","params":{...}}\n\n
        Stream->>-Feature: Notification Sent
    end
    
    %% Tool list changes
    rect rgb(240, 255, 240)
        Feature->>Feature: Tool Registered/Unregistered
        Feature->>+Stream: Send Notification
        Stream->>Client: data: {"jsonrpc":"2.0","method":"notifications/tools/list_changed"}\n\n
        Stream->>-Feature: Notification Sent
    end
    
    %% Progress updates
    rect rgb(240, 255, 240)
        Feature->>Feature: Long-running Operation
        Feature->>+Stream: Send Progress
        Stream->>Client: data: {"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"abc","progress":50}}\n\n
        Stream->>-Feature: Progress Sent
    end
    
    Note over Client, Feature: Connection Management

    %% Heartbeat/Ping
    loop Every 30 seconds
        Stream->>Client: data: {"type":"ping"}\n\n
        alt Client Responsive
            Client->>ActixWeb: POST /message<br/>{"jsonrpc":"2.0","method":"ping"}
            ActixWeb->>Handler: Handle Ping
            Handler->>ActixWeb: Pong Response
            ActixWeb->>Stream: Send Pong
            Stream->>Client: data: {"jsonrpc":"2.0","result":"pong","id":null}\n\n
        else Client Unresponsive
            Stream->>Stream: Mark Connection Stale
        end
    end
    
    %% Session Cleanup
    rect rgb(255, 240, 240)
        Session->>Session: Check Session Timeout
        alt Session Expired
            Session->>Session: Remove Session
            Session->>Stream: Close SSE Connection
            Stream-->>Client: Connection Closed
        end
    end
    
    %% Graceful Disconnection
    alt Client Disconnects
        Client->>ActixWeb: DELETE /sse
        ActixWeb->>+Session: Remove Session
        Session->>Session: Clean Up Resources
        Session->>-ActixWeb: Session Removed
        ActixWeb->>Stream: Close Stream
        Stream-->>Client: Connection Closed
        ActixWeb->>Client: 200 OK
    else Server Shutdown
        ActixWeb->>Stream: Close All Streams
        Stream-->>Client: Connection Closed
        ActixWeb->>Session: Clean Up All Sessions
    end

    Note over Client, Feature: Error Handling

    %% SSE Error Scenarios
    rect rgb(255, 240, 240)
        alt Network Error
            Stream-->>Client: Connection Lost
            Client->>Client: Attempt Reconnection
            Client->>ActixWeb: GET /sse (Reconnect)
        else Server Error
            Stream->>Client: data: {"type":"error","error":"Internal server error"}\n\n
            Stream-->>Client: Connection Closed
        else Invalid Message
            ActixWeb->>Stream: Send Error Event
            Stream->>Client: data: {"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}\n\n
        end
    end
```

## SSE Implementation Details

### 1. Connection Establishment
- Client sends GET request to `/sse` endpoint with `Accept: text/event-stream` header
- Server validates CORS origin and authentication credentials
- Session is created or retrieved, marked as SSE-connected
- SSE stream is established with appropriate headers

### 2. Stream Configuration
```http
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
Mcp-Session-Id: <session-id>
```

### 3. Message Format
SSE events follow the standard format:
```
data: <JSON-RPC message>\n\n
```

Special event types:
- `{"type":"connected"}` - Initial connection confirmation
- `{"type":"ping"}` - Heartbeat ping
- `{"type":"error","error":"..."}` - Error notifications

### 4. Bidirectional Communication
- **Client to Server**: POST requests to `/message` endpoint with session ID
- **Server to Client**: SSE events pushed through the established stream

### 5. Server-Initiated Notifications
The server can push notifications for:
- **Resource Updates**: When subscribed resources change
- **Tool List Changes**: When tools are registered/unregistered
- **Progress Updates**: For long-running operations
- **Logging Events**: When logging level changes

### 6. Connection Management
- **Heartbeat**: Periodic ping/pong to detect stale connections
- **Session Timeout**: Automatic cleanup of expired sessions
- **Graceful Shutdown**: Proper connection closure on server shutdown

### 7. Error Handling
- **Network Errors**: Client reconnection logic
- **Parse Errors**: JSON-RPC error responses via SSE
- **Authentication Errors**: Connection termination
- **Server Errors**: Error events with connection closure

### 8. Concurrency Considerations
- Multiple SSE connections per session (if needed)
- Thread-safe session management
- Async stream handling with Actix Web
- Proper resource cleanup on disconnection

## MCP Inspector Compatibility

The SSE implementation is designed to be fully compatible with the MCP Inspector client:
- Standard SSE event format
- JSON-RPC 2.0 message protocol
- Proper CORS handling
- Session-based connection management
- Standard MCP notification types

## Performance Characteristics

- **Low Latency**: Direct SSE push for real-time updates
- **Scalability**: Async handling of multiple concurrent connections
- **Resource Efficiency**: Automatic cleanup of stale connections
- **Reliability**: Heartbeat mechanism for connection health monitoring
