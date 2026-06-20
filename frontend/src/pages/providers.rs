use leptos::*;
use crate::api::fetch_dashboard;

#[component]
pub fn Providers() -> impl IntoView {
    let data = create_resource(|| (), |_| async move {
        fetch_dashboard("sk-test-abc123").await
    });

    view! {
        <div class="page">
            <h1>"Providers"</h1>
            <p>"Configured LLM providers."</p>

            <Suspense fallback=|| view! { <div class="loading">"Loading providers..."</div> }>
            {move || data.get().map(|d| {
                let providers = match d {
                    Ok(ref dd) => dd.providers.clone(),
                    Err(ref e) => return view! { <div class="error">"Error: " {e}</div> }.into_any(),
                };
                view! {
                    <div class="provider-grid">
                        {providers.into_iter().map(|p| {
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
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            })}
            </Suspense>
        </div>
    }
}
