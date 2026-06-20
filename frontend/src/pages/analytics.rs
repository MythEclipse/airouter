use leptos::*;
use crate::api::fetch_dashboard;

#[component]
pub fn Analytics() -> impl IntoView {
    let data = create_resource(|| (), |_| async move {
        fetch_dashboard("sk-test-abc123").await
    });

    view! {
        <div class="page">
            <h1>"Analytics"</h1>
            <p>"Gateway usage overview."</p>

            <Suspense fallback=|| view! { <div class="loading">"Loading..."</div> }>
            {move || data.get().map(|d| {
                let metrics = match d {
                    Ok(ref dd) => dd.metrics.clone(),
                    Err(ref e) => return view! { <div class="error">"Error: " {e}</div> }.into_any(),
                };
                let free_models: usize = 14;
                let pct = if metrics.total_models > 0 {
                    format!("{:.0}%", free_models as f64 / metrics.total_models as f64 * 100.0)
                } else { "-".into() };
                view! {
                    <div class="metrics-grid">
                        <div class="metric-card">
                            <h3>"Total Providers"</h3>
                            <div class="metric-value">{metrics.total_providers}</div>
                            <div class="metric-subtitle">"Configured"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Total Models"</h3>
                            <div class="metric-value">{metrics.total_models}</div>
                            <div class="metric-subtitle">"Available"</div>
                        </div>
                        <div class="metric-card">
                            <h3>"Free Models"</h3>
                            <div class="metric-value">{pct}</div>
                            <div class="metric-subtitle">"Of all models"</div>
                        </div>
                    </div>
                }.into_any()
            })}
            </Suspense>
        </div>
    }
}
