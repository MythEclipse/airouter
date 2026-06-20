use leptos::*;

#[component]
pub fn Card(
    #[prop(optional, into)] class: &'static str,
    #[prop(optional)] hover: bool,
    children: Children,
) -> impl IntoView {
    let hover_class = if hover {
        "transition-all duration-200 hover:border-surface"
    } else {
        ""
    };

    view! {
        <div class=move || format!(
            "bg-surface border border-border-subtle rounded-[14px] p-6 {} {}",
            class,
            hover_class
        )>
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
        <div class="border-b border-border-subtle pb-4 mb-4">
            <h3 class="text-sm font-semibold text-secondary uppercase tracking-wider mb-3">
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
    let value_class = if mono {
        "text-sm text-primary font-mono"
    } else {
        "text-sm text-primary"
    };

    view! {
        <div class="flex items-center justify-between py-2">
            <span class="text-sm text-secondary">{label}</span>
            <span class=value_class>{value}</span>
        </div>
    }
}

#[component]
pub fn CardGrid(children: Children) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            {children()}
        </div>
    }
}
