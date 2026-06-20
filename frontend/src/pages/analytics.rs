use leptos::*;
use crate::api::{fetch_dashboard, MetricsData, LiveMetrics};
use crate::components::skeleton::SkeletonCards;

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
        <div class="animate-fade-in">
            <div class="mb-8">
                <h1 class="text-2xl font-bold text-primary">"Analytics"</h1>
                <p class="text-sm text-secondary mt-1">"Gateway usage overview"</p>
            </div>

            {move || loading.get().then(|| view! { <SkeletonCards count=4/> })}

            {move || (!loading.get()).then(|| {
                let m = metrics.get();
                let lv = live.get();
                let fc = free_count.get();
                let fm = free_models.get();
                let f_pct = if m.total_models > 0 { (fm as f64 / m.total_models as f64 * 100.0) as u32 } else { 0 };
                let upt = format!("{}h {}m", lv.uptime_seconds / 3600, (lv.uptime_seconds % 3600) / 60);

                view! {
                    <h2 class="text-lg font-semibold text-primary mb-4">"Providers & Models"</h2>
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Total Providers"</p>
                            <p class="text-2xl font-bold text-primary">{m.total_providers.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Configured"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Total Models"</p>
                            <p class="text-2xl font-bold text-primary">{m.total_models.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Available"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Free Models"</p>
                            <p class="text-2xl font-bold text-primary">{f_pct.to_string() + "%"}</p>
                            <p class="text-xs text-muted mt-1">{fm.to_string() + " of " + &m.total_models.to_string() + " models"}</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Free Providers"</p>
                            <p class="text-2xl font-bold text-primary">{fc.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Built-in"</p>
                        </div>
                    </div>

                    <h2 class="text-lg font-semibold text-primary mb-4">"Live Metrics"</h2>
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Total Requests"</p>
                            <p class="text-2xl font-bold text-primary">{lv.total_requests.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Since start"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Errors"</p>
                            <p class="text-2xl font-bold text-primary">{lv.total_errors.to_string()}</p>
                            <p class="text-xs text-muted mt-1">{format!("{:.1}% error rate", lv.error_rate * 100.0)}</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Avg Latency"</p>
                            <p class="text-2xl font-bold text-primary">{format!("{:.0}ms", lv.avg_latency_ms)}</p>
                            <p class="text-xs text-muted mt-1">"Per request"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Uptime"</p>
                            <p class="text-2xl font-bold text-primary">{upt}</p>
                            <p class="text-xs text-muted mt-1">"Server running"</p>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
