# AIRouter — Claude Project Knowledge

## Arsitektur

### Backend: Axum 0.8
- Runtime: Tokio 1.x
- HTTP framework: Axum 0.8 (Tower middleware stack)
- Auth middleware via `from_fn_with_state`
- Static files via `ServeDir` + `fallback_service`
- **Yang bener**: `route_layer()` untuk auth per-route, bukan `layer()` pada merged router

### Frontend: Leptos 0.6 CSR + Trunk
- CSR (Client-Side Rendering) via WASM, bundle ~2.5MB (with router)
- Build: Trunk → `cd frontend && trunk build --dist ../frontend-dist`
- Entry point: `lib.rs` with `#[wasm_bindgen(start)] pub fn main()`

### Database: PostgreSQL + Redis
- **Single source of truth**: Semua provider, routes, API keys, settings dari PostgreSQL
- **No YAML config untuk providers** — `config.yaml` hanya berisi server + rate_limit
- **No env var untuk API key** — semua diisi via Dashboard UI
- Redis: cooldown management, round-robin counters, request tracking

## Pola Pikir — Provider System

### 1. Provider punya kategori, bukan hanya string type
Setiap provider type masuk kategori: `Free`, `FreeTier`, `ApiKey`, `OAuth`, `WebCookie`.  
Ini menentukan apakah perlu API key, apakah gratis, warna badge di UI, dan field apa yang ditampilkan di form.

Kategori ditentukan oleh **siapa yang menyediakan**, bukan apa yang bisa dilakukan.  
Contoh: `groq` itu FreeTier walaupun pake format OpenAI.

Implementasi:
- `KNOWN_PROVIDER_TYPES` table di `src/provider/mod.rs` — mapping type → display name → category
- `ProviderRegistry.categories` — per-provider-name lookup
- Frontend fetch `/api/dashboard/provider-types` untuk dropdown terkategori

### 2. Provider hidup di DB, bukan YAML atau env
YAML hanya untuk `server` dan `rate_limit`. Provider dan routes:
- Seed otomatis saat pertama startup (`seed_defaults()`)
- **Upsert setiap restart** — update model/url/strategi dari seed, tapi `api_key` kustom tetap aman
- User manage via Dashboard UI → CRUD API → DB
- `load_config_from_db()` adalah satu-satunya source untuk runtime

### 3. Auth tiap provider berbeda — jangan asumsi
Free provider auth caranya beda-beda. Jangan nebak — baca implementasi nyata:

| Provider | Auth Method | Header |
|----------|------------|--------|
| OpenCode Free | No Bearer | `x-opencode-client: desktop` |
| MiMo Free | JWT bootstrap | `Authorization: Bearer {jwt}`, `X-Mimo-Source: mimocode-cli-free` |
| OpenAI | API key | `Authorization: Bearer {key}` |
| Anthropic | API key | `x-api-key {key}` |

### 4. Router fallback — jangan hubungkan ke provider yg beda tipe
Route `mimo-auto` cuma di `mimo`, bukan `opencode` atau `groq`. Route `north-mini-code-free` cuma di `opencode`, bukan yang lain.  
Setiap model hanya terhubung ke provider yang benar-benar bisa handle model itu.

### 5. Default seed harus selalu sinkron dengan kode
Saat code di-update (model baru, endpoint baru), seed harus mengikut.  
Tapi data kustom (api_key user, route custom) harus tetap.  
**Upsert by name** — `existing ? update : insert` — api_key tidak di-update dari seed.

## Provider Types Reference

| Type | Category | Auth | Default Endpoint | Models (seed) |
|------|----------|------|-----------------|---------------|
| `opencode_free` | Free | `x-opencode-client: desktop` | `opencode.ai/zen/v1` | `deepseek-v4-flash-free`, `mimo-v2.5-free`, `nemotron-3-ultra-free`, `north-mini-code-free` |
| `mimo_free` | Free | JWT bootstrap | `api.xiaomimimo.com/api/free-ai` | `mimo-auto` |
| `gemini` | Free Tier | API key | `generativelanguage.googleapis.com/v1beta` | Gemini Pro/Flash |
| `groq` | Free Tier | API key | `api.groq.com/openai/v1` | Llama, Mixtral, DeepSeek |
| `openai` | ApiKey | `Bearer {key}` | `api.openai.com/v1` | GPT-4o, o3, o4-mini |
| `anthropic` | ApiKey | `x-api-key {key}` | `api.anthropic.com/v1` | Claude Sonnet/Opus/Haiku |
| `deepseek` | ApiKey | `Bearer {key}` | `api.deepseek.com/v1` | DeepSeek Chat/Reasoner |
| `openrouter` | ApiKey | `Bearer {key}` | `openrouter.ai/api/v1` | Multi-model access |
| `ollama` | ApiKey | optional | `localhost:11434/v1` | Local models |

## Build Commands

```bash
# Backend
cargo build --release           # production
cargo run                       # dev server

# Frontend
cd frontend && trunk build --dist ../frontend-dist   # CSR build

# Full run
./start.sh                      # build backend + frontend + run

# Tests
cargo test                      # 61 unit + integration tests
```

## Database

Tables: `providers`, `routes`, `api_keys`, `server_config`, `rate_limit_config`

Migration: `migrations/001_initial.sql`

Seed: `config::db::seed_defaults()` — upsert by name, api_key user tidak dihapus.
