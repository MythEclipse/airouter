use leptos::*;

async fn check_health() -> String {
    match reqwest::get("/health").await {
        Ok(r) if r.status().is_success() => "connected".into(),
        Ok(_) => "error".into(),
        Err(_) => "disconnected".into(),
    }
}

#[component]
pub fn Settings() -> impl IntoView {
    let status = create_resource(|| (), |_| async move { check_health().await });

    view! {
        <div class="page">
            <h1>"Settings"</h1>
            <p>"Gateway configuration."</p>

            <div class="settings-section">
                <div class="setting-row">
                    <span>"API Status"</span>
                    <span class=move || {
                        match status.get() {
                            None => "status-pending",
                            Some(ref s) if s == "connected" => "status-ok",
                            _ => "status-err",
                        }
                    }>
                        {move || status.get().unwrap_or("checking".into())}
                    </span>
                </div>
                <div class="setting-row">
                    <span>"API Key"</span>
                    <span>"sk-test-abc123"</span>
                </div>
                <div class="setting-row">
                    <span>"Free Providers"</span>
                    <span>"OpenCode Free + MiMo Free"</span>
                </div>
                <div class="setting-row">
                    <span>"Backend"</span>
                    <span>"Axum 0.8"</span>
                </div>
                <div class="setting-row">
                    <span>"Frontend"</span>
                    <span>"Leptos 0.6 / WASM"</span>
                </div>
            </div>
        </div>
    }
}
