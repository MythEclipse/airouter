use leptos::*;
use crate::api::*;

#[component]
pub fn ApiKeys() -> impl IntoView {
    let keys = create_rw_signal(Vec::<ApiKeyDetail>::new());
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let form_name = create_rw_signal(String::new());
    let new_key_data = create_rw_signal(Option::<ApiKeyCreateResponse>::None);

    let load = move || {
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match fetch_api_keys().await {
                    Ok(data) => { keys.set(data); loading.set(false); }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            }
        });
    };
    load();

    let create = move || {
        let body_str = serde_json::json!({"key_name": form_name.get()}).to_string();
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            let show_form = show_form.clone();
            let new_key_data = new_key_data.clone();
            let form_name = form_name.clone();
            async move {
                match create_api_key(&body_str).await {
                    Ok(resp) => {
                        new_key_data.set(Some(resp));
                        form_name.set(String::new());
                        show_form.set(false);
                        loading.set(true);
                        match fetch_api_keys().await {
                            Ok(data) => { keys.set(data); loading.set(false); }
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    let delete_ak = move |id: String| {
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match delete_api_key(&id).await {
                    Ok(()) => {
                        loading.set(true);
                        match fetch_api_keys().await {
                            Ok(data) => { keys.set(data); loading.set(false); }
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
                <h1>"API Keys"</h1>
                <button class="btn btn-primary" on:click=move|_|show_form.set(true)>"+ New Key"</button>
            </div>
            <p>"Manage API keys for client access."</p>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="error">{error.get()}</p> }
            )}
            {move || loading.get().then(|| view! { <p class="loading">"Loading..."</p> })}

            // New key reveal
            {move || new_key_data.get().map(|nk| {
                view! {
                    <div class="key-reveal">
                        <p><strong>"New API Key Created — Copy it now, it won't be shown again!"</strong></p>
                        <pre class="full-key">{nk.full_key.clone()}</pre>
                    </div>
                }
            })}

            {move || show_form.get().then(|| {
                view! {
                    <div class="modal-overlay">
                        <div class="modal">
                            <h2>"Create API Key"</h2>
                            <div class="form-group">
                                <label>"Key Name"</label>
                                <input type="text" prop:value=form_name.get()
                                    placeholder="e.g. Production key"
                                    on:input=move|ev|form_name.set(event_target_value(&ev))/>
                            </div>
                            <div class="form-actions">
                                <button class="btn" on:click=move|_|show_form.set(false)>"Cancel"</button>
                                <button class="btn btn-primary" on:click=move|_|create()>"Create"</button>
                            </div>
                        </div>
                    </div>
                }
            })}

            {move || (!loading.get() && !show_form.get()).then(|| {
                let ks = keys.get();
                view! {
                    <table class="data-table">
                        <thead><tr>
                            <th>"Name"</th><th>"Key Prefix"</th><th>"Created"</th><th>"Actions"</th>
                        </tr></thead>
                        <tbody>
                            {ks.into_iter().map(|k| {
                                let id = k.id.clone();
                                view! {
                                    <tr>
                                        <td>{k.key_name}</td>
                                        <td><code>{k.key_prefix}...</code></td>
                                        <td>{k.created_at[..10].to_string()}</td>
                                        <td class="actions">
                                            <button class="btn btn-sm btn-danger"
                                                on:click=move|_|delete_ak(id.clone())>"Revoke"</button>
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
