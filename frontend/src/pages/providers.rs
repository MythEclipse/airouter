use leptos::*;
use crate::api::*;
use crate::components::tag_input::TagInput;
use crate::components::skeleton::SkeletonTable;
use crate::components::provider_icon::{ProviderIcon, category_style, category_accent};

// ─── Category order for sections ─────────────────────────────────
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
    };
    load();

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

    view! {
        <div class="animate-fade-in">
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-primary">"Providers"</h1>
                    <p class="text-sm text-secondary mt-1">"Manage upstream LLM providers"</p>
                </div>
                <button on:click=move|_|show_add_form()
                    class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-accent hover:bg-accent-hover active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/></svg>
                    "Add Provider"
                </button>
            </div>

            {move || (!error.get().is_empty()).then(|| view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger">{error.get()}</p> })}
            {move || loading.get().then(|| view! { <SkeletonTable rows=4/> })}

            // Delete Confirm
            {move || delete_id.get().map(|id| {
                let name = providers.with(|p| p.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default());
                let id3 = id.clone();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in" on:click=move|_|delete_id.set(None)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6 w-full max-w-md mx-4 shadow-2xl animate-scale-in" on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-start gap-3 mb-4">
                                <div class="w-10 h-10 rounded-full bg-danger-bg flex items-center justify-center flex-shrink-0">
                                    <svg class="w-5 h-5 text-danger" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/></svg>
                                </div>
                                <div>
                                    <h3 class="text-base font-semibold text-primary">{format!("Delete \"{}\"?", name)}</h3>
                                    <p class="text-sm text-secondary mt-1">"This provider will be removed from all routes. This action cannot be undone."</p>
                                </div>
                            </div>
                            <div class="flex gap-2 justify-end">
                                <button on:click=move|_|delete_id.set(None) class="px-4 py-2 text-sm font-medium rounded-lg bg-transparent border border-surface text-secondary hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150">"Cancel"</button>
                                <button on:click=move|_|do_delete(id3.clone()) class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-danger hover:bg-red-600 active:scale-[0.97] transition-all duration-150">"Delete"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Modal Form (wait for provider types)
            {move || (show_form.get() && provider_types_loaded.get()).then(|| {
                let is_edit = edit_id.get().is_some();
                let (free, free_tier, apikey) = type_groups.get();
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-start justify-center pt-[10vh] z-50 animate-fade-in" on:click=move|_|show_form.set(false)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] w-full max-w-lg mx-4 max-h-[80vh] overflow-y-auto shadow-2xl animate-scale-in" on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-center justify-between px-6 py-4 border-b border-surface">
                                <h2 class="text-lg font-semibold text-primary">{if is_edit { "Edit Provider" } else { "Add Provider" }}</h2>
                                <button on:click=move|_|show_form.set(false) class="text-muted hover:text-primary transition-colors">
                                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                                </button>
                            </div>
                            <div class="p-6">
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Name"</label>
                                    <input type="text" prop:value=form_name.get() placeholder="e.g. my-openai" on:input=move|ev|form_name.set(event_target_value(&ev)) class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Type"</label>
                                    <select prop:value=form_type.get() on:change=move|ev|form_type.set(event_target_value(&ev)) class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors">
                                        <optgroup label="── Free (No Key) ──">
                                            {free.into_iter().map(|t| { let id = t.id.clone(); view! { <option value=id>{t.display_name}</option> } }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="── Free Tier (Signup) ──">
                                            {free_tier.into_iter().map(|t| { let id = t.id.clone(); view! { <option value=id>{t.display_name}</option> } }).collect::<Vec<_>>()}
                                        </optgroup>
                                        <optgroup label="── API Key (Paid) ──">
                                            {apikey.into_iter().map(|t| { let id = t.id.clone(); view! { <option value=id>{t.display_name}</option> } }).collect::<Vec<_>>()}
                                        </optgroup>
                                    </select>
                                </div>
                                {move || { let is_free = is_free_type.get(); if is_free {
                                    view! { <div class="mb-4 opacity-50 pointer-events-none"><label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label><input type="text" disabled=true value="(no key needed)" class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/></div> }.into_view()
                                } else {
                                    view! { <div class="mb-4"><label class="block text-xs text-secondary mb-1.5 font-medium">"API Key"</label><input type="password" prop:value=form_key.get() placeholder=if is_edit { "(unchanged on edit)" } else { "sk-..." } on:input=move|ev|form_key.set(event_target_value(&ev)) class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/></div> }.into_view()
                                }}}
                                {move || { let is_free = is_free_type.get(); let placeholder = if is_free { "" } else { "https://api.example.com/v1" }; if is_free {
                                    view! { <div class="mb-4 opacity-50 pointer-events-none"><label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label><input type="text" disabled=true value="(hardcoded)" class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-muted"/></div> }.into_view()
                                } else {
                                    view! { <div class="mb-4"><label class="block text-xs text-secondary mb-1.5 font-medium">"Base URL"</label><input type="text" prop:value=form_url.get() placeholder=placeholder on:input=move|ev|form_url.set(event_target_value(&ev)) class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/></div> }.into_view()
                                }}}
                                <TagInput label="Models (type + Enter or comma to add)".to_string() placeholder="e.g. gpt-4o".to_string() tags=form_models/>
                                <TagInput label="Capabilities".to_string() placeholder="e.g. vision".to_string() tags=form_caps/>
                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-surface">
                                    <button on:click=move|_|show_form.set(false) class="px-4 py-2 text-sm font-medium rounded-lg bg-transparent border border-surface text-secondary hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150">"Cancel"</button>
                                    <button on:click=move|_|save() disabled=saving.get() class="px-4 py-2 text-sm font-medium rounded-lg text-white bg-accent hover:bg-accent-hover disabled:opacity-50 active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                                        {saving.get().then(|| view! { "Saving..." }).unwrap_or(view! { "Save" })}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ═══ CATEGORY SECTIONS ═══
            {move || (!loading.get() && !show_form.get()).then(|| {
                let secs = sections.get();
                let is_expanded = expanded_id.get();
                let testing = testing_model.get();

                if secs.is_empty() {
                    view! {
                        <div class="flex flex-col items-center justify-center py-16 text-center">
                            <svg class="w-12 h-12 text-muted mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/></svg>
                            <p class="text-secondary text-sm">"No providers yet — click \"Add Provider\" to get started."</p>
                        </div>
                    }.into_view()
                } else {
                    view! {
                        <div class="flex flex-col gap-8">
                            {secs.into_iter().map(|(cat, items)| {
                                let accent = category_accent(cat);
                                let (_, cat_label) = category_style(cat);
                                let count = items.len();
                                let section_title = section_label(cat);

                                view! {
                                    <section>
                                        <div class="flex items-center gap-3 mb-4">
                                            <div class="w-1 h-6 rounded-full shrink-0" style=format!("background-color: {}", accent)></div>
                                            <h2 class="text-lg font-semibold text-primary">{section_title.clone()}</h2>
                                            <span class="text-xs text-muted bg-surface-2 px-2 py-0.5 rounded-full font-mono">{count}</span>
                                        </div>
                                        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                            {items.into_iter().map(|p| {
                                                let pid   = p.id.clone();
                                                let pid2  = pid.clone();
                                                let pid3  = pid.clone();
                                                let p_edit = p.clone();
                                                let is_this = is_expanded.as_deref() == Some(&pid);
                                                let models  = p.models.clone();
                                                let results = model_test_results.clone();
                                                let cat2    = p.category.clone();
                                                let ptype   = p.provider_type.clone();
                                                let pname   = p.name.clone();
                                                let _sec_lbl = cat_label;
                                                let sec_ttl = section_title.clone();

                                                view! {
                                                    <div class="bg-surface border border-border-subtle rounded-[14px] p-4 transition-all duration-200 hover:border-surface hover:-translate-y-0.5 hover:shadow-lg group cursor-pointer"
                                                        on:click=move|_| {
                                                            let eid = expanded_id.get();
                                                            if eid.as_deref() == Some(&pid) { expanded_id.set(None); }
                                                            else { expanded_id.set(Some(pid.clone())); }
                                                        }>

                                                        // Header: icon + name
                                                        <div class="flex items-center gap-3 mb-3">
                                                            <ProviderIcon provider_type=ptype.clone() name=pname.clone() size=40/>
                                                            <div class="min-w-0 flex-1">
                                                                <h3 class="font-semibold text-sm text-primary truncate">{p.name.clone()}</h3>
                                                                <span class="text-xs text-muted truncate block">{p.provider_type.clone()}</span>
                                                            </div>
                                                            <div class="flex items-center gap-2 shrink-0">
                                                                {if p.enabled { view! { <span class="flex items-center gap-1 text-xs text-success"><span class="w-1.5 h-1.5 rounded-full bg-success"></span>"Active"</span> }
                                                                } else { view! { <span class="flex items-center gap-1 text-xs text-muted"><span class="w-1.5 h-1.5 rounded-full bg-muted"></span>"Disabled"</span> } }}
                                                                <svg class="w-4 h-4 text-muted transition-transform duration-200" class:rotate-180=is_this fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                                                            </div>
                                                        </div>

                                                        // Category badge
                                                        <div class="mb-2">{{ let (cls, _) = category_style(&cat2); view! { <span class={cls}>{sec_ttl.clone()}</span> }}}</div>

                                                        // Collapsed summary
                                                        {if !is_this {
                                                            view! { <div class="text-xs text-secondary">{format!("{} models", models.len())}</div> }.into_view()
                                                        } else {
                                                            view! {
                                                                <div class="space-y-1.5 text-xs mt-2 pt-3 border-t border-border-subtle">
                                                                    <div class="flex items-center justify-between"><span class="text-secondary">Base URL</span><span class="text-primary truncate max-w-[180px] text-right font-mono">{p.base_url.clone()}</span></div>
                                                                    {if !models.is_empty() {
                                                                                                                                            let ptype2 = ptype.clone();
                                                                                                                                            let pname2 = pname.clone();
                                                                                                                                            view! {
                                                                                                                                                <div class="pt-2">
                                                                                                                                                    <span class="text-secondary block mb-1.5">Models</span>
                                                                                                                                                    <div class="flex flex-col gap-1">
                                                                                                                                                        {models.into_iter().map(|m| {
                                                                                                                                                            let mdl = m.clone();
                                                                                                                                                            let pid_test = pid2.clone();
                                                                                                                                                            let tk = format!("{}:{}", pid2, mdl);
                                                                                                                                                            let res = results.with(|r| r.get(&tk).cloned());
                                                                                                                                                            let busy = testing.as_deref() == Some(&tk);
                                                                                                                                                            view! {
                                                                                                                                                                <div class="flex items-center gap-2 py-1.5 px-2.5 rounded-lg bg-surface-2/50 hover:bg-surface-2 transition-colors">
                                                                                                                                                                    <span class="text-xs text-primary font-mono flex-1">{m.clone()}</span>
                                                                                                                                                                    <button on:click=move|ev| { ev.stop_propagation(); handle_test_model(&pid_test, &mdl); }
                                                                                                                                                                        class="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md text-secondary hover:text-accent hover:bg-accent-bg active:scale-[0.97] transition-all duration-150 border border-transparent hover:border-accent/30"
                                                                                                                                                                        title="Test model">
                                                                                                                                                                        <ProviderIcon provider_type=ptype2.clone() name=pname2.clone() size=16/>
                                                                                                                                                                        {if busy { "Testing…" } else { "Test" }}
                                                                                                                                                                    </button>
                                                                                                                                                                    {res.map(|r| view! {
                                                                                                                                                                        <span class="text-xs font-mono"
                                                                                                                                                                            style=if r.ok { "color:#22C55e" } else { "color:#ef4444" }>
                                                                                                                                                                            {if r.ok { format!("✓ {}ms", r.latency_ms) } else { "✗".to_string() }}
                                                                                                                                                                        </span>
                                                                                                                                                                    })}
                                                                                                                                                                </div>
                                                                                                                                                            }
                                                                                                                                                        }).collect::<Vec<_>>()}
                                                                                                                                                    </div>
                                                                                                                                                </div>
                                                                                                                                            }.into_view()
                                                                                                                                        } else { view! { <div class="pt-2 text-xs text-muted">"No models configured"</div> }.into_view() }}
                                                                    <div class="flex gap-2 justify-end pt-3 border-t border-border-subtle">
                                                                        <button on:click=move|ev| { ev.stop_propagation(); show_edit_form(p_edit.clone()); } class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-secondary border border-surface hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150">"Edit"</button>
                                                                        <button on:click=move|ev| { ev.stop_propagation(); delete_id.set(Some(pid3.clone())); } class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-danger border border-danger/30 hover:bg-danger-bg active:scale-[0.97] transition-all duration-150">"Delete"</button>
                                                                    </div>
                                                                </div>
                                                            }.into_view()
                                                        }}
                                                    </div>
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
