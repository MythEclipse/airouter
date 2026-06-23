use leptos::*;

fn icon_path(provider_type: &str) -> String {
    match provider_type {
        "cloudflare" => "/providers/cloudflare-ai.png".to_string(),
        "mimo_free" => "/providers/mimo-free.png".to_string(),
        "opencode_free" => "/providers/opencode.png".to_string(),
        "azure_openai" | "openai_compat" => "/providers/openai.png".to_string(),
        _ => format!("/providers/{}.png", provider_type),
    }
}

/// Returns (css_class, label) for category badge
pub fn category_style(cat: &str) -> (&'static str, &'static str) {
    match cat {
        "free" => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-success-bg text-success border-success/30",
            "Free",
        ),
        "free-tier" => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-info-bg text-info border-info/30",
            "Free Tier",
        ),
        "api-key" => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-accent-bg text-accent border-accent/30",
            "API Key",
        ),
        "oauth" => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-warning-bg text-warning border-warning/30",
            "OAuth",
        ),
        "web-cookie" => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-[rgba(219,107,154,0.12)] text-[#db6b9a] border-[rgba(219,107,154,0.3)]",
            "Web Cookie",
        ),
        _ => (
            "inline-flex px-2 py-0.5 text-[11px] font-medium rounded-md \
             border bg-surface-2 text-muted border-border",
            "Unknown",
        ),
    }
}

/// Category accent color for section dividers
pub fn category_accent(cat: &str) -> &'static str {
    match cat {
        "free" => "#2dd4bf",
        "free-tier" => "#60a5fa",
        "api-key" => "#d4875a",
        "oauth" => "#f0b429",
        "web-cookie" => "#db6b9a",
        _ => "#6b6763",
    }
}

/// ProviderIcon — renders PNG with colored-initial fallback while loading
#[component]
pub fn ProviderIcon(
    provider_type: String,
    name: String,
    #[prop(optional)] color: Option<String>,
    #[prop(optional)] size: Option<i32>,
) -> impl IntoView {
    let sz = size.unwrap_or(36);
    let bg = color.unwrap_or_else(|| "#60a5fa".to_string());
    let initial = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
    let src = icon_path(&provider_type);

    let img_loaded = create_rw_signal(false);
    let img_errored = create_rw_signal(false);

    view! {
        <div
            class="relative shrink-0 rounded-lg overflow-hidden ring-1 ring-border/50"
            style=format!("width: {}px; height: {}px", sz, sz)
        >
            {move || (!img_loaded.get()).then(|| view! {
                <div
                    class="absolute inset-0 flex items-center justify-center text-sm font-bold"
                    style=format!("background-color: {}18; color: {}", bg, bg)
                >
                    {initial.clone()}
                </div>
            })}
            <img
                src=src
                alt=name.clone()
                class="absolute inset-0 w-full h-full object-cover"
                style=move || if img_errored.get() { "display:none" } else { "display:block" }
                on:load=move |_| { img_loaded.set(true); img_errored.set(false); }
                on:error=move |_| { img_loaded.set(false); img_errored.set(true); }
            />
        </div>
    }
}
