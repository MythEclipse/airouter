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

    // Load provider types for the dropdown
    spawn_local({
        let pt = provider_types.clone();
        async move {
            let url = "/api/dashboard/provider-types";
            let window = web_sys::window().unwrap();
            let mut opts = web_sys::RequestInit::new();
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
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    // ─── Category badge color helper ────────────────────────────────
    let category_badge = |cat: &str| -> (&'static str, &'static str) {
        match cat {
            "free" => ("bg-green-500/10 text-green-400 border-green-500/30", "Free"),
            "free-tier" => ("bg-purple-500/10 text-purple-400 border-purple-500/30", "Free Tier"),
            "api-key" => ("bg-blue-500/10 text-blue-400 border-blue-500/30", "API Key"),
            "oauth" => ("bg-orange-500/10 text-orange-400 border-orange-500/30", "OAuth"),
            "web-cookie" => ("bg-pink-500/10 text-pink-400 border-pink-500/30", "Web Cookie"),
            _ => ("bg-gray-500/10 text-gray-400 border-gray-500/30", "Unknown"),
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
                           transition-all duration-150 flex items-center gap-2">
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
                let id2 = id.clone();
                let id3 = id.clone();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_|delete_id.set(None)>
                        <div class="bg-surface-alt border border-surface rounded-xl p-6
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
                                           bg-surface-active text-primary
                                           border border-surface
                                           hover:bg-border transition-all duration-150">
                                    "Cancel"
                                </button>
                                <button on:click=move|_|do_delete(id3.clone())
                                    class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                           bg-danger hover:bg-red-600 transition-all duration-150">
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
                let free_type = is_free_type.get();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-start justify-center pt-[10vh] z-50 animate-fade-in"
                        on:click=move|_|show_form.set(false)>
                        <div class="bg-surface-alt border border-surface rounded-xl
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
                                        class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg
                                               text-sm text-primary placeholder-muted
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Type"</label>
                                    <select prop:value=form_type.get()
                                        on:change=move|ev|form_type.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg
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
                                    (if is_free {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="text" disabled=true value="(no key needed)"
                                                    class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label>
                                                <input type="password" prop:value=form_key.get()
                                                    placeholder=if is_edit { "(unchanged on edit)" } else { "sk-..." }
                                                    on:input=move|ev|form_key.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    })
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
                                    (if is_free {
                                        view! {
                                            <div class="mb-4 opacity-50 pointer-events-none">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" disabled=true value="(hardcoded)"
                                                    class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg text-sm text-muted"/>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {
                                            <div class="mb-4">
                                                <label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label>
                                                <input type="text" prop:value=form_url.get()
                                                    placeholder=placeholder
                                                    on:input=move|ev|form_url.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-[#0d1117] border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                            </div>
                                        }.into_view()
                                    })
                                }}

                                <TagInput label="Models (type + Enter or comma to add)".to_string() placeholder="e.g. gpt-4o".to_string() tags=form_models/>
                                <TagInput label="Capabilities".to_string() placeholder="e.g. vision".to_string() tags=form_caps/>

                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-surface">
                                    <button on:click=move|_|show_form.set(false)
                                        class="px-4 py-2 text-sm font-medium rounded-lg
                                               bg-surface-active text-primary
                                               border border-surface
                                               hover:bg-border transition-all duration-150">
                                        "Cancel"
                                    </button>
                                    <button on:click=move|_|save() disabled=saving.get()
                                        class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                               bg-accent hover:bg-accent-hover
                                               disabled:opacity-50 transition-all duration-150 flex items-center gap-2">
                                        {saving.get().then(|| view! { "Saving..." }).unwrap_or(view! { "Save" })}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ─── Table ──────────────────────────────────────────────
            {move || (!loading.get() && !show_form.get()).then(|| {
                let provs = providers.get();
                let empty = provs.is_empty();
                view! {
                    <div class="bg-surface-alt border border-surface rounded-xl overflow-hidden animate-fade-in-up">
                        <table class="w-full">
                            <thead>
                                <tr class="bg-surface-hover">
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Name"</th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Type"</th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Category"</th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Models"</th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Capabilities"</th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Status"</th>
                                    <th class="text-right px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">"Actions"</th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-surface/50">
                                {provs.into_iter().map(|p| {
                                    let pid = p.id.clone();
                                    let (cb_cls, cb_label) = category_badge(&p.category);
                                    view! {
                                        <tr class="hover:bg-surface-hover/50 transition-colors duration-100">
                                            <td class="px-4 py-3 text-sm font-medium text-primary">{p.name.clone()}</td>
                                            <td class="px-4 py-3">
                                                <span class="inline-flex px-2 py-0.5 text-xs font-medium rounded-full bg-accent-bg text-accent border border-accent/30">
                                                    {p.provider_type.clone()}
                                                </span>
                                            </td>
                                            <td class="px-4 py-3">
                                                <span class=cb_cls>{cb_label}</span>
                                            </td>
                                            <td class="px-4 py-3 text-sm text-secondary">{p.models.len().to_string()}</td>
                                            <td class="px-4 py-3">
                                                <div class="flex flex-wrap gap-1">
                                                    {p.capabilities.iter().map(|c| {
                                                        view! { <span class="inline-flex px-1.5 py-0.5 text-xs rounded bg-surface-active text-muted">{c.clone()}</span> }
                                                    }).collect::<Vec<_>>()}
                                                </div>
                                            </td>
                                            <td class="px-4 py-3">{if p.enabled {
                                                view! { <span class="inline-flex items-center gap-1 text-xs text-success"><span class="w-1.5 h-1.5 rounded-full bg-success"></span>"Active"</span> }
                                            } else {
                                                view! { <span class="inline-flex items-center gap-1 text-xs text-muted"><span class="w-1.5 h-1.5 rounded-full bg-muted"></span>"Disabled"</span> }}}</td>
                                            <td class="px-4 py-3 text-right">
                                                <div class="flex gap-1.5 justify-end">
                                                    <button on:click=move|_|show_edit_form(p.clone())
                                                        class="px-2.5 py-1.5 text-xs font-medium rounded-lg bg-surface-active text-secondary hover:text-primary hover:bg-border transition-all duration-150">"Edit"</button>
                                                    <button on:click=move|_|delete_id.set(Some(pid.clone()))
                                                        class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-danger border border-danger/30 hover:bg-danger-bg transition-all duration-150">"Delete"</button>
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                        {empty.then(|| {
                            view! {
                                <div class="text-center py-12 text-muted text-sm">
                                    "No providers configured yet."
                                </div>
                            }
                        })}
                    </div>
                }
            })}
        </div>
    }
}
