pub mod acknowledge;
pub mod compare;
pub mod expr;
pub mod flashcard;
pub mod interactive;
pub mod math;
pub mod plot;
pub mod quiz;
pub mod resource_link;
pub mod slider;
pub mod stepper;
pub mod template;

use std::collections::HashMap;
use leptos::prelude::*;

#[derive(Debug, Clone)]
pub enum WidgetError {
    MissingConfig,
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct WidgetInstance {
    pub kind: String,
    pub id: String,
    pub config: Option<serde_json::Value>,
}

impl WidgetInstance {
    pub fn cfg(&self) -> Result<&serde_json::Value, WidgetError> {
        self.config
            .as_ref()
            .ok_or_else(|| {
                WidgetError::Invalid(format!(
                    "no [widgets.\"{}\"] entry in this course's config.toml",
                    self.id
                ))
            })
            .and_then(|v| {
                if v.is_null() {
                    Err(WidgetError::MissingConfig)
                } else {
                    Ok(v)
                }
            })
    }

    pub fn req_str(&self, key: &str) -> Result<String, WidgetError> {
        match self.cfg()?.get(key) {
            Some(serde_json::Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(self.type_error(key, "string")),
            None => Err(self.missing(key)),
        }
    }

    pub fn opt_str(&self, key: &str) -> Result<Option<String>, WidgetError> {
        match self.cfg()?.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(self.type_error(key, "string")),
        }
    }

    pub fn req_f64(&self, key: &str) -> Result<f64, WidgetError> {
        match self.cfg()?.get(key) {
            Some(v @ (serde_json::Value::Number(_) | serde_json::Value::String(_))) => {
                serde_json::from_value::<f64>(v.clone())
                    .map_err(|_| self.type_error(key, "number"))
            }
            Some(_) => Err(self.type_error(key, "number")),
            None => Err(self.missing(key)),
        }
    }

    pub fn opt_f64(&self, key: &str) -> Result<Option<f64>, WidgetError> {
        match self.cfg()?.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(v @ (serde_json::Value::Number(_) | serde_json::Value::String(_))) => {
                serde_json::from_value::<f64>(v.clone())
                    .map(Some)
                    .map_err(|_| self.type_error(key, "number"))
            }
            Some(_) => Err(self.type_error(key, "number")),
        }
    }

    pub fn opt_usize(&self, key: &str) -> Result<Option<usize>, WidgetError> {
        match self.cfg()?.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Number(n)) => n
                .as_u64()
                .map(|v| Some(v as usize))
                .ok_or_else(|| self.type_error(key, "non-negative integer")),
            Some(_) => Err(self.type_error(key, "non-negative integer")),
        }
    }

    pub fn req_arr(&self, key: &str) -> Result<Vec<serde_json::Value>, WidgetError> {
        match self.cfg()?.get(key) {
            Some(serde_json::Value::Array(a)) => Ok(a.clone()),
            Some(_) => Err(self.type_error(key, "array")),
            None => Err(self.missing(key)),
        }
    }

    pub fn opt_arr(&self, key: &str) -> Result<Option<Vec<serde_json::Value>>, WidgetError> {
        match self.cfg()?.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(v @ serde_json::Value::Array(_)) => {
                serde_json::from_value::<Vec<serde_json::Value>>(v.clone())
                    .map(Some)
                    .map_err(|_| self.type_error(key, "array"))
            }
            Some(_) => Err(self.type_error(key, "array")),
        }
    }

    pub fn opt_table(&self, key: &str) -> Result<Option<serde_json::Map<String, serde_json::Value>>, WidgetError> {
        match self.cfg()?.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Object(o)) => Ok(Some(o.clone())),
            Some(_) => Err(self.type_error(key, "table")),
        }
    }

    fn missing(&self, key: &str) -> WidgetError {
        WidgetError::Invalid(format!("missing required config key '{key}'"))
    }

    fn type_error(&self, key: &str, expected: &str) -> WidgetError {
        WidgetError::Invalid(format!("config key '{key}' must be a {expected}"))
    }
}

pub type RenderFn = fn(&WidgetInstance) -> Result<AnyView, WidgetError>;

pub struct WidgetDef {
    pub name: &'static str,
    pub render: RenderFn,
    pub script: &'static str,
    pub styles: &'static str,
}

#[derive(Default)]
pub struct WidgetRegistry {
    defs: HashMap<String, WidgetDef>,
}

impl std::fmt::Debug for WidgetRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetRegistry")
            .field("kinds", &self.kinds_sorted())
            .finish()
    }
}

impl WidgetRegistry {
    pub fn with_builtins() -> WidgetRegistry {
        let mut registry = WidgetRegistry::default();
        registry.register(WidgetDef {
            name: "quiz",
            render: quiz::render,
            script: include_str!("quiz.js"),
            styles: include_str!("quiz.css"),
        });
        registry.register(WidgetDef {
            name: "slider",
            render: slider::render,
            script: include_str!("slider.js"),
            styles: include_str!("slider.css"),
        });
        registry.register(WidgetDef {
            name: "plot",
            render: plot::render,
            script: include_str!("plot.js"),
            styles: include_str!("plot.css"),
        });
        registry.register(WidgetDef {
            name: "flashcard",
            render: flashcard::render,
            script: include_str!("flashcard.js"),
            styles: include_str!("flashcard.css"),
        });
        registry.register(WidgetDef {
            name: "math",
            render: math::render,
            script: include_str!("math.js"),
            styles: include_str!("math.css"),
        });
        registry.register(WidgetDef {
            name: "compare",
            render: compare::render,
            script: include_str!("compare.js"),
            styles: include_str!("compare.css"),
        });
        registry.register(WidgetDef {
            name: "stepper",
            render: stepper::render,
            script: include_str!("stepper.js"),
            styles: include_str!("stepper.css"),
        });
        registry.register(WidgetDef {
            name: "interactive",
            render: interactive::render,
            script: include_str!("interactive.js"),
            styles: include_str!("interactive.css"),
        });
        registry.register(WidgetDef {
            name: "acknowledge",
            render: acknowledge::render,
            script: include_str!("acknowledge.js"),
            styles: include_str!("acknowledge.css"),
        });
        registry.register(WidgetDef {
            name: "resource-link",
            render: resource_link::render,
            script: include_str!("resource_link.js"),
            styles: include_str!("resource_link.css"),
        });
        registry
    }

    pub fn register(&mut self, def: WidgetDef) {
        self.defs.insert(def.name.to_string(), def);
    }

    pub fn get(&self, kind: &str) -> Option<&WidgetDef> {
        self.defs.get(kind)
    }

    pub fn kinds(&self) -> Vec<String> {
        self.defs.keys().cloned().collect()
    }
}

pub fn widget_shell(
    kind: &str,
    id: &str,
    config_json: Option<String>,
    inner: AnyView,
) -> AnyView {
    let config_attr = config_json;
    let class = format!("widget {kind} widget-{kind}");
    view! {
        <div class=class data-widget-type=kind.to_string() data-widget-id=id.to_string() data-config=config_attr>
            {inner}
        </div>
    }
    .into_any()
}

pub fn error_shell(kind: &str, id: &str, message: &str) -> AnyView {
    let kind = kind.to_string();
    let id = id.to_string();
    let message = message.to_string();
    view! {
        <div class="widget widget-error" data-widget-type="error">
            <div class="widget-error-title">"Widget error"</div>
            <p class="widget-error-meta">{format!("type: {kind} · id: {id}")}</p>
            <p class="widget-error-msg">{message}</p>
        </div>
    }
    .into_any()
}

pub fn resolve(
    instance: &WidgetInstance,
    registry: &WidgetRegistry,
) -> (AnyView, Option<String>, Option<String>) {
    let Some(def) = registry.get(&instance.kind) else {
        let msg = format!(
            "unknown widget type '{}'. registered types: {}",
            instance.kind,
            registry.kinds_sorted().join(", ")
        );
        return (error_shell(&instance.kind, &instance.id, &msg), None, Some(msg));
    };
    match (def.render)(instance) {
        Ok(view) => (view, Some(def.name.to_string()), None),
        Err(WidgetError::MissingConfig) => {
            let msg = format!(
                "widget type '{}' needs a config entry: add [widgets.\"{}\"] to this course's config.toml",
                instance.kind, instance.id
            );
            (
                error_shell(&instance.kind, &instance.id, &msg),
                None,
                Some(msg),
            )
        }
        Err(WidgetError::Invalid(msg)) => (
            error_shell(&instance.kind, &instance.id, &msg),
            None,
            Some(msg),
        ),
    }
}

impl WidgetRegistry {
    fn kinds_sorted(&self) -> Vec<String> {
        let mut k = self.kinds();
        k.sort();
        k
    }
}

pub fn serialize_config(config: Option<&serde_json::Value>) -> Option<String> {
    config.map(|c| serde_json::to_string(c).unwrap_or_else(|_| "{}".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "quiz".into(),
            id: "t.w".into(),
            config,
        }
    }

    #[test]
    fn cfg_accessors() {
        let w = instance(Some(json!({
            "s": "text",
            "n": 2.5,
            "i": 7,
            "arr": [1, 2],
        })));
        assert_eq!(w.req_str("s").unwrap(), "text");
        assert_eq!(w.req_f64("n").unwrap(), 2.5);
        assert_eq!(w.req_f64("i").unwrap(), 7.0);
        assert_eq!(w.opt_usize("i").unwrap(), Some(7));
        assert_eq!(w.req_arr("arr").unwrap().len(), 2);
        assert_eq!(w.opt_str("missing").unwrap(), None);
        assert!(w.req_str("missing").is_err());
        assert!(w.req_f64("s").is_err());
        assert!(w.req_arr("s").is_err());
    }

    #[test]
    fn missing_config_is_reported() {
        let w = instance(None);
        assert!(matches!(w.cfg(), Err(WidgetError::Invalid(_))));
        let w = instance(Some(serde_json::Value::Null));
        assert!(matches!(w.cfg(), Err(WidgetError::MissingConfig)));
    }

    #[test]
    fn registry_has_builtins() {
        let r = WidgetRegistry::with_builtins();
        for kind in [
            "quiz",
            "slider",
            "plot",
            "flashcard",
            "math",
            "compare",
            "stepper",
            "interactive",
            "acknowledge",
            "resource-link",
        ] {
            let def = r.get(kind).expect(kind);
            assert!(!def.script.is_empty());
            assert!(!def.styles.is_empty());
        }
        assert!(r.get("nope").is_none());
    }

    #[test]
    fn resolve_unknown_type_gives_error_shell() {
        let r = WidgetRegistry::with_builtins();
        let w = WidgetInstance {
            kind: "bogus".into(),
            id: "t.w".into(),
            config: None,
        };
        let (view, kind, warning) = resolve(&w, &r);
        assert!(kind.is_none());
        assert!(warning.is_some());
        let html = crate::render::render_html(move || view);
        assert!(html.contains("unknown widget type"));
    }
}
