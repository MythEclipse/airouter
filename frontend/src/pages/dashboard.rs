use leptos::*;
use crate::api::{fetch_dashboard, ProviderStatus, MetricsData, LiveMetrics};
use crate::components::skeleton::SkeletonCards;

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
        <div class="animate-fade-in">
            <div class="flex items-center justify-between mb-8">
                <div>
                    <h1 class="text-2xl font-bold text-primary">"Dashboard"</h1>
                    <p class="text-sm text-secondary mt-1">"System overview and provider health"</p>
                </div>
            </div>

            {move || loading.get().then(|| view! { <SkeletonCards count=4/> })}

            {move || (!loading.get()).then(|| {
                let tp = metrics.with(|m| m.total_providers);
                let tm = metrics.with(|m| m.total_models);
                let tq = live.with(|m| m.total_requests);
                let lat = live.with(|m| format!("{:.1}ms", m.avg_latency_ms));
                let er = live.with(|m| format!("{:.1}%", m.error_rate * 100.0));
                let upt = live.with(|m| {
                    let h = m.uptime_seconds / 3600;
                    let m2 = (m.uptime_seconds % 3600) / 60;
                    format!("{}h {}m", h, m2)
                });
                let provs = providers.get();

                view! {
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4 mb-8">
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Providers"</p>
                            <p class="text-2xl font-bold text-primary">{tp.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Registered"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Models"</p>
                            <p class="text-2xl font-bold text-primary">{tm.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Available"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Requests"</p>
                            <p class="text-2xl font-bold text-primary">{tq.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Total processed"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Latency"</p>
                            <p class="text-2xl font-bold text-primary">{lat}</p>
                            <p class="text-xs text-muted mt-1">"Average"</p>
                        </div>
                        <div class="bg-surface-alt border border-surface rounded-xl p-5 hover:border-surface-hover transition-all duration-200 hover:-translate-y-0.5">
                            <p class="text-xs text-secondary mb-1.5 font-medium uppercase tracking-wider">"Error Rate"</p>
                            <p class="text-2xl font-bold text-primary">{er}</p>
                            <p class="text-xs text-muted mt-1">"Across all providers"</p>
                        </div>
                    </div>

                    <h2 class="text-lg font-semibold text-primary mb-4">"Providers"</h2>
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 animate-fade-in-up">
                        {provs.into_iter().map(|p| {
                            let color = p.color.clone();
                            let health = if p.healthy { "Healthy" } else { "Degraded" };
                            let health_color = if p.healthy { "text-success" } else { "text-warning" };
                            view! {
                                <div class="bg-surface-alt border-l-4 border-surface
                                            rounded-xl p-5 hover:border-l-accent
                                            transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg"
                                    style=format!("border-left-color: {}", color)>
                                    <div class="flex items-center gap-2.5 mb-3">
                                        <span class="w-3 h-3 rounded-full inline-block" style=format!("background-color: {}", color)></span>
                                        <span class="font-semibold text-sm text-primary">{p.name}</span>
                                    </div>
                                    <div class="flex items-center justify-between text-xs mb-2">
                                        <span class="text-secondary">{p.model_count.to_string() + " models"}</span>
                                        <span class=format!("font-medium {}", health_color)>{health}</span>
                                    </div>
                                    <div class="flex gap-4 text-xs text-muted">
                                        <span>"Reqs: " {p.request_count.to_string()}</span>
                                        <span>"Errs: " {p.error_count.to_string()}</span>
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
