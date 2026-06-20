use leptos::*;
use crate::api::*;

#[component]
pub fn RouteRules() -> impl IntoView {
    let route_list = create_rw_signal(Vec::<RouteDetail>::new());
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let edit_id = create_rw_signal(Option::<String>::None));
    let form_model = create_rw_signal(String::new());
    let form_strategy = create_rw_signal("fallback".into());
    let form_provider = create_rw_signal(String::new());
    let form_providers = create_rw_signal(String::new());

    // Combo fields
    let form_judge_model = create_rw_signal(String::new());
    let form_min_panel = create_rw_signal("1".into());
    let form_straggler_grace = create_rw_signal("2000".into());
    let form_panel_timeout = create_rw_signal("30000".into());
    let form_sticky_limit = create_rw_signal("".into());

    fn reset_combo_fields(
        judge: &RwSignal<String>, min: &RwSignal<String>, grace: &RwSignal<String>,
        timeout: &RwSignal<String>, sticky: &RwSignal<String>,
    ) {
        judge.set(String::new());
        min.set("1".into());
        grace.set("2000".into());
        timeout.set("30000".into());
        sticky.set(String::new());
    }

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
    };
    load();

    let show_add_form = move || {
        edit_id.set(None);
        form_model.set(String::new());
        form_strategy.set("fallback".into());
        form_provider.set(String::new());
        form_providers.set(String::new());
        reset_combo_fields(&form_judge_model, &form_min_panel, &form_straggler_grace, &form_panel_timeout, &form_sticky_limit);
        show_form.set(true);
    };

    let show_edit_form = move |r: RouteDetail| {
        edit_id.set(Some(r.id.clone()));
        form_model.set(r.model.clone());
        form_strategy.set(r.strategy.clone());
        form_provider.set(r.provider.clone().unwrap_or_default());
        form_providers.set(r.providers.clone().unwrap_or_default().join(", "));

        // Parse combo JSON
        if let serde_json::Value::Object(ref obj) = r.combo {
            form_judge_model.set(obj.get("judge_model").and_then(|v| v.as_str()).unwrap_or("").to_string());
            form_min_panel.set(obj.get("min_panel").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("1".into()));
            form_straggler_grace.set(obj.get("straggler_grace_ms").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("2000".into()));
            form_panel_timeout.set(obj.get("panel_hard_timeout_ms").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or("30000".into()));
            form_sticky_limit.set(obj.get("sticky_limit").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default());
        } else {
            reset_combo_fields(&form_judge_model, &form_min_panel, &form_straggler_grace, &form_panel_timeout, &form_sticky_limit);
        }
        show_form.set(true);
    };

    let save = move || {
        let mut body = serde_json::json!({
            "model": form_model.get(),
            "strategy": form_strategy.get(),
        });
        let strat = form_strategy.get();
        if strat == "single" {
            body["provider"] = serde_json::json!(form_provider.get());
        } else {
            let ps: Vec<String> = form_providers.get().split(',')
                .map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            body["providers"] = serde_json::json!(ps);
        }

        // Build combo for fusion / round-robin
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
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    let delete_rt = move |id: String| {
        spawn_local({
            let route_list = route_list.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match delete_route(&id).await {
                    Ok(()) => {
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

    // Helper: pretty-print combo JSON
    let fmt_combo = |r: &RouteDetail| -> String {
        if let serde_json::Value::Object(ref obj) = r.combo {
            let parts: Vec<String> = obj.iter().map(|(k, v)| {
                format!("{}: {}", k, match v {
                    serde_json::Value::String(s) => s.clone(),
                    _ => v.to_string(),
                })
            }).collect();
            parts.join(", ")
        } else {
            String::new()
        }
    };

    view! {
        <div class="page">
            <div style="display:flex; justify-content:space-between; align-items:center;">
                <h1>"Routes"</h1>
                <button class="btn btn-primary" on:click=move|_|show_add_form()>"+ Add Route"</button>
            </div>
            <p>"Model-to-provider routing rules. Provider order = priority (first = highest)."</p>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="error">{error.get()}</p> }
            )}
            {move || loading.get().then(|| view! { <p class="loading">"Loading..."</p> })}

            {move || show_form.get().then(|| {
                let strat = form_strategy.get();
                view! {
                    <div class="modal-overlay">
                        <div class="modal">
                            <h2>{if edit_id.get().is_some() { "Edit Route" } else { "Add Route" }}</h2>

                            <div class="form-group">
                                <label>"Model"</label>
                                <input type="text" prop:value=form_model.get()
                                    placeholder="e.g. gpt-4o, claude-sonnet-4, * (wildcard)"
                                    on:input=move|ev|form_model.set(event_target_value(&ev))/>
                            </div>

                            <div class="form-group">
                                <label>"Strategy / Mode"</label>
                                <select prop:value=form_strategy.get()
                                    on:change=move|ev|form_strategy.set(event_target_value(&ev))>
                                    <option value="single">"Single — use one provider"</option>
                                    <option value="fallback">"Fallback — try providers in order"</option>
                                    <option value="round-robin">"Round-Robin — rotate providers"</option>
                                    <option value="fusion">"Fusion — parallel fan-out + judge"</option>
                                </select>
                            </div>

                            // Provider(s)
                            {move || (form_strategy.get() == "single").then(|| {
                                view! {
                                    <div class="form-group">
                                        <label>"Provider"</label>
                                        <input type="text" prop:value=form_provider.get()
                                            placeholder="e.g. openai"
                                            on:input=move|ev|form_provider.set(event_target_value(&ev))/>
                                    </div>
                                }
                            })}
                            {move || (form_strategy.get() != "single").then(|| {
                                view! {
                                    <div class="form-group">
                                        <label>"Providers (comma separated, first = highest priority)"</label>
                                        <input type="text" prop:value=form_providers.get()
                                            placeholder="e.g. openai, anthropic, groq"
                                            on:input=move|ev|form_providers.set(event_target_value(&ev))/>
                                    </div>
                                }
                            })}

                            // ─── Fusion combo settings ──────────────────
                            {move || (form_strategy.get() == "fusion").then(|| {
                                view! {
                                    <>
                                        <h3 style="margin: 1rem 0 0.5rem; font-size: 0.9rem; color: #8b949e; border-bottom: 1px solid #30363d; padding-bottom: 0.5rem;">
                                            "Fusion Settings"
                                        </h3>
                                        <div class="form-group">
                                            <label>"Judge Model"</label>
                                            <input type="text" prop:value=form_judge_model.get()
                                                placeholder="e.g. gpt-4o-mini"
                                                on:input=move|ev|form_judge_model.set(event_target_value(&ev))/>
                                        </div>
                                        <div class="form-group">
                                            <label>"Min Panel (responses needed before synthesis)"</label>
                                            <input type="number" prop:value=form_min_panel.get() min="1"
                                                on:input=move|ev|form_min_panel.set(event_target_value(&ev))/>
                                        </div>
                                        <div class="form-group">
                                            <label>"Straggler Grace (ms, wait for slower providers)"</label>
                                            <input type="number" prop:value=form_straggler_grace.get() min="0" step="100"
                                                on:input=move|ev|form_straggler_grace.set(event_target_value(&ev))/>
                                        </div>
                                        <div class="form-group">
                                            <label>"Panel Hard Timeout (ms, max total wait)"</label>
                                            <input type="number" prop:value=form_panel_timeout.get() min="1000" step="1000"
                                                on:input=move|ev|form_panel_timeout.set(event_target_value(&ev))/>
                                        </div>
                                    </>
                                }
                            })}

                            // ─── Round-Robin combo settings ────────────
                            {move || (form_strategy.get() == "round-robin").then(|| {
                                view! {
                                    <>
                                        <h3 style="margin: 1rem 0 0.5rem; font-size: 0.9rem; color: #8b949e; border-bottom: 1px solid #30363d; padding-bottom: 0.5rem;">
                                            "Round-Robin Settings"
                                        </h3>
                                        <div class="form-group">
                                            <label>"Sticky Limit (keep same provider N consecutive requests, empty = no sticky)"</label>
                                            <input type="number" prop:value=form_sticky_limit.get() min="1"
                                                placeholder="e.g. 3"
                                                on:input=move|ev|form_sticky_limit.set(event_target_value(&ev))/>
                                        </div>
                                    </>
                                }
                            })}

                            <div class="form-actions">
                                <button class="btn" on:click=move|_|show_form.set(false)>"Cancel"</button>
                                <button class="btn btn-primary" on:click=move|_|save()>"Save"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            {move || (!loading.get() && !show_form.get()).then(|| {
                let rs = route_list.get();
                view! {
                    <table class="data-table">
                        <thead><tr>
                            <th>"Model"</th><th>"Mode"</th><th>"Providers"</th><th>"Combo Settings"</th><th>"Actions"</th>
                        </tr></thead>
                        <tbody>
                            {rs.into_iter().map(|r| {
                                let id = r.id.clone();
                                let prov_str = r.provider.clone().unwrap_or_else(||
                                    r.providers.clone().unwrap_or_default().join(", "));
                                let combo_str = fmt_combo(&r);
                                view! {
                                    <tr>
                                        <td><code>{r.model.clone()}</code></td>
                                        <td><span class="badge badge-paid">{r.strategy.clone()}</span></td>
                                        <td style="font-size:0.8rem;">{prov_str}</td>
                                        <td style="font-size:0.75rem; color:#8b949e; max-width:200px; overflow:hidden; text-overflow:ellipsis;">
                                            {combo_str}
                                        </td>
                                        <td class="actions">
                                            <button class="btn btn-sm" on:click=move|_|show_edit_form(r.clone())>"Edit"</button>
                                            <button class="btn btn-sm btn-danger" on:click=move|_|delete_rt(id.clone())>"Del"</button>
                                        </td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                }
            })}
        </div>
    }
}
