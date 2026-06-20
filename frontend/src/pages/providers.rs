use leptos::*;
use crate::api::*;

#[component]
pub fn Providers() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderDetail>::new());
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let edit_id = create_rw_signal(Option::<String>::None);
    let form_name = create_rw_signal(String::new());
    let form_type = create_rw_signal("openai_compat".into());
    let form_key = create_rw_signal(String::new());
    let form_url = create_rw_signal(String::new());
    let form_models = create_rw_signal(String::new());
    let form_caps = create_rw_signal(String::new());
    let key_revealed = create_rw_signal(Option::<String>::None);

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

    let show_add_form = move || {
        edit_id.set(None);
        form_name.set(String::new());
        form_type.set("openai_compat".into());
        form_key.set(String::new());
        form_url.set(String::new());
        form_models.set(String::new());
        form_caps.set(String::new());
        key_revealed.set(None);
        show_form.set(true);
    };

    let show_edit_form = move |p: ProviderDetail| {
        edit_id.set(Some(p.id.clone()));
        form_name.set(p.name);
        form_type.set(p.provider_type);
        form_key.set(String::new());
        form_url.set(p.base_url);
        form_models.set(p.models.join(", "));
        form_caps.set(p.capabilities.join(", "));
        key_revealed.set(None);
        show_form.set(true);
    };

    let save = move || {
        let is_edit = edit_id.get().is_some();
        let mut body = serde_json::json!({
            "name": form_name.get(),
            "provider_type": form_type.get(),
            "api_key": form_key.get(),
            "base_url": form_url.get(),
            "models": form_models.get().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>(),
            "capabilities": form_caps.get().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>(),
        });
        let body_str = serde_json::to_string(&body).unwrap_or_default();

        spawn_local({
            let providers = providers.clone();
            let loading = loading.clone();
            let error = error.clone();
            let show_form = show_form.clone();
            let edit_id = edit_id.clone();
            async move {
                let result = if let Some(id) = edit_id.get() {
                    update_provider(&id, &body_str).await
                } else {
                    create_provider(&body_str).await
                };
                match result {
                    Ok(_) => {
                        show_form.set(false);
                        edit_id.set(None);
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

    let delete_prov = move |id: String| {
        spawn_local({
            let providers = providers.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match delete_provider(&id).await {
                    Ok(()) => {
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

    view! {
        <div class="page">
            <div style="display:flex; justify-content:space-between; align-items:center;">
                <h1>"Providers"</h1>
                <button class="btn btn-primary" on:click=move|_|show_add_form()>"+ Add Provider"</button>
            </div>
            <p>"Manage upstream LLM providers."</p>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="error">{error.get()}</p> }
            )}

            {move || loading.get().then(|| view! { <p class="loading">"Loading..."</p> })}

            {move || show_form.get().then(|| {
                view! {
                    <div class="modal-overlay">
                        <div class="modal">
                            <h2>{if edit_id.get().is_some() { "Edit Provider" } else { "Add Provider" }}</h2>
                            <div class="form-group">
                                <label>"Name"</label>
                                <input type="text" prop:value=form_name.get()
                                    on:input=move|ev|form_name.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-group">
                                <label>"Type"</label>
                                <select prop:value=form_type.get()
                                    on:change=move|ev|form_type.set(event_target_value(&ev))>
                                    <option value="openai">"OpenAI"</option>
                                    <option value="anthropic">"Anthropic"</option>
                                    <option value="openai_compat">"OpenAI Compatible"</option>
                                    <option value="opencode_free">"OpenCode Free"</option>
                                    <option value="mimo_free">"MiMo Free"</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label>"API Key"</label>
                                <input type="password" prop:value=form_key.get()
                                    placeholder="(unchanged on edit)"
                                    on:input=move|ev|form_key.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-group">
                                <label>"Base URL"</label>
                                <input type="text" prop:value=form_url.get()
                                    on:input=move|ev|form_url.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-group">
                                <label>"Models (comma separated)"</label>
                                <input type="text" prop:value=form_models.get()
                                    on:input=move|ev|form_models.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-group">
                                <label>"Capabilities (comma separated: vision, audio)"</label>
                                <input type="text" prop:value=form_caps.get()
                                    on:input=move|ev|form_caps.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-actions">
                                <button class="btn" on:click=move|_|show_form.set(false)>"Cancel"</button>
                                <button class="btn btn-primary" on:click=move|_|save()>"Save"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            {move || (!loading.get() && !show_form.get()).then(|| {
                let provs = providers.get();
                view! {
                    <table class="data-table">
                        <thead><tr>
                            <th>"Name"</th><th>"Type"</th><th>"Models"</th><th>"Enabled"</th><th>"Actions"</th>
                        </tr></thead>
                        <tbody>
                            {provs.into_iter().map(|p| {
                                let id = p.id.clone();
                                view! {
                                    <tr>
                                        <td>{p.name.clone()}</td>
                                        <td><span class="badge badge-paid">{p.provider_type.clone()}</span></td>
                                        <td>{p.models.len().to_string()}</td>
                                        <td>{if p.enabled { "✓" } else { "✗" }}</td>
                                        <td class="actions">
                                            <button class="btn btn-sm" on:click=move|_|show_edit_form(p.clone())>"Edit"</button>
                                            <button class="btn btn-sm btn-danger" on:click=move|_|delete_prov(id.clone())>"Del"</button>
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
