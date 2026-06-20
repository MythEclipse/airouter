use leptos::*;
use crate::api::fetch_dashboard;

#[component]
pub fn Dashboard() -> impl IntoView {
    let data = create_resource(|| (), |_| async move {
        fetch_dashboard("sk-test-abc123").await
    });

    view! {
        <div class="dashboard">
            <h1>"Dashboard"</h1>

            <Suspense fallback=|| view! { <div class="loading">"Loading..."</div> }>
            {move || data.get().map(|d| {
                let (providers, metrics, models) = match d {
                    Ok(ref dd) => (dd.providers.clone(), dd.metrics.clone(), dd.models.clone()),
                    Err(ref e) => return view! { <div class="error">"Error: " {e}</div> }.into_any(),
                };
                view! {
                    <section>
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>"Providers"</h3>
                            <div class="metric-value">{metrics.total_providers}</div>
                            <div class="metric-subtitle">"Registered"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Models"</h3>
                            <div class="metric-value">{metrics.total_models}</div>
                            <div class="metric-subtitle">"Available"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Free Tier"</h3>
                            <div class="metric-value">{if metrics.built_in_free { "Yes" } else { "No" }}</div>
                            <div class="metric-subtitle">"Built-in free providers"</div>
                        </div>
                    </div>

                    <h2>"Providers"</h2>
                    <div class="provider-grid">
                        {providers.into_iter().map(|p| {
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
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>

                    <h2>"Models"</h2>
                    <div class="request-log">
                        <div class="log-header">
                            <span>"Model"</span>
                            <span>"Provider"</span>
                        </div>
                        {models.into_iter().map(|m| {
                            view! {
                                <div class="log-row">
                                    <span>{m.id}</span>
                                    <span>{m.owned_by}</span>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                    </section>
                }.into_any()
            })}
            </Suspense>
        </div>
    }
}
