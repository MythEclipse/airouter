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

## Key Fixes & Lessons

### 1. WASM Entry Point
**❌ Yang salah:** `fn main()` di `main.rs` dengan `crate-type = ["cdylib", "rlib"]`
- Trunk build **lib** target (cdylib), bukan **bin** target
- WASM file hash tidak pernah berubah karena lib target tidak punya `main()`
- WASM loaded (`wasmBindings: true`) tapi blank karena `main()` tidak pernah dipanggil

**✅ Solusi:** 
- Hapus `main.rs`, taruh semua di `lib.rs`
- Pakai `#[wasm_bindgen(start)] pub fn main()` di `lib.rs`
- `autobins = false` di Cargo.toml biar cargo tidak auto-detect `main.rs` sebagai bin

### 2. Leptos 0.6 vs 0.7 API
**❌ Yang salah:** Pakai `view! { cx, ... }` atau `use leptos::prelude::*` (Leptos 0.7 API)

**✅ Solusi:** Leptos 0.6:
- `view! { ... }` — tanpa `cx` parameter
- `mount_to_body(|| view! { <App/> })` — tanpa `cx` di closure
- `#[component] fn Name() -> impl IntoView` — tanpa `cx: Scope`
- `use leptos::*;` bukan `leptos::prelude`
- `leptos_router` CSR dengan `features = ["csr"]`

### 3. Auth Middleware Placement
**❌ Yang salah:** `router.layer(auth_middleware)` membungkus SEMUA request termasuk yang tidak match (menyebabkan frontend kena auth)

**✅ Solusi:** Pakai `route_layer()` pada sub-router API, bukan `layer()`:
```rust
api_routes.route_layer(from_fn_with_state(state, auth_middleware))
```
Auth middleware otomatis skip request yang tidak match route di sub-router itu.

### 4. Frontend Auth Bypass
**❌ Yang salah:** Auth middleware nangkep request ke static files dan fallback_service karena ditempatkan terlalu tinggi di router chain.

**✅ Solusi:** Pisahkan routing:
```rust
Router::new()
    .merge(api_routes.route_layer(auth))  // API → auth
    .fallback_service(ServeDir::new("frontend-dist")) // static → no auth
```

### 5. CSS Not Copied
**❌ Yang salah:** Trunk tidak otomatis copy `style/` directory. CSS 404.

**✅ Solusi:** Manual copy setelah trunk build:
```bash
cp -r frontend/style frontend-dist/style
```

### 6. WASM Cache Issue
**❌ Yang salah:** `cargo clean` di root project tidak membersihkan `frontend/target/`. Build tetap dari cache.

**✅ Solusi:** Hapus manual `frontend/target/` atau `rm -rf frontend/target` kalau wasm build aneh.

### 7. Module `components` Namespace Conflict
**❌ Yang salah:** `use leptos_router::components::A;` conflict dengan `src/components/` module lokal.

**✅ Solusi:** Import langsung: `use leptos_router::A;` (tanpa `components::`)

### 8. Request `body` Moved After Primary Provider Call
**❌ Yang salah:** Fallback loop pake `body.clone()` tapi body sudah move ke primary provider.

**✅ Solusi:** Clone sebelum panggil provider pertama, atau restructure fallback logic.

### 9. Env Var Resolution di Config
**❌ Yang salah:** `Settings::from_file()` tidak resolve `${VAR}` untuk `api_key` fields.

**✅ Solusi:** Loop semua provider setelah deserialize, panggil `resolve_env()` untuk setiap `api_key`.

### 10. Google Fonts / External Resources Not Blocked
Headless Chromium di environment terbatas kadang gagal load external resources. Pastikan semua resource self-hosted.

## Build Commands

```bash
# Backend
cargo build --release           # production
cargo build                     # debug
cargo run                       # dev server

# Frontend
cd frontend && trunk build --dist ../frontend-dist   # CSR build
cd frontend && trunk serve --dist ../frontend-dist   # dev with proxy

# Full run
./start.sh                      # build backend + frontend + run
make run                        # same thing
make dev                        # cargo run only (no frontend build)

# Tests
cargo test                      # 87 unit tests
bash test.sh                    # 23 shell integration tests
cd e2e && node test.mjs         # 25 Playwright E2E tests
make test-all                   # all tests
```

## Provider Types

| Type | Auth | Endpoint |
|------|------|----------|
| `openai` | `Bearer ${key}` | `https://api.openai.com/v1` |
| `anthropic` | `x-api-key ${key}` | `https://api.anthropic.com/v1` |
| `opencode_free` | `Bearer public` + `x-opencode-client` | `https://opencode.ai/zen/v1` |
| `mimo_free` | JWT bootstrap | `https://api.xiaomimimo.com/api/free-ai` |
| `openai_compat` | `Bearer ${key}` or none | user-defined |

Free providers (opencode, mimo) built-in — zero config needed.
