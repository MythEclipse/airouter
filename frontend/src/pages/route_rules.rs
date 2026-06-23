use leptos::*;
use std::collections::HashSet;
use crate::api::*;
use crate::components::skeleton::SkeletonTable;

fn build_curl(model: &str) -> String {
    format!(
        "curl -X POST http://localhost:3000/v1/chat/completions \\\n  -H \"Content-Type: application/json\" \\\n  -H \"Authorization: Bearer sk-your-key\" \\\n  -d '{{
  \"model\": \"{model}\",
  \"messages\": [{{\"role\": \"user\", \"content\": \"test\"}}]
}}'"
    )
}

fn combo_summary(combo: &serde_json::Value) -> Vec<(String, String)> {
    if let serde_json::Value::Object(ref obj) = combo {
        obj.iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| {
                let label = match k.as_str() {
                    "judge_model" => "Judge Model".into(),
                    "min_panel" => "Min Panel".into(),
                    "straggler_grace_ms" => "Straggler Grace".into(),
                    "panel_hard_timeout_ms" => "Panel Timeout".into(),
                    "sticky_limit" => "Sticky Limit".into(),
                    _ => k.clone(),
                };
                let val = match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => v.to_string(),
                };
                (label, val)
            })
            .collect()
    } else {
        vec![]
    }
}

#[component]
pub fn RouteRules() -> impl IntoView {
    let route_list = create_rw_signal(Vec::<RouteDetail>::new());
    let providers = create_rw_signal(Vec::<ProviderDetail>::new());
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let edit_id = create_rw_signal(Option::<String>::None);
    let form_model = create_rw_signal(String::new());
    let form_strategy = create_rw_signal("fallback".into());
    let form_provider = create_rw_signal(String::new());
    let form_providers = create_rw_signal(Vec::new());
    let saving = create_rw_signal(false);
    let delete_id = create_rw_signal(Option::<String>::None);
    let expanded = create_rw_signal(HashSet::<String>::new());
    let copied_id = create_rw_signal(Option::<String>::None);

    let form_judge_model = create_rw_signal(String::new());
    let form_min_panel = create_rw_signal("1".into());
    let form_straggler_grace = create_rw_signal("2000".into());
    let form_panel_timeout = create_rw_signal("30000".into());
    let form_sticky_limit = create_rw_signal(String::new());

    let load = move || {
        spawn_local({
            let route_list = route_list.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match fetch_routes().await {
                    Ok(data) => { route_list.set(data); loading.set(false); }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            }
        });
        spawn_local({
            let providers = providers.clone();
            async move {
                if let Ok(data) = fetch_providers().await {
                    providers.set(data);
                }
            }
        });
    };
    load();

    let provider_names = create_memo(move |_| {
        let mut names: Vec<String> = providers.get().into_iter().map(|p| p.name).collect();
        names.sort();
        names
    });

    let reset_combo = move || {
        form_judge_model.set(String::new());
        form_min_panel.set("1".into());
        form_straggler_grace.set("2000".into());
        form_panel_timeout.set("30000".into());
        form_sticky_limit.set(String::new());
    };

    let show_add_form = move || {
        edit_id.set(None);
        form_model.set(String::new());
        form_strategy.set("fallback".into());
        form_provider.set(String::new());
        form_providers.set(Vec::new());
        reset_combo();
        show_form.set(true);
    };

    let show_edit_form = move |r: RouteDetail| {
        edit_id.set(Some(r.id.clone()));
        form_model.set(r.model.clone());
        form_strategy.set(r.strategy.clone());
        form_provider.set(r.provider.clone().unwrap_or_default());
        form_providers.set(r.providers.clone().unwrap_or_default());

        if let serde_json::Value::Object(ref obj) = r.combo {
            form_judge_model.set(obj.get("judge_model").and_then(|v| v.as_str()).unwrap_or("").to_string());
            form_min_panel.set(obj.get("min_panel").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("1".into()));
            form_straggler_grace.set(obj.get("straggler_grace_ms").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("2000".into()));
            form_panel_timeout.set(obj.get("panel_hard_timeout_ms").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("30000".into()));
            form_sticky_limit.set(obj.get("sticky_limit").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default());
        } else {
            reset_combo();
        }
        show_form.set(true);
    };

    let save = move || {
        saving.set(true);
        error.set(String::new());
        let mut body = serde_json::json!({
            "model": form_model.get(),
            "strategy": form_strategy.get(),
        });
        let strat = form_strategy.get();
        if strat == "single" {
            body["provider"] = serde_json::json!(form_provider.get());
        } else {
            body["providers"] = serde_json::json!(form_providers.get());
        }

        if strat == "fusion" || strat == "round-robin" {
            let mut combo = serde_json::Map::new();
            if strat == "fusion" {
                let jm = form_judge_model.get();
                if !jm.is_empty() { combo.insert("judge_model".into(), serde_json::json!(jm)); }
                let mp = form_min_panel.get().parse::<u64>().unwrap_or(1);
                combo.insert("min_panel".into(), serde_json::json!(mp));
                let sg = form_straggler_grace.get().parse::<u64>().unwrap_or(2000);
                combo.insert("straggler_grace_ms".into(), serde_json::json!(sg));
                let pt = form_panel_timeout.get().parse::<u64>().unwrap_or(30000);
                combo.insert("panel_hard_timeout_ms".into(), serde_json::json!(pt));
            }
            if strat == "round-robin" {
                let sl = form_sticky_limit.get();
                if !sl.is_empty() {
                    if let Ok(n) = sl.parse::<u64>() {
                        combo.insert("sticky_limit".into(), serde_json::json!(n));
                    }
                }
            }
            body["combo"] = serde_json::json!(combo);
        }

        let body_str = serde_json::to_string(&body).unwrap_or_default();

        spawn_local({
            let route_list = route_list.clone();
            let loading = loading.clone();
            let error = error.clone();
            let show_form = show_form.clone();
            let edit_id = edit_id.clone();
            let saving = saving.clone();
            async move {
                let result = if let Some(id) = edit_id.get() {
                    update_route(&id, &body_str).await
                } else {
                    create_route(&body_str).await
                };
                match result {
                    Ok(_) => {
                        show_form.set(false);
                        loading.set(true);
                        edit_id.set(None);
                        match fetch_routes().await {
                            Ok(data) => { route_list.set(data); loading.set(false); }
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
            let route_list = route_list.clone();
            let loading = loading.clone();
            let error = error.clone();
            let delete_id = delete_id.clone();
            async move {
                match delete_route(&id).await {
                    Ok(()) => {
                        delete_id.set(None);
                        loading.set(true);
                        match fetch_routes().await {
                            Ok(data) => { route_list.set(data); loading.set(false); }
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    let strat_badge = |s: &str| -> (&'static str, &'static str) {
        match s {
            "single" => ("bg-info-bg text-info border-info/30", "Single"),
            "fallback" => ("bg-accent-bg text-accent border-accent/30", "Fallback"),
            "round-robin" => ("bg-success-bg text-success border-success/30", "Round-Robin"),
            "fusion" => ("bg-warning-bg text-warning border-warning/30", "Fusion"),
            _ => ("bg-surface-2 text-muted border-border", "Unknown"),
        }
    };

    view! {
        <div class="animate-fade-in">
            // Page header
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-primary font-display tracking-tight">"Routes"</h1>
                    <p class="text-sm text-secondary mt-1">"Model-to-provider routing rules"</p>
                </div>
                <button on:click=move|_|show_add_form()
                    class="btn-base px-4 py-2 text-sm rounded-lg bg-accent hover:bg-accent-hover text-white">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4"/></svg>
                    "Add Route"
                </button>
            </div>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger/20">{error.get()}</p> }
            )}
            {move || loading.get().then(|| view! { <SkeletonTable rows=5/> })}

            // Delete confirm modal
            {move || delete_id.get().map(|id| {
                let model = route_list.with(|r| r.iter().find(|x| x.id == id).map(|x| x.model.clone()).unwrap_or_default());
                let _id2 = id.clone();
                view! {
                    <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_|delete_id.set(None)>
                        <div class="bg-surface border border-border-subtle rounded-xl p-6 w-full max-w-md mx-4 shadow-[var(--shadow-elev)] animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-start gap-3 mb-4">
                                <div class="w-10 h-10 rounded-full bg-danger-bg flex items-center justify-center shrink-0">
                                    <svg class="w-5 h-5 text-danger" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/>
                                    </svg>
                                </div>
                                <div>
                                    <h3 class="text-base font-semibold text-primary font-display tracking-tight">{format!("Delete route \"{}\"?", model)}</h3>
                                    <p class="text-sm text-secondary mt-1">"This routing rule will be permanently removed."</p>
                                </div>
                            </div>
                            <div class="flex gap-2 justify-end">
                                <button on:click=move|_|delete_id.set(None)
                                    class="btn-base px-4 py-2 text-sm rounded-lg bg-surface-2 text-primary border border-surface hover:bg-surface-3">"Cancel"</button>
                                <button on:click=move|_|do_delete(id.clone())
                                    class="btn-base px-4 py-2 text-sm rounded-lg bg-danger/10 text-danger border border-danger/20 hover:bg-danger/20">"Delete"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Modal Form
            {move || show_form.get().then(|| {
                let is_edit = edit_id.get().is_some();
                let _strat = form_strategy.get();
                let names = provider_names.get();
                view! {
                    <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-start justify-center pt-[8vh] z-50 animate-fade-in"
                        on:click=move|_|show_form.set(false)>
                        <div class="bg-surface border border-border-subtle rounded-xl w-full max-w-lg mx-4 max-h-[84vh] overflow-y-auto shadow-[var(--shadow-elev)] animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-center justify-between px-6 py-4 border-b border-border-subtle">
                                <h2 class="text-base font-semibold text-primary font-display tracking-tight">{if is_edit { "Edit Route" } else { "Add Route" }}</h2>
                                <button on:click=move|_|show_form.set(false)
                                    class="text-muted hover:text-primary transition-colors p-1 rounded-lg hover:bg-surface-2">
                                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                                    </svg>
                                </button>
                            </div>
                            <div class="p-6">
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Model"</label>
                                    <input type="text" prop:value=form_model.get() placeholder="e.g. gpt-4o, claude-sonnet-4, * (wildcard)"
                                        on:input=move|ev|form_model.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Strategy"</label>
                                    <select prop:value=form_strategy.get()
                                        on:change=move|ev|form_strategy.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors">
                                        <option value="single">"Single"</option>
                                        <option value="fallback">"Fallback"</option>
                                        <option value="round-robin">"Round-Robin"</option>
                                        <option value="fusion">"Fusion"</option>
                                    </select>
                                </div>

                                {move || (form_strategy.get() == "single").then(|| {
                                    view! {
                                        <div class="mb-4">
                                            <label class="block text-xs text-secondary mb-1.5 font-medium">"Provider"</label>
                                            <select prop:value=form_provider.get()
                                                on:change=move|ev|form_provider.set(event_target_value(&ev))
                                                class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors">
                                                <option value="">"-- Select --"</option>
                                                {names.iter().map(|n| { let n2 = n.clone(); view! { <option value=n.clone()>{n2}</option> }}).collect::<Vec<_>>()}
                                            </select>
                                        </div>
                                    }
                                })}

                                {move || (form_strategy.get() != "single").then(|| {
                                    let provs = form_providers.get();
                                    let pnames = provider_names.get().clone();
                                    view! {
                                        <div class="mb-4">
                                            <label class="block text-xs text-secondary mb-1.5 font-medium">"Providers (first = highest)"</label>
                                            <div class="flex flex-wrap gap-1.5 p-2.5 bg-surface-2 border border-border-subtle rounded-lg min-h-[42px]">
                                                {provs.iter().enumerate().map(|(i, name)| {
                                                    let idx = i;
                                                    let n = name.clone();
                                                    view! {
                                                        <span class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium bg-accent-bg text-accent border border-accent/30 rounded-full animate-fade-in">
                                                            {n}
                                                            <button type="button" on:click=move|_|{ form_providers.update(|v| { v.remove(idx); }); }
                                                                class="hover:text-danger transition-colors leading-none text-sm">"×"</button>
                                                        </span>
                                                    }
                                                }).collect::<Vec<_>>()}
                                                <div class="relative flex-1 min-w-[120px]">
                                                    <select on:change=move|ev| {
                                                        let v = event_target_value(&ev);
                                                        if !v.is_empty() && !form_providers.with(|p| p.contains(&v)) {
                                                            form_providers.update(|p| p.push(v));
                                                        }
                                                    } class="w-full bg-transparent border-none text-sm text-primary focus:outline-none cursor-pointer appearance-none">
                                                        <option value="">"+ Add..."</option>
                                                        {pnames.iter().filter(|n| !form_providers.with(|p| p.contains(n)))
                                                            .map(|n| { let n2 = n.clone(); view! { <option value=n.clone()>{n2}</option> }}).collect::<Vec<_>>()}
                                                    </select>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                })}

                                {move || (form_strategy.get() == "fusion").then(|| {
                                    view! {
                                        <>
                                            <div class="mb-4 pt-3 border-t border-border-subtle"><p class="text-xs font-semibold text-muted uppercase tracking-widest mb-3">"Fusion Settings"</p></div>
                                            <div class="mb-4"><label class="block text-xs text-secondary mb-1.5">"Judge Model"</label>
                                                <input type="text" prop:value=form_judge_model.get() placeholder="e.g. gpt-4o-mini" on:input=move|ev|form_judge_model.set(event_target_value(&ev))
                                                class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/></div>
                                            <div class="grid grid-cols-3 gap-3">
                                                <div><label class="block text-xs text-secondary mb-1.5">"Min Panel"</label>
                                                    <input type="number" prop:value=form_min_panel.get() min="1" on:input=move|ev|form_min_panel.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors"/></div>
                                                <div><label class="block text-xs text-secondary mb-1.5">"Grace (ms)"</label>
                                                    <input type="number" prop:value=form_straggler_grace.get() min="0" step="100" on:input=move|ev|form_straggler_grace.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors"/></div>
                                                <div><label class="block text-xs text-secondary mb-1.5">"Timeout (ms)"</label>
                                                    <input type="number" prop:value=form_panel_timeout.get() min="1000" step="1000" on:input=move|ev|form_panel_timeout.set(event_target_value(&ev))
                                                    class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary focus:border-accent focus:outline-none transition-colors"/></div>
                                            </div>
                                        </>
                                    }
                                })}

                                {move || (form_strategy.get() == "round-robin").then(|| {
                                    view! {
                                        <>
                                            <div class="mb-4 pt-3 border-t border-border-subtle"><p class="text-xs font-semibold text-muted uppercase tracking-widest mb-3">"Round-Robin Settings"</p></div>
                                            <div><label class="block text-xs text-secondary mb-1.5">"Sticky Limit (empty = no sticky)"</label>
                                                <input type="number" prop:value=form_sticky_limit.get() min="1" placeholder="e.g. 3" on:input=move|ev|form_sticky_limit.set(event_target_value(&ev))
                                                class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors"/></div>
                                        </>
                                    }
                                })}

                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-border-subtle">
                                    <button on:click=move|_|show_form.set(false)
                                        class="btn-base px-4 py-2 text-sm rounded-lg bg-surface-2 text-primary border border-surface hover:bg-surface-3">"Cancel"</button>
                                    <button on:click=move|_|save() disabled=saving.get()
                                        class="btn-base px-4 py-2 text-sm rounded-lg bg-accent hover:bg-accent-hover text-white">
                                        {if saving.get() { "Saving..." } else { "Save" }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Route cards grid
            {move || (!loading.get() && !show_form.get()).then(|| {
                let rs = route_list.get();
                let cid = copied_id.get();
                let empty = rs.is_empty();
                view! {
                    {if !empty {
                        view! {
                            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 animate-fade-in-up">
                                {rs.into_iter().map(|r| {
                                    let rid = r.id.clone();
                                    let is_expanded = expanded.with(|e| e.contains(&rid));
                                    let (badge_cls, badge_label) = strat_badge(&r.strategy);
                                    let prov_str = r.provider.clone().unwrap_or_else(|| r.providers.clone().unwrap_or_default().join(", "));
                                    let has_combo = r.combo.is_object() && !r.combo.as_object().unwrap().is_empty();
                                    let combo_items = if r.combo.is_object() { combo_summary(&r.combo) } else { vec![] };
                                    let curl = build_curl(&r.model);
                                    let curl_id = rid.clone();
                                    let curl_for_btn = curl_id.clone();
                                    let curl_cid = curl_id.clone();
                                    let r_edit = r.clone();
                                    view! {
                                        <div class="card-base p-5">
                                            <div class="flex items-start justify-between gap-2 mb-3">
                                                <div class="min-w-0 flex-1">
                                                    <code class="text-sm font-mono text-accent font-semibold break-all">{r.model.clone()}</code>
                                                </div>
                                                <span class=format!("{} {}", badge_cls, "inline-flex items-center px-2 py-0.5 text-[11px] font-medium rounded-md border shrink-0")>{badge_label}</span>
                                            </div>

                                            <div class="space-y-1.5 mb-3 text-xs">
                                                <div class="flex items-center justify-between gap-2">
                                                    <span class="text-secondary text-[11px] shrink-0">"Providers"</span>
                                                    <span class="text-primary text-right text-[11px] truncate max-w-[200px]">{prov_str}</span>
                                                </div>
                                            </div>

                                            {has_combo.then(|| {
                                                let rid2 = rid.clone();
                                                view! {
                                                    <>
                                                        <button on:click=move|_| {
                                                            let mut s = expanded.get();
                                                            if s.contains(&rid2) { s.remove(&rid2); } else { s.insert(rid2.clone()); }
                                                            expanded.set(s);
                                                        } class="flex items-center gap-1 text-[11px] text-muted hover:text-primary transition-colors mb-2">
                                                            <svg class="w-3 h-3 transition-transform" class:rotate-180=is_expanded fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                                                <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/>
                                                            </svg>
                                                            {if is_expanded { "Hide Config" } else { "Config" }}
                                                        </button>
                                                    </>
                                                }
                                            })}

                                            {is_expanded.then(|| {
                                                view! {
                                                    <div class="animate-fade-in-up grid grid-cols-2 gap-2 mb-3 p-3 bg-surface-2 rounded-lg">
                                                        {combo_items.into_iter().map(|(label, val)| {
                                                            view! {
                                                                <div>
                                                                    <p class="text-[10px] text-muted uppercase tracking-wider mb-0.5">{label}</p>
                                                                    <p class="text-xs font-mono text-primary">{val}</p>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                }
                                            })}

                                            {(!has_combo && r.strategy != "single").then(||
                                                view! { <p class="text-xs text-warning/60 mb-2">"(default config)"</p> }
                                            )}

                                            <div class="flex items-center justify-between pt-3 border-t border-border-subtle">
                                                <button on:click=move|_| {
                                                    let curl2 = curl.clone();
                                                    let fbtn = curl_for_btn.clone();
                                                    let ccid = curl_cid.clone();
                                                    spawn_local(async move {
                                                        if let Some(clip) = web_sys::window().map(|w| w.navigator().clipboard()) {
                                                            let _ = clip.write_text(&curl2);
                                                            copied_id.set(Some(fbtn));
                                                            let cid2 = copied_id;
                                                            gloo_timers::future::TimeoutFuture::new(2000).await;
                                                            if cid2.with(|v| *v == Some(ccid)) { cid2.set(None); }
                                                        }
                                                    });
                                                } class="btn-base px-2.5 py-1.5 text-xs rounded-lg border border-accent/30 text-accent hover:bg-accent-bg">
                                                    {if cid.as_ref() == Some(&curl_id) {
                                                        view! { <>
                                                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                                                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7"/>
                                                            </svg>
                                                            "Copied!"
                                                        </> }
                                                    } else {
                                                        view! { <>
                                                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                                                <path stroke-linecap="round" stroke-linejoin="round" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"/>
                                                            </svg>
                                                            "cURL"
                                                        </> }
                                                    }}
                                                </button>
                                                <div class="flex gap-1.5 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity">
                                                    <button on:click=move|_|show_edit_form(r_edit.clone())
                                                        class="btn-base px-2.5 py-1.5 text-xs rounded-lg bg-surface-2 text-secondary hover:text-primary hover:bg-surface-3">"Edit"</button>
                                                    <button on:click=move|_|delete_id.set(Some(rid.clone()))
                                                        class="btn-base px-2.5 py-1.5 text-xs rounded-lg text-danger border border-danger/20 hover:bg-danger-bg">"Delete"</button>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="flex flex-col items-center justify-center py-16 text-center">
                                <svg class="w-12 h-12 text-muted mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7"/>
                                </svg>
                                <p class="text-secondary text-sm">"No routes configured yet"</p>
                                <p class="text-muted text-xs mt-1">"Add a route to start routing requests"</p>
                            </div>
                        }.into_view()
                    }}
                }
            })}
        </div>
    }
}
