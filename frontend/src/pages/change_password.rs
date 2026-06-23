use leptos::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[component]
pub fn ChangePassword() -> impl IntoView {
    let new_password = create_rw_signal(String::new());
    let confirm_password = create_rw_signal(String::new());
    let error = create_rw_signal(String::new());
    let loading = create_rw_signal(false);

    let do_submit = move || {
        let pwd = new_password.get();
        let confirm = confirm_password.get();
        if pwd.is_empty() || confirm.is_empty() {
            error.set("Please fill in both fields".into());
            return;
        }
        if pwd != confirm {
            error.set("Passwords do not match".into());
            return;
        }
        if pwd.len() < 12 {
            error.set("Password must be at least 12 characters".into());
            return;
        }
        loading.set(true);
        error.set(String::new());

        let body = serde_json::json!({
            "new_password": pwd,
            "confirm_password": confirm,
        }).to_string();

        let window = web_sys::window().unwrap();
        let change_token = window.local_storage().ok().flatten()
            .and_then(|s| s.get_item("dashboard_token").ok().flatten())
            .unwrap_or_default();

        spawn_local({
            let error = error.clone();
            let loading = loading.clone();
            async move {
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_mode(web_sys::RequestMode::Cors);
                opts.set_body(&wasm_bindgen::JsValue::from_str(&body));

                let request = web_sys::Request::new_with_str_and_init(
                    "/api/auth/change-password", &opts,
                ).unwrap();
                request.headers().set("Content-Type", "application/json").ok();
                request.headers().set("Authorization", &format!("Bearer {}", change_token)).ok();

                match JsFuture::from(window.fetch_with_request(&request)).await {
                    Ok(r) => {
                        let r: web_sys::Response = r.dyn_into().unwrap();
                        if r.status() == 200 {
                            let json = JsFuture::from(r.json().unwrap()).await;
                            if let Ok(j) = json {
                                use serde::Deserialize;
                                #[derive(Deserialize)]
                                struct ChangePwdResp { token: String }
                                if let Ok(resp) = serde_wasm_bindgen::from_value::<ChangePwdResp>(j) {
                                    let storage = window.local_storage().ok().flatten();
                                    if let Some(s) = storage {
                                        let _ = s.set_item("dashboard_token", &resp.token);
                                    }
                                    let loc = window.location();
                                    let _ = loc.set_href("/");
                                }
                            }
                        } else {
                            let json_body = JsFuture::from(r.json().unwrap()).await;
                            if let Ok(j) = json_body {
                                if let Some(msg) = j.as_string() {
                                    error.set(msg);
                                }
                            } else {
                                error.set("Failed to change password".into());
                            }
                        }
                    }
                    Err(_) => error.set("Network error".into()),
                }
                loading.set(false);
            }
        });
    };

    view! {
        <div class="min-h-screen bg-bg flex items-center justify-center p-4">
            <div class="w-full max-w-sm relative">
                <div class="text-center mb-8">
                    <h1 class="text-3xl font-bold text-primary font-display tracking-tight">"Change Password"</h1>
                    <p class="text-secondary text-sm mt-2">"You must change your password before continuing"</p>
                </div>

                <div class="bg-surface border border-border-subtle rounded-xl p-6 shadow-[var(--shadow-elev)]">
                    <div class="mb-4">
                        <label class="block text-xs text-secondary mb-1.5 font-medium">"New Password"</label>
                        <input
                            type="password"
                            placeholder="Minimum 12 characters"
                            prop:value=new_password.get()
                            on:input=move|ev| new_password.set(event_target_value(&ev))
                            disabled=move || loading.get()
                            class="w-full px-3 py-2.5 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:ring-2 focus:ring-accent/20 focus:outline-none transition-all disabled:opacity-50"
                        />
                    </div>

                    <div class="mb-4">
                        <label class="block text-xs text-secondary mb-1.5 font-medium">"Confirm Password"</label>
                        <input
                            type="password"
                            placeholder="Re-enter new password"
                            prop:value=confirm_password.get()
                            on:input=move|ev| confirm_password.set(event_target_value(&ev))
                            disabled=move || loading.get()
                            class="w-full px-3 py-2.5 bg-surface-2 border border-border-subtle rounded-lg text-sm text-primary placeholder-muted focus:border-accent focus:ring-2 focus:ring-accent/20 focus:outline-none transition-all disabled:opacity-50"
                        />
                    </div>

                    {move || (!error.get().is_empty()).then(|| {
                        view! {
                            <p class="mb-4 text-sm text-danger bg-danger-bg p-2.5 rounded-lg border border-danger/20">
                                {error.get()}
                            </p>
                        }
                    })}

                    <button
                        on:click=move|_| do_submit()
                        disabled=move || loading.get()
                        class="btn-base w-full px-4 py-2.5 text-sm rounded-lg bg-accent hover:bg-accent-hover text-white disabled:opacity-50"
                    >
                        {move || if loading.get() {
                            view! { "Changing password..." }.into_view()
                        } else {
                            view! { "Change Password" }.into_view()
                        }}
                    </button>
                </div>
            </div>
        </div>
    }
}
