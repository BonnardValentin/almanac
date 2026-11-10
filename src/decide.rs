//! Every threshold lives here, as pure functions over the model's answers.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Allow,
    Ask,
    Deny,
}

impl Gate {
    pub fn as_str(self) -> &'static str {
        match self {
            Gate::Allow => "allow",
            Gate::Ask => "ask",
            Gate::Deny => "deny",
        }
    }
}

pub const AUTO_ALLOW: f64 = 0.95;
pub const AUTO_DENY: f64 = 0.8;

/// `external` and `other` always go to the human, whatever the confidence.
pub fn gate(class: &str, p: f64) -> Gate {
    match class {
        "read_only" | "local_edit" if p >= AUTO_ALLOW => Gate::Allow,
        "destructive" if p >= AUTO_DENY => Gate::Deny,
        _ => Gate::Ask,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Allow,
    /// Stop, but tell the user the model was not sure. An unsure verdict that interrupts is
    /// worse than one that flags.
    AllowUnsure,
    Block,
}

/// `p` is the probability that memory already holds everything this session learned.
pub fn stop(p: f64) -> Stop {
    if p > 0.9 {
        Stop::Allow
    } else if p < 0.5 {
        Stop::Block
    } else {
        Stop::AllowUnsure
    }
}

pub fn needs_work(p: f64) -> bool {
    p >= 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_bands() {
        assert_eq!(gate("read_only", 0.99), Gate::Allow);
        assert_eq!(gate("read_only", 0.9), Gate::Ask);
        assert_eq!(gate("local_edit", 0.95), Gate::Allow);
        assert_eq!(gate("destructive", 0.85), Gate::Deny);
        assert_eq!(gate("destructive", 0.6), Gate::Ask);
        assert_eq!(gate("external", 1.0), Gate::Ask);
        assert_eq!(gate("other", 1.0), Gate::Ask);
    }

    #[test]
    fn stop_bands() {
        assert_eq!(stop(0.95), Stop::Allow);
        assert_eq!(stop(0.7), Stop::AllowUnsure);
        assert_eq!(stop(0.2), Stop::Block);
    }
}
