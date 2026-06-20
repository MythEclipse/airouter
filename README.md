# AIRouter

**LLM Gateway Proxy** — Rust-native API gateway with OpenAI & Anthropic compatible endpoints. Routes requests to upstream LLM providers with load balancing, fallback chains, fusion, rate limiting, and format translation.

## Features

- **OpenAI-compatible API** — `/v1/chat/completions`, `/v1/models`
- **Anthropic-compatible API** — `/v1/messages`
- **Dashboard UI** — manage providers, routes, API keys via web interface
- **9 built-in providers** — seed automatically on first run
- **Multi-strategy routing** — Single, Fallback chain, Round-Robin, Fusion (parallel fan-out)
- **Format translation** — OpenAI ↔ Anthropic request/response conversion
- **Rate limiting** — per-API-key token bucket
- **Bearer auth** — virtual API key validation
- **Observability** — structured JSON tracing, Prometheus-ready

## Quick Start

```bash
# Requirements
# - PostgreSQL (provide DATABASE_URL in .env)
# - Redis (provide REDIS_URL in .env, default redis://127.0.0.1:6379)
# - Rust nightly, Trunk (wasm-pack)

# Copy & edit .env
cp .env.example .env

# Run (builds backend + frontend + starts server)
./start.sh

# Dashboard: http://localhost:3000
# Default API key: sk-test-abc123
```

## Provider Types

Providers are managed through the Dashboard UI and stored in the database.
On first run, 9 providers are seeded automatically:

| Type | Category | Default Endpoint | Models |
|------|----------|-----------------|--------|
| `opencode_free` | **Free** (no key) | `opencode.ai/zen/v1` | Kimi K2.6, GLM 5, Qwen, MiniMax |
| `mimo_free` | **Free** (no key) | `xiaomimimo.com/api/free-ai` | MiMo V2.5, V2 Omni, V2 Flash |
| `gemini` | **Free Tier** | `generativelanguage.googleapis.com` | Gemini 2.5 Pro, 2.0 Flash |
| `groq` | **Free Tier** | `api.groq.com/openai/v1` | Llama 3.3, Mixtral, DeepSeek R1 |
| `openai` | **API Key** | `api.openai.com/v1` | GPT-4o, GPT-4o-mini, o3, o4-mini |
| `anthropic` | **API Key** | `api.anthropic.com/v1` | Claude Sonnet 4, Opus 4, Haiku |
| `deepseek` | **API Key** | `api.deepseek.com/v1` | DeepSeek Chat, DeepSeek Reasoner |
| `openrouter` | **API Key** | `openrouter.ai/api/v1` | GPT-4o, Claude Sonnet 4, Gemini 2.0 |
| `ollama` | **Local** | `localhost:11434/v1` | Llama 3.2, Mistral |

> **No API key in env vars.** API keys are stored securely in the database via the Dashboard.

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
| `GET /api/dashboard` | Dashboard data (auth required) |
| `GET/POST /api/dashboard/providers` | Provider CRUD |
| `GET/POST/PUT/DELETE /api/dashboard/routes` | Route CRUD |
| `GET/POST/DELETE /api/dashboard/api-keys` | API key management |
| `GET/PUT /api/dashboard/settings` | Server settings |
| `GET /api/dashboard/provider-types` | Known provider type reference |

## Routing Strategies

| Strategy | Description |
|----------|-------------|
| **Single** | Route to exactly one provider |
| **Fallback** | Try providers in order, stop at first success |
| **Round-Robin** | Rotate starting provider per request (with sticky limit) |
| **Fusion** | Fan-out to ALL providers in parallel, judge synthesizes best answer (non-streaming) |

## Architecture

```
Client → Auth Middleware → Route Engine → Provider → Upstream API
                                       └→ Fallback / Round-Robin / Fusion
```

```
┌─────────────┐   ┌──────────┐   ┌──────────────┐   ┌─────────────┐
│  Dashboard  │ → │ Database │ → │ Route Engine │ → │  Providers  │
│  (Leptos)   │   │(Postgres)│   │   (Axum)     │   │ (Rust impl) │
└─────────────┘   └──────────┘   └──────────────┘   └─────────────┘
                                    │       ↑
                                    ↓       │
                                 ┌──────────┴──┐
                                 │    Redis    │
                                 │ (cooldowns, │
                                 │  counters)  │
                                 └─────────────┘
```

Built with **Axum** (Tower middleware stack) for the backend, **Leptos 0.6 CSR** for the dashboard.

## License

MIT
