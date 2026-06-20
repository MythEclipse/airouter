# AIRouter

**LLM Gateway Proxy** — Rust-native API gateway with OpenAI & Anthropic compatible endpoints. Routes requests to upstream LLM providers with load balancing, fallback chains, rate limiting, and format translation.

## Features

- **OpenAI-compatible API** — `/v1/chat/completions`, `/v1/models`
- **Anthropic-compatible API** — `/v1/messages`
- **Multi-provider routing** — single, fallback chain strategies
- **Format translation** — OpenAI ↔ Anthropic request/response conversion
- **Rate limiting** — per-API-key token bucket
- **Bearer auth** — virtual API key validation
- **Observability** — structured JSON tracing, Prometheus-ready

## Quick Start

```bash
# Configure
cp config.example.yaml config.yaml
# Edit config.yaml with your API keys

# Run
AIROUTER_CONFIG=config.yaml cargo run
```

## Configuration

See `config.example.yaml` for full reference.

```yaml
server:
  host: "0.0.0.0"
  port: 3000

keys:
  - sk-test-abc123

providers:
  - name: "openai"
    type: openai
    api_key: "${OPENAI_API_KEY}"
    base_url: "https://api.openai.com/v1"
    models: ["gpt-4o", "gpt-4o-mini"]

routes:
  - model: "gpt-4o"
    strategy: single
    provider: "openai"

  - model: "*"
    strategy: fallback
    providers: ["openai", "groq"]
```

## API

### Chat Completions (OpenAI format)

```bash
curl http://localhost:3000/v1/chat/completions \
  -H "Authorization: Bearer sk-test-abc123" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"hello"}]}'
```

### Chat Completions (Anthropic format)

```bash
curl http://localhost:3000/v1/messages \
  -H "Authorization: Bearer sk-test-abc123" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-4-20250514","messages":[{"role":"user","content":"hello"}],"max_tokens":256}'
```

### List Models

```bash
curl http://localhost:3000/v1/models -H "Authorization: Bearer sk-test-abc123"
```

## Routes

| Path | Description |
|------|-------------|
| `GET /health` | Health check |
| `POST /v1/chat/completions` | OpenAI chat completions |
| `GET /v1/models` | List available models |
| `POST /v1/messages` | Anthropic messages |
| `POST /openai/v1/chat/completions` | OpenAI alt path |
| `POST /anthropic/v1/messages` | Anthropic alt path |

## Architecture

```
Client → Auth Middleware → Router Engine → Provider → Upstream API
                                           └→ Fallback Chain
```

Built with **Axum** (Tower middleware stack) for the backend.

## License

MIT
