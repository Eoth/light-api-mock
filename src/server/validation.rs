use crate::models::Service;

const RESERVED_NAMES: &[&str] =
    &["api", "auth", "index.html", "assets", "favicon.ico", "runtime-config.json"];
const VALID_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];

const RESERVED_PATH_PREFIXES: &[&str] = &[
    "/api/", "/api", "/index.html", "/assets/", "/favicon.ico", "/runtime-config.json",
];

static NAME_CHARSET_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Za-z0-9_-]+$").unwrap());

// Les deux schemas de nommage generes par MockStore : "mock-config-{ts}-{seq}.yaml"
// (backups/) et "pre-reset-{ts}.yaml" (backups/protected/). Le charset exclut
// tout separateur de chemin ('/', '\\') et toute sequence '..', empechant une
// traversee de repertoire meme si le nom vient d'un segment d'URL decode.
static BACKUP_FILENAME_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Za-z0-9_-]+\.yaml$").unwrap());

pub fn is_reserved_name(name: &str) -> bool {
    let normalized = name.trim().to_lowercase();
    RESERVED_NAMES.contains(&normalized.as_str())
}

pub fn is_internal_route(path: &str) -> bool {
    if path == "/" || path.is_empty() {
        return true;
    }
    let lower = path.to_lowercase();
    RESERVED_PATH_PREFIXES
        .iter()
        .any(|prefix| lower == *prefix || lower.starts_with(prefix))
}

// Sous-ensemble volontairement RESTREINT de `is_internal_route` : les memes
// prefixes SAUF "/api"/"/api/". Utilise exclusivement par `auth_middleware`
// (src/auth/middleware.rs) pour laisser un navigateur SANS token charger la
// coquille de la SPA (index.html + bundle JS/CSS + favicon) quand
// AUTH_ENABLED=true — sans quoi le JS qui affiche l'ecran de connexion
// (LoginForm.svelte) ne peut lui-meme jamais etre telecharge (probleme de
// poule et l'oeuf). Aucun `/api/*` ne doit JAMAIS apparaitre ici : ce serait
// exactement l'affaiblissement de la protection API que ce bypass doit
// eviter. Les 4 routes /api/auth/* deja exemptees dans auth_middleware
// restent gerees separement, par egalite stricte de chemin (pas par ce
// prefixe). `/runtime-config.json` est inclus ici pour la meme raison que
// les assets statiques : le frontend doit pouvoir le lire AVANT de savoir
// s'il est authentifie (c'est ce fichier qui lui indique ou se trouve l'API).
const STATIC_ASSET_PATH_PREFIXES: &[&str] =
    &["/index.html", "/assets/", "/favicon.ico", "/runtime-config.json"];

pub fn is_static_asset_route(path: &str) -> bool {
    if path == "/" || path.is_empty() {
        return true;
    }
    let lower = path.to_lowercase();
    STATIC_ASSET_PATH_PREFIXES
        .iter()
        .any(|prefix| lower == *prefix || lower.starts_with(prefix))
}

fn is_dangerous_listen_path(_listen_path: &str) -> bool {
    false
}

#[derive(Debug)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

pub fn validate_service(service: &Service) -> Result<(), ValidationError> {
    let name = service.name.trim();

    if name.is_empty() {
        return Err(ValidationError {
            field: "name",
            message: "Le nom du service est requis.".into(),
        });
    }

    if is_reserved_name(name) {
        return Err(ValidationError {
            field: "name",
            message: format!(
                "Le nom \"{name}\" est reserve par lightMock (noms interdits : {}).",
                RESERVED_NAMES.join(", ")
            ),
        });
    }

    if name.contains('/') || name.contains('\\') {
        return Err(ValidationError {
            field: "name",
            message: "Le nom du service ne peut pas contenir de separateur de chemin (/ ou \\)."
                .into(),
        });
    }

    if !NAME_CHARSET_RE.is_match(name) {
        return Err(ValidationError {
            field: "name",
            message: "Le nom du service ne peut contenir que des lettres, chiffres, tirets (-) et underscores (_).".into(),
        });
    }

    if is_dangerous_listen_path(&service.listen_path) {
        return Err(ValidationError {
            field: "listen_path",
            message: "Le chemin d'ecoute est dangereux : un chemin vide, \"/\" ou \"/*\" au premier niveau capturerait la racine de lightMock et masquerait l'interface.".into(),
        });
    }

    let effective = format!(
        "/{}/{}",
        name,
        service.listen_path.trim().trim_start_matches('/')
    );
    if is_internal_route(&effective) {
        return Err(ValidationError {
            field: "listen_path",
            message: format!(
                "Le pattern effectif \"{effective}\" entrerait en conflit avec une route interne de lightMock."
            ),
        });
    }

    // Service "purement mocke" : real_target_url vide est une valeur
    // volontaire ("aucune cible configuree"), pas une omission a rejeter. En
    // revanche is_mocked=false (proxy pur niveau service) sans cible n'a
    // aucun sens : ce serait forcement un proxy vers une URL vide a chaque
    // requete. Bloque a la source plutot que de laisser cette combinaison
    // invalide atteindre le pipeline HTTP.
    if !service.is_mocked && service.real_target_url.trim().is_empty() {
        return Err(ValidationError {
            field: "real_target_url",
            message: "Un service sans cible ne peut pas etre en proxy direct (is_mocked=false) : activez le mode mock ou renseignez une cible.".into(),
        });
    }

    let mut seen_rules = std::collections::HashSet::new();
    for rule in &service.rules {
        let rn = rule.name.trim();
        if rn.is_empty() {
            return Err(ValidationError {
                field: "rules",
                message: "Le nom de la regle est requis.".into(),
            });
        }
        let method_upper = rule.method.trim().to_uppercase();
        if !VALID_METHODS.contains(&method_upper.as_str()) {
            return Err(ValidationError {
                field: "rules",
                message: format!(
                    "Methode HTTP invalide \"{}\" pour la regle \"{rn}\". Valeurs acceptees : {}.",
                    rule.method,
                    VALID_METHODS.join(", ")
                ),
            });
        }
        if !seen_rules.insert(rn.to_lowercase()) {
            return Err(ValidationError {
                field: "rules",
                message: format!(
                    "Le nom de regle \"{rn}\" est utilise plusieurs fois dans ce service. Chaque regle doit avoir un nom unique."
                ),
            });
        }
    }

    Ok(())
}

/// Valide un nom de fichier de backup recu depuis un segment d'URL
/// (`POST /api/config/restore/:filename`) avant toute operation disque.
/// Rejette explicitement '..' et les separateurs de chemin AVANT le test de
/// charset, pour renvoyer un message specifique sur la tentative de
/// traversee plutot qu'un simple "format invalide".
pub fn validate_backup_filename(filename: &str) -> Result<(), ValidationError> {
    let name = filename.trim();

    if name.is_empty() {
        return Err(ValidationError {
            field: "filename",
            message: "Le nom du fichier de sauvegarde est requis.".into(),
        });
    }

    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ValidationError {
            field: "filename",
            message: "Le nom du fichier ne peut pas contenir de separateur de chemin (/ ou \\) ni de sequence \"..\".".into(),
        });
    }

    if !BACKUP_FILENAME_RE.is_match(name) {
        return Err(ValidationError {
            field: "filename",
            message: "Nom de fichier de sauvegarde invalide.".into(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Service;

    use crate::models::{Rule, RuleAction, MockResponse, BodyFragment, WsdlMode};

    fn svc(name: &str, listen_path: &str) -> Service {
        Service {
            name: name.into(),
            listen_path: listen_path.into(),
            real_target_url: "http://example.com".into(),
            is_mocked: false,
            rewrite_directory_urls: false,
            group_name: None,
            wsdl_mode: WsdlMode::default(),
            rules: vec![],
        }
    }

    fn svc_with_rules(name: &str, rule_names: &[&str]) -> Service {
        let mut s = svc(name, "/v1/*");
        s.rules = rule_names.iter().map(|rn| Rule {
            name: rn.to_string(),
            method: "GET".into(),
            sub_path: None,
            action: RuleAction::default(),
            pre_script: None,
            script: None,
            post_script: None,
            response_mode: None,
            conditions: Default::default(),
            response: MockResponse {
                status: 200,
                headers: vec![],
                body: vec![BodyFragment::Literal { value: "ok".into() }],
                chaos: None,
            },
        }).collect();
        s
    }

    #[test]
    fn reject_empty_name() {
        assert!(validate_service(&svc("", "/foo")).is_err());
        assert!(validate_service(&svc("  ", "/foo")).is_err());
    }

    #[test]
    fn reject_reserved_names() {
        assert!(validate_service(&svc("api", "/foo")).is_err());
        assert!(validate_service(&svc("API", "/foo")).is_err());
        assert!(validate_service(&svc("index.html", "/foo")).is_err());
        assert!(validate_service(&svc("assets", "/foo")).is_err());
        assert!(validate_service(&svc("favicon.ico", "/foo")).is_err());
        // Meme si le charset (pas de '.') rejetterait deja ce nom, la liste
        // reservee le documente explicitement (defense en profondeur, meme
        // logique que "index.html"/"favicon.ico" ci-dessus).
        assert!(validate_service(&svc("runtime-config.json", "/foo")).is_err());
    }

    #[test]
    fn reject_name_with_slashes() {
        assert!(validate_service(&svc("my/svc", "/foo")).is_err());
        assert!(validate_service(&svc("my\\svc", "/foo")).is_err());
    }

    #[test]
    fn reject_name_with_spaces() {
        assert!(validate_service(&svc("my svc", "/foo")).is_err());
    }

    #[test]
    fn reject_name_with_special_chars() {
        assert!(validate_service(&svc("svc@name", "/foo")).is_err());
        assert!(validate_service(&svc("svc.name", "/foo")).is_err());
        assert!(validate_service(&svc("100%svc", "/foo")).is_err());
        assert!(validate_service(&svc("caf\u{e9}-svc", "/foo")).is_err());
        assert!(validate_service(&svc("svc#1", "/foo")).is_err());
    }

    #[test]
    fn accept_name_with_underscore_and_digits() {
        assert!(validate_service(&svc("svc_v2-42", "/foo")).is_ok());
        assert!(validate_service(&svc("SVC_NAME", "/foo")).is_ok());
    }

    #[test]
    fn accept_empty_listen_path() {
        assert!(validate_service(&svc("my-svc", "")).is_ok());
        assert!(validate_service(&svc("my-svc", "  ")).is_ok());
        assert!(validate_service(&svc("my-svc", "/")).is_ok());
    }

    #[test]
    fn accept_root_wildcard() {
        assert!(validate_service(&svc("my-svc", "/*")).is_ok());
        assert!(validate_service(&svc("my-svc", "*")).is_ok());
    }

    #[test]
    fn accept_scoped_wildcard() {
        assert!(validate_service(&svc("my-svc", "/v1/*")).is_ok());
        assert!(validate_service(&svc("my-svc", "/users/{id}")).is_ok());
    }

    #[test]
    fn reject_effective_pattern_hitting_internal() {
        assert!(validate_service(&svc("api", "/config")).is_err());
    }

    #[test]
    fn accept_valid_service() {
        assert!(validate_service(&svc("insee", "/v4/sirene/{siret}")).is_ok());
        assert!(validate_service(&svc("users-api", "/v1/users/{id}")).is_ok());
    }

    #[test]
    fn accept_empty_target_when_purely_mocked() {
        // Service "purement mocke" : real_target_url vide est accepte tant
        // que is_mocked reste true (les regles sont toujours evaluees, aucun
        // proxy n'est jamais tente).
        let mut s = svc("purely-mocked", "/v1/*");
        s.real_target_url = "".into();
        s.is_mocked = true;
        assert!(validate_service(&s).is_ok());
    }

    #[test]
    fn reject_empty_target_without_is_mocked() {
        // is_mocked=false (proxy pur niveau service) sans cible est une
        // combinaison invalide : ce serait un proxy vers une URL vide a
        // chaque requete.
        let mut s = svc("broken", "/v1/*");
        s.real_target_url = "".into();
        s.is_mocked = false;
        let err = validate_service(&s).unwrap_err();
        assert_eq!(err.field, "real_target_url");
    }

    #[test]
    fn reject_whitespace_only_target_without_is_mocked() {
        let mut s = svc("broken2", "/v1/*");
        s.real_target_url = "   ".into();
        s.is_mocked = false;
        assert!(validate_service(&s).is_err());
    }

    #[test]
    fn is_internal_route_works() {
        assert!(is_internal_route("/"));
        assert!(is_internal_route(""));
        assert!(is_internal_route("/api/services"));
        assert!(is_internal_route("/api"));
        assert!(is_internal_route("/index.html"));
        assert!(is_internal_route("/assets/main.js"));
        assert!(is_internal_route("/runtime-config.json"));
        assert!(!is_internal_route("/my-svc/foo"));
        assert!(!is_internal_route("/insee/v4/sirene/123"));
    }

    #[test]
    fn is_static_asset_route_covers_spa_shell() {
        assert!(is_static_asset_route("/"));
        assert!(is_static_asset_route(""));
        assert!(is_static_asset_route("/index.html"));
        assert!(is_static_asset_route("/assets/main.js"));
        assert!(is_static_asset_route("/assets/index-B2Cp0FJn.css"));
        assert!(is_static_asset_route("/favicon.ico"));
        assert!(is_static_asset_route("/runtime-config.json"));
    }

    #[test]
    fn is_static_asset_route_never_matches_api_routes() {
        // Regression cible : ce bypass ne doit JAMAIS s'etendre a /api/*,
        // sans quoi auth_middleware laisserait passer des routes protegees.
        assert!(!is_static_asset_route("/api"));
        assert!(!is_static_asset_route("/api/services"));
        assert!(!is_static_asset_route("/api/auth/me"));
        // Piege de prefixe potentiel : "/api/assets/x" NE commence PAS par
        // "/assets/", donc aucune collision malgre le nom partage "assets".
        assert!(!is_static_asset_route("/api/assets/x"));
    }

    #[test]
    fn is_static_asset_route_does_not_match_user_service_routes() {
        assert!(!is_static_asset_route("/my-svc/foo"));
        assert!(!is_static_asset_route("/insee/v4/sirene/123"));
    }

    #[test]
    fn reject_duplicate_rule_names() {
        assert!(validate_service(&svc_with_rules("svc", &["r1", "r1"])).is_err());
    }

    #[test]
    fn reject_duplicate_rule_names_case_insensitive() {
        assert!(validate_service(&svc_with_rules("svc", &["Rule-A", "rule-a"])).is_err());
    }

    #[test]
    fn accept_unique_rule_names() {
        assert!(validate_service(&svc_with_rules("svc", &["r1", "r2", "r3"])).is_ok());
    }

    #[test]
    fn reject_empty_rule_name() {
        assert!(validate_service(&svc_with_rules("svc", &[""])).is_err());
    }

    #[test]
    fn accept_same_rule_name_across_services() {
        assert!(validate_service(&svc_with_rules("svc-a", &["shared-rule"])).is_ok());
        assert!(validate_service(&svc_with_rules("svc-b", &["shared-rule"])).is_ok());
    }

    #[test]
    fn accept_valid_backup_filenames() {
        assert!(validate_backup_filename("mock-config-1690000000000-000001.yaml").is_ok());
        assert!(validate_backup_filename("pre-reset-1690000000000.yaml").is_ok());
    }

    #[test]
    fn reject_empty_backup_filename() {
        assert!(validate_backup_filename("").is_err());
        assert!(validate_backup_filename("   ").is_err());
    }

    #[test]
    fn reject_backup_filename_path_traversal() {
        assert!(validate_backup_filename("../mock-config.yaml").is_err());
        assert!(validate_backup_filename("../../etc/passwd").is_err());
        assert!(validate_backup_filename("protected/../../mock-config.yaml").is_err());
        assert!(validate_backup_filename("..\\mock-config.yaml").is_err());
    }

    #[test]
    fn reject_backup_filename_with_path_separators() {
        assert!(validate_backup_filename("protected/pre-reset-1.yaml").is_err());
        assert!(validate_backup_filename("subdir\\mock-config.yaml").is_err());
        assert!(validate_backup_filename("/etc/passwd").is_err());
    }

    #[test]
    fn reject_backup_filename_wrong_extension_or_charset() {
        assert!(validate_backup_filename("mock-config.txt").is_err());
        assert!(validate_backup_filename("mock config.yaml").is_err());
        assert!(validate_backup_filename("mock-config.yaml.bak").is_err());
        assert!(validate_backup_filename("caf\u{e9}.yaml").is_err());
    }
}
