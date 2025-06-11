# Architecture Overview

This diagram shows the high-level system architecture of the MCP server implementation, including all major components and their relationships.

```mermaid
graph TB
    subgraph "Client Layer"
        MC[MCP Inspector Client]
        HC[HTTP Client]
        SC[STDIO Client]
    end

    subgraph "Transport Layer"
        HT[HTTP Transport<br/>Actix Web Server]
        ST[STDIO Transport]
        SM[Session Manager]
        
        HT --> SM
    end

    subgraph "Protocol Layer"
        PH[Protocol Handler<br/>JSON-RPC Router]
        VM[Validation Module]
        PM[Protocol Messages]
        
        PH --> VM
        PH --> PM
    end

    subgraph "Feature Managers"
        TM[Tool Manager<br/>Dynamic Registration]
        RM[Resource Manager<br/>File System & HTTP]
        PRM[Prompt Manager<br/>Template Engine]
        SAM[Sampling Manager<br/>LLM Integration]
        LM[Logging Manager]
        CM[Completion Manager]
    end

    subgraph "Business Logic"
        TH[Tool Handlers<br/>Echo, Calculator, etc.]
        RP[Resource Providers<br/>FileSystem, HTTP]
        PG[Prompt Generators<br/>Greeting, CodeReview]
        
        TM --> TH
        RM --> RP
        PRM --> PG
    end

    subgraph "Configuration & State"
        CFG[Config Manager<br/>TOML/JSON/YAML]
        AUTH[Auth Manager<br/>API Key, JWT, Bearer]
        ERR[Error Handler<br/>Structured Errors]
        LOG[Logger<br/>Tracing & Structured Logs]
    end

    subgraph "Storage & Persistence"
        FS[File System<br/>Resources & Templates]
        MEM[In-Memory State<br/>Sessions & Cache]
        EXT[External APIs<br/>HTTP Resources]
    end

    %% Client connections
    MC -.->|HTTP/SSE| HT
    HC -.->|HTTP/SSE| HT
    SC -.->|STDIN/STDOUT| ST

    %% Transport to Protocol
    HT --> PH
    ST --> PH

    %% Protocol to Features
    PH --> TM
    PH --> RM
    PH --> PRM
    PH --> SAM
    PH --> LM
    PH --> CM

    %% Cross-cutting concerns
    CFG -.-> HT
    CFG -.-> ST
    CFG -.-> PH
    CFG -.-> TM
    CFG -.-> RM
    CFG -.-> PRM

    AUTH -.-> HT
    AUTH -.-> PH

    ERR -.-> PH
    ERR -.-> TM
    ERR -.-> RM
    ERR -.-> PRM

    LOG -.-> HT
    LOG -.-> PH
    LOG -.-> TM

    %% Storage connections
    RP --> FS
    RP --> EXT
    SM --> MEM
    TM --> MEM
    PRM --> FS

    %% Styling
    classDef client fill:#e1f5fe
    classDef transport fill:#f3e5f5
    classDef protocol fill:#e8f5e8
    classDef feature fill:#fff3e0
    classDef business fill:#fce4ec
    classDef config fill:#f1f8e9
    classDef storage fill:#e0f2f1

    class MC,HC,SC client
    class HT,ST,SM transport
    class PH,VM,PM protocol
    class TM,RM,PRM,SAM,LM,CM feature
    class TH,RP,PG business
    class CFG,AUTH,ERR,LOG config
    class FS,MEM,EXT storage
```

## Component Descriptions

### Client Layer
- **MCP Inspector Client**: Official MCP client for testing and debugging
- **HTTP Client**: Any HTTP client supporting JSON-RPC over HTTP/SSE
- **STDIO Client**: Clients using standard input/output communication

### Transport Layer
- **HTTP Transport**: Actix Web-based HTTP server with SSE support
- **STDIO Transport**: Standard input/output transport for subprocess communication
- **Session Manager**: HTTP session lifecycle management with automatic cleanup

### Protocol Layer
- **Protocol Handler**: Central JSON-RPC message router and processor
- **Validation Module**: Request/response validation against MCP specification
- **Protocol Messages**: MCP message type definitions and serialization

### Feature Managers
- **Tool Manager**: Dynamic tool registration and execution framework
- **Resource Manager**: File system and HTTP resource access with subscriptions
- **Prompt Manager**: Template-based prompt generation with Handlebars
- **Sampling Manager**: LLM sampling and message creation capabilities
- **Logging Manager**: Structured logging with configurable levels
- **Completion Manager**: Argument completion for prompts and resources

### Business Logic
- **Tool Handlers**: Concrete tool implementations (Echo, Calculator, etc.)
- **Resource Providers**: File system and HTTP resource access implementations
- **Prompt Generators**: Template-based prompt generation implementations

### Configuration & State
- **Config Manager**: TOML/JSON/YAML configuration loading and validation
- **Auth Manager**: Authentication and authorization (API Key, JWT, Bearer)
- **Error Handler**: Structured error handling and JSON-RPC error responses
- **Logger**: Tracing-based structured logging with multiple output formats

### Storage & Persistence
- **File System**: Local file access for resources and templates
- **In-Memory State**: Session storage, caching, and runtime state
- **External APIs**: HTTP-based external resource access

## Key Design Principles

1. **Separation of Concerns**: Clear boundaries between transport, protocol, and business logic
2. **Modularity**: Pluggable components with well-defined interfaces
3. **Thread Safety**: All components designed for concurrent access
4. **Error Handling**: Comprehensive error propagation with structured responses
5. **Configuration**: Flexible configuration with sensible defaults
6. **Extensibility**: Dynamic registration system for tools and resources
