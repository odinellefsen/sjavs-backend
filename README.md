# Sjavs Backend

A high-performance, real-time backend server for **Sjavs**, a traditional Faroese card game of the Schafkopf family. Built with Rust using modern async/await patterns to support multiplayer gaming with WebSocket communication and JWT authentication.

## 🎮 About Sjavs

Sjavs (pronounced "shouse") is a 4-player trick-taking card game popular in the Faroe Islands. This backend provides the foundation for online multiplayer matches with:

- **4-player partnerships** (2v2 teams)
- **32-card deck** with complex trump hierarchy
- **Bidding phase** for trump suit selection
- **Real-time card play** with WebSocket communication
- **Scoring system** with rubber matches

## 🏗️ Architecture

### Technology Stack

- **Runtime**: Rust with Tokio async runtime
- **Web Framework**: Axum (modern async web framework)
- **Authentication**: JWT with Clerk integration
- **Database**: Redis with connection pooling
- **Real-time Communication**: WebSockets
- **CORS**: Configured for web client integration

### Key Components

```
src/
├── main.rs                 # Application entry point & server setup
├── auth.rs                 # JWT verification with JWKS caching
├── auth_layer.rs           # Authentication middleware
├── api/                    # REST API endpoints
│   ├── routes.rs           # Route definitions
│   └── handlers/           # Request handlers
│       ├── normal_match.rs # Match creation
│       ├── normal_match_join.rs # Join match
│       ├── normal_match_leave.rs # Leave match
│       └── debug.rs        # Debug utilities
├── websocket/              # Real-time communication
│   ├── handler.rs          # WebSocket connection management
│   ├── types.rs            # Message types
│   ├── routes.rs           # WebSocket routes
│   └── events/             # Game events
│       ├── join.rs         # Join game events
│       ├── team_up_request.rs # Team formation
│       └── team_up_response.rs # Team responses
└── redis/                  # Data persistence layer
    ├── normal_match/       # Match data structures
    ├── player/             # Player management
    ├── pubsub/             # Real-time messaging
    └── notification/       # Game notifications
```

## 🚀 Getting Started

### Prerequisites

- **Rust** (1.70 or later)
- **Redis Server** (local or remote)
- **Clerk Account** (for authentication)

### Installation

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd sjavs-backend
   ```

2. **Install dependencies**
   ```bash
   cargo build
   ```

3. **Set up Redis**
   ```bash
   # Install Redis (macOS)
   brew install redis
   
   # Start Redis server
   redis-server
   ```

4. **Configure environment**
   
   Update the Redis URL in `src/main.rs` if needed:
   ```rust
   let mut cfg = Config::from_url("redis://127.0.0.1/");
   ```

5. **Run the server**
   ```bash
   cargo run
   ```

The server will start on `http://localhost:3000`

## 🔧 Configuration

### Redis Connection Pool

The application uses `deadpool-redis` with optimized settings:

```rust
// Configured for high concurrency (1000+ users)
cfg.pool = Some(deadpool_redis::PoolConfig::new(30)); // 30 max connections
```

### CORS Settings

Currently configured for development:
```rust
.allow_origin("http://192.168.1.187:5173") // Frontend URL
```

### JWT Authentication

Uses Clerk for authentication with advanced JWKS caching:
- **24-hour cache TTL** (industry standard)
- **5-minute refresh rate limiting**
- **Automatic key rotation handling**
- **Graceful fallback** during outages

## 📡 API Endpoints

### REST API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/normal-match` | Create a new match |
| `POST` | `/normal-match/join` | Join an existing match |
| `DELETE` | `/normal-match/leave` | Leave a match |
| `POST` | `/debug/flush` | Clear Redis cache (debug) |

### Authentication

All endpoints require JWT authentication via query parameter:
```
GET /normal-match?token=<jwt_token>
```

### WebSocket Events

Connect to `/ws?token=<jwt_token>` for real-time communication:

| Event | Direction | Description |
|-------|-----------|-------------|
| `join` | Client → Server | Join a game |
| `team_up_request` | Client → Server | Request team formation |
| `team_up_response` | Client → Server | Respond to team request |
| `game_update` | Server → Client | Game state changes |

## 🎯 Performance Features

### Optimized for Scale

- **1000+ concurrent users** support
- **Connection pooling** with 30 Redis connections
- **JWKS caching** eliminates auth bottlenecks
- **Pub/Sub messaging** for real-time updates
- **Async/await** throughout for non-blocking operations

### Caching Strategy

- **JWKS keys**: 24-hour cache with automatic refresh
- **Game state**: Redis persistence with pub/sub
- **Connection registry**: In-memory with automatic cleanup

## 🏛️ Data Models

### Match Structure

```rust
pub struct NormalMatch {
    pub id: String,
    pub pin: u32,                    // 4-digit join code
    pub status: NormalMatchStatus,   // waiting/in_progress/completed
    pub number_of_crosses: u32,      // Rubber match length
    pub current_cross: u32,          // Current game in rubber
    pub created_timestamp: u64,
}
```

### Game State Management

- **Redis persistence** for match data
- **In-memory state** for active connections
- **Pub/Sub notifications** for real-time updates
- **Partnership tracking** for team games

## 🔒 Security

### Authentication Flow

1. **Frontend**: Obtains JWT from Clerk
2. **Client**: Sends requests with `?token=<jwt>`
3. **Middleware**: Validates JWT using cached JWKS
4. **Handler**: Processes authenticated request

### Key Security Features

- **JWT signature verification** with RSA-256
- **Token expiration validation**
- **Issuer verification** (Clerk-specific)
- **JWKS key rotation** handling
- **CORS protection** for web clients

## 🧪 Development

### Running Tests

```bash
cargo test
```

### Debug Mode

Use the debug endpoint to clear Redis state:
```bash
curl -X POST http://localhost:3000/debug/flush?token=<jwt>
```

### Monitoring

The application provides console logging for:
- Redis connection status
- JWKS cache hits/misses
- WebSocket connections
- Game state changes

## 🔮 Future Enhancements

### Planned Features

- **Sjavs game rules engine** (bidding, card play, scoring)
- **Advanced validation** for game actions
- **Tournament support** with brackets
- **Spectator mode** for observers
- **Replay system** for completed games

### Scalability Improvements

- **Redis clustering** for horizontal scaling
- **Load balancing** with session affinity
- **Metrics collection** with Prometheus
- **Rate limiting** for API protection

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Anthony Smith** for documenting Sjavs rules
- **Clerk** for authentication infrastructure
- **Rust community** for excellent async ecosystem
- **Faroese players** who preserve this traditional game 