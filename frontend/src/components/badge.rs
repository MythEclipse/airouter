use leptos::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Primary,
    Success,
    Warning,
    Error,
    Info,
}

impl BadgeVariant {
    fn classes(&self) -> &'static str {
        match self {
            BadgeVariant::Default => "text-muted bg-surface-2",
            BadgeVariant::Primary => "text-accent bg-accent-bg",
            BadgeVariant::Success => "text-success bg-success-bg",
            BadgeVariant::Warning => "text-warning bg-warning-bg",
            BadgeVariant::Error => "text-danger bg-danger-bg",
            BadgeVariant::Info => "text-info bg-info-bg",
        }
    }

    fn dot_class(&self) -> &'static str {
        match self {
            BadgeVariant::Default => "bg-muted",
            BadgeVariant::Primary => "bg-accent",
            BadgeVariant::Success => "bg-success",
            BadgeVariant::Warning => "bg-warning",
            BadgeVariant::Error => "bg-danger",
            BadgeVariant::Info => "bg-info",
        }
    }
}

impl std::fmt::Display for BadgeVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            BadgeVariant::Default => "default",
            BadgeVariant::Primary => "primary",
            BadgeVariant::Success => "success",
            BadgeVariant::Warning => "warning",
            BadgeVariant::Error => "error",
            BadgeVariant::Info => "info",
        })
    }
}

impl std::str::FromStr for BadgeVariant {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "primary" => BadgeVariant::Primary,
            "success" => BadgeVariant::Success,
            "warning" => BadgeVariant::Warning,
            "error" => BadgeVariant::Error,
            "info" => BadgeVariant::Info,
            _ => BadgeVariant::Default,
        })
    }
}

#[component]
pub fn Badge(
    #[prop(optional)] variant: BadgeVariant,
    #[prop(optional)] dot: bool,
    children: Children,
) -> impl IntoView {
    let base = "inline-flex items-center gap-1.5 px-2 py-0.5 text-xs font-medium rounded-full";
    let classes = format!("{} {}", base, variant.classes());

    view! {
        <span class=classes>
            {dot.then(|| view! {
                <span class=format!("w-1.5 h-1.5 rounded-full {}", variant.dot_class())></span>
            })}
            {children()}
        </span>
    }
}
