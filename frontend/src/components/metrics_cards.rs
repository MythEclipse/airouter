use leptos::*;

#[derive(Debug, Clone)]
pub struct MetricCard {
    pub title: String,
    pub value: String,
    pub subtitle: String,
}

#[component]
pub fn MetricsCards(cards: Vec<MetricCard>) -> impl IntoView {
    view! {
        <div class="metrics-grid">
            {cards.into_iter().map(|card| {
                view! {
                    <div class="metric-card">
                        <h3>{card.title}</h3>
                        <div class="metric-value">{card.value}</div>
                        <div class="metric-subtitle">{card.subtitle}</div>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}
