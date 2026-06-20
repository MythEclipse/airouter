use leptos::*;
use crate::api::{fetch_dashboard, ProviderStatus, MetricsData, LiveMetrics};

#[component]
pub fn Dashboard() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderStatus>::new());
    let metrics = create_rw_signal(MetricsData { total_providers: 0, total_models: 0, built_in_free: true });
    let live = create_rw_signal(LiveMetrics { total_requests: 0, total_errors: 0, avg_latency_ms: 0.0, error_rate: 0.0, uptime_seconds: 0 });
    let loading = create_rw_signal(true);

    spawn_local(async move {
        if let Ok(data) = fetch_dashboard().await {
            providers.set(data.providers);
            metrics.set(data.metrics);
            live.set(data.live_metrics);
            loading.set(false);
        }
    });

    view! {
        <div class="dashboard">
            <h1>"Dashboard"</h1>
            {move || loading.get().then(|| view! { <p class="loading">"Loading dashboard..."</p> })}

            {move || (!loading.get()).then(|| {
                let tp = metrics.with(|m| m.total_providers);
                let tm = metrics.with(|m| m.total_models);
                let tq = live.with(|m| m.total_requests);
                let er = live.with(|m| format!("{:.1}%", m.error_rate * 100.0));
                let provs = providers.get();

                view! {
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>"Providers"</h3>
                            <div class="metric-value">{tp.to_string()}</div>
                            <div class="metric-subtitle">"Registered"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Models"</h3>
                            <div class="metric-value">{tm.to_string()}</div>
                            <div class="metric-subtitle">"Available"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Requests"</h3>
                            <div class="metric-value">{tq.to_string()}</div>
                            <div class="metric-subtitle">"Total processed"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Error Rate"</h3>
                            <div class="metric-value">{er}</div>
                            <div class="metric-subtitle">"Across all providers"</div>
                        </div>
                    </div>

                    <h2>"Providers"</h2>
                    <div class="provider-grid">
                        {provs.into_iter().map(|p| {
                            let health = if p.healthy { "healthy" } else { "degraded" };
                            view! {
                                <div class="provider-card" style=format!("border-left-color: {}", p.color)>
                                    <div class="provider-status">
                                        <span class="status-dot" style=format!("background-color: {}", p.color)></span>
                                        <span class="provider-name">{p.name}</span>
                                    </div>
                                    <div class="provider-details">
                                        <span class="model-count">{p.model_count.to_string() + " models"}</span>
                                        <span>{health}</span>
                                    </div>
                                    <div class="provider-stats">
                                        <span>"reqs: " {p.request_count.to_string()}</span>
                                        <span>" errs: " {p.error_count.to_string()}</span>
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
