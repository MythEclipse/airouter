use leptos::*;

/// Badge variant that determines colors for the badge background, text, and optional dot.
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
    fn text_class(&self) -> &'static str {
        match self {
            BadgeVariant::Default => "text-muted",
            BadgeVariant::Primary => "text-accent",
            BadgeVariant::Success => "text-success",
            BadgeVariant::Warning => "text-warning",
            BadgeVariant::Error => "text-danger",
            BadgeVariant::Info => "text-[#60a5fa]",
        }
    }

    fn bg_class(&self) -> &'static str {
        match self {
            BadgeVariant::Default => "bg-surface-2",
            BadgeVariant::Primary => "bg-accent-bg",
            BadgeVariant::Success => "bg-success-bg",
            BadgeVariant::Warning => "bg-[rgba(251,191,36,0.15)]",
            BadgeVariant::Error => "bg-danger-bg",
            BadgeVariant::Info => "bg-[rgba(96,165,250,0.15)]",
        }
    }

    fn dot_class(&self) -> &'static str {
        self.text_class()
    }
}

impl std::fmt::Display for BadgeVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BadgeVariant::Default => write!(f, "default"),
            BadgeVariant::Primary => write!(f, "primary"),
            BadgeVariant::Success => write!(f, "success"),
            BadgeVariant::Warning => write!(f, "warning"),
            BadgeVariant::Error => write!(f, "error"),
            BadgeVariant::Info => write!(f, "info"),
        }
    }
}

impl std::str::FromStr for BadgeVariant {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(BadgeVariant::Default),
            "primary" => Ok(BadgeVariant::Primary),
            "success" => Ok(BadgeVariant::Success),
            "warning" => Ok(BadgeVariant::Warning),
            "error" => Ok(BadgeVariant::Error),
            "info" => Ok(BadgeVariant::Info),
            _ => Err(()),
        }
    }
}

#[component]
pub fn Badge(
    #[prop(optional)] variant: BadgeVariant,
    #[prop(optional)] dot: bool,
    children: Children,
) -> impl IntoView {
    let base = "inline-flex items-center gap-1.5 px-2 py-0.5 text-xs font-medium rounded-full";
    let classes = format!("{} {} {}", base, variant.bg_class(), variant.text_class());

    let dot_classes = format!("w-1.5 h-1.5 rounded-full {}", variant.dot_class());

    view! {
        <span class=classes>
            {dot.then(|| view! {
                <span class=dot_classes></span>
            })}
            {children()}
        </span>
    }
}
