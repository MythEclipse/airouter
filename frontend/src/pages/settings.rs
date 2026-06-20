use leptos::*;
use crate::api::*;

#[component]
pub fn Settings() -> impl IntoView {
    let settings = create_rw_signal(Option::<SettingsData>::None);
    let loading = create_rw_signal(true);
    let saving = create_rw_signal(false);
    let error = create_rw_signal(String::new());
    let success = create_rw_signal(String::new());

    let form_host = create_rw_signal("0.0.0.0".into());
    let form_port = create_rw_signal("3000".into());
    let form_rl_enabled = create_rw_signal(true);
    let form_rl_rpm = create_rw_signal("60".into());
    let form_rl_burst = create_rw_signal("20".into());

    let load = move || {
        spawn_local({
            let settings = settings.clone();
            let loading = loading.clone();
            let error = error.clone();
            let form_host = form_host.clone();
            let form_port = form_port.clone();
            let form_rl_enabled = form_rl_enabled.clone();
            let form_rl_rpm = form_rl_rpm.clone();
            let form_rl_burst = form_rl_burst.clone();
            async move {
                match fetch_settings().await {
                    Ok(data) => {
                        form_host.set(data.server.host.clone());
                        form_port.set(data.server.port.to_string());
                        form_rl_enabled.set(data.rate_limit.enabled);
                        form_rl_rpm.set(data.rate_limit.requests_per_minute.to_string());
                        form_rl_burst.set(data.rate_limit.burst_size.to_string());
                        settings.set(Some(data));
                        loading.set(false);
                    }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            }
        });
    };
    load();

    let save = move || {
        saving.set(true);
        success.set(String::new());
        let body = serde_json::json!({
            "server": {
                "host": form_host.get(),
                "port": form_port.get().parse::<i32>().unwrap_or(3000),
            },
            "rate_limit": {
                "enabled": form_rl_enabled.get(),
                "requests_per_minute": form_rl_rpm.get().parse::<i64>().unwrap_or(60),
                "burst_size": form_rl_burst.get().parse::<i32>().unwrap_or(20),
            }
        });
        let body_str = serde_json::to_string(&body).unwrap_or_default();

        spawn_local({
            let settings = settings.clone();
            let saving = saving.clone();
            let error = error.clone();
            let success = success.clone();
            async move {
                match update_settings(&body_str).await {
                    Ok(data) => {
                        settings.set(Some(data));
                        saving.set(false);
                        success.set("Settings saved successfully!".into());
                    }
                    Err(e) => { error.set(e); saving.set(false); }
                }
            }
        });
    };

    view! {
        <div class="page">
            <h1>"Settings"</h1>
            <p>"Server and rate limit configuration."</p>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="error">{error.get()}</p> }
            )}
            {move || (!success.get().is_empty()).then(||
                view! { <p class="success-msg">{success.get()}</p> }
            )}
            {move || loading.get().then(|| view! { <p class="loading">"Loading..."</p> })}

            {move || settings.get().map(|_| {
                view! {
                    <div class="settings-section">
                        <h3>"Server"</h3>
                        <div class="form-group">
                            <label>"Host"</label>
                            <input type="text" prop:value=form_host.get()
                                on:input=move|ev|form_host.set(event_target_value(&ev))/>
                        </div>
                        <div class="form-group">
                            <label>"Port"</label>
                            <input type="number" prop:value=form_port.get()
                                on:input=move|ev|form_port.set(event_target_value(&ev))/>
                        </div>
                    </div>
                    <div class="settings-section">
                        <h3>"Rate Limit"</h3>
                        <div class="form-group">
                            <label>"Enabled"</label>
                            <input type="checkbox" prop:checked=form_rl_enabled.get()
                                on:change=move|ev|form_rl_enabled.set(event_target_checked(&ev))/>
                        </div>
                        <div class="form-group">
                            <label>"Requests per Minute"</label>
                            <input type="number" prop:value=form_rl_rpm.get()
                                on:input=move|ev|form_rl_rpm.set(event_target_value(&ev))/>
                        </div>
                        <div class="form-group">
                            <label>"Burst Size"</label>
                            <input type="number" prop:value=form_rl_burst.get()
                                on:input=move|ev|form_rl_burst.set(event_target_value(&ev))/>
                        </div>
                    </div>
                    <div class="form-actions">
                        <button class="btn btn-primary" on:click=move|_|save()
                            disabled=saving.get()>
                            {move || if saving.get() { "Saving..." } else { "Save Settings" }}
                        </button>
                    </div>
                }
            })}
        </div>
    }
}
