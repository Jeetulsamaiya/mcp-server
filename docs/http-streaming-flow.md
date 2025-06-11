# HTTP Streaming Flow

This diagram shows the Streamable HTTP transport connection establishment and message handling using the `/mcp` endpoint.

```mermaid
sequenceDiagram
    participant Client as MCP Client
    participant ActixWeb as Actix Web Server
    participant Auth as Auth Middleware
    participant Session as Session Manager
    participant Stream as HTTP Stream
    participant Handler as Protocol Handler
    participant Feature as Feature Manager

    Note over Client, Feature: Streamable HTTP Transport

    %% POST Request for JSON-RPC Messages
    Client->>+ActixWeb: POST /mcp<br/>Accept: application/json, text/event-stream<br/>JSON-RPC Request
    
    %% CORS Validation
    ActixWeb->>ActixWeb: Validate CORS Origin
    alt Invalid Origin
        ActixWeb-->>Client: 403 Forbidden
    end
    
    %% Accept Header Validation
    ActixWeb->>ActixWeb: Check Accept Header
    alt Missing required types
        ActixWeb-->>Client: 400 Bad Request
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
    end
    Session->>-ActixWeb: Session ID
    
    %% Message Processing
    ActixWeb->>ActixWeb: Parse JSON-RPC Message(s)
    
    alt Only Responses/Notifications
        ActixWeb->>-Client: 202 Accepted
    else Contains Requests
        ActixWeb->>+Handler: Handle Request(s)
        Handler->>Handler: Parse JSON-RPC
        Handler->>Handler: Route Method
        
        alt Single Request
            Handler->>+Feature: Process Request
            Feature->>Feature: Execute Logic
            Feature->>-Handler: Response Data
            Handler->>-ActixWeb: JSON-RPC Response
            
            %% Return JSON Response
            ActixWeb->>-Client: 200 OK<br/>Content-Type: application/json<br/>Mcp-Session-Id: <session-id>
            
        else Multiple Requests/Complex
            Handler->>+Stream: Create SSE Stream
            Stream->>Stream: Set Headers:<br/>Content-Type: text/event-stream<br/>Cache-Control: no-cache<br/>Connection: keep-alive
            
            %% Process each request
            loop For each request
                Handler->>+Feature: Process Request
                Feature->>Feature: Execute Logic
                Feature->>-Handler: Response Data
                Handler->>Stream: Send Response
                Stream->>Client: data: {"jsonrpc":"2.0","result":{...},"id":1}\n\n
            end
            
            Handler->>-Stream: Close Stream
            ActixWeb->>-Client: SSE Stream Complete
        end
    end
    
    Note over Client, Feature: Optional GET for Server-Initiated Messages

    %% Optional GET Request for SSE Stream
    Client->>+ActixWeb: GET /mcp<br/>Accept: text/event-stream
    
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
    
    %% Session Management
    ActixWeb->>+Session: Get/Create Session
    Session->>-ActixWeb: Session ID
    
    %% Check for resumability
    ActixWeb->>ActixWeb: Check Last-Event-ID Header
    alt Has Last-Event-ID
        ActixWeb->>ActixWeb: Resume from Event ID
    end
    
    %% SSE Stream Setup
    ActixWeb->>+Stream: Create SSE Stream
    Stream->>Stream: Set Headers:<br/>Content-Type: text/event-stream<br/>Cache-Control: no-cache<br/>Connection: keep-alive
    Stream->>Stream: Add Session ID Header
    
    %% Initial Connection Event
    Stream->>Client: data: {"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n\n
    ActixWeb->>-Client: 200 OK<br/>SSE Stream Established
    
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
    
    Note over Client, Feature: Session Management

    %% Session Termination
    alt Client Terminates Session
        Client->>ActixWeb: DELETE /mcp<br/>Mcp-Session-Id: <session-id>
        ActixWeb->>+Session: Remove Session
        Session->>Session: Clean Up Resources
        Session->>-ActixWeb: Session Removed
        ActixWeb->>Client: 200 OK
    else Session Expires
        Session->>Session: Check Session Timeout
        Session->>Session: Remove Expired Session
        Session->>Stream: Close Associated Streams
        Stream-->>Client: Connection Closed
    end

    Note over Client, Feature: Error Handling

    %% HTTP Streaming Error Scenarios
    rect rgb(255, 240, 240)
        alt Network Error
            Stream-->>Client: Connection Lost
            Client->>Client: Attempt Reconnection
            Client->>ActixWeb: GET /mcp<br/>Last-Event-ID: <last-id>
        else Server Error
            ActixWeb->>Client: 500 Internal Server Error
        else Invalid Message
            ActixWeb->>Client: 400 Bad Request<br/>JSON-RPC Error Response
        end
    end
```

## HTTP Streaming Implementation Details

### 1. Single MCP Endpoint
- All communication uses a single `/mcp` endpoint
- POST requests handle client-to-server messages
- GET requests optionally provide server-to-client SSE streams
- DELETE requests terminate sessions

### 2. POST Request Handling
```http
POST /mcp HTTP/1.1
Accept: application/json, text/event-stream
Content-Type: application/json
Mcp-Session-Id: <session-id>

{"jsonrpc":"2.0","method":"initialize","params":{...},"id":1}
```

Response options:
- **JSON Response**: `Content-Type: application/json` for single responses
- **SSE Stream**: `Content-Type: text/event-stream` for multiple messages

### 3. GET Request Handling
```http
GET /mcp HTTP/1.1
Accept: text/event-stream
Mcp-Session-Id: <session-id>
Last-Event-ID: <event-id>
```

### 4. Session Management
- Session ID assigned during initialization via `Mcp-Session-Id` header
- All subsequent requests must include the session ID
- Sessions can be explicitly terminated with DELETE requests
- Automatic cleanup of expired sessions

### 5. Message Format
HTTP streaming uses standard JSON-RPC 2.0 format:
```json
{"jsonrpc":"2.0","method":"example","params":{},"id":1}
```

SSE events follow the standard format:
```
data: <JSON-RPC message>\n\n
```

### 6. Resumability and Redelivery
- Servers can attach `id` fields to SSE events
- Clients can resume with `Last-Event-ID` header
- Event IDs are unique per stream within a session

### 7. Error Handling
- **Parse Errors**: HTTP 400 with JSON-RPC error response
- **Authentication Errors**: HTTP 401/403
- **Server Errors**: HTTP 500 with error details
- **Session Errors**: HTTP 404 for invalid sessions

### 8. Security Considerations
- Origin header validation for CORS protection
- Bind to localhost for local servers
- Proper authentication and session management
- Protection against DNS rebinding attacks

## MCP Inspector Compatibility

The HTTP streaming implementation maintains full compatibility with the MCP Inspector client:
- Standard JSON-RPC 2.0 message protocol
- Proper CORS handling
- Session-based connection management
- Optional SSE streaming for real-time updates
- Backwards compatibility detection

## Performance Characteristics

- **Flexibility**: Choose between JSON responses and SSE streaming
- **Scalability**: Async handling of multiple concurrent connections
- **Reliability**: Session management and resumable streams
- **Efficiency**: Single endpoint reduces complexity
