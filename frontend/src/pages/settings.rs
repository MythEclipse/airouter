use leptos::*;

#[component]
pub fn Settings() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"Settings"</h1>
            <p>"Gateway configuration."</p>
            <div class="settings-section">
                <div class="setting-row">
                    <span>"API Status"</span>
                    <span class="status-ok">"connected"</span>
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
                <div class="setting-row">
                    <span>"Total Tests"</span>
                    <span>"87 unit + 23 E2E"</span>
                </div>
            </div>
        </div>
    }
}
