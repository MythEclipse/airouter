use leptos::*;
use std::cell::RefCell;

/// Button component with variant, size, disabled, and loading states.
#[component]
pub fn Button(
    #[prop(optional, default = "primary".to_string())]
    variant: String,
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
        "sm" => "px-2.5 py-1.5 text-xs rounded-lg",
        "lg" => "px-6 py-2.5 text-sm rounded-xl",
        _ => "px-4 py-2 text-sm rounded-lg",
    };

    let variant_class = match variant.as_str() {
        "secondary" => "bg-surface-2 text-primary border border-surface hover:bg-surface-3 hover:text-primary",
        "danger" => "bg-danger/10 text-danger border border-danger/20 hover:bg-danger/20",
        "danger-solid" => "bg-danger hover:bg-[#d43a4a] text-white",
        "ghost" => "text-secondary hover:text-primary hover:bg-surface-2",
        _ => "bg-accent hover:bg-accent-hover text-white",
    };

    let base = "btn-base";
    let class = format!("{base} {size_class} {variant_class}");

    let content = children();
    let on_click = on_click.map(RefCell::new);

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
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
                            </svg>
                            {content.clone()}
                        </span>
                    }
                } else {
                    view! { <span>{content.clone()}</span> }
                }
            }}
        </button>
    }
}
