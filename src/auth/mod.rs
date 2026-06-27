pub mod keycloak;
pub mod middleware;

use crate::models::{Group, MockConfig};

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub keycloak_url: String,
    pub realm: String,
    pub client_id: String,
    pub super_admins: Vec<String>,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("AUTH_ENABLED")
            .unwrap_or_else(|_| "false".into())
            .eq_ignore_ascii_case("true");

        let keycloak_url = std::env::var("KEYCLOAK_URL").unwrap_or_default();
        let realm = std::env::var("KEYCLOAK_REALM").unwrap_or_default();
        let client_id = std::env::var("KEYCLOAK_CLIENT_ID").unwrap_or_default();
        let super_admins: Vec<String> = std::env::var("SUPER_ADMINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if enabled && (keycloak_url.is_empty() || realm.is_empty() || client_id.is_empty()) {
            panic!(
                "AUTH_ENABLED=true requires KEYCLOAK_URL, KEYCLOAK_REALM, and KEYCLOAK_CLIENT_ID"
            );
        }

        Self {
            enabled,
            keycloak_url,
            realm,
            client_id,
            super_admins,
        }
    }

    pub fn is_super_admin(&self, username: &str) -> bool {
        self.super_admins.iter().any(|sa| sa == username)
    }
}

pub fn can_access_service(
    username: &str,
    is_super_admin: bool,
    service: &crate::models::Service,
    groups: &[Group],
) -> bool {
    if is_super_admin {
        return true;
    }
    match &service.group_name {
        None => false,
        Some(gn) => groups.iter().any(|g| {
            g.name == *gn
                && (g.admins.contains(&username.to_string())
                    || g.members.contains(&username.to_string()))
        }),
    }
}

pub fn can_manage_group(username: &str, is_super_admin: bool, group: &Group) -> bool {
    is_super_admin || group.admins.contains(&username.to_string())
}

pub fn visible_services(
    username: &str,
    is_super_admin: bool,
    config: &MockConfig,
) -> Vec<crate::models::Service> {
    if is_super_admin {
        return config.services.clone();
    }
    config
        .services
        .iter()
        .filter(|s| can_access_service(username, false, s, &config.groups))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Group, MockConfig, Service, WsdlMode};

    fn test_config() -> AuthConfig {
        AuthConfig {
            enabled: true,
            keycloak_url: "https://kc.example.com".into(),
            realm: "test".into(),
            client_id: "lightmock".into(),
            super_admins: vec!["admin1".into()],
        }
    }

    fn test_groups() -> Vec<Group> {
        vec![Group {
            name: "team-a".into(),
            code: "tma".into(),
            admins: vec!["lead-a".into()],
            members: vec!["dev-a1".into(), "dev-a2".into()],
        }]
    }

    fn test_service(group: Option<&str>) -> Service {
        Service {
            name: "svc".into(),
            listen_path: "/v1/*".into(),
            real_target_url: "http://svc:80".into(),
            is_mocked: true,
            rewrite_directory_urls: false,
            group_name: group.map(String::from),
            wsdl_mode: WsdlMode::default(),
            rules: vec![],
        }
    }

    #[test]
    fn super_admin_from_config() {
        let cfg = test_config();
        assert!(cfg.is_super_admin("admin1"));
        assert!(!cfg.is_super_admin("nobody"));
    }

    #[test]
    fn super_admin_can_access_any_service() {
        let groups = test_groups();
        assert!(can_access_service("admin1", true, &test_service(Some("team-a")), &groups));
        assert!(can_access_service("admin1", true, &test_service(None), &groups));
    }

    #[test]
    fn group_member_can_access_own_service() {
        let groups = test_groups();
        assert!(can_access_service("dev-a1", false, &test_service(Some("team-a")), &groups));
        assert!(can_access_service("lead-a", false, &test_service(Some("team-a")), &groups));
    }

    #[test]
    fn outsider_cannot_access_service() {
        let groups = test_groups();
        assert!(!can_access_service("outsider", false, &test_service(Some("team-a")), &groups));
    }

    #[test]
    fn no_group_service_only_super_admin() {
        let groups = test_groups();
        assert!(!can_access_service("dev-a1", false, &test_service(None), &groups));
    }

    #[test]
    fn can_manage_group_admin_or_super() {
        let group = &test_groups()[0];
        assert!(can_manage_group("lead-a", false, group));
        assert!(can_manage_group("anyone", true, group));
        assert!(!can_manage_group("dev-a1", false, group));
    }

    #[test]
    fn visible_services_filters_by_group() {
        let config = MockConfig {
            services: vec![
                test_service(Some("team-a")),
                Service {
                    name: "svc-b".into(),
                    group_name: Some("team-b".into()),
                    ..test_service(None)
                },
            ],
            groups: test_groups(),
        };
        let visible = visible_services("dev-a1", false, &config);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "svc");
    }

    #[test]
    fn member_cannot_manage_group() {
        let group = &test_groups()[0];
        assert!(!can_manage_group("dev-a1", false, group));
        assert!(!can_manage_group("dev-a2", false, group));
    }

    #[test]
    fn group_admin_can_manage() {
        let group = &test_groups()[0];
        assert!(can_manage_group("lead-a", false, group));
    }

    #[test]
    fn super_admin_can_manage_any_group() {
        let group = &test_groups()[0];
        assert!(can_manage_group("random-user", true, group));
    }

    #[test]
    fn service_with_group_visible_to_members() {
        let config = MockConfig {
            services: vec![
                test_service(Some("team-a")),
            ],
            groups: test_groups(),
        };
        assert_eq!(visible_services("dev-a1", false, &config).len(), 1);
        assert_eq!(visible_services("dev-a2", false, &config).len(), 1);
        assert_eq!(visible_services("outsider", false, &config).len(), 0);
        assert_eq!(visible_services("admin1", true, &config).len(), 1);
    }
}
