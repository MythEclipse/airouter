use leptos::*;

/// Button component with variant, size, disabled, and loading states.
#[component]
pub fn Button(
    /// "primary" | "secondary" | "danger" | "ghost"
    #[prop(optional, default = "primary".to_string())]
    variant: String,
    /// "sm" | "md" | "lg"
    #[prop(optional, default = "md".to_string())]
    size: String,
    #[prop(optional, default = MaybeSignal::Static(false))]
    disabled: MaybeSignal<bool>,
    #[prop(optional, default = MaybeSignal::Static(false))]
    loading: MaybeSignal<bool>,
    #[prop(optional)]
    on_click: Option<Box<dyn FnMut(web_sys::MouseEvent)>>,
    children: Children,
) -> impl IntoView {
    let size_class = match size.as_str() {
        "sm" => "px-2.5 py-1.5 text-xs",
        "lg" => "px-6 py-2.5 text-sm",
        _ => "px-4 py-2 text-sm",
    };

    let variant_class = match variant.as_str() {
        "secondary" => "bg-surface-2 text-primary hover:bg-surface-3 border border-surface",
        "danger" => "bg-danger hover:bg-red-600 text-white",
        "ghost" => "text-secondary hover:text-primary hover:bg-surface-2",
        _ => "bg-accent hover:bg-accent-hover text-white",
    };

    let base_classes = "rounded-lg font-medium transition-all duration-150 active:scale-[0.97]";
    let disabled_classes = "disabled:opacity-50 disabled:pointer-events-none";

    let class = format!("{base_classes} {disabled_classes} {size_class} {variant_class}");

    // Children is FnOnce — call once at init, clone the resulting Fragment for both branches
    let content = children();

    // Wrap optional FnMut in RefCell so it can be called from an Fn closure
    let on_click = on_click.map(std::cell::RefCell::new);

    view! {
        <button
            class=class
            disabled=disabled
            on:click=move |ev| {
                if let Some(ref cb) = on_click {
                    cb.borrow_mut()(ev);
                }
            }
        >
            {move || {
                if loading.get() {
                    view! {
                        <span class="inline-flex items-center gap-2">
                            <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
                                <circle
                                    class="opacity-25"
                                    cx="12"
                                    cy="12"
                                    r="10"
                                    stroke="currentColor"
                                    stroke-width="4"
                                />
                                <path
                                    class="opacity-75"
                                    fill="currentColor"
                                    d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                                />
                            </svg>
                            {content.clone()}
                        </span>
                    }
                } else {
                    view! {
                        <span>{content.clone()}</span>
                    }
                }
            }}
        </button>
    }
}
