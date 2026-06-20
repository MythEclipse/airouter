use leptos::*;
use crate::api::{fetch_dashboard, MetricsData, LiveMetrics};

#[component]
pub fn Analytics() -> impl IntoView {
    let metrics = create_rw_signal(MetricsData { total_providers: 0, total_models: 0, built_in_free: true });
    let live = create_rw_signal(LiveMetrics { total_requests: 0, total_errors: 0, avg_latency_ms: 0.0, error_rate: 0.0, uptime_seconds: 0 });
    let free_count = create_rw_signal(0usize);
    let free_models = create_rw_signal(0usize);
    let loading = create_rw_signal(true);

    spawn_local(async move {
        if let Ok(data) = fetch_dashboard().await {
            let fc = data.providers.iter().filter(|p| p.provider_type == "opencode_free" || p.provider_type == "mimo_free").count();
            let fm: usize = data.providers.iter().filter(|p| p.provider_type == "opencode_free" || p.provider_type == "mimo_free").map(|p| p.model_count).sum();
            metrics.set(data.metrics);
            live.set(data.live_metrics);
            free_count.set(fc);
            free_models.set(fm);
            loading.set(false);
        }
    });

    view! {
        <div class="page">
            <h1>"Analytics"</h1>
            <p>"Gateway usage overview."</p>
            {move || loading.get().then(|| view! { <p class="loading">"Loading analytics..."</p> })}

            {move || (!loading.get()).then(|| {
                let m = metrics.get();
                let lv = live.get();
                let fc = free_count.get();
                let fm = free_models.get();
                let f_pct = if m.total_models > 0 { (fm as f64 / m.total_models as f64 * 100.0) as u32 } else { 0 };

                view! {
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>"Total Providers"</h3>
                            <div class="metric-value">{m.total_providers.to_string()}</div>
                            <div class="metric-subtitle">"Configured"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Total Models"</h3>
                            <div class="metric-value">{m.total_models.to_string()}</div>
                            <div class="metric-subtitle">"Available"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Free Models"</h3>
                            <div class="metric-value">{f_pct.to_string() + "%"}</div>
                            <div class="metric-subtitle">{fm.to_string() + " of " + &m.total_models.to_string() + " models"}</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Free Providers"</h3>
                            <div class="metric-value">{fc.to_string()}</div>
                            <div class="metric-subtitle">"Built-in"</div>
                        </div>
                    </div>
                    <h2>"Live Metrics"</h2>
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>"Total Requests"</h3>
                            <div class="metric-value">{lv.total_requests.to_string()}</div>
                            <div class="metric-subtitle">"Since start"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Errors"</h3>
                            <div class="metric-value">{lv.total_errors.to_string()}</div>
                            <div class="metric-subtitle">{format!("{:.1}%", lv.error_rate * 100.0)}</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Avg Latency"</h3>
                            <div class="metric-value">{format!("{:.0}ms", lv.avg_latency_ms)}</div>
                            <div class="metric-subtitle">"Per request"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Uptime"</h3>
                            <div class="metric-value">{format!("{}s", lv.uptime_seconds)}</div>
                            <div class="metric-subtitle">"Server running"</div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
