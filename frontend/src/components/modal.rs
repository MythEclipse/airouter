use leptos::*;

/// Reusable modal overlay with backdrop, optional title, size variants, and
/// children for body content.
///
/// Visibility is toggled via CSS classes (`hidden` / visible) so that the
/// `Children` closure (FnOnce) is only called during initial render. Clicking
/// the backdrop closes the modal.
#[component]
pub fn Modal(
    /// Reactive signal controlling visibility.
    show: RwSignal<bool>,
    /// Optional modal title. When provided, a header row with a close button
    /// is rendered above the body.
    #[prop(optional)]
    title: String,
    /// Size variant: "sm", "md", or "lg".  Defaults to "md".
    #[prop(default = "md")]
    size: &'static str,
    /// Body content rendered inside the modal panel.
    children: Children,
) -> impl IntoView {
    let max_w = match size {
        "sm" => "max-w-sm",
        "lg" => "max-w-2xl",
        _ => "max-w-lg",
    };

    view! {
        // ── Backdrop ──────────────────────────────────────────────
        <div
            class=move || {
                if show.get() {
                    "fixed inset-0 bg-black/50 backdrop-blur-[2px] z-50 \
                     animate-fade-in \
                     flex items-center justify-center \
                     max-sm:items-start max-sm:pt-[10vh]"
                } else {
                    "hidden"
                }
            }
            on:click=move |_| show.set(false)
        >
            // ── Panel ────────────────────────────────────────────
            <div
                class=format!(
                    "bg-surface border border-border-subtle \
                     rounded-[14px] shadow-[var(--shadow-elev)] \
                     w-full mx-4 max-h-[80vh] overflow-y-auto \
                     animate-scale-in {}",
                    max_w,
                )
                on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
            >
                // ── Header (optional) ────────────────────────────
                {(!title.is_empty()).then(|| {
                    view! {
                        <div class="flex items-center justify-between px-6 py-4 border-b border-border-subtle">
                            <h2 class="text-lg font-semibold text-primary">{title.clone()}</h2>
                            <button
                                on:click=move |_| show.set(false)
                                class="text-muted hover:text-primary transition-colors"
                            >
                                // SVG X icon
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        stroke-width="2"
                                        d="M6 18L18 6M6 6l12 12"
                                    />
                                </svg>
                            </button>
                        </div>
                    }
                })}

                // ── Body ─────────────────────────────────────────
                <div class="p-6">
                    {children()}
                </div>
            </div>
        </div>
    }
}
