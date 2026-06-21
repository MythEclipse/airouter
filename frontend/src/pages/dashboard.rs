use leptos::*;
use crate::api::{fetch_dashboard, ProviderStatus, MetricsData, LiveMetrics};
use crate::components::skeleton::SkeletonCards;

#[component]
pub fn Dashboard() -> impl IntoView {
    let providers = create_rw_signal(Vec::<ProviderStatus>::new());
    let metrics = create_rw_signal(MetricsData {
        total_providers: 0,
        total_models: 0,
        built_in_free: true,
    });
    let live = create_rw_signal(LiveMetrics {
        total_requests: 0,
        total_errors: 0,
        avg_latency_ms: 0.0,
        error_rate: 0.0,
        uptime_seconds: 0,
    });
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());

    spawn_local({
        let providers = providers.clone();
        let metrics = metrics.clone();
        let live = live.clone();
        let loading = loading.clone();
        let error = error.clone();
        async move {
            match fetch_dashboard().await {
                Ok(data) => {
                    providers.set(data.providers);
                    metrics.set(data.metrics);
                    live.set(data.live_metrics);
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
        <div class=move || if loading.get() { "animate-fade-in" } else { "animate-fade-in" }>
            <div class="mb-8">
                <h1 class="text-2xl font-bold text-primary">"Dashboard"</h1>
                <p class="text-sm text-secondary mt-1">"System overview"</p>
            </div>

            {move || (!error.get().is_empty()).then(||
                view! { <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger/30">{error.get()}</p> }
            )}

            {move || loading.get().then(|| view! { <SkeletonCards count=5/> })}

            {move || (!loading.get()).then(|| {
                let tp = metrics.with(|m| m.total_providers);
                let tm = metrics.with(|m| m.total_models);
                let tq = live.with(|m| m.total_requests);
                let lat = live.with(|m| format!("{:.1}ms", m.avg_latency_ms));
                let er = live.with(|m| format!("{:.1}%", m.error_rate * 100.0));
                let provs = providers.get();

                view! {
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4 mb-8">
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5">
                            <p class="text-xs text-secondary font-medium mb-1 uppercase tracking-wider">"Providers"</p>
                            <p class="text-2xl font-bold text-primary">{tp.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Registered"</p>
                        </div>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5">
                            <p class="text-xs text-secondary font-medium mb-1 uppercase tracking-wider">"Models"</p>
                            <p class="text-2xl font-bold text-primary">{tm.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Available"</p>
                        </div>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5">
                            <p class="text-xs text-secondary font-medium mb-1 uppercase tracking-wider">"Requests"</p>
                            <p class="text-2xl font-bold text-primary">{tq.to_string()}</p>
                            <p class="text-xs text-muted mt-1">"Total processed"</p>
                        </div>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5">
                            <p class="text-xs text-secondary font-medium mb-1 uppercase tracking-wider">"Latency"</p>
                            <p class="text-2xl font-bold text-primary">{lat}</p>
                            <p class="text-xs text-muted mt-1">"Average"</p>
                        </div>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5">
                            <p class="text-xs text-secondary font-medium mb-1 uppercase tracking-wider">"Error Rate"</p>
                            <p class="text-2xl font-bold text-primary">{er}</p>
                            <p class="text-xs text-muted mt-1">"Across all providers"</p>
                        </div>
                    </div>

                    <h2 class="text-lg font-semibold text-primary mb-4">"Providers"</h2>
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 animate-fade-in-up">
                        {provs
                            .into_iter()
                            .map(|p| {
                                let color = p.color.clone();
                                let health = if p.healthy { "Healthy" } else { "Degraded" };
                                let health_color = if p.healthy {
                                    "text-success"
                                } else {
                                    "text-warning"
                                };
                                view! {
                                    <div class="bg-surface border border-border-subtle border-l-[4px] rounded-[14px] p-5 transition-all duration-200 hover:border-surface hover:-translate-y-0.5 hover:shadow-lg"
                                        style=format!("border-left-color: {}", color)>
                                        <div class="flex items-center gap-2.5 mb-3">
                                            <span
                                                class="w-3 h-3 rounded-full inline-block"
                                                style=format!("background-color: {}", color)
                                            ></span>
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
                            })
                            .collect::<Vec<_>>()}
                    </div>
                }
            })}
        </div>
    }
}
