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

        engine.register_fn("now_iso", || -> String {
            crate::engine::template::epoch_to_iso(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            )
        });

        engine.register_fn("year", || -> i64 {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let iso = crate::engine::template::epoch_to_iso(secs);
            iso[..4].parse().unwrap_or(2026)
        });

        // date_now/date_past/date_future : format configurable ("iso" par defaut,
        // "fr" = JJ/MM/AAAA, "en" = MM/JJ/AAAA). Remplace les anciennes date()/
        // date_past()/date_future() (sortie ISO figee, sans parametre) : pas de
        // contrainte de retrocompatibilite sur ces fonctions (peu d'utilisateurs a
        // date), l'occasion d'assainir plutot que d'empiler un 2e nom. days<=0 pour
        // date_past/date_future est traite comme "aujourd'hui" (borne a 0), un choix
        // deterministe plutot qu'une inversion silencieuse vers le futur/passe.
        engine.register_fn("date_now", || -> String { format_date_offset(0, "iso") });
        engine.register_fn("date_now", |format: &str| -> String {
            format_date_offset(0, format)
        });
        engine.register_fn("date_past", |days: i64| -> String {
            format_date_offset(-days.max(0), "iso")
        });
        engine.register_fn("date_past", |days: i64, format: &str| -> String {
            format_date_offset(-days.max(0), format)
        });
        engine.register_fn("date_future", |days: i64| -> String {
            format_date_offset(days.max(0), "iso")
        });
        engine.register_fn("date_future", |days: i64, format: &str| -> String {
            format_date_offset(days.max(0), format)
        });

        engine.register_fn("uuid", || -> String {
            uuid::Uuid::new_v4().to_string()
        });

        engine.register_fn("fake", |kind: &str| -> String {
            crate::engine::template::resolve_fake_public(kind)
        });

        // seeded_int/seeded_pick : tirage deterministe pour un meme seed (ex. un
        // meme SIRET en path param retourne toujours le meme resultat). Reutilise
        // le hash FNV-1a deja en place pour Group.code (src/server/codegen.rs) au
        // lieu d'ajouter une dependance de hashing dediee. `seed` accepte n'importe
        // quel type Rhai (string, int, bool...) via Dynamic::to_string().
        engine.register_fn("seeded_int", |seed: rhai::Dynamic, min: i64, max: i64| -> i64 {
            seeded_int_impl(&seed.to_string(), min, max)
        });
        engine.register_fn(
            "seeded_pick",
            |seed: rhai::Dynamic, list: rhai::Array| -> rhai::Dynamic {
                seeded_pick_impl(&seed.to_string(), &list)
            },
        );

        Self {
            engine: Arc::new(engine),
        }
    }

    pub fn validate(&self, script: &str) -> Result<(), String> {
        self.engine
            .compile(script)
            .map(|_| ())
            .map_err(|e| e.to_string())
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

// Formate epoch_secs + days_delta*86400 selon "iso" (defaut/fallback)/"fr"/"en".
// Reutilise civil_from_days (deja ecrit pour epoch_to_iso, algorithme de Howard
// Hinnant) plutot que d'ajouter une dependance de formatage de date.
fn format_date_offset(days_delta: i64, format: &str) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = (now_secs + days_delta * 86400).div_euclid(86400);
    let (y, mo, d) = crate::engine::template::civil_from_days(days);
    match format {
        "fr" => format!("{d:02}/{mo:02}/{y:04}"),
        "en" => format!("{mo:02}/{d:02}/{y:04}"),
        _ => format!("{y:04}-{mo:02}-{d:02}"),
    }
}

fn seeded_int_impl(seed: &str, min: i64, max: i64) -> i64 {
    if min >= max {
        return min;
    }
    let hash = crate::server::codegen::fnv1a_hash(seed);
    let range = (max - min + 1) as u64;
    min + (hash % range) as i64
}

fn seeded_pick_impl(seed: &str, list: &rhai::Array) -> rhai::Dynamic {
    if list.is_empty() {
        return rhai::Dynamic::UNIT;
    }
    let hash = crate::server::codegen::fnv1a_hash(seed);
    let idx = (hash % list.len() as u64) as usize;
    list[idx].clone()
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

    #[test]
    fn year_returns_4_digits() {
        let engine = ScriptEngine::new();
        let result = engine.execute("year()", &empty_ctx()).unwrap();
        let y: i64 = result.value.parse().unwrap();
        assert!(y >= 2025 && y <= 2100);
    }

    #[test]
    fn date_now_returns_iso_date_by_default() {
        let engine = ScriptEngine::new();
        let result = engine.execute("date_now()", &empty_ctx()).unwrap();
        assert_eq!(result.value.len(), 10);
        assert!(result.value.contains('-'));
    }

    #[test]
    fn date_now_explicit_iso_matches_default() {
        let engine = ScriptEngine::new();
        let default = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let explicit = engine
            .execute(r#"date_now("iso")"#, &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(default, explicit);
    }

    #[test]
    fn date_now_fr_format() {
        let engine = ScriptEngine::new();
        let iso = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let fr = engine
            .execute(r#"date_now("fr")"#, &empty_ctx())
            .unwrap()
            .value;
        let parts: Vec<&str> = iso.split('-').collect();
        assert_eq!(fr, format!("{}/{}/{}", parts[2], parts[1], parts[0]));
    }

    #[test]
    fn date_now_en_format() {
        let engine = ScriptEngine::new();
        let iso = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let en = engine
            .execute(r#"date_now("en")"#, &empty_ctx())
            .unwrap()
            .value;
        let parts: Vec<&str> = iso.split('-').collect();
        assert_eq!(en, format!("{}/{}/{}", parts[1], parts[2], parts[0]));
    }

    #[test]
    fn date_now_unknown_format_falls_back_to_iso() {
        let engine = ScriptEngine::new();
        let iso = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let unknown = engine
            .execute(r#"date_now("klingon")"#, &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(iso, unknown);
    }

    #[test]
    fn date_past_is_before_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let past = engine.execute("date_past(10)", &empty_ctx()).unwrap().value;
        assert!(past < today);
    }

    #[test]
    fn date_future_is_after_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let future = engine
            .execute("date_future(10)", &empty_ctx())
            .unwrap()
            .value;
        assert!(future > today);
    }

    #[test]
    fn date_past_zero_days_is_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let past = engine.execute("date_past(0)", &empty_ctx()).unwrap().value;
        assert_eq!(past, today);
    }

    #[test]
    fn date_past_negative_days_clamped_to_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let past = engine
            .execute("date_past(-30)", &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(past, today);
    }

    #[test]
    fn date_future_zero_days_is_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let future = engine
            .execute("date_future(0)", &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(future, today);
    }

    #[test]
    fn date_future_negative_days_clamped_to_today() {
        let engine = ScriptEngine::new();
        let today = engine.execute("date_now()", &empty_ctx()).unwrap().value;
        let future = engine
            .execute("date_future(-30)", &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(future, today);
    }

    #[test]
    fn date_past_with_format() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(r#"date_past(5, "fr")"#, &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(result.len(), 10);
        assert_eq!(result.chars().filter(|c| *c == '/').count(), 2);
    }

    #[test]
    fn date_future_with_format() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(r#"date_future(5, "en")"#, &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(result.len(), 10);
        assert_eq!(result.chars().filter(|c| *c == '/').count(), 2);
    }

    #[test]
    fn seeded_int_is_deterministic_for_same_seed() {
        let engine = ScriptEngine::new();
        let a = engine
            .execute(r#"seeded_int("44306184100047", 0, 1000)"#, &empty_ctx())
            .unwrap()
            .value;
        let b = engine
            .execute(r#"seeded_int("44306184100047", 0, 1000)"#, &empty_ctx())
            .unwrap()
            .value;
        assert_eq!(a, b);
    }

    #[test]
    fn seeded_int_respects_bounds() {
        let engine = ScriptEngine::new();
        for seed in ["a", "b", "c", "siret-123", "0", "-1"] {
            let script = format!(r#"seeded_int("{seed}", 10, 20)"#);
            let result = engine.execute(&script, &empty_ctx()).unwrap();
            let val: i64 = result.value.parse().unwrap();
            assert!((10..=20).contains(&val), "seeded_int out of bounds: {val}");
        }
    }

    #[test]
    fn seeded_int_different_seeds_can_differ() {
        let engine = ScriptEngine::new();
        let values: Vec<String> = (0..10)
            .map(|i| {
                let script = format!(r#"seeded_int("seed-{i}", 0, 1000000)"#);
                engine.execute(&script, &empty_ctx()).unwrap().value
            })
            .collect();
        let unique: std::collections::HashSet<_> = values.iter().collect();
        assert!(
            unique.len() > 1,
            "expected different seeds to produce at least some different values"
        );
    }

    #[test]
    fn seeded_pick_is_deterministic_for_same_seed() {
        let engine = ScriptEngine::new();
        let script =
            r#"seeded_pick(request.path.siret, ["Dupont SARL", "Martin SAS", "Petit EURL"])"#;
        let mut path = HashMap::new();
        path.insert("siret".into(), "44306184100047".into());
        let ctx = ScriptContext {
            path_params: path,
            ..empty_ctx()
        };
        let a = engine.execute(script, &ctx).unwrap().value;
        let b = engine.execute(script, &ctx).unwrap().value;
        assert_eq!(a, b);
        assert!(["Dupont SARL", "Martin SAS", "Petit EURL"].contains(&a.as_str()));
    }

    #[test]
    fn seeded_pick_different_seed_can_pick_different_element() {
        let engine = ScriptEngine::new();
        let script = |siret: &str| -> String {
            let mut path = HashMap::new();
            path.insert("siret".into(), siret.into());
            let ctx = ScriptContext {
                path_params: path,
                ..empty_ctx()
            };
            engine
                .execute(
                    r#"seeded_pick(request.path.siret, ["Dupont SARL", "Martin SAS", "Petit EURL", "Durand SA", "Leroy SCI"])"#,
                    &ctx,
                )
                .unwrap()
                .value
        };
        let picks: std::collections::HashSet<String> = (0..10)
            .map(|i| script(&format!("siret-{i}")))
            .collect();
        assert!(
            picks.len() > 1,
            "expected different SIRET values to yield at least some different picks"
        );
    }

    #[test]
    fn uuid_format() {
        let engine = ScriptEngine::new();
        let result = engine.execute("uuid()", &empty_ctx()).unwrap();
        assert_eq!(result.value.len(), 36);
        assert_eq!(result.value.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn fake_returns_data() {
        let engine = ScriptEngine::new();
        let result = engine.execute(r#"fake("FirstName")"#, &empty_ctx()).unwrap();
        assert!(!result.value.is_empty());
    }

    #[test]
    fn compose_date_with_string() {
        let engine = ScriptEngine::new();
        let result = engine.execute(r#"`${year()}-05-10`"#, &empty_ctx()).unwrap();
        assert!(result.value.ends_with("-05-10"));
        assert_eq!(result.value.len(), 10);
    }
}
