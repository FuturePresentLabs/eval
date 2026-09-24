//! Shared identity for the two independent models in an eval configuration.

use clap::Args;
use serde::{Deserialize, Serialize};

/// The two model roles a design eval may exercise.
///
/// `llm_model` is the open-ended generative/agent model. `rlcd_model` is the
/// bounded System-One decision model. A backend may consume one or both, but
/// must preserve both as provenance rather than collapsing them into one slug.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Args)]
pub struct ModelSelection {
    /// Outer agent/open-ended generative model slug.
    #[arg(long, env = "EVAL_LLM_MODEL")]
    #[serde(default)]
    pub llm_model: Option<String>,

    /// Bounded RLCD/System-One decision model slug.
    #[arg(long, env = "EVAL_RLCD_MODEL", visible_alias = "model")]
    #[serde(default)]
    pub rlcd_model: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Debug, Parser)]
    struct Cli {
        #[command(flatten)]
        models: ModelSelection,
    }

    #[test]
    fn parses_llm_and_rlcd_as_independent_slugs() {
        let cli = Cli::parse_from([
            "test",
            "--llm-model",
            "provider/generative",
            "--rlcd-model",
            "provider/decision",
        ]);
        assert_eq!(cli.models.llm_model.as_deref(), Some("provider/generative"));
        assert_eq!(cli.models.rlcd_model.as_deref(), Some("provider/decision"));
    }
}
