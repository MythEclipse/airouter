use leptos::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct LoginResponse {
    ok: bool,
    dashboard_token: String,
    ai_token: String,
    message: String,
}

#[component]
pub fn Login() -> impl IntoView {
    let password = create_rw_signal(String::new());
    let error_msg = create_rw_signal(String::new());
    let logging_in = create_rw_signal(false);

    let do_login = move || {
        let pwd = password.get();
        if pwd.is_empty() {
            error_msg.set("Please enter a password".into());
            return;
        }
        logging_in.set(true);
        error_msg.set(String::new());

        let body = serde_json::json!({ "password": pwd }).to_string();
        spawn_local({
            let password = password.clone();
            let error_msg = error_msg.clone();
            let logging_in = logging_in.clone();
            async move {
                let window = web_sys::window().unwrap();
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_mode(web_sys::RequestMode::Cors);
                opts.set_body(&wasm_bindgen::JsValue::from_str(&body));
                let request = web_sys::Request::new_with_str_and_init("/api/auth/login", &opts).unwrap();
                request.headers().set("Content-Type", "application/json").ok();

                match JsFuture::from(window.fetch_with_request(&request)).await {
                    Ok(r) => {
                        let r: web_sys::Response = r.dyn_into().unwrap();
                        let json = JsFuture::from(r.json().unwrap()).await;
                        match json {
                            Ok(j) => {
                                if let Ok(resp) = serde_wasm_bindgen::from_value::<LoginResponse>(j) {
                                    if resp.ok {
                                        // Store tokens in localStorage
                                        let storage = window.local_storage().ok().flatten();
                                        if let Some(storage) = storage {
                                            let _ = storage.set_item("dashboard_token", &resp.dashboard_token);
                                            let _ = storage.set_item("ai_token", &resp.ai_token);
                                        }
                                        // Redirect to dashboard
                                        let loc = window.location();
                                        let _ = loc.set_href("/");
                                    } else {
                                        error_msg.set(resp.message);
                                    }
                                } else {
                                    error_msg.set("Login failed".into());
                                }
                            }
                            Err(_) => error_msg.set("Login failed".into()),
                        }
                    }
                    Err(_) => error_msg.set("Network error".into()),
                }
                logging_in.set(false);
            }
        });
    };

    // Check if already logged in
    let window = web_sys::window().unwrap();
    let storage = window.local_storage().ok().flatten();
    let has_token = storage.map(|s| s.get_item("dashboard_token").ok().flatten().is_some()).unwrap_or(false);
    if has_token {
        let loc = window.location();
        let _ = loc.set_href("/");
    }

    view! {
        <div class="min-h-screen bg-bg flex items-center justify-center p-4">
            <div class="w-full max-w-sm">
                <div class="text-center mb-8">
                    <h1 class="text-3xl font-bold text-primary">"AIRouter"</h1>
                    <p class="text-secondary text-sm mt-2">"Sign in to your dashboard"</p>
                </div>

                <div class="bg-surface border border-border-subtle rounded-[14px] p-6 shadow-xl">
                    <div class="mb-4">
                        <label class="block text-xs text-secondary mb-1.5 font-medium">"Password"</label>
                        <input
                            type="password"
                            prop:value=password.get()
                            placeholder="Enter password"
                            on:input=move|ev| password.set(event_target_value(&ev))
                            on:keydown=move|ev| {
                                if ev.key() == "Enter" { do_login(); }
                            }
                            disabled=move || logging_in.get()
                            class="w-full px-3 py-2 bg-surface-2 border border-border-subtle rounded-lg
                                   text-sm text-primary placeholder-muted
                                   focus:border-accent focus:outline-none transition-colors
                                   disabled:opacity-50"
                        />
                    </div>

                    {move || (!error_msg.get().is_empty()).then(|| {
                        view! {
                            <p class="mb-4 text-sm text-danger bg-danger-bg p-2.5 rounded-lg border border-danger/30">
                                {error_msg.get()}
                            </p>
                        }
                    })}

                    <button
                        on:click=move|_| do_login()
                        disabled=move || logging_in.get()
                        class="w-full px-4 py-2.5 text-sm font-medium rounded-lg text-white
                               bg-accent hover:bg-accent-hover
                               active:scale-[0.97] transition-all duration-150
                               disabled:opacity-50 disabled:cursor-not-allowed
                               flex items-center justify-center gap-2"
                    >
                        {move || if logging_in.get() {
                            view! {
                                <>
                                    <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
                                    </svg>
                                    "Signing in..."
                                </>
                            }.into_view()
                        } else {
                            view! { "Sign In" }.into_view()
                        }}
                    </button>

                    <p class="text-xs text-muted text-center mt-4">
                        "Default password: "
                        <code class="font-mono text-primary bg-surface-2 px-1.5 py-0.5 rounded">123456</code>
                    </p>
                </div>
            </div>
        </div>
    }
}
