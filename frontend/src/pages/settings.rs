use leptos::*;
use crate::api::*;

#[allow(unused_variables)]
#[component]
pub fn Settings() -> impl IntoView {
    let settings = create_rw_signal(Option::<SettingsData>::None);
    let loading = create_rw_signal(true);
    let saving = create_rw_signal(false);
    let error = create_rw_signal(String::new());

    let form_host = create_rw_signal("0.0.0.0".into());
    let form_port = create_rw_signal("3000".into());
    let form_max_tokens = create_rw_signal(String::new());
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
            let form_max_tokens = form_max_tokens.clone();
            let form_rl_enabled = form_rl_enabled.clone();
            let form_rl_rpm = form_rl_rpm.clone();
            let form_rl_burst = form_rl_burst.clone();
            async move {
                match fetch_settings().await {
                    Ok(data) => {
                        form_host.set(data.server.host.clone());
                        form_port.set(data.server.port.to_string());
                        form_max_tokens.set(data.server.default_max_tokens.map(|v| v.to_string()).unwrap_or_default());
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
        error.set(String::new());
        let mt_str = form_max_tokens.get();
        let default_max_tokens: Option<i32> = if mt_str.is_empty() {
            None
        } else {
            mt_str.parse::<i32>().ok().filter(|&n| n > 0)
        };
        let body = serde_json::json!({
            "server": {
                "host": form_host.get(),
                "port": form_port.get().parse::<i32>().unwrap_or(3000),
                "default_max_tokens": default_max_tokens,
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
            async move {
                match update_settings(&body_str).await {
                    Ok(data) => {
                        settings.set(Some(data));
                        saving.set(false);
                    }
                    Err(e) => { error.set(e); saving.set(false); }
                }
            }
        });
    };

    view! {
        <div class="animate-fade-in max-w-2xl">
            <div class="mb-8">
                <h1 class="text-2xl font-bold text-primary">"Settings"</h1>
                <p class="text-sm text-secondary mt-1">"Server and rate limit configuration"</p>
            </div>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger">{error.get()}</p> }
            )}
            {move || loading.get().then(|| {
                view! {
                    <div class="space-y-4">
                        { (0..4).map(|_| view! { <div class="h-16 bg-surface-hover skeleton rounded-[14px]"></div> }).collect::<Vec<_>>() }
                    </div>
                }
            })}

            {move || settings.get().map(|_| {
                view! {
                    <div class="space-y-6">
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6 animate-fade-in-up">
                            <h3 class="text-sm font-semibold text-secondary uppercase tracking-wider mb-4 pb-3 border-b border-border-subtle">
                                "Server"
                            </h3>
                            <div class="space-y-4">
                                <div>
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Host"</label>
                                    <input type="text" prop:value=form_host.get()
                                        on:input=move|ev|form_host.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div>
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Port"</label>
                                    <input type="number" prop:value=form_port.get()
                                        on:input=move|ev|form_port.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                            </div>
                        </div>

                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6 animate-fade-in-up">
                            <h3 class="text-sm font-semibold text-secondary uppercase tracking-wider mb-4 pb-3 border-b border-border-subtle">
                                "Max Tokens"
                            </h3>
                            <div>
                                <label class="block text-xs text-secondary mb-1.5 font-medium">
                                    "Default Max Tokens"
                                </label>
                                <input type="number" prop:value=form_max_tokens.get() min="0"
                                    placeholder="Leave empty = auto / use provider default"
                                    on:input=move|ev|form_max_tokens.set(event_target_value(&ev))
                                    class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                           text-sm text-primary placeholder-muted
                                           focus:border-accent focus:outline-none transition-colors"/>
                                <p class="text-xs text-muted mt-1.5">
                                    "When set, all requests without an explicit max_tokens will use this value. Leave empty to let the provider decide."
                                </p>
                            </div>
                        </div>

                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6 animate-fade-in-up">
                            <h3 class="text-sm font-semibold text-secondary uppercase tracking-wider mb-4 pb-3 border-b border-border-subtle">
                                "Rate Limit"
                            </h3>
                            <div class="space-y-4">
                                <div class="flex items-center gap-3">
                                    <input type="checkbox" prop:checked=form_rl_enabled.get()
                                        on:change=move|ev|form_rl_enabled.set(event_target_checked(&ev))
                                        class="w-4 h-4 rounded border-surface bg-surface-2 transition-colors"/>
                                    <label class="text-sm text-primary">"Enabled"</label>
                                </div>
                                <div>
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Requests per Minute"</label>
                                    <input type="number" prop:value=form_rl_rpm.get()
                                        on:input=move|ev|form_rl_rpm.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div>
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">"Burst Size"</label>
                                    <input type="number" prop:value=form_rl_burst.get()
                                        on:input=move|ev|form_rl_burst.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                            </div>
                        </div>

                        <div class="flex justify-end pt-2">
                            <button on:click=move|_|save() disabled=saving.get()
                                class="px-6 py-2.5 text-sm font-medium rounded-lg text-white
                                       bg-accent hover:bg-accent-hover active:scale-[0.97]
                                       disabled:opacity-50 transition-all duration-150 flex items-center gap-2">
                                {saving.get().then(|| view! { "Saving..." }).unwrap_or(view! { "Save Settings" })}
                            </button>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
