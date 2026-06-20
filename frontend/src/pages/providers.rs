use leptos::*;
use crate::api::{fetch_dashboard, ProviderStatus};

#[component]
pub fn Providers() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderStatus>::new());
    let loading = create_rw_signal(true);

    spawn_local(async move {
        if let Ok(data) = fetch_dashboard().await {
            providers.set(data.providers);
            loading.set(false);
        }
    });

    view! {
        <div class="page">
            <h1>"Providers"</h1>
            <p>"Configured LLM providers."</p>
            {move || loading.get().then(|| view! { <p class="loading">"Loading providers..."</p> })}

            {move || (!loading.get()).then(|| {
                let provs = providers.get();
                view! {
                    <div class="provider-grid">
                        {provs.into_iter().map(|p| {
                            let is_free = p.provider_type == "opencode_free" || p.provider_type == "mimo_free";
                            view! {
                                <div class="provider-card" style=format!("border-left-color: {}", p.color)>
                                    <div class="provider-status">
                                        <span class="status-dot" style=format!("background-color: {}", p.color)></span>
                                        <span class="provider-name">{p.name}</span>
                                    </div>
                                    <div class="provider-details">
                                        <span class="provider-type">{p.provider_type}</span>
                                        <span class="provider-models">{p.model_count.to_string() + " models"}</span>
                                    </div>
                                    <div class="provider-badge">
                                        {if is_free {
                                            view! { <span class="badge badge-free">"FREE"</span> }
                                        } else {
                                            view! { <span class="badge badge-paid">"API KEY"</span> }
                                        }}
                                    </div>
                                    <div class="provider-extra">
                                        <span>"reqs: " {p.request_count.to_string()}</span>
                                        <span>" errors: " {p.error_count.to_string()}</span>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }
            })}
        </div>
    }
}
