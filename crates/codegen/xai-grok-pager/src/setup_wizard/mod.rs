//! Setup wizard startup gating.
//!
//! Task 6 adds the cold-start state and gate. Task 7 will fill in the
//! interactive flow and rendering.

use agent_client_protocol as acp;

/// Minimal placeholder state for the provider setup wizard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupWizardState;

impl SetupWizardState {
    pub fn new() -> Self {
        Self
    }
}

/// Fresh users can start with no advertised auth methods after provider-neutral
/// defaults land. At cold start, that should open provider setup instead of the
/// interactive login flow.
pub fn cold_start_needs_provider_setup(auth_methods: &[acp::AuthMethod], force_login: bool) -> bool {
    auth_methods.is_empty() && !force_login
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auth_method(id: &str, name: &str) -> acp::AuthMethod {
        acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
            acp::AuthMethodId::new(id),
            name.to_string(),
        ))
    }

    #[test]
    fn empty_auth_methods_request_provider_setup_not_login() {
        assert!(cold_start_needs_provider_setup(&[], false));
    }

    #[test]
    fn advertised_auth_methods_skip_provider_setup() {
        let methods = vec![make_auth_method("grok.com", "grok.com")];
        assert!(!cold_start_needs_provider_setup(&methods, false));
    }

    #[test]
    fn force_login_disables_provider_setup_gate() {
        assert!(!cold_start_needs_provider_setup(&[], true));
    }
}
