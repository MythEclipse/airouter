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
    let error = create_rw_signal(String::new());

    spawn_local({
        let metrics = metrics.clone();
        let live = live.clone();
        let free_count = free_count.clone();
        let free_models = free_models.clone();
        let loading = loading.clone();
        let error = error.clone();
        async move {
            match fetch_dashboard().await {
                Ok(data) => {
                    // Use category field instead of hardcoded provider_type strings
                    let fc = data.providers.iter()
                        .filter(|p| p.category.as_deref() == Some("free") || p.category.as_deref() == Some("free-tier"))
                        .count();
                    let fm: usize = data.providers.iter()
                        .filter(|p| p.category.as_deref() == Some("free") || p.category.as_deref() == Some("free-tier"))
                        .map(|p| p.model_count)
                        .sum();
                    metrics.set(data.metrics);
                    live.set(data.live_metrics);
                    free_count.set(fc);
                    free_models.set(fm);
                    loading.set(false);
                }
                Err(e) => {
                    error.set(e);
                    loading.set(false);
                }
            }
        }
    });

    view! {
        <div class="animate-fade-in">
            <div class="mb-8">
                <h1 class="text-2xl font-bold text-primary font-display tracking-tight">"Analytics"</h1>
                <p class="text-sm text-secondary mt-1">"Gateway usage overview"</p>
            </div>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger/20">{error.get()}</p> }
            )}

            {move || loading.get().then(|| view! { <SkeletonCards count=4/> })}

            {move || (!loading.get()).then(|| {
                let m = metrics.get();
                let lv = live.get();
                let fc = free_count.get();
                let fm = free_models.get();
                let f_pct = if m.total_models > 0 { (fm as f64 / m.total_models as f64 * 100.0) as u32 } else { 0 };
                let upt = format!("{}h {}m", lv.uptime_seconds / 3600, (lv.uptime_seconds % 3600) / 60);

                view! {
                    <h2 class="text-base font-semibold text-primary font-display tracking-tight mb-4">"Providers & Models"</h2>
                    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
                        <StatCard label="Total Providers" value=m.total_providers.to_string() sub="Configured".to_string()/>
                        <StatCard label="Total Models" value=m.total_models.to_string() sub="Available".to_string()/>
                        <StatCard label="Free Models" value=format!("{}%", f_pct) sub=format!("{} of {} models", fm, m.total_models)/>
                        <StatCard label="Free Providers" value=fc.to_string() sub="Built-in".to_string()/>
                    </div>

                    <h2 class="text-base font-semibold text-primary font-display tracking-tight mb-4">"Live Metrics"</h2>
                    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
                        <StatCard label="Total Requests" value=lv.total_requests.to_string() sub="Since start".to_string()/>
                        <StatCard label="Errors" value=lv.total_errors.to_string() sub=format!("{:.1}% error rate", lv.error_rate * 100.0)/>
                        <StatCard label="Avg Latency" value=format!("{:.0}ms", lv.avg_latency_ms) sub="Per request".to_string()/>
                        <StatCard label="Uptime" value=upt sub="Server running".to_string()/>
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn StatCard(label: &'static str, value: String, sub: String) -> impl IntoView {
    view! {
        <div class="card-base p-5">
            <p class="text-[11px] text-muted font-semibold uppercase tracking-wider mb-1.5">{label}</p>
            <p class="text-2xl font-bold text-primary font-display tracking-tight">{value}</p>
            <p class="text-xs text-secondary mt-1">{sub}</p>
        </div>
    }
}
