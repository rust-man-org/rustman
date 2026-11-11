//! `rustman-engine` as a [`ScriptHost`], for the app and the CLI alike.

use rustman_core::script::{Effect, ScriptError, ScriptHost, ScriptInput};
use rustman_engine::{HostInput, ResponseInput};

/// Runs scripts in Rustman's scripting language.
#[derive(Debug, Clone, Copy, Default)]
pub struct Engine;

impl ScriptHost for Engine {
    fn run(&self, source: &str, input: ScriptInput) -> Result<Vec<Effect>, ScriptError> {
        let input = HostInput {
            env_vars: input.env,
            headers: input.headers,
            cookies: input.cookies,
            body: input.body,
            url: input.url,
            response: input.response.map(|r| ResponseInput {
                status: r.status,
                body: r.body,
            }),
        };
        let outcome =
            rustman_engine::run(source, input).map_err(|e| ScriptError::new(e.to_string()))?;
        Ok(outcome.effects.into_iter().map(effect).collect())
    }
}

fn effect(effect: rustman_engine::Effect) -> Effect {
    match effect {
        rustman_engine::Effect::SetEnv(name, value) => Effect::SetEnv { name, value },
        rustman_engine::Effect::SetHeader(name, value) => Effect::SetHeader { name, value },
        rustman_engine::Effect::SetBody(body) => Effect::SetBody(body),
        rustman_engine::Effect::Test { name, passed } => Effect::Test { name, passed },
        rustman_engine::Effect::Log(text) => Effect::Log(text),
    }
}

#[cfg(test)]
mod tests {
    use rustman_core::script::ScriptResponse;

    use super::*;

    #[test]
    fn effects_come_back_in_order() {
        let effects = Engine
            .run(
                "set_env(\"a\", \"1\")\nset_header(\"X-A\", \"2\")\nset_body(\"b\")\ntest(\"t\", true)\nprint(\"hi\")",
                ScriptInput::default(),
            )
            .unwrap();
        assert_eq!(
            effects,
            [
                Effect::SetEnv {
                    name: "a".into(),
                    value: "1".into()
                },
                Effect::SetHeader {
                    name: "X-A".into(),
                    value: "2".into()
                },
                Effect::SetBody("b".into()),
                Effect::Test {
                    name: "t".into(),
                    passed: true
                },
                Effect::Log("hi".into()),
            ]
        );
    }

    #[test]
    fn scripts_read_their_input() {
        let input = ScriptInput {
            env: vec![("token".into(), "t0k".into())],
            response: Some(ScriptResponse {
                status: 201,
                body: "{\"id\": 7}".into(),
            }),
            ..ScriptInput::default()
        };
        let effects = Engine
            .run(
                "test(\"created\", response.status == 201)\ntest(\"token\", env(\"token\") == \"t0k\")",
                input,
            )
            .unwrap();
        assert!(
            effects
                .iter()
                .all(|e| matches!(e, Effect::Test { passed: true, .. })),
            "{effects:?}"
        );
    }

    #[test]
    fn a_broken_script_is_an_error_worded_for_the_user() {
        let err = Engine.run("set_env(", ScriptInput::default()).unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}
