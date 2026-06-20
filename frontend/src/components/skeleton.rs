use leptos::*;

#[component]
pub fn SkeletonTable(rows: u32) -> impl IntoView {
    view! {
        <div class="animate-fade-in">
            <div class="h-10 bg-surface-hover skeleton rounded-t-lg mb-0.5"></div>
            { (0..rows).map(|_| {
                view! {
                    <div class="h-14 bg-surface-alt skeleton border-b border-surface/50"></div>
                }
            }).collect::<Vec<_>>() }
            <div class="h-10 bg-surface-hover skeleton rounded-b-lg"></div>
        </div>
    }
}

#[component]
pub fn SkeletonCards(count: u32) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            { (0..count).map(|_| {
                view! {
                    <div class="bg-surface-alt border border-surface rounded-xl p-5 skeleton">
                        <div class="h-3 w-20 mb-3 rounded bg-surface-hover/50"></div>
                        <div class="h-8 w-16 mb-2 rounded bg-surface-hover/50"></div>
                        <div class="h-3 w-24 rounded bg-surface-hover/50"></div>
                    </div>
                }
            }).collect::<Vec<_>>() }
        </div>
    }
}
