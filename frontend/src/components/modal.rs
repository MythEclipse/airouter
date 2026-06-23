use leptos::*;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

/// Reusable modal overlay with backdrop, escape-to-close, optional title, and size variants.
#[component]
pub fn Modal(
    show: RwSignal<bool>,
    #[prop(optional)]
    title: String,
    #[prop(default = "md")]
    size: &'static str,
    children: Children,
) -> impl IntoView {
    let max_w = match size {
        "sm" => "max-w-sm",
        "lg" => "max-w-2xl",
        _ => "max-w-lg",
    };

    // Escape key handler
    Effect::new(move |_| {
        if show.get() {
            let window = web_sys::window().unwrap();
            let handler = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" {
                    show.set(false);
                }
            });
            window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref()).ok();
            handler.forget();
        }
    });

    view! {
        <div
            class=move || {
                if show.get() {
                    "fixed inset-0 bg-black/50 backdrop-blur-sm z-50 \
                     flex items-center justify-center \
                     max-sm:items-start max-sm:pt-[10vh] \
                     animate-fade-in"
                } else {
                    "hidden"
                }
            }
            on:click=move |_| show.set(false)
        >
            <div class=format!(
                "bg-surface border border-border-subtle \
                 rounded-xl shadow-[var(--shadow-elev)] \
                 w-full mx-4 max-h-[80vh] overflow-y-auto \
                 animate-scale-in {}",
                max_w,
            )
                on:click=move |ev: web_sys::MouseEvent| ev.stop_propagation()
            >
                {(!title.is_empty()).then(|| {
                    view! {
                        <div class="flex items-center justify-between px-6 py-4 border-b border-border-subtle">
                            <h2 class="text-base font-semibold text-primary font-display tracking-tight">{title.clone()}</h2>
                            <button
                                on:click=move |_| show.set(false)
                                class="text-muted hover:text-primary transition-colors rounded-lg p-1 hover:bg-surface-2"
                            >
                                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                                </svg>
                            </button>
                        </div>
                    }
                })}
                <div class="p-6">
                    {children()}
                </div>
            </div>
        </div>
    }
}
