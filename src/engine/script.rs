// Moteur de scripts utilisateur base sur Rhai (https://rhai.rs).
// Sandboxe : 10K operations max, 1MB strings, pas d'acces fichier/reseau.
// Chaque regle peut avoir un champ `script` optionnel qui est execute
// avant le rendu du template. Le resultat est accessible via {{script}}
// (si string) ou {{script.champ}} (si l'objet retourne est un map #{}).
//
// parse_json/to_json/parse_xml_items/xml_element (voir plus bas) : ajoutees
// pour le cas d'usage "la requete contient une liste d'objets, la reponse
// doit contenir le meme nombre d'elements construits par position" (JSON et
// XML/SOAP) — ni {{variable}} ni les conditions de regle ne peuvent boucler.
// Cf docs/scripts-rhai.md pour deux exemples complets verifies bout-en-bout.
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

        // parse_json/to_json : pont JSON <-> structure Rhai navigable (Array/Map),
        // pour le pattern "boucler sur une liste d'objets de la requete et
        // construire N elements de reponse" (aucun {{variable}}/condition de
        // regle ne peut boucler). `parse_json(request.body)` retourne un Array/Map
        // Rhai ; `to_json(valeur)` serialise en texte JSON, a inserer tel quel
        // dans un champ du map retourne par le script (ex. `#{ items: to_json(out) }`
        // puis `{{script.items}}` dans le template de reponse — simple substitution
        // texte, deja geree par le moteur de template existant).
        engine.register_fn("parse_json", |text: &str| -> rhai::Dynamic {
            serde_json::from_str::<serde_json::Value>(text)
                .map(|v| json_value_to_dynamic(&v))
                .unwrap_or(rhai::Dynamic::UNIT)
        });
        engine.register_fn("to_json", |value: rhai::Dynamic| -> String {
            serde_json::to_string(&dynamic_to_json_value(&value)).unwrap_or_default()
        });

        // parse_xml_items/xml_element : equivalent XML/SOAP de parse_json/to_json
        // pour le meme cas d'usage. Pas d'analogue generique "XML <-> Dynamic"
        // (l'XML n'a pas de mapping 1:1 evident objet/tableau, contrairement au
        // JSON) : parse_xml_items() extrait directement les elements REPETES a un
        // chemin donne (le besoin reel : une liste d'objets) en Array de Map (un
        // niveau de champs enfants, cf limitation documentee sur la fonction),
        // xml_element() construit un element (et ses enfants, recursivement) a
        // partir d'une valeur Rhai. `path` reutilise la meme syntaxe segment/segment
        // (prefixe de namespace ignore) que ConditionSource::XPath (§ matcher.rs).
        engine.register_fn("parse_xml_items", |text: &str, path: &str| -> rhai::Array {
            parse_xml_items_impl(text, path)
        });
        engine.register_fn("xml_element", |tag: &str, value: rhai::Dynamic| -> String {
            xml_element_impl(tag, &value)
        });

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

// --- JSON <-> Dynamic (parse_json / to_json) ---

fn json_value_to_dynamic(value: &serde_json::Value) -> rhai::Dynamic {
    match value {
        serde_json::Value::Null => rhai::Dynamic::UNIT,
        serde_json::Value::Bool(b) => rhai::Dynamic::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rhai::Dynamic::from(i)
            } else {
                rhai::Dynamic::from(n.as_f64().unwrap_or_default())
            }
        }
        serde_json::Value::String(s) => rhai::Dynamic::from(s.clone()),
        serde_json::Value::Array(arr) => {
            let items: rhai::Array = arr.iter().map(json_value_to_dynamic).collect();
            rhai::Dynamic::from_array(items)
        }
        serde_json::Value::Object(obj) => {
            let map: rhai::Map = obj
                .iter()
                .map(|(k, v)| (k.as_str().into(), json_value_to_dynamic(v)))
                .collect();
            rhai::Dynamic::from_map(map)
        }
    }
}

fn dynamic_to_json_value(value: &rhai::Dynamic) -> serde_json::Value {
    if value.is_unit() {
        serde_json::Value::Null
    } else if value.is_bool() {
        serde_json::Value::Bool(value.as_bool().unwrap_or_default())
    } else if value.is_int() {
        serde_json::Value::from(value.as_int().unwrap_or_default())
    } else if value.is_float() {
        serde_json::Number::from_f64(value.as_float().unwrap_or_default())
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    } else if value.is_array() {
        let arr = value.clone().into_array().unwrap_or_default();
        serde_json::Value::Array(arr.iter().map(dynamic_to_json_value).collect())
    } else if value.is_map() {
        let map = value.clone().cast::<rhai::Map>();
        let obj: serde_json::Map<String, serde_json::Value> = map
            .iter()
            .map(|(k, v)| (k.to_string(), dynamic_to_json_value(v)))
            .collect();
        serde_json::Value::Object(obj)
    } else {
        // string (et tout type sans equivalent JSON direct) : fallback texte,
        // coherent avec le reste du moteur (ScriptResult.fields fait deja
        // system­atiquement `.to_string()` sur les valeurs de map, cf execute()).
        serde_json::Value::String(value.to_string())
    }
}

// --- XML : extraction d'elements repetes + construction (parse_xml_items / xml_element) ---

// Extrait TOUS les elements repetes au chemin `path` (segments separes par
// "/", dernier segment = tag de l'element repete ; prefixes de namespace
// ignores via MatchEngine::local_name, meme convention que ConditionSource::
// XPath) en Array de Map Rhai (un champ Map par element, cle = tag de
// l'enfant, valeur = son texte). Limitation assumee (cas d'usage cible :
// items plats type SOAP `<product><sku>A</sku><qty>2</qty></product>`) :
// seul le PREMIER niveau d'enfants de chaque item est capture ; une
// structure imbriquee plus profonde a l'interieur d'un item n'est pas
// supportee (ignoree silencieusement) — un vrai parseur XML->arbre generique
// serait disproportionne pour ce besoin (cf CLAUDE.md, sobriete des API).
fn parse_xml_items_impl(xml: &str, path: &str) -> rhai::Array {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let Some((item_tag, parent_segments)) = segments.split_last() else {
        return rhai::Array::new();
    };

    let mut reader = Reader::from_str(xml);
    let mut parent_depth = 0usize;
    let mut in_parent = parent_segments.is_empty();
    let mut items = rhai::Array::new();

    // item_depth : 0 = hors item, 1 = a l'interieur de l'element item lui-meme,
    // 2 = a l'interieur d'un champ enfant de l'item (feuille capturee).
    let mut item_depth: u32 = 0;
    let mut current_item = rhai::Map::new();
    let mut current_field = String::new();
    let mut current_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = crate::engine::matcher::MatchEngine::local_name(&e);
                if item_depth == 0 {
                    if in_parent {
                        if local == *item_tag {
                            item_depth = 1;
                            current_item = rhai::Map::new();
                        }
                    } else if parent_depth < parent_segments.len()
                        && local == parent_segments[parent_depth]
                    {
                        parent_depth += 1;
                        if parent_depth == parent_segments.len() {
                            in_parent = true;
                        }
                    }
                } else if item_depth == 1 {
                    current_field = local;
                    current_text.clear();
                    item_depth = 2;
                } else {
                    item_depth += 1;
                }
            }
            Ok(Event::Text(e)) => {
                if item_depth == 2 {
                    if let Ok(t) = e.unescape() {
                        current_text.push_str(&t);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if item_depth == 2 {
                    current_item.insert(
                        std::mem::take(&mut current_field).into(),
                        rhai::Dynamic::from(current_text.trim().to_string()),
                    );
                    current_text.clear();
                    item_depth = 1;
                } else if item_depth == 1 {
                    items.push(rhai::Dynamic::from_map(std::mem::take(&mut current_item)));
                    item_depth = 0;
                } else if item_depth > 2 {
                    item_depth -= 1;
                } else if in_parent {
                    parent_depth = parent_depth.saturating_sub(1);
                    if parent_depth < parent_segments.len() {
                        in_parent = false;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    items
}

// Construit un element XML `<tag>...</tag>` a partir d'une valeur Rhai :
// - Map -> un element enfant par cle (recursif) ; une valeur Array sous une
//   cle REPETE le tag de cette cle (une occurrence par element) plutot que de
//   produire un tableau litteral (l'XML n'a pas de syntaxe de tableau) ;
// - Array (au niveau racine de l'appel) -> le tag lui-meme repete une fois
//   par element (usage : concatener `xml_element("item", it)` dans une
//   boucle Rhai, ou passer directement le tableau si tous les items partagent
//   un seul tag) ;
// - scalaire (string/int/float/bool) -> texte echappe ;
// - unit (absent/null) -> element auto-ferme `<tag/>`.
fn xml_element_impl(tag: &str, value: &rhai::Dynamic) -> String {
    if value.is_map() {
        let map = value.clone().cast::<rhai::Map>();
        let mut inner = String::new();
        for (k, v) in map.iter() {
            if v.is_array() {
                let arr = v.clone().into_array().unwrap_or_default();
                for item in &arr {
                    inner.push_str(&xml_element_impl(&k.to_string(), item));
                }
            } else {
                inner.push_str(&xml_element_impl(&k.to_string(), v));
            }
        }
        format!("<{tag}>{inner}</{tag}>")
    } else if value.is_array() {
        let arr = value.clone().into_array().unwrap_or_default();
        arr.iter()
            .map(|item| xml_element_impl(tag, item))
            .collect()
    } else if value.is_unit() {
        format!("<{tag}/>")
    } else {
        format!("<{tag}>{}</{tag}>", xml_escape_text(&value.to_string()))
    }
}

fn xml_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

    // --- parse_json / to_json ---

    #[test]
    fn parse_json_array_len_and_index_access() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: r#"[{"id":"1"},{"id":"2"},{"id":"3"}]"#.into(),
            ..empty_ctx()
        };
        let result = engine
            .execute("parse_json(request.body).len()", &ctx)
            .unwrap();
        assert_eq!(result.value, "3");
        let result = engine
            .execute("parse_json(request.body)[1].id", &ctx)
            .unwrap();
        assert_eq!(result.value, "2");
    }

    #[test]
    fn parse_json_object_field_access() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: r#"{"name":"Alice","age":30}"#.into(),
            ..empty_ctx()
        };
        let result = engine.execute("parse_json(request.body).name", &ctx).unwrap();
        assert_eq!(result.value, "Alice");
        let result = engine.execute("parse_json(request.body).age", &ctx).unwrap();
        assert_eq!(result.value, "30");
    }

    #[test]
    fn parse_json_invalid_returns_unit_not_error() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: "not valid json".into(),
            ..empty_ctx()
        };
        let result = engine
            .execute("type_of(parse_json(request.body))", &ctx)
            .unwrap();
        assert_eq!(result.value, "()");
    }

    #[test]
    fn to_json_scalar_and_map() {
        let engine = ScriptEngine::new();
        let result = engine.execute(r#"to_json("hello")"#, &empty_ctx()).unwrap();
        assert_eq!(result.value, "\"hello\"");
        let result = engine
            .execute(r#"to_json(#{ id: 1, name: "Alice" })"#, &empty_ctx())
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.value).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["name"], "Alice");
    }

    #[test]
    fn parse_json_then_to_json_roundtrip_array_of_objects() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: r#"[{"sku":"A1","qty":2},{"sku":"B2","qty":5}]"#.into(),
            ..empty_ctx()
        };
        let script = r#"
            let items = parse_json(request.body);
            let out = [];
            for it in items {
                out.push(#{ sku: it.sku, doubled: it.qty * 2 });
            }
            to_json(out)
        "#;
        let result = engine.execute(script, &ctx).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result.value).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["sku"], "A1");
        assert_eq!(parsed[0]["doubled"], 4);
        assert_eq!(parsed[1]["sku"], "B2");
        assert_eq!(parsed[1]["doubled"], 10);
    }

    #[test]
    fn parse_json_then_to_json_empty_array_stays_empty() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: "[]".into(),
            ..empty_ctx()
        };
        let script = r#"
            let items = parse_json(request.body);
            let out = [];
            for it in items { out.push(it); }
            to_json(out)
        "#;
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.value, "[]");
    }

    // --- parse_xml_items / xml_element ---

    #[test]
    fn parse_xml_items_extracts_repeated_flat_elements() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: "<products><product><sku>A1</sku><qty>2</qty></product><product><sku>B2</sku><qty>5</qty></product></products>".into(),
            ..empty_ctx()
        };
        let result = engine
            .execute(
                r#"parse_xml_items(request.body, "products/product").len()"#,
                &ctx,
            )
            .unwrap();
        assert_eq!(result.value, "2");
        let result = engine
            .execute(
                r#"parse_xml_items(request.body, "products/product")[0].sku"#,
                &ctx,
            )
            .unwrap();
        assert_eq!(result.value, "A1");
        let result = engine
            .execute(
                r#"parse_xml_items(request.body, "products/product")[1].qty"#,
                &ctx,
            )
            .unwrap();
        assert_eq!(result.value, "5");
    }

    #[test]
    fn parse_xml_items_ignores_namespace_prefixes() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: r#"<soap:Envelope><soap:Body><products><product><sku>A1</sku></product></products></soap:Body></soap:Envelope>"#.into(),
            ..empty_ctx()
        };
        let result = engine
            .execute(
                r#"parse_xml_items(request.body, "Envelope/Body/products/product").len()"#,
                &ctx,
            )
            .unwrap();
        assert_eq!(result.value, "1");
    }

    #[test]
    fn parse_xml_items_no_match_returns_empty_array() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: "<products></products>".into(),
            ..empty_ctx()
        };
        let result = engine
            .execute(
                r#"parse_xml_items(request.body, "products/product").len()"#,
                &ctx,
            )
            .unwrap();
        assert_eq!(result.value, "0");
    }

    #[test]
    fn xml_element_scalar_escapes_special_chars() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(r#"xml_element("name", "A & <B>")"#, &empty_ctx())
            .unwrap();
        assert_eq!(result.value, "<name>A &amp; &lt;B&gt;</name>");
    }

    #[test]
    fn xml_element_map_builds_nested_children() {
        let engine = ScriptEngine::new();
        let result = engine
            .execute(
                r#"xml_element("product", #{ sku: "A1", qty: 2 })"#,
                &empty_ctx(),
            )
            .unwrap();
        assert!(result.value.starts_with("<product>"));
        assert!(result.value.ends_with("</product>"));
        assert!(result.value.contains("<sku>A1</sku>"));
        assert!(result.value.contains("<qty>2</qty>"));
    }

    #[test]
    fn parse_xml_items_then_xml_element_roundtrip_builds_same_count() {
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: "<products><product><sku>A1</sku><qty>2</qty></product><product><sku>B2</sku><qty>5</qty></product><product><sku>C3</sku><qty>9</qty></product></products>".into(),
            ..empty_ctx()
        };
        let script = r#"
            let items = parse_xml_items(request.body, "products/product");
            let out = "";
            for it in items {
                out += xml_element("item", #{ sku: it.sku, doubledQty: parse_int(it.qty) * 2 });
            }
            out
        "#;
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.value.matches("<item>").count(), 3);
        assert!(result.value.contains("<sku>A1</sku>"));
        assert!(result.value.contains("<doubledQty>4</doubledQty>"));
        assert!(result.value.contains("<sku>C3</sku>"));
        assert!(result.value.contains("<doubledQty>18</doubledQty>"));
    }

    // --- Scripts "complexes" representatifs (cf CLAUDE.md, "Visibilite des
    // erreurs de script" + docs/scripts-rhai.md) : map/lookup, boucle avec
    // condition, acces combine a plusieurs sources de contexte. Objectif :
    // securiser ces usages a l'avenir (pas seulement le cas precis
    // remonte), en couvrant la VRAIE syntaxe correcte (verifiee au moment du
    // diagnostic : indexation de map `#{}` par cle, `.contains()`, `in`,
    // `.get()`, `switch` fonctionnent tous sans erreur, y compris sur une
    // cle absente qui renvoie simplement une valeur vide plutot que de
    // lever une exception).

    #[test]
    fn map_lookup_returns_mapped_value_for_known_key() {
        let engine = ScriptEngine::new();
        let script = r#"
            let mapping = #{ "svc-a": "id-1", "svc-b": "id-2" };
            let key = request.path.name;
            if mapping.contains(key) { mapping[key] } else { "unknown" }
        "#;
        let mut path = HashMap::new();
        path.insert("name".into(), "svc-b".into());
        let ctx = ScriptContext { path_params: path, ..empty_ctx() };
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.value, "id-2");
    }

    #[test]
    fn map_lookup_falls_back_for_unknown_key() {
        let engine = ScriptEngine::new();
        let script = r#"
            let mapping = #{ "svc-a": "id-1", "svc-b": "id-2" };
            let key = request.path.name;
            if mapping.contains(key) { mapping[key] } else { "unknown" }
        "#;
        let mut path = HashMap::new();
        path.insert("name".into(), "svc-zzz".into());
        let ctx = ScriptContext { path_params: path, ..empty_ctx() };
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.value, "unknown");
    }

    #[test]
    fn map_lookup_via_switch_expression() {
        // Variante avec `switch`, alternative valide au if/mapping.contains
        // ci-dessus — les deux syntaxes sont documentees dans
        // docs/scripts-rhai.md.
        let engine = ScriptEngine::new();
        let script = r#"
            let key = request.path.name;
            switch key {
                "svc-a" => "id-1",
                "svc-b" => "id-2",
                _ => "unknown"
            }
        "#;
        let mut path = HashMap::new();
        path.insert("name".into(), "svc-a".into());
        let ctx = ScriptContext { path_params: path, ..empty_ctx() };
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.value, "id-1");
    }

    #[test]
    fn loop_with_condition_counts_matching_items() {
        // Boucle avec condition : compte les lignes d'une liste JSON dont la
        // quantite depasse un seuil, construit un resume dans un map.
        let engine = ScriptEngine::new();
        let ctx = ScriptContext {
            body: r#"[{"sku":"A","qty":1},{"sku":"B","qty":5},{"sku":"C","qty":12}]"#.into(),
            ..empty_ctx()
        };
        let script = r#"
            let items = parse_json(request.body);
            let big_count = 0;
            let skus = [];
            for it in items {
                if it.qty > 3 {
                    big_count += 1;
                    skus.push(it.sku);
                }
            }
            #{ big_count: big_count, big_skus: to_json(skus) }
        "#;
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.fields.get("big_count").unwrap(), "2");
        let skus: serde_json::Value = serde_json::from_str(result.fields.get("big_skus").unwrap()).unwrap();
        assert_eq!(skus, serde_json::json!(["B", "C"]));
    }

    #[test]
    fn combines_path_query_headers_and_body_in_one_script() {
        // Acces combine a toutes les sources de contexte disponibles dans un
        // seul script (path/query/headers/body), avec un fallback via
        // if/else sur une en-tete absente. Rhai n'a PAS d'operateur
        // ternaire `?:` (verifie : "Unknown operator: '?'") — if/else est la
        // seule forme valide ici.
        let engine = ScriptEngine::new();
        let mut path = HashMap::new();
        path.insert("id".into(), "42".into());
        let mut query = HashMap::new();
        query.insert("verbose".into(), "true".into());
        let mut headers = HashMap::new();
        headers.insert("x-request-id".into(), "req-abc".into());
        let ctx = ScriptContext {
            body: r#"{"note":"hello"}"#.into(),
            headers,
            query_params: query,
            path_params: path,
        };
        let script = r#"
            let id = request.path.id;
            let verbose = request.query.verbose;
            let req_id = if request.headers.contains("x-request-id") { request.headers["x-request-id"] } else { "none" };
            let missing = if request.headers.contains("x-absent") { request.headers["x-absent"] } else { "none" };
            let note = parse_json(request.body).note;
            #{ id: id, verbose: verbose, req_id: req_id, missing: missing, note: note }
        "#;
        let result = engine.execute(script, &ctx).unwrap();
        assert_eq!(result.fields.get("id").unwrap(), "42");
        assert_eq!(result.fields.get("verbose").unwrap(), "true");
        assert_eq!(result.fields.get("req_id").unwrap(), "req-abc");
        assert_eq!(result.fields.get("missing").unwrap(), "none");
        assert_eq!(result.fields.get("note").unwrap(), "hello");
    }

    // --- Visibilite des erreurs d'execution (cause racine du sujet
    // "Visibilite des erreurs de script") : appeler une fonction Rhai
    // inexistante EST une vraie erreur d'execution (contrairement a un
    // acces a une cle de map absente, qui renvoie silencieusement une
    // valeur vide sans jamais lever d'erreur, cf tests ci-dessus). C'est
    // cette classe d'erreur que engine.execute() remonte via Err(...), et
    // que run_rule_script (intercept.rs) avale en soft-fail, et que
    // /api/rule-test (server/api.rs) rend desormais visible au testeur de
    // regle. Ne pas supprimer ce test : c'est la preuve que le mecanisme de
    // detection a une vraie erreur a se mettre sous la dent.

    #[test]
    fn calling_undefined_function_is_a_real_execution_error() {
        let engine = ScriptEngine::new();
        let result = engine.execute("totally_undefined_fn(1, 2)", &empty_ctx());
        let err = result.expect_err("un appel de fonction inexistante doit etre une erreur d'execution");
        assert!(err.contains("totally_undefined_fn"));
    }

    #[test]
    fn missing_map_key_access_is_not_an_error_unlike_undefined_function() {
        // Documente la difference exacte diagnostiquee : contrairement a une
        // fonction inexistante (test ci-dessus), une cle de map absente
        // n'est PAS une erreur — c'est la raison pour laquelle une
        // validation "a vide" (dry-run avec un contexte synthetique) ne
        // suffirait pas a detecter un mauvais nom de cle, et pourquoi le
        // testeur de regle (contre une VRAIE requete capturee) reste le bon
        // outil pour ce cas — cf commentaire sur /api/rule-test.
        let engine = ScriptEngine::new();
        let result = engine.execute("request.path.this_key_does_not_exist", &empty_ctx());
        assert!(result.is_ok(), "une cle absente ne doit jamais lever d'erreur");
        assert_eq!(result.unwrap().value, "");
    }
}
