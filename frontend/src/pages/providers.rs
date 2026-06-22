use leptos::*;
use crate::api::*;
use crate::components::tag_input::TagInput;
use crate::components::skeleton::SkeletonTable;
use crate::components::provider_icon::category_accent;
use crate::components::provider_card::ProviderCard;
use wasm_bindgen::prelude::*;

const CATEGORY_ORDER: &[&str] = &["free", "free-tier", "api-key", "oauth", "web-cookie"];
const CATEGORY_LABELS: &[(&str, &str)] = &[
    ("free",       "Free (No Key)"),
    ("free-tier",  "Free Tier"),
    ("api-key",    "API Key"),
    ("oauth",      "OAuth"),
    ("web-cookie", "Web Cookie"),
];

fn section_label(cat: &str) -> String {
    for (c, label) in CATEGORY_LABELS {
        if *c == cat { return label.to_string(); }
    }
    cat.to_string()
}

// ─── OAuth flow state ────────────────────────────────────────────
#[derive(Debug, Clone)]
enum OAuthFlowState {
    Idle,
    Authorizing { provider: String },
    WaitingDevice { provider: String, device_code: String, interval: u64 },
    Done,
    Error(String),
}

#[component]
pub fn Providers() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderDetail>::new());
    let provider_types = create_rw_signal(Vec::<ProviderTypeInfo>::new());
    let provider_types_loaded = create_rw_signal(false);
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let edit_id = create_rw_signal(Option::<String>::None);
    let form_name = create_rw_signal(String::new());
    let form_type = create_rw_signal("openai_compat".into());
    let form_key = create_rw_signal(String::new());
    let form_url = create_rw_signal(String::new());
    let form_models = create_rw_signal(Vec::new());
    let form_caps = create_rw_signal(Vec::new());
    let saving = create_rw_signal(false);
    let delete_id = create_rw_signal(Option::<String>::None);
    let expanded_id = create_rw_signal(Option::<String>::None);
    let model_test_results = create_rw_signal(std::collections::HashMap::<String, TestProviderResponse>::new());
    let testing_model = create_rw_signal(Option::<String>::None);

    // OAuth specific
    let oauth_flow = create_rw_signal(OAuthFlowState::Idle);
    let connections = create_rw_signal(Vec::<OAuthConnectionItem>::new());
    let oauth_device_code_data = create_rw_signal(Option::<DeviceCodeData>::None);

    // ─── Load types ────────────────────────────────────────────────
    spawn_local({
        let pt = provider_types.clone();
        let pt_loaded = provider_types_loaded.clone();
        async move {
            if let Ok(data) = fetch_provider_types().await {
                pt.set(data);
            }
            pt_loaded.set(true);
        }
    });

    let load = move || {
        spawn_local({
            let providers = providers.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match fetch_providers().await {
                    Ok(data) => { providers.set(data); loading.set(false); }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            }
        });
        spawn_local({
            let connections = connections.clone();
            async move {
                if let Ok(data) = fetch_connections().await {
                    connections.set(data);
                }
            }
        });
    };
    load();

    // ─── OAuth popup callback listener ──────────────────────────────
    {
        use wasm_bindgen::prelude::Closure;
        let oauth_flow = oauth_flow.clone();
        let providers = providers.clone();
        let loading = loading.clone();
        let error = error.clone();
        let connections = connections.clone();
        if let Some(w) = web_sys::window() {
            let closure = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let val = ev.data();
                if !val.is_object() { return; }
                let get_str = |key: &str| -> Option<String> {
                    js_sys::Reflect::get(&val, &wasm_bindgen::JsValue::from_str(key)).ok()
                        .and_then(|v| v.as_string())
                };
                if get_str("type").as_deref() != Some("oauth_callback") { return; }
                let code = get_str("code").unwrap_or_default();
                let state = get_str("state").unwrap_or_default();
                if code.is_empty() { return; }
                if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
                    let saved_state = storage.get_item("oauth_state").ok().flatten().unwrap_or_default();
                    if !saved_state.is_empty() && saved_state != state { return; }
                    let provider = storage.get_item("oauth_provider").ok().flatten().unwrap_or_default();
                    let verifier = storage.get_item("oauth_verifier").ok().flatten().unwrap_or_default();
                    if provider.is_empty() { return; }
                    let pf = oauth_flow.clone();
                    let providers = providers.clone();
                    let loading = loading.clone();
                    let error = error.clone();
                    let connections = connections.clone();
                    spawn_local(async move {
                        let redirect = web_sys::window()
                            .and_then(|w| Some(format!("{}/callback.html", w.location().origin().ok()?)))
                            .unwrap_or_else(|| "http://localhost:3000/callback.html".to_string());
                        match exchange_code(&provider, &code, &redirect, &verifier).await {
                            Ok(_) => {
                                pf.set(OAuthFlowState::Done);
                                loading.set(true);
                                if let Ok(data) = fetch_providers().await { providers.set(data); }
                                if let Ok(data) = fetch_connections().await { connections.set(data); }
                                loading.set(false);
                            }
                            Err(e) => error.set(e),
                        }
                    });
                }
            });
            w.add_event_listener_with_callback("message", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
    }

    // ─── Type groups ──────────────────────────────────────────────
    let type_groups = create_memo(move |_| {
        let types = provider_types.get();
        let mut free = Vec::new();
        let mut free_tier = Vec::new();
        let mut apikey = Vec::new();
        let mut oauth = Vec::new();
        let mut webcookie = Vec::new();
        for t in &types {
            match t.category.as_str() {
                "free" => free.push(t.clone()),
                "free-tier" => free_tier.push(t.clone()),
                "oauth" => oauth.push(t.clone()),
                "web-cookie" => webcookie.push(t.clone()),
                _ => apikey.push(t.clone()),
            }
        }
        (free, free_tier, apikey, oauth, webcookie)
    });

    // ─── Section groups ───────────────────────────────────────────
    let sections = create_memo(move |_| {
        let provs = providers.get();
        let mut secs: Vec<(&str, Vec<ProviderDetail>)> = Vec::new();
        for cat in CATEGORY_ORDER {
            let items: Vec<ProviderDetail> = provs.iter()
                .filter(|p| p.category == *cat)
                .cloned()
                .collect();
            if !items.is_empty() { secs.push((cat, items)); }
        }
        secs
    });

    // ─── OAuth helpers ────────────────────────────────────────────
    let supports_device_code = move |p: &str| -> bool {
        matches!(p, "github" | "kimi_coding" | "codebuddy" | "cursor" | "kilocode")
    };

    let do_oauth_login = {
        let oauth_flow = oauth_flow.clone();
        move |provider_name: String| {
            oauth_flow.set(OAuthFlowState::Idle);
            let p = provider_name.clone();
            let pf = oauth_flow.clone();
            spawn_local(async move {
                match initiate_authorize(&p).await {
                    Ok(resp) => {
                        // Store OAuth state in sessionStorage for callback
                        if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
                            let _ = storage.set_item("oauth_state", &resp.state);
                            let _ = storage.set_item("oauth_verifier", &resp.code_verifier);
                            let _ = storage.set_item("oauth_provider", &p);
                        }
                        if let Some(window) = web_sys::window() {
                            let _ = window.open_with_url_and_target_and_features(
                                &resp.auth_url,
                                &format!("oauth_{}", p),
                                "width=600,height=700,menubar=no,toolbar=no,location=yes",
                            );
                        }
                        pf.set(OAuthFlowState::Authorizing { provider: p.clone() });
                    }
                    Err(e) => pf.set(OAuthFlowState::Error(e)),
                }
            });
        }
    };

    let do_device_code = {
        let oauth_flow = oauth_flow.clone();
        let oauth_device_code_data = oauth_device_code_data.clone();
        move |provider_name: String| {
            oauth_flow.set(OAuthFlowState::Idle);
            let p = provider_name.clone();
            let pf = oauth_flow.clone();
            let dc = oauth_device_code_data.clone();
            spawn_local(async move {
                match initiate_device_code(&p).await {
                    Ok(data) => {
                        dc.set(Some(data.clone()));
                        pf.set(OAuthFlowState::WaitingDevice {
                            provider: p.clone(),
                            device_code: data.device_code.clone(),
                            interval: data.interval.max(5),
                        });
                        if let Some(window) = web_sys::window() {
                            let _ = window.navigator().clipboard().write_text(&data.user_code);
                        }
                    }
                    Err(e) => pf.set(OAuthFlowState::Error(e)),
                }
            });
        }
    };

    let do_import_token = {
        let oauth_flow = oauth_flow.clone();
        let providers = providers.clone();
        let loading = loading.clone();
        let connections = connections.clone();
        let error = error.clone();
        move |provider_name: String, token_val: String| {
            if token_val.is_empty() { return; }
            let p = provider_name.clone();
            let pf = oauth_flow.clone();
            let providers = providers.clone();
            let loading = loading.clone();
            let connections = connections.clone();
            let error = error.clone();
            spawn_local(async move {
                match import_token(&p, &token_val).await {
                    Ok(_) => {
                        pf.set(OAuthFlowState::Done);
                        loading.set(true);
                        if let Ok(data) = fetch_providers().await {
                            providers.set(data);
                            loading.set(false);
                        }
                        if let Ok(data) = fetch_connections().await {
                            connections.set(data);
                        }
                    }
                    Err(e) => {
                        pf.set(OAuthFlowState::Error(e.clone()));
                        error.set(e);
                    }
                }
            });
        }
    };

    // ─── Form logic ───────────────────────────────────────────────
    let show_add_form = move || {
        edit_id.set(None);
        form_name.set(String::new());
        form_type.set("openai_compat".into());
        form_key.set(String::new());
        form_url.set(String::new());
        form_models.set(Vec::new());
        form_caps.set(Vec::new());
        oauth_flow.set(OAuthFlowState::Idle);
        oauth_device_code_data.set(None);
        show_form.set(true);
    };

    let show_edit_form = move |p: ProviderDetail| {
        edit_id.set(Some(p.id.clone()));
        form_name.set(p.name);
        form_type.set(p.provider_type);
        form_key.set(String::new());
        form_url.set(p.base_url);
        form_models.set(p.models.clone());
        form_caps.set(p.capabilities.clone());
        oauth_flow.set(OAuthFlowState::Idle);
        oauth_device_code_data.set(None);
        show_form.set(true);
    };

    let selected_type_info = create_memo(move |_| {
        let t = form_type.get();
        provider_types.get().into_iter().find(|pt| pt.id == t)
    });

    let is_free_type = create_memo(move |_| {
        selected_type_info.get().map(|t| t.category == "free").unwrap_or(false)
    });
    let is_oauth_selected = create_memo(move |_| {
        selected_type_info.get().map(|t| t.category == "oauth").unwrap_or(false)
    });
    let is_webcookie_selected = create_memo(move |_| {
        selected_type_info.get().map(|t| t.category == "web-cookie").unwrap_or(false)
    });

    let do_save = move || {
        saving.set(true);
        error.set(String::new());
        let body = serde_json::json!({
            "name": form_name.get(),
            "provider_type": form_type.get(),
            "api_key": form_key.get(),
            "base_url": form_url.get(),
            "models": form_models.get(),
            "capabilities": form_caps.get(),
        });
        let body_str = serde_json::to_string(&body).unwrap_or_default();

        spawn_local({
            let providers = providers.clone();
            let loading = loading.clone();
            let error = error.clone();
            let show_form = show_form.clone();
            let edit_id = edit_id.clone();
            let saving = saving.clone();
            async move {
                let result = if let Some(id) = edit_id.get() {
                    update_provider(&id, &body_str).await
                } else {
                    create_provider(&body_str).await
                };
                match result {
                    Ok(_) => {
                        show_form.set(false);
                        loading.set(true);
                        edit_id.set(None);
                        match fetch_providers().await {
                            Ok(data) => { providers.set(data); loading.set(false); }
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                        saving.set(false);
                    }
                    Err(e) => { error.set(e); saving.set(false); }
                }
            }
        });
    };

    let do_delete = move |id: String| {
        spawn_local({
            let providers = providers.clone();
            let loading = loading.clone();
            let error = error.clone();
            let delete_id = delete_id.clone();
            async move {
                match delete_provider(&id).await {
                    Ok(()) => {
                        delete_id.set(None);
                        loading.set(true);
                        match fetch_providers().await {
                            Ok(data) => { providers.set(data); loading.set(false); }
                            Err(e) => error.set(e),
                        }
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    let handle_test_model = move |provider_id: String, model: String| {
        let key = format!("{}:{}", provider_id, model);
        if testing_model.with(|t| t.as_deref() == Some(&key)) {
            return;
        }
        testing_model.set(Some(key.clone()));
        spawn_local({
            let pid = provider_id.clone();
            let mdl = model.clone();
            let testing_model = testing_model.clone();
            let model_test_results = model_test_results.clone();
            async move {
                let result = test_provider_model(&pid, &mdl).await;
                match result {
                    Ok(r) => {
                        model_test_results.update(|m| { m.insert(key.clone(), r); });
                    }
                    Err(e) => {
                        model_test_results.update(|m| {
                            m.insert(key.clone(), TestProviderResponse {
                                ok: false, latency_ms: 0, model: mdl.clone(),
                                error: Some(e),
                            });
                        });
                    }
                }
                testing_model.set(None);
            }
        });
    };

    // ─── OAuth form renderers ─────────────────────────────────────
    let render_oauth_form = move |ptype: String, pname: String| -> Vec<Box<dyn Fn() -> leptos::HtmlElement<leptos::html::AnyElement>>> {
        let mut els: Vec<Box<dyn Fn() -> leptos::HtmlElement<leptos::html::AnyElement>>> = Vec::new();

        // Login button
        let bt_ptype = ptype.clone();
        els.push(Box::new(move || {
            let bt = bt_ptype.clone();
            let pn = pname.clone();
            view! {
                <div class="mb-4">
                    <label class="block text-xs text-secondary mb-1.5 font-medium">"OAuth Login"</label>
                    <button on:click=move|_| do_oauth_login(bt.clone())
                        class="w-full px-4 py-3 text-sm font-medium rounded-lg text-white bg-[#db6b28] hover:bg-[#c05d22] active:scale-[0.97] transition-all duration-150 flex items-center justify-center gap-2"
                    >
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1"/></svg>
                        "Login with "{pn.clone()}
                    </button>
                </div>
            }.into_any()
        }));

        // Device code
        if supports_device_code(&ptype) {
            let ptype = ptype.clone();
            els.push(Box::new(move || {
                let is_dev = oauth_flow.with(|f| matches!(f, OAuthFlowState::WaitingDevice { .. }));
                let dev_data = oauth_device_code_data.get();
                let dc_ptype = ptype.clone();
                view! {
                    <div class="mb-4">
                        <label class="block text-xs text-secondary mb-1.5 font-medium">"Or use Device Code"</label>
                        <button on:click=move|_| do_device_code(dc_ptype.clone())
                            disabled=is_dev
                            class="w-full px-4 py-2 text-sm font-medium rounded-lg border border-[#db6b28]/30 text-[#db6b28] hover:bg-[rgba(219,107,40,0.1)] disabled:opacity-50 active:scale-[0.97] transition-all duration-150 flex items-center justify-center gap-2"
                        >
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>
                            "Device Code"
                        </button>
                        {dev_data.map(|dd| view! {
                            <div class="mt-3 p-3 rounded-lg bg-surface-2 border border-border-subtle">
                                <p class="text-xs text-secondary mb-1">"Open" <a href=dd.verification_uri.clone() target="_blank" class="text-accent underline">{dd.verification_uri.clone()}</a></p>
                                <p class="text-xs text-secondary mb-1">"Enter code:"</p>
                                <p class="text-lg font-mono font-bold text-primary tracking-wider text-center py-2 bg-surface rounded-lg">{dd.user_code.clone()}</p>
                                <p class="text-xs text-muted mt-1">"Code expires in {dd.expires_in}s (copied to clipboard)"</p>
                            </div>
                        })}
                    </div>
                }.into_any()
            }));
        }

        // Token/cookie import
        if matches!(ptype.as_str(), "iflow" | "codex" | "cursor") {
            let ptype = ptype.clone();
            els.push(Box::new(move || {
                let cookie_val = create_rw_signal(String::new());
                let im_ptype = ptype.clone();
                view! {
                    <div class="mb-4">
                        <label class="block text-xs text-secondary mb-1.5 font-medium">"Or Import Token / Cookie"</label>
                        <div class="flex gap-2">
                            <input type="password" prop:value=move || cookie_val.get()
                                placeholder="paste token or cookie value..."
                                on:input=move|ev| cookie_val.set(event_target_value(&ev))
                                class="flex-1 px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"
                            />
                            <button on:click=move|_| do_import_token(im_ptype.clone(), cookie_val.get())
                                class="px-3 py-2 text-sm font-medium rounded-lg bg-accent text-white hover:bg-accent-hover active:scale-[0.97] transition-all duration-150"
                            >"Import"</button>
                        </div>
                    </div>
                }.into_any()
            }));
        }

        els
    };

    let render_webcookie_form = move |ptype: String, pname: String| -> Vec<Box<dyn Fn() -> leptos::HtmlElement<leptos::html::AnyElement>>> {
        let mut els: Vec<Box<dyn Fn() -> leptos::HtmlElement<leptos::html::AnyElement>>> = Vec::new();
        let pn = pname.clone();

        els.push(Box::new(move || {
            let cookie_val = create_rw_signal(String::new());
            let wc_ptype = ptype.clone();
            view! {
                <div class="mb-4">
                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Session Cookie"</label>
                    <p class="text-xs text-muted mb-2">"Copy the session cookie from" <span class="font-mono text-secondary">{pn.clone()}</span> "and paste below."</p>
                    <div class="flex gap-2">
                        <input type="password" prop:value=move || cookie_val.get()
                            placeholder="e.g. sso=abc123... or __Secure-next-auth.session-token=..."
                            on:input=move|ev| cookie_val.set(event_target_value(&ev))
                            class="flex-1 px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"
                        />
                        <button on:click=move|_| do_import_token(wc_ptype.clone(), cookie_val.get())
                            class="px-3 py-2 text-sm font-medium rounded-lg bg-[#db6b9a] text-white hover:bg-[#c05d82] active:scale-[0.97] transition-all duration-150"
                        >"Import Cookie"</button>
                    </div>
                </div>
            }.into_any()
        }));

        els
    };

    // ═══ VIEW ═══════════════════════════════════════════════════════
    view! {
        <div class="animate-fade-in">
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-primary">"Providers"</h1>
                    <p class="text-sm text-secondary mt-1">"Manage upstream LLM providers"</p>
                </div>
                <button on:click=move|_| show_add_form()
                    class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-accent hover:bg-accent-hover active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/></svg>
                    "Add Provider"
                </button>
            </div>

            // Error banner
            {move || (!error.get().is_empty()).then(|| view! {
                <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger">{error.get()}</p>
            })}
            {move || loading.get().then(|| view! { <SkeletonTable rows=4/> })}

            // OAuth flow status
            {move || match oauth_flow.get() {
                OAuthFlowState::Authorizing { ref provider } => Some(view! {
                    <div class="mb-4 p-3 rounded-lg bg-[rgba(219,107,40,0.1)] border border-[#db6b28]/30 text-sm text-[#db6b28] flex items-center gap-2">
                        <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
                        "Waiting for OAuth login for " {provider.clone()} "... complete the login in the popup window."
                    </div>
                }),
                OAuthFlowState::WaitingDevice { ref provider, .. } => Some(view! {
                    <div class="mb-4 p-3 rounded-lg bg-[rgba(219,107,40,0.1)] border border-[#db6b28]/30 text-sm text-[#db6b28] flex items-center gap-2">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z"/></svg>
                        "Waiting for device code authentication for " {provider.clone()} "..."
                    </div>
                }),
                OAuthFlowState::Done => Some(view! {
                    <div class="mb-4 p-3 rounded-lg bg-success/10 border border-success/30 text-sm text-success flex items-center gap-2">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>
                        "OAuth connection established! Reloading providers..."
                    </div>
                }),
                OAuthFlowState::Error(ref msg) => Some(view! {
                    <div class="mb-4 p-3 rounded-lg bg-danger-bg border border-danger/30 text-sm text-danger flex items-center gap-2">
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/></svg>
                        {msg.clone()}
                    </div>
                }),
                _ => None,
            }}

            // Delete Confirm
            {move || delete_id.get().map(|id| {
                let name = providers.with(|p| p.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default());
                let id3 = id.clone();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_| delete_id.set(None)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6 w-full max-w-md mx-4 shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-start gap-3 mb-4">
                                <div class="w-10 h-10 rounded-full bg-danger-bg flex items-center justify-center flex-shrink-0">
                                    <svg class="w-5 h-5 text-danger" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/></svg>
                                </div>
                                <div>
                                    <h3 class="text-base font-semibold text-primary">{format!("Delete \"{}\"?", name)}</h3>
                                    <p class="text-sm text-secondary mt-1">"This provider will be removed from all routes."</p>
                                </div>
                            </div>
                            <div class="flex gap-2 justify-end">
                                <button on:click=move|_| delete_id.set(None)
                                    class="px-4 py-2 text-sm font-medium rounded-lg bg-transparent border border-surface text-secondary hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150">"Cancel"</button>
                                <button on:click=move|_| do_delete(id3.clone())
                                    class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-danger hover:bg-red-600 active:scale-[0.97] transition-all duration-150">"Delete"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Modal form
            {move || (show_form.get() && provider_types_loaded.get()).then(|| {
                let is_edit = edit_id.get().is_some();
                let (free, free_tier, apikey, oauth_types, webcookie_types) = type_groups.get();
                let oauth = oauth_types;
                let webcookie = webcookie_types;
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-start justify-center pt-[10vh] z-50 animate-fade-in"
                        on:click=move|_| show_form.set(false)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-center justify-between px-6 py-4 border-b border-surface">
                                <h2 class="text-lg font-semibold text-primary">{if is_edit { "Edit Provider" } else { "Add Provider" }}</h2>
                                <button on:click=move|_| show_form.set(false) class="text-muted hover:text-primary transition-colors">
                                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                                </button>
                            </div>
                            <div class="p-6">
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Name"</label>
                                    <input type="text" prop:value=form_name.get() placeholder="e.g. my-openai"
                                        on:input=move|ev| form_name.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Type"</label>
                                    <select prop:value=form_type.get() on:change=move|ev| form_type.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors">
                                        <optgroup label="Free (No Key)">
                                            {free.into_iter().map(|t| view! { <option value=t.id.clone()>{t.display_name}</option> }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="Free Tier (Signup)">
                                            {free_tier.into_iter().map(|t| view! { <option value=t.id.clone()>{t.display_name}</option> }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="API Key (Paid)">
                                            {apikey.into_iter().map(|t| view! { <option value=t.id.clone()>{t.display_name}</option> }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="OAuth (Login)">
                                            {oauth.into_iter().map(|t| view! { <option value=t.id.clone()>{t.display_name}</option> }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="Web Cookie">
                                            {webcookie.into_iter().map(|t| view! { <option value=t.id.clone()>{t.display_name}</option> }).collect::<Vec<_>>()}
                                        </optgroup>
                                    </select>
                                </div>

                                // Auth fields based on type
                                {move || {
                                    if is_free_type.get() {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="text" disabled=true value="(no key needed)" class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else if is_oauth_selected.get() {
                                        let sel = selected_type_info.get();
                                        let pn = sel.as_ref().map(|i| i.display_name.clone()).unwrap_or_default();
                                        let pt = form_type.get();
                                        view! {
                                            {render_oauth_form(pt.clone(), pn).into_iter().map(|f| f()).collect::<Vec<_>>()}
                                        }.into_view()
                                    } else if is_webcookie_selected.get() {
                                        let sel = selected_type_info.get();
                                        let pn = sel.as_ref().map(|i| i.display_name.clone()).unwrap_or_default();
                                        let pt = form_type.get();
                                        view! {
                                            {render_webcookie_form(pt.clone(), pn).into_iter().map(|f| f()).collect::<Vec<_>>()}
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="password" prop:value=form_key.get()
                                                    placeholder=if is_edit { "(unchanged on edit)" } else { "sk-..." }
                                                    on:input=move|ev| form_key.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    }
                                }}

                                // Base URL
                                {move || {
                                    let is_free = is_free_type.get();
                                    let is_oauth = is_oauth_selected.get();
                                    let is_wc = is_webcookie_selected.get();
                                    let placeholder = if is_free || is_oauth || is_wc { "" } else { "https://api.example.com/v1" };
                                    if is_free || is_oauth || is_wc {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" disabled=true
                                                    value={if is_free { "(hardcoded)" } else { "(auto)" }}
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" prop:value=form_url.get() placeholder=placeholder
                                                    on:input=move|ev| form_url.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    }
                                }}

                                <TagInput label="Models (type + Enter or comma to add)".to_string()
                                    placeholder="e.g. gpt-4o".to_string() tags=form_models/>
                                <TagInput label="Capabilities".to_string()
                                    placeholder="e.g. vision".to_string() tags=form_caps/>

                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-surface">
                                    <button on:click=move|_| show_form.set(false)
                                        class="px-4 py-2 text-sm font-medium rounded-lg bg-transparent border border-surface text-secondary hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150">"Cancel"</button>
                                    {move || {
                                        if !is_oauth_selected.get() && !is_webcookie_selected.get() {
                                            view! {
                                                <button on:click=move|_| do_save() disabled=saving.get()
                                                    class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-accent hover:bg-accent-hover disabled:opacity-50 active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                                                    {if saving.get() { "Saving..." } else { "Save" }}
                                                </button>
                                            }.into_view()
                                        } else {
                                            view! { <span></span> }.into_view()
                                        }
                                    }}
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Provider grid sections
            {move || (!loading.get() && !show_form.get()).then(|| {
                let secs = sections.get();
                let conns = connections.get();
                let is_expanded = expanded_id;
                let deleting = delete_id;
                let test_res = model_test_results;
                let testing = testing_model;

                if secs.is_empty() {
                    view! {
                        <div class="flex flex-col items-center justify-center py-16 text-center">
                            <svg class="w-12 h-12 text-muted mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/></svg>
                            <p class="text-secondary text-sm">"No providers yet"</p>
                        </div>
                    }.into_view()
                } else {
                    view! {
                        <div class="flex flex-col gap-8">
                            {secs.into_iter().map(|(cat, items)| {
                                let accent = category_accent(cat);
                                let section_title = section_label(cat);
                                let count = items.len();
                                view! {
                                    <section>
                                        <div class="flex items-center gap-3 mb-4">
                                            <div class="w-1 h-6 rounded-full shrink-0" style=format!("background-color: {}", accent)></div>
                                            <h2 class="text-lg font-semibold text-primary">{section_title.clone()}</h2>
                                            <span class="text-xs text-muted bg-surface-2 px-2 py-0.5 rounded-full font-mono">{count}</span>
                                        </div>
                                        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                            {items.into_iter().map(|p| {
                                                let pid = p.id.clone();
                                                let conns_clone = conns.clone();
                                                let is_expanded = is_expanded;
                                                let deleting = deleting;
                                                let test_res = test_res;
                                                let testing = testing;
                                                let on_edit = show_edit_form;
                                                let on_test = handle_test_model.clone();
                                                let on_ol = do_oauth_login.clone();
                                                let on_dc = do_device_code.clone();
                                                let on_it = do_import_token.clone();

                                                view! {
                                                    <ProviderCard
                                                        provider=p
                                                        expanded=is_expanded
                                                        deleting=deleting
                                                        model_test_results=test_res
                                                        testing_model=testing
                                                        connections=conns_clone
                                                        on_edit=Box::new(move |p: ProviderDetail| show_edit_form(p))
                                                        on_test=Box::new(handle_test_model.clone())
                                                        on_oauth_login=Box::new(do_oauth_login.clone())
                                                        on_device_code=Box::new(do_device_code.clone())
                                                        on_import_token=Box::new(do_import_token.clone())
                                                    />
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </section>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_view()
                }
            })}
        </div>
    }
}
