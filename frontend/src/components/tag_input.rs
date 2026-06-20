use leptos::*;

/// Tag input component — type to add, click to remove
#[component]
pub fn TagInput(
    label: String,
    placeholder: String,
    tags: RwSignal<Vec<String>>,
) -> impl IntoView {
    let input_value = create_rw_signal(String::new());
    let show_suggestions = create_rw_signal(false);

    let add_tag = move |val: String| {
        let trimmed = val.trim().to_string();
        if !trimmed.is_empty() && !tags.with(|t| t.contains(&trimmed)) {
            tags.update(|t| t.push(trimmed));
        }
        input_value.set(String::new());
        show_suggestions.set(false);
    };

    let remove_tag = move |idx: usize| {
        tags.update(|t| { t.remove(idx); });
    };

    view! {
        <div class="mb-4">
            <label class="block text-xs text-secondary mb-1.5 font-medium">
                {label}
            </label>
            <div class="flex flex-wrap gap-1.5 p-2.5 bg-surface-2 border border-surface rounded-lg
                        focus-within:border-accent transition-colors min-h-[42px]">
                {move || tags.get().into_iter().enumerate().map(|(i, t)| {
                    view! {
                        <span class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium
                                    bg-accent-bg text-accent
                                    border border-accent/30 rounded-full
                                    animate-fade-in">
                            {t}
                            <button type="button" on:click=move|_|remove_tag(i)
                                class="hover:text-danger transition-colors leading-none text-sm">
                                "×"
                            </button>
                        </span>
                    }
                }).collect::<Vec<_>>()}
                <div class="relative flex-1 min-w-[80px]">
                    <input type="text"
                        prop:value=input_value.get()
                        on:input=move|ev| {
                            let v = event_target_value(&ev);
                            input_value.set(v);
                        }
                        on:keydown=move|ev: web_sys::KeyboardEvent| {
                            let k = ev.key();
                            let val = input_value.get();
                            let trimmed = val.trim().to_string();
                            if (k == "Enter" || k == ",") && !trimmed.is_empty() {
                                ev.prevent_default();
                                add_tag(trimmed);
                            }
                            if k == "Backspace" && val.is_empty() {
                                tags.update(|t| { t.pop(); });
                            }
                        }
                        on:blur=move|_| {
                            let v = input_value.get();
                            if !v.is_empty() { add_tag(v); }
                        }
                        placeholder=placeholder
                        class="bg-transparent border-none outline-none text-sm text-primary
                               w-full placeholder-muted"
                    />
                </div>
            </div>
        </div>
    }
}
