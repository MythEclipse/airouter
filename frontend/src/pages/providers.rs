use leptos::*;
use crate::api::*;
use crate::components::tag_input::TagInput;
use crate::components::skeleton::SkeletonTable;

#[component]
pub fn Providers() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderDetail>::new());
    let provider_types = create_rw_signal(Vec::<ProviderTypeInfo>::new());
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
    // Expand & test state
    let expanded_id = create_rw_signal(Option::<String>::None);
    let model_test_results = create_rw_signal(std::collections::HashMap::<String, TestProviderResponse>::new());
    let testing_model = create_rw_signal(Option::<String>::None); // "provider_id:model"

    // Load provider types for the dropdown
    spawn_local({
        let pt = provider_types.clone();
        async move {
            let url = "/api/dashboard/provider-types";
            let window = web_sys::window().unwrap();
            let opts = web_sys::RequestInit::new();
            opts.set_method("GET");
            opts.set_mode(web_sys::RequestMode::Cors);
            let request = web_sys::Request::new_with_str_and_init(url, &opts).unwrap();
            let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await;
            if let Ok(r) = resp {
                let r: web_sys::Response = wasm_bindgen::JsCast::dyn_into(r).unwrap();
                let json = wasm_bindgen_futures::JsFuture::from(r.json().unwrap()).await;
                if let Ok(j) = json {
                    if let Ok(data) = serde_wasm_bindgen::from_value::<Vec<ProviderTypeInfo>>(j) {
                        pt.set(data);
                    }
                }
            }
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
    };
    load();

    // Categorized provider type groups for the dropdown
    let type_groups = create_memo(move |_| {
        let types = provider_types.get();
        let mut free = Vec::new();
        let mut free_tier = Vec::new();
        let mut apikey = Vec::new();
        for t in &types {
            match t.category.as_str() {
                "free" => free.push(t.clone()),
                "free-tier" => free_tier.push(t.clone()),
                _ => apikey.push(t.clone()),
            }
        }
        (free, free_tier, apikey)
    });

    let show_add_form = move || {
        edit_id.set(None);
        form_name.set(String::new());
        form_type.set("openai_compat".into());
        form_key.set(String::new());
        form_url.set(String::new());
        form_models.set(Vec::new());
        form_caps.set(Vec::new());
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
        show_form.set(true);
    };

    // Derived signals for field visibility
    let selected_type_info = create_memo(move |_| {
        let t = form_type.get();
        provider_types.get().into_iter().find(|pt| pt.id == t)
    });
    let is_free_type = create_memo(move |_| {
        selected_type_info.get().map(|t| t.category == "free").unwrap_or(false)
    });

    let save = move || {
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

    let handle_test_model = move |provider_id: &str, model: &str| {
        let pid = provider_id.to_string();
        let mdl = model.to_string();
        let key = format!("{}:{}", pid, mdl);
        if testing_model.with(|t| t.as_deref() == Some(&key)) {
            return;
        }
        testing_model.set(Some(key.clone()));
        spawn_local({
            let pid2 = pid.clone();
            let mdl2 = mdl.clone();
            let testing_model = testing_model.clone();
            let model_test_results = model_test_results.clone();
            async move {
                let result = test_provider_model(&pid2, &mdl2).await;
                match result {
                    Ok(r) => {
                        model_test_results.update(|m| { m.insert(key.clone(), r); });
                    }
                    Err(e) => {
                        model_test_results.update(|m| {
                            m.insert(key.clone(), TestProviderResponse {
                                ok: false, latency_ms: 0, model: mdl2.clone(),
                                error: Some(e),
                            });
                        });
                    }
                }
                testing_model.set(None);
            }
        });
    };

    // ─── Category badge color helper ────────────────────────────────
    let category_badge = |cat: &str| -> (&'static str, &'static str) {
        match cat {
            "free" => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-[rgba(34,197,94,0.1)] text-success border-success/30", "Free"),
            "free-tier" => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-[rgba(229,106,74,0.1)] text-accent border-accent/30", "Free Tier"),
            "api-key" => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-[rgba(96,165,250,0.1)] text-[#60a5fa] border-[rgba(96,165,250,0.3)]", "API Key"),
            "oauth" => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-[rgba(251,191,36,0.1)] text-warning border-warning/30", "OAuth"),
            "web-cookie" => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-[rgba(239,68,68,0.1)] text-danger border-danger/30", "Web Cookie"),
            _ => ("inline-flex px-2 py-0.5 text-xs font-medium rounded-lg border bg-gray-500/10 text-gray-400 border-gray-500/30", "Unknown"),
        }
    };

    view! {
        <div class="animate-fade-in">
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-primary">"Providers"</h1>
                    <p class="text-sm text-secondary mt-1">"Manage upstream LLM providers"</p>
                </div>
                <button on:click=move|_|show_add_form()
                    class="px-4 py-2 text-sm font-medium rounded-lg text-white
                           bg-accent hover:bg-accent-hover
                           active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
                    </svg>
                    "Add Provider"
                </button>
            </div>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger">{error.get()}</p> }
            )}
            {move || loading.get().then(|| view! { <SkeletonTable rows=4/> })}

            // ─── Delete Confirm Dialog ────────────────────────────
            {move || delete_id.get().map(|id| {
                let name = providers.with(|p| p.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default());
                let id3 = id.clone();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_|delete_id.set(None)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6
                                    w-full max-w-md mx-4 shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-start gap-3 mb-4">
                                <div class="w-10 h-10 rounded-full bg-danger-bg flex items-center justify-center flex-shrink-0">
                                    <svg class="w-5 h-5 text-danger" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/>
                                    </svg>
                                </div>
                                <div>
                                    <h3 class="text-base font-semibold text-primary">
                                        {format!("Delete \"{}\"?", name)}
                                    </h3>
                                    <p class="text-sm text-secondary mt-1">
                                        "This provider will be removed from all routes. This action cannot be undone."
                                    </p>
                                </div>
                            </div>
                            <div class="flex gap-2 justify-end">
                                <button on:click=move|_|delete_id.set(None)
                                    class="px-4 py-2 text-sm font-medium rounded-lg
                                           bg-transparent border border-surface text-secondary
                                           hover:text-primary hover:bg-surface-2
                                           active:scale-[0.97] transition-all duration-150">
                                    "Cancel"
                                </button>
                                <button on:click=move|_|do_delete(id3.clone())
                                    class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                           bg-danger hover:bg-red-600 active:scale-[0.97] transition-all duration-150">
                                    "Delete"
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ─── Modal Form ──────────────────────────────────────
            {move || show_form.get().then(|| {
                let is_edit = edit_id.get().is_some();
                let (free, free_tier, apikey) = type_groups.get();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-start justify-center pt-[10vh] z-50 animate-fade-in"
                        on:click=move|_|show_form.set(false)>
                        <div class="bg-surface border border-border-subtle rounded-[14px]
                                    w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-center justify-between px-6 py-4 border-b border-surface">
                                <h2 class="text-lg font-semibold text-primary">
                                    {if is_edit { "Edit Provider" } else { "Add Provider" }}
                                </h2>
                                <button on:click=move|_|show_form.set(false)
                                    class="text-muted hover:text-primary transition-colors">
                                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                                    </svg>
                                </button>
                            </div>
                            <div class="p-6">
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Name"</label>
                                    <input type="text" prop:value=form_name.get()
                                        placeholder="e.g. my-openai"
                                        on:input=move|ev|form_name.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary placeholder-muted
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Type"</label>
                                    <select prop:value=form_type.get()
                                        on:change=move|ev|form_type.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary
                                               focus:border-accent focus:outline-none transition-colors">
                                        <optgroup label="── Free (No Key) ──">
                                            {free.into_iter().map(|t| {
                                                let id = t.id.clone();
                                                let name = t.display_name.clone();
                                                view! { <option value=id>{name}</option> }
                                            }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="── Free Tier (Signup) ──">
                                            {free_tier.into_iter().map(|t| {
                                                let id = t.id.clone();
                                                let name = t.display_name.clone();
                                                view! { <option value=id>{name}</option> }
                                            }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="── API Key (Paid) ──">
                                            {apikey.into_iter().map(|t| {
                                                let id = t.id.clone();
                                                let name = t.display_name.clone();
                                                view! { <option value=id>{name}</option> }
                                            }).collect::<Vec<_>>()}
                                        </optgroup>
                                    </select>
                                </div>

                                // ── API Key field ─────────────────────────────
                                {move || {
                                    let is_free = is_free_type.get();
                                    if is_free {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="text" disabled=true value="(no key needed)"
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="password" prop:value=form_key.get()
                                                    placeholder=if is_edit { "(unchanged on edit)" } else { "sk-..." }
                                                    on:input=move|ev|form_key.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    }
                                }}

                                // ── Base URL field ────────────────────────────
                                {move || {
                                    let is_free = is_free_type.get();
                                    let info = selected_type_info.get();
                                    let placeholder = info.as_ref()
                                        .map(|t| {
                                            match t.id.as_str() {
                                                "openai" => "https://api.openai.com/v1",
                                                "anthropic" => "https://api.anthropic.com/v1",
                                                "deepseek" => "https://api.deepseek.com/v1",
                                                "openrouter" => "https://openrouter.ai/api/v1",
                                                "groq" => "https://api.groq.com/openai/v1",
                                                "gemini" => "https://generativelanguage.googleapis.com/v1beta",
                                                "ollama" => "http://localhost:11434/v1",
                                                _ => "https://api.example.com/v1",
                                            }
                                        })
                                        .unwrap_or("https://api.example.com/v1");
                                    if is_free {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" disabled=true value="(hardcoded)"
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" prop:value=form_url.get()
                                                    placeholder=placeholder
                                                    on:input=move|ev|form_url.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    }
                                }}

                                <TagInput label="Models (type + Enter or comma to add)".to_string() placeholder="e.g. gpt-4o".to_string() tags=form_models/>
                                <TagInput label="Capabilities".to_string() placeholder="e.g. vision".to_string() tags=form_caps/>

                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-surface">
                                    <button on:click=move|_|show_form.set(false)
                                        class="px-4 py-2 text-sm font-medium rounded-lg
                                               bg-transparent border border-surface text-secondary
                                               hover:text-primary hover:bg-surface-2
                                               active:scale-[0.97] transition-all duration-150">
                                        "Cancel"
                                    </button>
                                    <button on:click=move|_|save() disabled=saving.get()
                                        class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                               bg-accent hover:bg-accent-hover
                                               disabled:opacity-50 active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                                        {saving.get().then(|| view! { "Saving..." }).unwrap_or(view! { "Save" })}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ─── Card Grid ──────────────────────────────────────────────
            {move || (!loading.get() && !show_form.get()).then(|| {
                let provs = providers.get();
                let is_expanded = expanded_id.get();
                let testing = testing_model.get();
                let empty = provs.is_empty();
                view! {
                    {if !empty {
                        view! {
                            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 animate-fade-in-up">
                                {provs.into_iter().map(|p| {
                                    let pid = p.id.clone();
                                    let pid_click = pid.clone();
                                    let (cb_cls, cb_label) = category_badge(&p.category);
                                    let p_edit = p.clone();
                                    let is_this_expanded = is_expanded.as_deref() == Some(&pid);
                                    let models = p.models.clone();
                                    let results = model_test_results.clone();

                                    view! {
                                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5 hover:shadow-lg group"
                                            on:click=move|_| {
                                                let eid = expanded_id.get();
                                                if eid.as_deref() == Some(&pid_click) {
                                                    expanded_id.set(None);
                                                } else {
                                                    expanded_id.set(Some(pid_click.clone()));
                                                }
                                            }>
                                            <div class="flex items-start justify-between mb-3">
                                                <div class="flex items-center gap-2.5 min-w-0">
                                                    <div class="w-9 h-9 rounded-lg shrink-0 flex items-center justify-center text-sm font-bold"
                                                        style=format!("background-color: rgba(96,165,250,0.15); color: #60a5fa")>
                                                        {p.name.chars().next().map(|c| c.to_string()).unwrap_or_default()}
                                                    </div>
                                                    <div class="min-w-0">
                                                        <h3 class="font-semibold text-sm text-primary truncate">{p.name.clone()}</h3>
                                                        <span class={cb_cls}>{cb_label}</span>
                                                    </div>
                                                </div>
                                                <div class="flex items-center gap-2 shrink-0">
                                                    {if p.enabled {
                                                        view! { <span class="flex items-center gap-1 text-xs text-success"><span class="w-1.5 h-1.5 rounded-full bg-success"></span>"Active"</span> }
                                                    } else {
                                                        view! { <span class="flex items-center gap-1 text-xs text-muted"><span class="w-1.5 h-1.5 rounded-full bg-muted"></span>"Disabled"</span> }
                                                    }}
                                                    <svg class="w-4 h-4 text-muted transition-transform duration-200" class:rotate-180=is_this_expanded fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                                                    </svg>
                                                </div>
                                            </div>

                                            // ── Collapsed: summary ──────────────
                                            {if !is_this_expanded {
                                                view! {
                                                    <>
                                                        <div class="space-y-1.5 mb-3 text-xs">
                                                            <div class="flex items-center justify-between">
                                                                <span class="text-secondary">Type</span>
                                                                <span class="inline-flex px-2 py-0.5 text-xs font-medium rounded-full bg-accent-bg text-accent border border-accent/30 truncate max-w-[140px]">
                                                                    {p.provider_type.clone()}
                                                                </span>
                                                            </div>
                                                            <div class="flex items-center justify-between">
                                                                <span class="text-secondary">Models</span>
                                                                <span class="text-primary font-mono font-medium">{p.models.len().to_string()}</span>
                                                            </div>
                                                        </div>
                                                        {(!p.capabilities.is_empty()).then(|| {
                                                            view! {
                                                                <div class="flex flex-wrap gap-1 mb-3">
                                                                    {p.capabilities.iter().map(|c| {
                                                                        view! { <span class="inline-flex px-1.5 py-0.5 text-xs rounded bg-surface-2 text-muted">{c.clone()}</span> }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            }
                                                        })}
                                                        <div class="flex items-center justify-between pt-3 border-t border-border-subtle">
                                                            <button on:click=move|ev| { ev.stop_propagation(); show_edit_form(p_edit.clone()); }
                                                                class="px-2.5 py-1.5 text-xs font-medium rounded-lg bg-surface-2 text-secondary hover:text-primary hover:bg-surface-3 transition-all duration-150 opacity-0 group-hover:opacity-100">"Edit"</button>
                                                            <button on:click=move|ev| { ev.stop_propagation(); delete_id.set(Some(pid.clone())); }
                                                                class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-danger border border-danger/30 hover:bg-danger-bg transition-all duration-150 opacity-0 group-hover:opacity-100">"Delete"</button>
                                                        </div>
                                                    </>
                                                }.into_view()
                                            } else {
                                                // ── Expanded: model list ────────
                                                view! {
                                                    <>
                                                        <div class="text-xs">
                                                            <div class="flex items-center justify-between mb-2">
                                                                <span class="text-secondary font-medium">"Base URL"</span>
                                                                <code class="text-primary font-mono text-[10px] truncate max-w-[180px]">{p.base_url.clone()}</code>
                                                            </div>
                                                            <div class="pt-2 border-t border-border-subtle mb-2">
                                                                <p class="text-xs text-secondary font-medium mb-2">"Models"</p>
                                                            </div>
                                                        </div>
                                                        {if models.is_empty() {
                                                            view! { <p class="text-xs text-muted italic mb-3">"No models configured"</p> }.into_view()
                                                        } else {
                                                            view! {
                                                                <div class="flex flex-col gap-1.5 mb-3 max-h-[260px] overflow-y-auto">
                                                                    {models.into_iter().map(|model_name| {
                                                                        let m_key = format!("{}:{}", pid, model_name);
                                                                        let test_result = results.with(|m| m.get(&m_key).cloned());
                                                                        let test_ok = test_result.as_ref().map(|r| r.ok).unwrap_or(false);
                                                                        let is_testing = testing.as_ref().map(|t| t == &m_key).unwrap_or(false);
                                                                        let mn = model_name.clone();
                                                                        let p_id = pid.clone();
                                                                        view! {
                                                                            <div class="flex items-center justify-between gap-2 px-2.5 py-1.5 rounded-lg bg-surface-2 hover:bg-surface-3 transition-colors">
                                                                                <code class="text-xs font-mono text-primary truncate">{model_name.clone()}</code>
                                                                                <div class="flex items-center gap-1.5 shrink-0">
                                                                                    {test_result.as_ref().map(|r| {
                                                                                        if r.ok {
                                                                                            view! { <span class="text-[10px] text-success font-mono">{r.latency_ms.to_string() + "ms"}</span> }.into_view()
                                                                                        } else {
                                                                                            let err_title = r.error.clone().unwrap_or_default();
                                                                                            view! { <span class="text-[10px] text-danger" title=err_title>"FAIL"</span> }.into_view()
                                                                                        }
                                                                                    })}
                                                                                    <button on:click=move|ev| {
                                                                                        ev.stop_propagation();
                                                                                        handle_test_model(&p_id, &mn);
                                                                                    } disabled=is_testing
                                                                                        class="px-2 py-1 text-[10px] font-medium rounded-lg border 
                                                                                               transition-all duration-150 flex items-center gap-1
                                                                                               active:scale-[0.97]
                                                                                               disabled:opacity-50"
                                                                                        class=if is_testing {
                                                                                            "bg-accent/20 border-accent/30 text-accent"
                                                                                        } else if test_ok {
                                                                                            "bg-green-500/10 border-green-500/30 text-success"
                                                                                        } else {
                                                                                            "bg-surface-2 border-border-subtle text-secondary hover:text-primary hover:bg-surface-3"
                                                                                        }>
                                                                                        {if is_testing {
                                                                                            view! {
                                                                                                <>
                                                                                                    <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                                                                                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
                                                                                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
                                                                                                    </svg>
                                                                                                    "Test"
                                                                                                </>
                                                                                            }.into_view()
                                                                                        } else {
                                                                                            view! { <>"Test"</> }.into_view()
                                                                                        }}
                                                                                    </button>
                                                                                </div>
                                                                            </div>
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>
                                                            }.into_view()
                                                        }}
                                                        <div class="flex items-center justify-between pt-3 border-t border-border-subtle">
                                                            <button on:click=move|ev| { ev.stop_propagation(); show_edit_form(p_edit.clone()); }
                                                                class="px-2.5 py-1.5 text-xs font-medium rounded-lg bg-surface-2 text-secondary hover:text-primary hover:bg-surface-3 transition-all duration-150">"Edit"</button>
                                                            <button on:click=move|ev| { ev.stop_propagation(); delete_id.set(Some(pid.clone())); }
                                                                class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-danger border border-danger/30 hover:bg-danger-bg transition-all duration-150">"Delete"</button>
                                                        </div>
                                                    </>
                                                }.into_view()
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="text-center py-12 bg-surface border border-border-subtle rounded-[14px]">
                                <p class="text-muted text-sm">"No providers configured yet."</p>
                            </div>
                        }.into_view()
                    }}
                }
            })}
        </div>
    }
}
