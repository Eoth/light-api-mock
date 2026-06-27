// Moteur de scripts utilisateur base sur Rhai (https://rhai.rs).
// Sandboxe : 10K operations max, 1MB strings, pas d'acces fichier/reseau.
// Chaque regle peut avoir un champ `script` optionnel qui est execute
// avant le rendu du template. Le resultat est accessible via {{script}}
// (si string) ou {{script.champ}} (si l'objet retourne est un map #{}).
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ScriptEngine {
    engine: Arc<rhai::Engine>,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptResult {
    pub value: String,
    pub fields: HashMap<String, String>,
}

pub struct ScriptContext {
    pub body: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub path_params: HashMap<String, String>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = rhai::Engine::new();
        engine.set_max_operations(10_000);
        engine.set_max_string_size(1_048_576);
        engine.set_max_array_size(1_000);
        engine.set_max_map_size(500);

        engine.register_fn("random_int", |min: i64, max: i64| -> i64 {
            if min >= max { return min; }
            min + (fastrand::i64(..) % (max - min + 1)).abs()
        });

        engine.register_fn("now_ms", || -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

        Self {
            engine: Arc::new(engine),
        }
    }

    pub fn execute(&self, script: &str, context: &ScriptContext) -> Result<ScriptResult, String> {
        let mut scope = rhai::Scope::new();

        let mut request = rhai::Map::new();
        request.insert("body".into(), context.body.clone().into());

        let headers: rhai::Map = context
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().into(), rhai::Dynamic::from(v.clone())))
            .collect();
        request.insert("headers".into(), rhai::Dynamic::from_map(headers));

        let query: rhai::Map = context
            .query_params
            .iter()
            .map(|(k, v)| (k.as_str().into(), rhai::Dynamic::from(v.clone())))
            .collect();
        request.insert("query".into(), rhai::Dynamic::from_map(query));

        let path: rhai::Map = context
            .path_params
            .iter()
            .map(|(k, v)| (k.as_str().into(), rhai::Dynamic::from(v.clone())))
            .collect();
        request.insert("path".into(), rhai::Dynamic::from_map(path));

        scope.push("request", rhai::Dynamic::from_map(request));

        let result = self
            .engine
            .eval_with_scope::<rhai::Dynamic>(&mut scope, script)
            .map_err(|e| e.to_string())?;

        if result.is_map() {
            let map = result.cast::<rhai::Map>();
            let fields: HashMap<String, String> = map
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Ok(ScriptResult {
                value: String::new(),
                fields,
            })
        } else {
            Ok(ScriptResult {
                value: result.to_string(),
                fields: HashMap::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> ScriptContext {
        ScriptContext {
            body: String::new(),
            headers: HashMap::new(),
            query_params: HashMap::new(),
            path_params: HashMap::new(),
        }
    }

    #[test]
    fn simple_string_result() {
        let engine = ScriptEngine::new();
        let result = engine.execute(r#""hello""#, &empty_ctx()).unwrap();
        assert_eq!(result.value, "hello");
        assert!(result.fields.is_empty());
    }

    #[test]
    fn map_result_returns_fields() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(r#"#{ greeting: "hi", count: "42" }"#, &empty_ctx())
            .unwrap();
        assert_eq!(result.fields.get("greeting").unwrap(), "hi");
        assert_eq!(result.fields.get("count").unwrap(), "42");
    }

    #[test]
    fn access_request_context() {
        let engine = ScriptEngine::new();
        let mut path = HashMap::new();
        path.insert("id".into(), "123".into());
        let ctx = ScriptContext {
            path_params: path,
            ..empty_ctx()
        };
        let result = engine.execute("request.path.id", &ctx).unwrap();
        assert_eq!(result.value, "123");
    }

    #[test]
    fn string_manipulation() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(r#"let x = "hello"; x.to_upper()"#, &empty_ctx())
            .unwrap();
        assert_eq!(result.value, "HELLO");
    }

    #[test]
    fn invalid_script_returns_error() {
        let engine = ScriptEngine::new();
        let result = engine.execute("this is not valid", &empty_ctx());
        assert!(result.is_err());
    }

    #[test]
    fn access_query_params() {
        let engine = ScriptEngine::new();
        let mut query = HashMap::new();
        query.insert("page".into(), "5".into());
        let ctx = ScriptContext {
            query_params: query,
            ..empty_ctx()
        };
        let result = engine.execute("request.query.page", &ctx).unwrap();
        assert_eq!(result.value, "5");
    }

    #[test]
    fn random_int_in_range() {
        let engine = ScriptEngine::new();
        for _ in 0..20 {
            let result = engine.execute("random_int(1, 5)", &empty_ctx()).unwrap();
            let val: i64 = result.value.parse().unwrap();
            assert!((1..=5).contains(&val), "random_int(1,5) returned {val}");
        }
    }

    #[test]
    fn now_ms_returns_timestamp() {
        let engine = ScriptEngine::new();
        let result = engine.execute("now_ms()", &empty_ctx()).unwrap();
        let val: i64 = result.value.parse().unwrap();
        assert!(val > 1_700_000_000_000, "now_ms too small: {val}");
    }

    #[test]
    fn ratio_script_returns_map() {
        let engine = ScriptEngine::new();
        let script = r#"
            let roll = random_int(1, 5);
            if roll <= 4 {
                #{ status: "actif", code: "200" }
            } else {
                #{ status: "suspendu", code: "403" }
            }
        "#;
        let result = engine.execute(script, &empty_ctx()).unwrap();
        let status = result.fields.get("status").unwrap();
        assert!(status == "actif" || status == "suspendu");
    }
}
