use leptos::*;

#[component]
pub fn SkeletonTable(rows: u32) -> impl IntoView {
    view! {
        <div class="animate-fade-in overflow-hidden rounded-xl border border-border-subtle">
            <div class="h-11 bg-surface-2 skeleton"></div>
            { (0..rows).map(|_| {
                view! {
                    <div class="h-14 bg-surface skeleton border-t border-border-subtle/50"></div>
                }
            }).collect::<Vec<_>>() }
        </div>
    }
}

#[component]
pub fn SkeletonCards(count: u32) -> impl IntoView {
    view! {
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
            { (0..count).map(|_| {
                view! {
                    <div class="card-base p-5 space-y-3">
                        <div class="h-3 w-20 rounded bg-surface-2/50 skeleton"></div>
                        <div class="h-7 w-16 rounded bg-surface-2/50 skeleton"></div>
                        <div class="h-3 w-24 rounded bg-surface-2/50 skeleton"></div>
                    </div>
                }
            }).collect::<Vec<_>>() }
        </div>
    }
}
