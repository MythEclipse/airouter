use leptos::*;

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub provider_type: String,
    pub status: String,
    pub latency: u64,
}

#[component]
pub fn ProviderStatusCard(provider: ProviderInfo) -> impl IntoView {
    let status_color = move || match provider.status.as_str() {
        "online" => "green",
        "error" => "red",
        _ => "gray",
    };

    view! {
        <div class="provider-card">
            <div class="provider-status">
                <span class="status-dot" style=format!("background-color: {}", status_color())></span>
                <span class="provider-name">{provider.name.clone()}</span>
            </div>
            <div class="provider-details">
                <span class="provider-type">{provider.provider_type.clone()}</span>
                <span class="provider-latency">{format!("{}ms", provider.latency)}</span>
            </div>
        </div>
    }
}
