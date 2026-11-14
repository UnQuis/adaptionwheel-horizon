//! AdaptionWheel native mod scaffold (Horizon).
//! PR-0: placeholder hooks only. Real adaptation logic/state/sync will be added incrementally.

use horizon_client::mod_api::{Hooks, Host, PlayerLoggedInEvent, Text};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Mod)
}

struct Mod;

impl Hooks for Mod {
    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        let line = format!(
            "[AdaptionWheel port scaffold] hello, {}! Native hooks not implemented yet.",
            event.name
        );
        host.tell(event.player, Text::literal(line));
    }
}
