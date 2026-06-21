use leptos::*;

/// Map provider_type to the actual icon PNG path
fn icon_path(provider_type: &str) -> String {
    match provider_type {
        "cloudflare" => "/providers/cloudflare-ai.png".to_string(),
        "mimo_free" => "/providers/mimo-free.png".to_string(),
        "opencode_free" => "/providers/opencode.png".to_string(),
        "azure_openai" | "openai_compat" => "/providers/openai.png".to_string(),
        _ => format!("/providers/{}.png", provider_type),
    }
}

/// Category badge colors (matching backend)
pub fn category_style(cat: &str) -> (&'static str, &'static str) {
    match cat {
        "free" => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-[rgba(34,197,94,0.1)] text-success border-success/30",
            "Free",
        ),
        "free-tier" => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-[rgba(154,107,219,0.1)] text-[#9a6bdb] border-[#9a6bdb]/30",
            "Free Tier",
        ),
        "api-key" => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-[rgba(88,166,255,0.1)] text-[#58a6ff] border-[rgba(88,166,255,0.3)]",
            "API Key",
        ),
        "oauth" => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-[rgba(219,107,40,0.1)] text-[#db6b28] border-[rgba(219,107,40,0.3)]",
            "OAuth",
        ),
        "web-cookie" => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-[rgba(219,107,154,0.1)] text-[#db6b9a] border-[rgba(219,107,154,0.3)]",
            "Web Cookie",
        ),
        _ => (
            "inline-flex px-2 py-0.5 text-xs font-medium rounded-lg \
             border bg-gray-500/10 text-gray-400 border-gray-500/30",
            "Unknown",
        ),
    }
}

/// Category section color for the left accent bar
pub fn category_accent(cat: &str) -> &'static str {
    match cat {
        "free" => "#22C55e",
        "free-tier" => "#9a6bdb",
        "api-key" => "#58a6ff",
        "oauth" => "#db6b28",
        "web-cookie" => "#db6b9a",
        _ => "#6b7280",
    }
}

/// ProviderIcon — renders PNG image with colored-initial fallback.
/// Fallback only shows while image loads or on error — no more overlap.
#[component]
pub fn ProviderIcon(
    provider_type: String,
    name: String,
    #[prop(optional)] color: Option<String>,
    #[prop(optional)] size: Option<i32>,
) -> impl IntoView {
    let sz = size.unwrap_or(36);
    let bg = color.unwrap_or_else(|| "#60a5fa".to_string());
    let fallback_text = name.chars().next().map(|c| c.to_string()).unwrap_or_default();
    let src = icon_path(&provider_type);

    let img_loaded = create_rw_signal(false);
    let img_errored = create_rw_signal(false);

    view! {
        <div
            class="relative shrink-0 rounded-lg overflow-hidden"
            style=format!("width: {}px; height: {}px", sz, sz)
        >
            // Fallback layer — visible only while image hasn't loaded yet
            {move || (!img_loaded.get()).then(|| view! {
                <div
                    class="absolute inset-0 w-full h-full flex items-center justify-center text-sm font-bold rounded-lg"
                    style=format!("background-color: {}15; color: {}", bg, bg)
                >
                    {fallback_text.clone()}
                </div>
            })}

            // Image layer — shows when loaded, hides on error
            <img
                src=src
                alt=name.clone()
                class="absolute inset-0 w-full h-full object-cover rounded-lg"
                style=move || if img_errored.get() { "display: none" } else { "display: block" }
                on:load=move |_| { img_loaded.set(true); img_errored.set(false); }
                on:error=move |_| { img_loaded.set(false); img_errored.set(true); }
            />
        </div>
    }
}
