//! `/provider` -- open the provider setup wizard.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct ProviderCommand;

impl SlashCommand for ProviderCommand {
    fn name(&self) -> &str {
        "provider"
    }

    fn description(&self) -> &str {
        "Configure an LLM provider interactively"
    }

    fn usage(&self) -> &str {
        "/provider"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenSetupWizard)
    }
}
