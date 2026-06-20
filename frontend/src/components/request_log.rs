use leptos::*;

#[derive(Debug, Clone)]
pub struct RequestEntry {
    pub id: String,
    pub method: String,
    pub path: String,
    pub provider: String,
    pub status: u16,
    pub latency: u64,
}

#[component]
pub fn RequestLog(entries: Vec<RequestEntry>) -> impl IntoView {
    view! {
        <div class="request-log">
            <h2>"Request Log"</h2>
            <div class="log-table">
                <div class="log-header">
                    <span>"Method"</span>
                    <span>"Path"</span>
                    <span>"Provider"</span>
                    <span>"Status"</span>
                    <span>"Latency"</span>
                </div>
                {entries.into_iter().map(|e| {
                    view! {
                        <div class="log-row">
                            <span>{e.method}</span>
                            <span>{e.path}</span>
                            <span>{e.provider}</span>
                            <span>{e.status.to_string()}</span>
                            <span>{format!("{}ms", e.latency)}</span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
