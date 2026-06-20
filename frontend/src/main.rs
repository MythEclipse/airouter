use leptos::*;
use airouter_frontend::app::App;

fn main() {
    mount_to_body(|| view! { <App/> });
}
