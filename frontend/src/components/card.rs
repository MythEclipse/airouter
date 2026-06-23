use leptos::*;

#[component]
pub fn Card(
    #[prop(optional, into)] class: &'static str,
    #[prop(optional)] hover: bool,
    children: Children,
) -> impl IntoView {
    let base = "card-base p-5";
    let hover_class = if hover { "hover:border-surface" } else { "" };

    view! {
        <div class=move || format!("{} {} {}", base, class, hover_class)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardSection(
    title: String,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="mb-5">
            <h3 class="text-xs font-semibold text-muted uppercase tracking-widest mb-3 pb-2.5 border-b border-border-subtle">
                {title}
            </h3>
            {children()}
        </div>
    }
}

#[component]
pub fn CardRow(
    label: String,
    value: String,
    #[prop(optional)] mono: bool,
) -> impl IntoView {
    let val_cls = if mono {
        "text-sm font-mono text-primary truncate max-w-[200px] text-right"
    } else {
        "text-sm text-primary text-right truncate max-w-[200px]"
    };
    view! {
        <div class="flex items-center justify-between gap-4 py-1.5">
            <span class="text-xs text-secondary shrink-0">{label}</span>
            <span class=val_cls>{value}</span>
        </div>
    }
}

#[component]
pub fn CardGrid(children: Children) -> impl IntoView {
    view! { <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">{children()}</div> }
}
